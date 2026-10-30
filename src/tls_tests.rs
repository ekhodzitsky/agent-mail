#![allow(clippy::unwrap_used, clippy::expect_used)] // Ephemeral test certificates and SMTP fixtures.
use super::*;
use crate::{
    config::Smtp,
    smtp::{send_using, transport_using_tls},
};
use lettre::transport::smtp::client::Certificate;
use lettre::transport::smtp::client::TlsParameters;
use rcgen::{BasicConstraints, CertificateParams, CertifiedIssuer, IsCa, KeyPair, KeyUsagePurpose};
use rustls::{ServerConfig, ServerConnection, StreamOwned, pki_types::PrivatePkcs8KeyDer};
use std::time::Duration;
use std::{
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    sync::Arc,
    thread,
    time::Instant,
};

struct Identity {
    server: Arc<ServerConfig>,
    root: Certificate,
}
impl Identity {
    fn new(name: &str, expired: bool) -> Self {
        let mut ca = CertificateParams::new(Vec::<String>::new()).unwrap();
        ca.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        ca.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
        let issuer = CertifiedIssuer::self_signed(ca, KeyPair::generate().unwrap()).unwrap();
        let key = KeyPair::generate().unwrap();
        let mut params = CertificateParams::new(vec![name.to_string()]).unwrap();
        if expired {
            params.not_before = rcgen::date_time_ymd(2000, 1, 1);
            params.not_after = rcgen::date_time_ymd(2001, 1, 1);
        }
        let cert = params.signed_by(&key, &issuer).unwrap();
        let server =
            ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
                .with_safe_default_protocol_versions()
                .unwrap()
                .with_no_client_auth()
                .with_single_cert(
                    vec![cert.der().clone()],
                    PrivatePkcs8KeyDer::from(key.serialize_der()).into(),
                )
                .unwrap();
        Self {
            server: Arc::new(server),
            root: Certificate::from_pem(issuer.pem().as_bytes()).unwrap(),
        }
    }
}
enum Wire {
    Plain(TcpStream),
    Tls(Box<StreamOwned<ServerConnection, TcpStream>>),
}
impl Read for Wire {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Plain(s) => s.read(buf),
            Self::Tls(s) => s.read(buf),
        }
    }
}
impl Write for Wire {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        match self {
            Self::Plain(s) => s.write(b),
            Self::Tls(s) => s.write(b),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Plain(s) => s.flush(),
            Self::Tls(s) => s.flush(),
        }
    }
}
fn line(w: &mut Wire) -> io::Result<String> {
    let mut b = Vec::new();
    loop {
        let mut one = [0];
        w.read_exact(&mut one)?;
        b.push(one[0]);
        if one[0] == b'\n' {
            break;
        }
        if b.len() > 65536 {
            return Err(io::Error::other("fixture line limit"));
        }
    }
    String::from_utf8(b).map_err(|_| io::Error::other("fixture invalid UTF8"))
}
fn reply(w: &mut Wire, text: &str) -> io::Result<()> {
    w.write_all(text.as_bytes())?;
    w.flush()
}
fn upgrade(w: Wire, config: Arc<ServerConfig>) -> io::Result<Wire> {
    match w {
        Wire::Plain(s) => {
            let mut tls =
                StreamOwned::new(ServerConnection::new(config).map_err(io::Error::other)?, s);
            while tls.conn.is_handshaking() {
                tls.conn.complete_io(&mut tls.sock)?;
            }
            Ok(Wire::Tls(Box::new(tls)))
        }
        Wire::Tls(_) => Err(io::Error::other("already TLS")),
    }
}
#[derive(Clone, Copy)]
enum Mode {
    Implicit,
    StartTls,
    NoStartTls,
    RefuseUpgrade,
}
#[derive(Default, Debug)]
struct Report {
    tls: bool,
    auth: usize,
    mail: usize,
    data: usize,
    plaintext_sensitive: bool,
    post_tls_ehlo: bool,
    credentials_match: bool,
}
struct Server {
    port: u16,
    handle: thread::JoinHandle<Report>,
}
impl Server {
    fn start(identity: &Identity, mode: Mode, reject_auth: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        listener.set_nonblocking(true).unwrap();
        let config = identity.server.clone();
        let handle = thread::spawn(move || {
            let mut report = Report::default();
            let deadline = Instant::now() + Duration::from_secs(5);
            let stream = loop {
                match listener.accept() {
                    Ok((s, _)) => break s,
                    Err(e)
                        if e.kind() == io::ErrorKind::WouldBlock && Instant::now() < deadline =>
                    {
                        thread::sleep(Duration::from_millis(5))
                    }
                    Err(_) => return report,
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut wire = Wire::Plain(stream);
            if matches!(mode, Mode::Implicit) {
                wire = match upgrade(wire, config.clone()) {
                    Ok(w) => w,
                    Err(_) => return report,
                };
                report.tls = true;
            }
            if reply(&mut wire, "220 localhost synthetic TLS SMTP\r\n").is_err() {
                return report;
            }
            loop {
                let command = match line(&mut wire) {
                    Ok(c) => c,
                    Err(_) => return report,
                };
                let response = if command.starts_with("EHLO") {
                    if report.tls {
                        report.post_tls_ehlo = true;
                        "250-localhost\r\n250-DSN\r\n250 AUTH PLAIN\r\n"
                    } else if matches!(mode, Mode::NoStartTls) {
                        "250-localhost\r\n250-DSN\r\n250 AUTH PLAIN\r\n"
                    } else {
                        "250-localhost\r\n250-STARTTLS\r\n250-DSN\r\n250 AUTH PLAIN\r\n"
                    }
                } else if command == "STARTTLS\r\n" {
                    if matches!(mode, Mode::RefuseUpgrade) {
                        "454 Upgrade unavailable synthetic-secret\r\n"
                    } else {
                        if reply(&mut wire, "220 Begin TLS\r\n").is_err() {
                            return report;
                        }
                        wire = match upgrade(wire, config.clone()) {
                            Ok(w) => w,
                            Err(_) => return report,
                        };
                        report.tls = true;
                        continue;
                    }
                } else if command.starts_with("AUTH ") {
                    report.auth += 1;
                    report.plaintext_sensitive |= !report.tls;
                    use base64::Engine;
                    let encoded = command.trim().strip_prefix("AUTH PLAIN ").unwrap_or("");
                    report.credentials_match = base64::engine::general_purpose::STANDARD
                        .decode(encoded)
                        .is_ok_and(|b| b == b"\0synthetic-user\0synthetic-secret");
                    if reject_auth {
                        "535 5.7.8 synthetic-secret rejected\r\n"
                    } else {
                        "235 Authenticated\r\n"
                    }
                } else if command.starts_with("MAIL FROM") {
                    report.mail += 1;
                    report.plaintext_sensitive |= !report.tls;
                    "250 OK\r\n"
                } else if command.starts_with("RCPT TO") {
                    report.plaintext_sensitive |= !report.tls;
                    "250 OK\r\n"
                } else if command == "DATA\r\n" {
                    report.data += 1;
                    report.plaintext_sensitive |= !report.tls;
                    if reply(&mut wire, "354 Send data\r\n").is_err() {
                        return report;
                    }
                    loop {
                        match line(&mut wire) {
                            Ok(s) if s == ".\r\n" => break,
                            Ok(_) => (),
                            Err(_) => return report,
                        }
                    }
                    "250 Accepted\r\n"
                } else if command == "QUIT\r\n" {
                    let _ = reply(&mut wire, "221 Bye\r\n");
                    return report;
                } else {
                    "500 Unexpected command\r\n"
                };
                if reply(&mut wire, response).is_err() {
                    return report;
                }
            }
        });
        Self { port, handle }
    }
}
fn run(
    identity: &Identity,
    mode: Mode,
    reject_auth: bool,
    trust: bool,
) -> (std::result::Result<Value, Error>, Value, Report) {
    let (_dir, store, mut cfg, id) = crate::storage_tests::fixture();
    let server = Server::start(identity, mode, reject_auth);
    cfg.smtp = Some(Smtp {
        total_timeout_seconds: 60,
        host: "127.0.0.1".into(),
        port: server.port,
        security: if matches!(mode, Mode::Implicit) {
            "tls"
        } else {
            "starttls"
        }
        .into(),
        username: Some("synthetic-user".into()),
        password_env: Some("SYNTHETIC_PASSWORD".into()),
        timeout_seconds: 2,
    });
    let result = send_using(&store, &cfg, &id, false, |c| {
        transport_using_tls(
            c,
            |host| {
                let builder = TlsParameters::builder(host.to_string());
                if trust {
                    builder.add_root_certificate(identity.root.clone()).build()
                } else {
                    builder.build()
                }
            },
            |_| Ok("synthetic-secret".into()),
        )
    });
    let status = store.status(&id).unwrap();
    let report = server.handle.join().unwrap();
    if let Err(ref e) = result {
        assert!(!format!("{e:?}").contains("synthetic-secret"));
    }
    assert!(!status.to_string().contains("synthetic-secret"));
    assert!(!report.plaintext_sensitive, "{report:?}");
    (result, status, report)
}
#[test]
fn verified_tls_and_starttls_submit_with_authentication() {
    for mode in [Mode::Implicit, Mode::StartTls] {
        let identity = Identity::new("127.0.0.1", false);
        let (result, status, report) = run(&identity, mode, false, true);
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(status["state"], "accepted");
        assert!(
            report.tls && report.post_tls_ehlo && report.credentials_match,
            "{report:?}"
        );
        assert_eq!(report.auth, 1);
        assert_eq!(report.data, 1);
    }
}
#[test]
fn untrusted_wrong_name_and_expired_certificates_never_send_credentials_or_mail() {
    for mode in [Mode::Implicit, Mode::StartTls] {
        for (name, expired, trust) in [
            ("127.0.0.1", false, false),
            ("wrong.example.org", false, true),
            ("127.0.0.1", true, true),
        ] {
            let identity = Identity::new(name, expired);
            let (result, status, report) = run(&identity, mode, false, trust);
            assert_eq!(result.unwrap_err().code, "outcome_unknown");
            assert_eq!(status["state"], "unknown");
            assert_eq!(report.auth, 0);
            assert_eq!(report.mail, 0);
            assert_eq!(report.data, 0);
        }
    }
}
#[test]
fn starttls_downgrade_and_upgrade_refusal_never_send_credentials_or_mail() {
    for mode in [Mode::NoStartTls, Mode::RefuseUpgrade] {
        let identity = Identity::new("127.0.0.1", false);
        let (result, _, report) = run(&identity, mode, false, true);
        assert!(result.is_err());
        assert_eq!(report.auth, 0);
        assert_eq!(report.mail, 0);
        assert_eq!(report.data, 0);
    }
}
#[test]
fn authentication_rejection_is_recorded_without_secret_or_message_data() {
    for mode in [Mode::Implicit, Mode::StartTls] {
        let identity = Identity::new("127.0.0.1", false);
        let (result, status, report) = run(&identity, mode, true, true);
        assert_eq!(result.unwrap_err().code, "smtp_rejected");
        assert_eq!(status["state"], "rejected");
        assert_eq!(status["attempts"][0]["server_response"]["code"], "535");
        assert_eq!(report.auth, 1);
        assert_eq!(report.mail, 0);
        assert_eq!(report.data, 0);
    }
}

#[test]
fn dsn_typed_client_verifies_tls_before_auth_and_never_downgrades() {
    for mode in [
        Mode::Implicit,
        Mode::StartTls,
        Mode::NoStartTls,
        Mode::RefuseUpgrade,
    ] {
        for valid in [true, false] {
            let identity = Identity::new(
                if valid {
                    "127.0.0.1"
                } else {
                    "wrong.example.org"
                },
                false,
            );
            let server = Server::start(&identity, mode, false);
            let (_dir, mut store, mut cfg, _) = crate::storage_tests::fixture();
            cfg.smtp = Some(Smtp {
                host: "127.0.0.1".into(),
                port: server.port,
                security: if matches!(mode, Mode::Implicit) {
                    "tls"
                } else {
                    "starttls"
                }
                .into(),
                username: Some("synthetic-user".into()),
                password_env: Some("SYNTHETIC_PASSWORD".into()),
                timeout_seconds: 2,
                total_timeout_seconds: 10,
            });
            let prepared = crate::package::prepare_content(
                &mut store,
                &cfg,
                "recipient@example.org".into(),
                "Synthetic".into(),
                crate::package::Content {
                    body: "Synthetic body".into(),
                    html: None,
                    dsn: Some(crate::dsn::DsnMode::Required),
                    operation_key: None,
                },
                vec![("test.pdf".into(), b"%PDF-1.4 synthetic".to_vec())],
                json!({}),
            )
            .unwrap();
            let id = prepared["package_id"].as_str().unwrap();
            let p = store.load(id).unwrap();
            store.conn.execute("INSERT INTO attempts VALUES('tls-attempt',?1,'2026-01-01T00:00:00+00:00',NULL,'unknown',?2,NULL)",rusqlite::params![id,p.manifest.message_id]).unwrap();
            let result = crate::dsn::submit_with_tls(
                &store,
                &cfg,
                &p,
                "tls-attempt",
                |host| {
                    TlsParameters::builder(host.to_string())
                        .add_root_certificate(identity.root.clone())
                        .build()
                },
                |_| Ok("synthetic-secret".into()),
            )
            .unwrap();
            let report = server.handle.join().unwrap();
            if valid && matches!(mode, Mode::Implicit | Mode::StartTls) {
                assert_eq!(result.unwrap().code().to_string(), "250");
                assert_eq!(report.auth, 1);
                assert_eq!(report.data, 1);
                assert!(report.credentials_match);
            } else {
                assert!(result.is_err());
                assert_eq!(report.auth, 0);
                assert_eq!(report.mail, 0);
                assert_eq!(report.data, 0);
            }
            assert!(!report.plaintext_sensitive);
        }
    }
}
