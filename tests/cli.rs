use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    process::{Command, Output},
};
use tempfile::TempDir;

struct Fixture {
    dir: TempDir,
}
fn binary() -> std::ffi::OsString {
    std::env::var_os("AGENT_MAIL_TEST_BIN")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_agent-mail").into())
}
impl Fixture {
    fn new() -> Self {
        let s = Self {
            dir: tempfile::tempdir().unwrap(),
        };
        fs::write(
            s.dir.path().join("config.toml"),
            "sender = 'sender@example.org'\n",
        )
        .unwrap();
        fs::write(s.dir.path().join("subject.txt"), "Synthetic application").unwrap();
        fs::write(
            s.dir.path().join("body.txt"),
            "Synthetic letter.\nRegards, Test\n",
        )
        .unwrap();
        fs::write(
            s.dir.path().join("resume.pdf"),
            b"%PDF-1.4\nsynthetic test fixture\n%%EOF",
        )
        .unwrap();
        s
    }
    fn cmd(&self, args: &[&str]) -> Command {
        // Test harness only: exercise an explicitly installed binary in release checks.
        let mut c = Command::new(binary());
        c.arg("--home")
            .arg(self.dir.path().join("store"))
            .arg("--config")
            .arg(self.dir.path().join("config.toml"))
            .arg("--json")
            .args(args);
        c
    }
    fn call(&self, args: &[&str]) -> (i32, Value) {
        decode(self.cmd(args).output().unwrap())
    }
    fn prepare(&self) -> String {
        let o = self
            .cmd(&["prepare", "--to", "recipient@example.org", "--subject-file"])
            .arg(self.dir.path().join("subject.txt"))
            .arg("--body-file")
            .arg(self.dir.path().join("body.txt"))
            .arg("--attach")
            .arg(self.dir.path().join("resume.pdf"))
            .arg("--company-ref")
            .arg("synthetic-company")
            .output()
            .unwrap();
        let (code, v) = decode(o);
        assert_eq!(code, 0, "{v}");
        v["result"]["package_id"].as_str().unwrap().to_string()
    }
    fn prepare_dsn(&self, mode: &str) -> String {
        let (code, result) = decode(
            self.cmd(&[
                "prepare",
                "--to",
                "recipient@example.org",
                "--dsn",
                mode,
                "--subject-file",
            ])
            .arg(self.dir.path().join("subject.txt"))
            .arg("--body-file")
            .arg(self.dir.path().join("body.txt"))
            .arg("--attach")
            .arg(self.dir.path().join("resume.pdf"))
            .output()
            .unwrap(),
        );
        assert_eq!(code, 0, "{result}");
        result["result"]["package_id"].as_str().unwrap().into()
    }
    #[cfg(feature = "test-smtp")]
    fn smtp(&self, port: u16) {
        fs::write(self.dir.path().join("config.toml"),format!("sender='sender@example.org'\n[smtp]\nhost='127.0.0.1'\nport={port}\nsecurity='loopback_test'\ntimeout_seconds=1\n")).unwrap();
    }
}
fn decode(o: Output) -> (i32, Value) {
    assert!(
        o.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["schema_version"], 1);
    (o.status.code().unwrap(), v)
}
#[test]
fn agent_mail_keeps_legacy_default_state_and_environment_resend_blocks() {
    let f = Fixture::new();
    let ids = [f.prepare(), f.prepare()];
    let before: Vec<_> = ids.iter().map(|id| f.call(&["inspect", id]).1).collect();
    let store = f.dir.path().join("store");
    fs::write(store.join("config.toml"), "sender='sender@example.org'\n[smtp]\nhost='smtp.example.invalid'\nport=465\nsecurity='tls'\n").unwrap();
    {
        let db = rusqlite::Connection::open(store.join("mail.sqlite3")).unwrap();
        for ((id, package), outcome) in ids.iter().zip(&before).zip(["accepted", "unknown"]) {
            let message_id = package["result"]["manifest"]["message_id"]
                .as_str()
                .unwrap();
            db.execute(
                "INSERT INTO attempts VALUES(?,?,?,?,?,?,?)",
                rusqlite::params![
                    outcome,
                    id,
                    "2026-01-01T00:00:00+00:00",
                    Option::<String>::None,
                    outcome,
                    message_id,
                    Option::<String>::None,
                ],
            )
            .unwrap();
        }
    }
    let legacy = f.dir.path().join(".local/share/agent-mailer");
    fs::create_dir_all(legacy.parent().unwrap()).unwrap();
    fs::rename(store, &legacy).unwrap();
    for explicit_env in [false, true] {
        for (id, expected) in ids.iter().zip(&before) {
            for args in [vec!["inspect", id.as_str()], vec!["send", id.as_str()]] {
                let mut cmd = Command::new(binary());
                cmd.env_clear().env("HOME", f.dir.path()).arg("--json");
                if explicit_env {
                    // Ensure the legacy override is used even with a different HOME.
                    cmd.env("HOME", f.dir.path().join("different-home"))
                        .env("AGENT_MAILER_HOME", &legacy);
                }
                let (code, value) = decode(cmd.args(&args).output().unwrap());
                if args[0] == "inspect" {
                    assert_eq!(code, 0);
                    assert_eq!(&value, expected);
                } else {
                    assert_eq!(code, 5);
                    assert_eq!(value["error"]["code"], "send_blocked");
                }
            }
        }
    }
    assert!(!f.dir.path().join(".local/share/mailnock").exists());
    let (code, version) = decode(
        Command::new(binary())
            .args(["--version", "--json"])
            .output()
            .unwrap(),
    );
    assert_eq!(code, 0);
    assert!(
        version["result"]["help"]
            .as_str()
            .unwrap()
            .starts_with("agent-mail ")
    );
}
#[test]
fn preparation_freezes_exact_materials_and_json_errors() {
    let f = Fixture::new();
    let id = f.prepare();
    fs::write(f.dir.path().join("body.txt"), "Changed").unwrap();
    let (code, v) = f.call(&["inspect", &id]);
    assert_eq!(code, 0);
    assert_eq!(
        v["result"]["manifest"]["body"],
        "Synthetic letter.\nRegards, Test\n"
    );
    assert_eq!(f.call(&["status", &id]).1["result"]["state"], "prepared");
    assert_eq!(
        f.call(&["list"]).1["result"]["packages"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(f.call(&["doctor"]).1["result"]["smtp_ready"], false);
    assert_eq!(f.call(&["send", &id, "--dry-run"]).0, 3);
    let (code, v) = f.call(&["invalid", "secret-token"]);
    assert_eq!(code, 2);
    assert!(!v.to_string().contains("secret-token"));
}
#[cfg(unix)]
#[test]
fn uri_like_state_paths_use_the_checked_private_database_and_journal() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    fs::create_dir(f.dir.path().join("shadow")).unwrap();
    let absolute_home = f.dir.path().join("file:shadow");
    let call = |home: &std::path::Path, args: &[&str]| {
        let mut cmd = Command::new(binary());
        cmd.current_dir(f.dir.path())
            .arg("--home")
            .arg(home)
            .arg("--config")
            .arg(f.dir.path().join("config.toml"))
            .arg("--json")
            .args(args);
        decode(cmd.output().unwrap())
    };
    let relative = std::path::Path::new("file:shadow");
    let (code, refused) = call(relative, &["doctor"]);
    assert_eq!(code, 3);
    assert_eq!(refused["error"]["code"], "configuration");
    assert!(!absolute_home.exists());
    assert!(!f.dir.path().join("shadow/mail.sqlite3").exists());
    let (code, prepared) = call(
        &absolute_home,
        &[
            "prepare",
            "--to",
            "recipient@example.org",
            "--subject-file",
            "subject.txt",
            "--body-file",
            "body.txt",
            "--attach",
            "resume.pdf",
        ],
    );
    assert_eq!(code, 0);
    let path = f.dir.path().join("file:shadow/mail.sqlite3");
    assert!(fs::read(&path).unwrap().starts_with(b"SQLite format 3\0"));
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert!(!f.dir.path().join("shadow/mail.sqlite3").exists());
    let id = prepared["result"]["package_id"].as_str().unwrap();
    let before = call(&absolute_home, &["inspect", id]).1;
    let message_id = prepared["result"]["manifest"]["message_id"]
        .as_str()
        .unwrap();
    {
        let db = rusqlite::Connection::open(&path).unwrap();
        db.execute("INSERT INTO attempts VALUES('synthetic',?1,'2026-01-01T00:00:00+00:00',NULL,'unknown',?2,NULL)", rusqlite::params![id,message_id]).unwrap();
    }
    fs::write(f.dir.path().join("config.toml"), "sender='sender@example.org'\n[smtp]\nhost='smtp.example.invalid'\nport=465\nsecurity='tls'\n").unwrap();
    assert_eq!(call(&absolute_home, &["inspect", id]).1, before);
    let (code, blocked) = call(&absolute_home, &["send", id]);
    assert_eq!(code, 5);
    assert_eq!(blocked["error"]["code"], "send_blocked");
}
#[test]
fn tampering_is_detected_before_network() {
    for target in ["manifest", "eml", "attachment"] {
        let f = Fixture::new();
        let id = f.prepare();
        let db = rusqlite::Connection::open(f.dir.path().join("store/mail.sqlite3")).unwrap();
        assert!(db.execute("UPDATE packages SET eml=x'00'", []).is_err());
        let trigger_sql: Vec<String> = db.prepare("SELECT sql FROM sqlite_schema WHERE name IN ('packages_no_update','attachments_no_update') ORDER BY name").unwrap().query_map([], |r| r.get(0)).unwrap().map(|r| r.unwrap()).collect();
        db.execute_batch("DROP TRIGGER packages_no_update; DROP TRIGGER attachments_no_update;")
            .unwrap();
        match target {
            "manifest" => db
                .execute("UPDATE packages SET manifest=x'00'", [])
                .unwrap(),
            "eml" => db.execute("UPDATE packages SET eml=x'00'", []).unwrap(),
            _ => db
                .execute("UPDATE attachments SET content=x'00'", [])
                .unwrap(),
        };
        for sql in &trigger_sql {
            db.execute_batch(sql).unwrap();
        }
        let output = f.dir.path().join("corrupt.eml");
        assert_eq!(
            decode(
                f.cmd(&["export-eml", &id, "--output"])
                    .arg(&output)
                    .output()
                    .unwrap()
            )
            .0,
            4
        );
        assert!(!output.exists());
        assert_eq!(f.call(&["inspect", &id]).0, 4);
        assert_eq!(f.call(&["send", &id]).0, 4);
    }
}
#[test]
fn import_checks_hashes_preserves_source_and_blocks_receipts() {
    let f = Fixture::new();
    let source = f.dir.path().join("A0001");
    fs::create_dir(&source).unwrap();
    let mut manifest = json!({"schema_version":1,"id":"A0001","company":"synthetic","role":null});
    for (key, name, original) in [
        ("pdf", "resume.pdf", "resume.pdf"),
        ("letter", "cover-letter.txt", "body.txt"),
        ("subject", "subject.txt", "subject.txt"),
    ] {
        let bytes = fs::read(f.dir.path().join(original)).unwrap();
        fs::write(source.join(name), &bytes).unwrap();
        manifest[key] = json!({"file":name,"source_filename":name,"sha256":format!("{:x}",Sha256::digest(&bytes)),"bytes":bytes.len()});
    }
    let raw = serde_json::to_vec(&manifest).unwrap();
    fs::write(source.join("manifest.json"), &raw).unwrap();
    let args = [
        "import",
        source.to_str().unwrap(),
        "--to",
        "recipient@example.org",
    ];
    let (code, v) = f.call(&args);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["result"]["manifest"]["metadata"]["source_id"], "A0001");
    assert_eq!(fs::read(source.join("manifest.json")).unwrap(), raw);
    fs::write(source.join("resume.pdf"), "changed").unwrap();
    assert_eq!(f.call(&args).0, 4);
    fs::write(source.join("receipt.json"), "{}").unwrap();
    assert_eq!(f.call(&args).0, 5);
}

#[cfg(feature = "test-smtp")]
mod smtp {
    use super::*;
    use std::{
        io::{BufRead, BufReader, Write},
        net::{TcpListener, TcpStream},
        sync::mpsc::{self, Receiver, Sender},
        thread::{self, JoinHandle},
        time::Duration,
    };
    enum Behavior {
        Accept,
        DsnAccept,
        DsnReject,
        DsnDrop,
        UnexpectedFinal,
        Reject,
        Drop,
        Pause,
    }
    struct Server {
        port: u16,
        commands: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
        data: Receiver<Vec<u8>>,
        release: Sender<()>,
        thread: JoinHandle<()>,
    }
    impl Server {
        fn start(behavior: Behavior) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            let (data_tx, data) = mpsc::channel();
            let (release, rx) = mpsc::channel();
            let commands = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
            let wire = commands.clone();
            let thread = thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(10)))
                    .unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                reply(&mut stream, "220 localhost synthetic SMTP\r\n");
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 {
                        break;
                    }
                    wire.lock().unwrap().push(line.clone());
                    if line.starts_with("EHLO") {
                        if matches!(
                            behavior,
                            Behavior::DsnAccept | Behavior::DsnReject | Behavior::DsnDrop
                        ) {
                            reply(&mut stream, "250-localhost\r\n250-DSN\r\n250 8BITMIME\r\n");
                        } else {
                            reply(&mut stream, "250-localhost\r\n250 8BITMIME\r\n");
                        }
                    } else if line.starts_with("MAIL FROM") || line.starts_with("RCPT TO") {
                        reply(&mut stream, "250 OK\r\n");
                    } else if line == "DATA\r\n" {
                        reply(&mut stream, "354 Send data\r\n");
                        let mut bytes = Vec::new();
                        loop {
                            let mut b = Vec::new();
                            if reader.read_until(b'\n', &mut b).unwrap_or(0) == 0 {
                                return;
                            }
                            if b == b".\r\n" {
                                break;
                            }
                            bytes.extend(b);
                        }
                        data_tx.send(bytes).unwrap();
                        match behavior {
                            Behavior::Accept | Behavior::DsnAccept => {
                                reply(&mut stream, "250 2.0.0 accepted synthetic\r\n")
                            }
                            Behavior::UnexpectedFinal => {
                                reply(&mut stream, "354 Unexpected intermediate reply\r\n")
                            }
                            Behavior::Reject | Behavior::DsnReject => {
                                reply(&mut stream, "550 5.7.1 rejected secret-token\r\n")
                            }
                            Behavior::Drop | Behavior::DsnDrop => return,
                            Behavior::Pause => {
                                let _ = rx.recv_timeout(Duration::from_secs(10));
                                reply(&mut stream, "250 accepted\r\n");
                            }
                        }
                    } else if line == "QUIT\r\n" {
                        reply(&mut stream, "221 Bye\r\n");
                        break;
                    } else {
                        reply(&mut stream, "250 OK\r\n");
                    }
                }
            });
            Self {
                port,
                commands,
                data,
                release,
                thread,
            }
        }
    }
    fn reply(s: &mut TcpStream, value: &str) {
        let _ = s.write_all(value.as_bytes());
    }
    #[test]
    fn dsn_capabilities_parameters_and_selected_reports() {
        for (mode, supported, expected) in [
            ("required", true, 0),
            ("optional", true, 0),
            ("optional", false, 0),
            ("required", false, 5),
            ("off", true, 0),
        ] {
            let f = Fixture::new();
            let server = Server::start(if supported {
                Behavior::DsnAccept
            } else {
                Behavior::Accept
            });
            f.smtp(server.port);
            let id = f.prepare_dsn(mode);
            assert_eq!(f.call(&["send", &id, "--dry-run"]).0, 0);
            assert_eq!(f.call(&["status", &id]).1["result"]["attempts"], json!([]));
            let (code, result) = f.call(&["send", &id]);
            assert_eq!(code, expected, "{result}");
            server.thread.join().unwrap();
            let wire = server.commands.lock().unwrap().join("");
            let status = f.call(&["status", &id]).1["result"].clone();
            assert_eq!(status["attempts"].as_array().unwrap().len(), 1);
            let attempt = status["attempts"][0]["attempt_id"].as_str().unwrap();
            if mode == "required" && !supported {
                assert!(!wire.contains("MAIL FROM"));
                assert!(!wire.contains("DATA"));
                assert_eq!(result["error"]["code"], "dsn_unsupported");
                assert_eq!(status["state"], "unknown");
            } else {
                assert_eq!(status["state"], "accepted");
                if supported && mode != "off" {
                    assert!(wire.contains(&format!("ENVID={id}.{attempt}")));
                    assert!(wire.contains("RET=HDRS"));
                    assert!(wire.contains("NOTIFY=SUCCESS,FAILURE,DELAY"));
                    for (action, enhanced) in [
                        ("delivered", "2.0.0"),
                        ("delayed", "4.2.0"),
                        ("failed", "5.1.1"),
                        ("relayed", "2.0.0"),
                    ] {
                        let raw = dsn_raw(&id, attempt, "recipient@example.org", action, enhanced);
                        let path = f.dir.path().join("dsn.eml");
                        fs::write(&path, &raw).unwrap();
                        let args = [
                            "import-dsn",
                            &id,
                            "--attempt-id",
                            attempt,
                            "--file",
                            path.to_str().unwrap(),
                        ];
                        let (code, imported) = f.call(&args);
                        assert_eq!(code, 0, "{imported}");
                        assert_eq!(imported["result"]["report"]["action"], action);
                        let rid = imported["result"]["report_id"].as_str().unwrap();
                        assert_eq!(f.call(&["inspect-dsn", rid]).0, 0);
                        assert_eq!(f.call(&args).1["result"]["already_recorded"], true);
                    }
                    let after = f.call(&["status", &id]).1["result"].clone();
                    assert_eq!(after["attempts"], status["attempts"]);
                    assert_eq!(after["delivery_confirmed"], false);
                    assert_eq!(after["dsn"]["reports"].as_array().unwrap().len(), 4);
                } else {
                    assert!(!wire.contains("NOTIFY="));
                    assert!(!wire.contains("ENVID="));
                }
            }
            assert_eq!(f.call(&["send", &id]).0, 5);
        }
    }

    #[test]
    fn dsn_refusal_and_lost_reply_preserve_retry_policy() {
        for (behavior, exit, state) in [
            (Behavior::DsnReject, 6, "rejected"),
            (Behavior::DsnDrop, 8, "unknown"),
        ] {
            let f = Fixture::new();
            let server = Server::start(behavior);
            f.smtp(server.port);
            let id = f.prepare_dsn("required");
            assert_eq!(f.call(&["send", &id]).0, exit);
            let status = f.call(&["status", &id]).1["result"].clone();
            assert_eq!(status["state"], state);
            assert_eq!(status["dsn"]["events"].as_array().unwrap().len(), 4);
            if state == "unknown" {
                assert_eq!(f.call(&["send", &id]).0, 5);
            }
            server.thread.join().unwrap();
        }
    }

    #[test]
    fn unexpected_positive_data_reply_remains_unknown() {
        let f = Fixture::new();
        let server = Server::start(Behavior::UnexpectedFinal);
        f.smtp(server.port);
        let id = f.prepare();
        assert_eq!(f.call(&["send", &id]).0, 8);
        let status = f.call(&["status", &id]).1;
        assert_eq!(status["result"]["state"], "unknown");
        assert_eq!(
            status["result"]["attempts"][0]["server_response"]["code"],
            "354"
        );
        assert_eq!(f.call(&["send", &id]).0, 5);
        server.thread.join().unwrap();
    }
    #[test]
    fn vertical_scenario_and_duplicate_block() {
        let f = Fixture::new();
        let id = f.prepare();
        let server = Server::start(Behavior::Accept);
        f.smtp(server.port);
        assert_eq!(f.call(&["inspect", &id]).0, 0);
        assert_eq!(f.call(&["send", &id, "--dry-run"]).0, 0);
        assert!(server.data.try_recv().is_err());
        assert_eq!(f.call(&["status", &id]).1["result"]["attempts"], json!([]));
        let (code, v) = f.call(&["send", &id]);
        assert_eq!(code, 0, "{v}");
        assert_eq!(v["result"]["state"], "accepted");
        let bytes = server.data.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(String::from_utf8_lossy(&bytes).contains("Message-ID:"));
        let db = rusqlite::Connection::open(f.dir.path().join("store/mail.sqlite3")).unwrap();
        let stored: Vec<u8> = db
            .query_row("SELECT eml FROM packages", [], |r| r.get(0))
            .unwrap();
        // lettre appends CRLF as part of SMTP DATA termination.
        assert_eq!(bytes, [stored, b"\r\n".to_vec()].concat());
        assert_eq!(f.call(&["send", &id]).0, 5);
        assert_eq!(
            f.call(&["status", &id]).1["result"]["attempts"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        server.thread.join().unwrap();
    }
    #[test]
    fn explicit_refusal_and_unknown_outcome() {
        for (behavior, expected, state) in [
            (Behavior::Reject, 6, "rejected"),
            (Behavior::Drop, 8, "unknown"),
        ] {
            let f = Fixture::new();
            let id = f.prepare();
            let server = Server::start(behavior);
            f.smtp(server.port);
            let (code, v) = f.call(&["send", &id]);
            assert_eq!(code, expected, "{v}");
            assert!(!v.to_string().contains("secret-token"));
            let status = f.call(&["status", &id]).1;
            assert_eq!(status["result"]["state"], state);
            if state == "unknown" {
                assert_eq!(f.call(&["send", &id]).0, 5);
            } else {
                assert_eq!(
                    status["result"]["attempts"][0]["server_response"]["code"],
                    "550"
                );
            }
            server.thread.join().unwrap();
        }
    }
    #[test]
    fn concurrent_process_and_crash_after_data_never_retry() {
        let f = Fixture::new();
        let id = f.prepare();
        let server = Server::start(Behavior::Pause);
        f.smtp(server.port);
        let mut child = f
            .cmd(&["send", &id])
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        server.data.recv_timeout(Duration::from_secs(5)).unwrap();
        // The durable unknown attempt must already be visible while DATA is outstanding.
        assert_eq!(f.call(&["status", &id]).1["result"]["state"], "unknown");
        assert_eq!(f.call(&["send", &id]).0, 5);
        child.kill().unwrap();
        child.wait().unwrap();
        server.release.send(()).unwrap();
        server.thread.join().unwrap();
        assert_eq!(f.call(&["send", &id]).0, 5);
        assert_eq!(
            f.call(&["status", &id]).1["result"]["attempts"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }
    #[test]
    fn timeout_after_data_blocks_retry() {
        let f = Fixture::new();
        let id = f.prepare();
        let server = Server::start(Behavior::Pause);
        f.smtp(server.port);
        assert_eq!(f.call(&["send", &id]).0, 8);
        assert_eq!(f.call(&["send", &id]).0, 5);
        server.release.send(()).unwrap();
        server.thread.join().unwrap();
    }
    #[test]
    fn total_deadline_bounds_slow_greeting() {
        let f = Fixture::new();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        f.smtp(listener.local_addr().unwrap().port());
        let config = f.dir.path().join("config.toml");
        fs::write(
            &config,
            fs::read_to_string(&config).unwrap() + "total_timeout_seconds=1\n",
        )
        .unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            for _ in 0..40 {
                if stream.write_all(b"2").is_err() {
                    break;
                }
                thread::sleep(Duration::from_millis(100));
            }
        });
        let id = f.prepare();
        let started = std::time::Instant::now();
        assert_eq!(f.call(&["send", &id]).0, 8);
        assert!(started.elapsed() < Duration::from_secs(3));
        assert_eq!(f.call(&["status", &id]).1["result"]["state"], "unknown");
        assert_eq!(f.call(&["send", &id]).0, 5);
        server.join().unwrap();
    }

    #[test]
    fn orphaned_worker_deadline_releases_lock_and_blocks_resend() {
        use fs2::FileExt;
        let f = Fixture::new();
        let server = Server::start(Behavior::Pause);
        f.smtp(server.port);
        let config = f.dir.path().join("config.toml");
        fs::write(
            &config,
            fs::read_to_string(&config)
                .unwrap()
                .replace("timeout_seconds=1", "timeout_seconds=10")
                + "total_timeout_seconds=1\n",
        )
        .unwrap();
        let id = f.prepare();
        let mut parent = f
            .cmd(&["send", &id])
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        server.data.recv_timeout(Duration::from_secs(5)).unwrap();
        parent.kill().unwrap();
        parent.wait().unwrap();
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(f.dir.path().join("store").join(format!("{id}.lock")))
            .unwrap();
        let started = std::time::Instant::now();
        while lock.try_lock_exclusive().is_err() {
            assert!(started.elapsed() < Duration::from_secs(3));
            thread::sleep(Duration::from_millis(20));
        }
        FileExt::unlock(&lock).unwrap();
        let status = f.call(&["status", &id]).1;
        assert_eq!(status["result"]["state"], "unknown");
        assert_eq!(status["result"]["attempts"].as_array().unwrap().len(), 1);
        assert_eq!(f.call(&["send", &id]).0, 5);
        server.release.send(()).unwrap();
        server.thread.join().unwrap();
    }

    #[test]
    fn dry_run_does_not_connect_even_to_unavailable_port() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let f = Fixture::new();
        let id = f.prepare();
        f.smtp(port);
        assert_eq!(f.call(&["send", &id, "--dry-run"]).0, 0);
    }
}

#[test]
fn unsafe_configuration_is_rejected_without_an_attempt() {
    let f = Fixture::new();
    let id = f.prepare();
    for (host, security, credentials) in [
        ("192.0.2.1", "loopback_test", ""),
        ("localhost", "loopback_test", ""),
        ("smtp.example.org", "none", ""),
        (
            "smtp.example.org",
            "starttls",
            "username='synthetic'\npassword_env='AGENT_MAIL_TEST_MISSING_SECRET'\n",
        ),
        ("smtp.example.org", "tls", "password='secret-token'\n"),
    ] {
        fs::write(f.dir.path().join("config.toml"),format!("sender='sender@example.org'\n[smtp]\nhost='{host}'\nport=465\nsecurity='{security}'\n{credentials}")).unwrap();
        let (code, v) = decode(
            f.cmd(&["send", &id, "--dry-run"])
                .env_remove("AGENT_MAIL_TEST_MISSING_SECRET")
                .output()
                .unwrap(),
        );
        assert_eq!(code, 3, "{v}");
        assert!(!v.to_string().contains("secret-token"));
        assert_eq!(f.call(&["status", &id]).1["result"]["attempts"], json!([]));
    }
    #[cfg(not(feature = "test-smtp"))]
    {
        fs::write(f.dir.path().join("config.toml"),"sender='sender@example.org'\n[smtp]\nhost='127.0.0.1'\nport=2525\nsecurity='loopback_test'\n").unwrap();
        assert_eq!(f.call(&["send", &id, "--dry-run"]).0, 3);
    }
    // A structurally valid TLS config is checked without DNS or a socket.
    fs::write(f.dir.path().join("config.toml"),"sender='sender@example.org'\n[smtp]\nhost='smtp.example.invalid'\nport=465\nsecurity='tls'\n").unwrap();
    assert_eq!(f.call(&["send", &id, "--dry-run"]).0, 0);
}

#[test]
fn mime_parts_preserve_binary_pdf_and_exact_letter_bytes() {
    use base64::Engine;
    let f = Fixture::new();
    let pdf = b"%PDF-1.7\n\0\xff\r\nbinary\rfinal";
    let body = "Письмо\n.строка\r\nПодпись\n";
    fs::write(f.dir.path().join("resume.pdf"), pdf).unwrap();
    fs::write(f.dir.path().join("body.txt"), body).unwrap();
    f.prepare();
    let db = rusqlite::Connection::open(f.dir.path().join("store/mail.sqlite3")).unwrap();
    let eml: Vec<u8> = db
        .query_row("SELECT eml FROM packages", [], |r| r.get(0))
        .unwrap();
    let text = String::from_utf8(eml).unwrap();
    let parts: Vec<Vec<u8>> = text
        .split("Content-Transfer-Encoding: base64\r\n\r\n")
        .skip(1)
        .map(|part| {
            let encoded = part.split("\r\n--").next().unwrap().replace("\r\n", "");
            base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .unwrap()
        })
        .collect();
    assert_eq!(parts, vec![body.as_bytes().to_vec(), pdf.to_vec()]);
}

#[test]
fn input_limits_and_header_injection_leave_no_package() {
    let f = Fixture::new();
    for text in [
        "Subject\r\nBcc: victim@example.org".to_string(),
        "x".repeat(999),
    ] {
        fs::write(f.dir.path().join("subject.txt"), text).unwrap();
        let o = f
            .cmd(&["prepare", "--to", "recipient@example.org", "--subject-file"])
            .arg(f.dir.path().join("subject.txt"))
            .arg("--body-file")
            .arg(f.dir.path().join("body.txt"))
            .arg("--attach")
            .arg(f.dir.path().join("resume.pdf"))
            .output()
            .unwrap();
        assert_eq!(decode(o).0, 3);
    }
    fs::write(f.dir.path().join("subject.txt"), "valid").unwrap();
    let file = fs::OpenOptions::new()
        .write(true)
        .open(f.dir.path().join("resume.pdf"))
        .unwrap();
    file.set_len(25 * 1024 * 1024 + 1).unwrap();
    let o = f
        .cmd(&["prepare", "--to", "recipient@example.org", "--subject-file"])
        .arg(f.dir.path().join("subject.txt"))
        .arg("--body-file")
        .arg(f.dir.path().join("body.txt"))
        .arg("--attach")
        .arg(f.dir.path().join("resume.pdf"))
        .output()
        .unwrap();
    assert_eq!(decode(o).1["error"]["code"], "input_limit");
    assert_eq!(f.call(&["list"]).1["result"]["packages"], json!([]));
}

#[test]
fn future_storage_schema_reports_json_and_prevents_send() {
    let f = Fixture::new();
    let id = f.prepare();
    let db = rusqlite::Connection::open(f.dir.path().join("store/mail.sqlite3")).unwrap();
    db.pragma_update(None, "user_version", 999).unwrap();
    drop(db);
    for command in ["doctor", "inspect", "send"] {
        let args = if command == "doctor" {
            vec![command]
        } else {
            vec![command, &id]
        };
        let (code, result) = f.call(&args);
        assert_eq!(code, 9);
        assert_eq!(result["error"]["code"], "storage_schema");
    }
    let db = rusqlite::Connection::open(f.dir.path().join("store/mail.sqlite3")).unwrap();
    let count: i64 = db
        .query_row("SELECT count(*) FROM attempts", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

fn unknown_attempt(f: &Fixture, id: &str) -> Value {
    let manifest = f.call(&["inspect", id]).1["result"].clone();
    let db = rusqlite::Connection::open(f.dir.path().join("store/mail.sqlite3")).unwrap();
    db.execute("INSERT INTO attempts(id,package_id,started_at,outcome,message_id) VALUES('synthetic-attempt',?1,'2026-01-01T00:00:00+00:00','unknown',?2)",rusqlite::params![id,manifest["manifest"]["message_id"].as_str().unwrap()]).unwrap();
    json!({"schema_version":1,"package_id":id,"attempt_id":"synthetic-attempt","message_id":manifest["manifest"]["message_id"],"manifest_sha256":manifest["manifest_sha256"],"observed_at":"2026-01-02T12:00:00+03:00","checked_by":"synthetic-owner","source":"provider_log","conclusion":"accepted","evidence_ref":"synthetic-ticket-1","summary":"private synthetic evidence"})
}
fn evidence_file(f: &Fixture, e: &Value) -> String {
    let path = f.dir.path().join("evidence.json");
    fs::write(&path, serde_json::to_vec_pretty(e).unwrap()).unwrap();
    path.to_str().unwrap().into()
}
#[test]
fn reconciliation_is_append_only_private_idempotent_and_never_enables_retry() {
    let f = Fixture::new();
    let id = f.prepare();
    let mut evidence = unknown_attempt(&f, &id);
    let before = f.call(&["status", &id]).1["result"]["attempts"].clone();
    for conclusion in ["accepted", "not_accepted", "inconclusive"] {
        evidence["conclusion"] = json!(conclusion);
        let path = evidence_file(&f, &evidence);
        let (code, result) = f.call(&["reconcile", &id, "--evidence-file", &path]);
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["result"]["retry_authorized"], false);
        assert!(!result.to_string().contains("private synthetic evidence"));
        let record = result["result"]["record"]["record_id"].as_str().unwrap();
        let inspected = f.call(&["inspect-reconciliation", record]);
        assert_eq!(inspected.0, 0);
        assert_eq!(inspected.1["result"]["evidence"], evidence);
        let duplicate = f.call(&["reconcile", &id, "--evidence-file", &path]);
        assert_eq!(duplicate.0, 0);
        assert_eq!(duplicate.1["result"]["already_recorded"], true);
        assert_eq!(duplicate.1["result"]["record"]["record_id"], record);
        assert_eq!(f.call(&["send", &id]).0, 5);
        let status = f.call(&["status", &id]).1;
        assert_eq!(status["result"]["attempts"], before);
        assert_eq!(status["result"]["state"], "unknown");
        assert_eq!(status["result"]["delivery_confirmed"], false);
        assert!(!status.to_string().contains("private synthetic evidence"));
    }
    let db = rusqlite::Connection::open(f.dir.path().join("store/mail.sqlite3")).unwrap();
    assert_eq!(
        f.call(&["status", &id]).1["result"]["reconciliations"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert!(
        db.execute("UPDATE reconciliations SET evidence=x'00'", [])
            .is_err()
    );
    assert!(db.execute("DELETE FROM reconciliations", []).is_err());
}
#[test]
fn invalid_or_mismatched_reconciliation_never_records_evidence() {
    let f = Fixture::new();
    let id = f.prepare();
    let valid = unknown_attempt(&f, &id);
    for (field, value) in [
        ("package_id", json!("other")),
        ("attempt_id", json!("other")),
        ("message_id", json!("<other@example.org>")),
        ("manifest_sha256", json!("wrong")),
        ("observed_at", json!("2026-01-01T12:00:00")),
        ("schema_version", json!(2)),
        ("summary", json!("")),
        ("conclusion", json!("delivered")),
        ("unknown_field", json!("secret-marker")),
    ] {
        let mut e = valid.clone();
        e[field] = value;
        let path = evidence_file(&f, &e);
        let (code, result) = f.call(&["reconcile", &id, "--evidence-file", &path]);
        assert!(code == 3 || code == 5, "{result}");
        assert!(!result.to_string().contains("secret-marker"));
    }
    assert_eq!(
        f.call(&["status", &id]).1["result"]["reconciliations"],
        json!([])
    );
}
#[test]
fn reconciliation_respects_send_lock_and_detects_evidence_tampering() {
    let f = Fixture::new();
    let id = f.prepare();
    let e = unknown_attempt(&f, &id);
    let path = evidence_file(&f, &e);
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(f.dir.path().join(format!("store/{id}.lock")))
        .unwrap();
    fs2::FileExt::lock_exclusive(&lock).unwrap();
    assert_eq!(
        f.call(&["reconcile", &id, "--evidence-file", &path]).1["error"]["code"],
        "busy"
    );
    drop(lock);
    let result = f.call(&["reconcile", &id, "--evidence-file", &path]).1;
    let record = result["result"]["record"]["record_id"].as_str().unwrap();
    let db = rusqlite::Connection::open(f.dir.path().join("store/mail.sqlite3")).unwrap();
    let sql: String = db
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE name='reconciliations_no_update'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    db.execute_batch(
        "DROP TRIGGER reconciliations_no_update; UPDATE reconciliations SET evidence=x'00';",
    )
    .unwrap();
    db.execute_batch(&sql).unwrap();
    assert_eq!(f.call(&["inspect-reconciliation", record]).0, 4);
    assert_eq!(f.call(&["status", &id]).0, 4);
    assert_eq!(f.call(&["send", &id]).0, 5);
}
#[test]
fn prepared_or_definitive_attempts_cannot_be_reconciled_as_unknown() {
    let f = Fixture::new();
    let id = f.prepare();
    let e = unknown_attempt(&f, &id);
    let path = evidence_file(&f, &e);
    let db = rusqlite::Connection::open(f.dir.path().join("store/mail.sqlite3")).unwrap();
    db.execute(
        "UPDATE attempts SET outcome='accepted',finished_at='2026-01-01T00:01:00+00:00'",
        [],
    )
    .unwrap();
    assert_eq!(f.call(&["reconcile", &id, "--evidence-file", &path]).0, 5);
    let prepared = f.prepare();
    let mut e = e;
    e["package_id"] = json!(prepared);
    let path = evidence_file(&f, &e);
    assert_eq!(
        f.call(&["reconcile", &prepared, "--evidence-file", &path])
            .0,
        5
    );
}

#[cfg(unix)]
#[test]
fn unsafe_state_paths_and_nonregular_inputs_fail_without_modifying_targets() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let f = Fixture::new();
    let id = f.prepare();
    let outside = f.dir.path().join("untouched");
    fs::write(&outside, b"unchanged").unwrap();
    let lock = f.dir.path().join(format!("store/{id}.lock"));
    symlink(&outside, &lock).unwrap();
    assert_eq!(f.call(&["send", &id]).0, 7);
    fs::remove_file(&lock).unwrap();
    fs::hard_link(&outside, &lock).unwrap();
    assert_eq!(f.call(&["send", &id]).0, 7);
    assert_eq!(fs::read(&outside).unwrap(), b"unchanged");
    fs::remove_file(&lock).unwrap();
    let input = f.dir.path().join("input-link");
    symlink(&outside, &input).unwrap();
    assert_eq!(
        f.call(&["reconcile", &id, "--evidence-file", input.to_str().unwrap()])
            .0,
        3
    );
    let pipe = f.dir.path().join("pipe");
    rustix::fs::mkfifoat(
        rustix::fs::CWD,
        &pipe,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
    )
    .unwrap();
    assert_eq!(
        f.call(&["reconcile", &id, "--evidence-file", pipe.to_str().unwrap()])
            .0,
        3
    );
    let db = f.dir.path().join("store/mail.sqlite3");
    fs::set_permissions(&db, fs::Permissions::from_mode(0o666)).unwrap();
    assert_eq!(f.call(&["status", &id]).0, 0);
    assert_eq!(
        fs::metadata(&db).unwrap().permissions().mode() & 0o777,
        0o600
    );
    fs::rename(&db, f.dir.path().join("saved.sqlite3")).unwrap();
    symlink(&outside, &db).unwrap();
    assert_eq!(f.call(&["doctor"]).0, 7);
    assert_eq!(fs::read(&outside).unwrap(), b"unchanged");
}

#[test]
fn complete_offline_backup_preserves_reconciliation_and_retry_block() {
    let f = Fixture::new();
    let id = f.prepare();
    let evidence = unknown_attempt(&f, &id);
    let path = evidence_file(&f, &evidence);
    assert_eq!(f.call(&["reconcile", &id, "--evidence-file", &path]).0, 0);
    let before = f.call(&["status", &id]).1;
    // All writer subprocesses have exited. Copy only this synthetic test store.
    let restored = Fixture::new();
    fs::create_dir(restored.dir.path().join("store")).unwrap();
    fs::copy(
        f.dir.path().join("store/mail.sqlite3"),
        restored.dir.path().join("store/mail.sqlite3"),
    )
    .unwrap();
    let after = restored.call(&["status", &id]).1;
    assert_eq!(after["result"], before["result"]);
    assert_eq!(
        after["next"][0]["argv"][2],
        restored.dir.path().join("store").to_str().unwrap()
    );
    assert_eq!(restored.call(&["send", &id]).0, 5);
}

#[test]
fn discovery_needs_no_state_and_guidance_never_grants_send_permission() {
    let f = Fixture::new();
    let output = Command::new(binary())
        .env_clear()
        .args(["--home"])
        .arg(f.dir.path().join("absent"))
        .args(["describe", "--json"])
        .output()
        .unwrap();
    let (code, result) = decode(output);
    assert_eq!(code, 0);
    assert!(!f.dir.path().join("absent").exists());
    let commands = result["result"]["commands"].as_array().unwrap();
    assert!(!commands.iter().any(|c| c["name"] == "smtp-worker"));
    let send = commands.iter().find(|c| c["name"] == "send").unwrap();
    assert_eq!(send["requires_owner_instruction"], true);
    assert!(
        send["arguments"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["long"] == "dry-run")
    );
    assert!(
        send["arguments"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["long"] == "home")
    );
    let prepared = f.prepare();
    let status = f.call(&["status", &prepared]).1;
    assert_eq!(status["next"][1]["requires_owner_instruction"], true);
    assert!(
        status["next"]
            .as_array()
            .unwrap()
            .iter()
            .all(|step| step["argv"].is_null()
                || !step["argv"].as_array().unwrap().iter().any(|v| v == "send"))
    );
    unknown_attempt(&f, &prepared);
    let unknown = f.call(&["status", &prepared]).1;
    let argv = unknown["next"][0]["argv"].as_array().unwrap();
    assert!(argv.iter().any(|v| v == "status"));
    assert!(!argv.iter().any(|v| v == "send"));
    assert_eq!(argv[2], f.dir.path().join("store").to_str().unwrap());
}

fn operation_command(f: &Fixture, key: &str) -> Command {
    operation_with(f, key, "recipient@example.org", "resume.pdf")
}
fn operation_with(f: &Fixture, key: &str, recipient: &str, filename: &str) -> Command {
    let mut cmd = f.cmd(&["prepare", "--to", recipient, "--operation-key", key]);
    cmd.arg("--subject-file")
        .arg(f.dir.path().join("subject.txt"))
        .arg("--body-file")
        .arg(f.dir.path().join("body.txt"))
        .arg("--attach")
        .arg(f.dir.path().join(filename));
    cmd
}

#[test]
fn operation_keys_recover_exact_packages_conflict_and_preserve_unknown() {
    let f = Fixture::new();
    let (code, first) = decode(operation_command(&f, "synthetic/job-1").output().unwrap());
    assert_eq!(code, 0);
    let id = first["result"]["package_id"].as_str().unwrap();
    let (code, recovered) = decode(operation_command(&f, "synthetic/job-1").output().unwrap());
    assert_eq!(code, 0);
    assert_eq!(first["result"]["manifest"], recovered["result"]["manifest"]);
    assert_eq!(recovered["result"]["already_prepared"], true);
    assert_eq!(
        first["result"]["manifest_sha256"],
        recovered["result"]["manifest_sha256"]
    );
    unknown_attempt(&f, id);
    let recovered = decode(operation_command(&f, "synthetic/job-1").output().unwrap()).1;
    assert_eq!(recovered["result"]["state"], "unknown");
    assert!(!recovered["next"].to_string().contains("\"send\""));
    assert_eq!(f.call(&["send", id]).0, 5);
    {
        let db = rusqlite::Connection::open(f.dir.path().join("store/mail.sqlite3")).unwrap();
        db.execute("UPDATE attempts SET outcome='accepted',finished_at='2026-01-01T00:01:00+00:00' WHERE package_id=?1",[id]).unwrap();
    }
    let accepted = decode(operation_command(&f, "synthetic/job-1").output().unwrap()).1;
    assert_eq!(accepted["result"]["package_id"], id);
    assert_eq!(accepted["result"]["state"], "accepted");
    assert_eq!(f.call(&["send", id]).0, 5);
    assert_eq!(
        decode(
            operation_with(&f, "synthetic/job-1", "other@example.org", "resume.pdf")
                .output()
                .unwrap()
        )
        .0,
        5
    );
    fs::copy(
        f.dir.path().join("resume.pdf"),
        f.dir.path().join("renamed.pdf"),
    )
    .unwrap();
    assert_eq!(
        decode(
            operation_with(
                &f,
                "synthetic/job-1",
                "recipient@example.org",
                "renamed.pdf"
            )
            .output()
            .unwrap()
        )
        .0,
        5
    );
    for target in ["subject.txt", "body.txt", "resume.pdf"] {
        let path = f.dir.path().join(target);
        let original = fs::read(&path).unwrap();
        fs::write(&path, [original.as_slice(), b"changed"].concat()).unwrap();
        let (code, error) = decode(operation_command(&f, "synthetic/job-1").output().unwrap());
        assert_eq!(code, 5);
        assert_eq!(error["error"]["code"], "operation_conflict");
        fs::write(path, original).unwrap();
    }
    for extra in [
        vec!["--dsn", "required"],
        vec!["--company-ref", "different"],
        vec!["--vacancy-ref", "different"],
    ] {
        assert_eq!(
            decode(
                operation_command(&f, "synthetic/job-1")
                    .args(extra)
                    .output()
                    .unwrap()
            )
            .0,
            5
        );
    }
    fs::write(f.dir.path().join("html"), "<p>Different</p>").unwrap();
    assert_eq!(
        decode(
            operation_command(&f, "synthetic/job-1")
                .arg("--html-file")
                .arg(f.dir.path().join("html"))
                .output()
                .unwrap()
        )
        .0,
        5
    );
    fs::write(
        f.dir.path().join("config.toml"),
        "sender='different@example.org'\n",
    )
    .unwrap();
    assert_eq!(
        decode(operation_command(&f, "synthetic/job-1").output().unwrap()).0,
        5
    );
    assert_eq!(
        f.call(&["list"]).1["result"]["packages"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    for invalid in ["", "private letter with spaces", "key\n"] {
        assert_eq!(
            decode(operation_command(&f, invalid).output().unwrap()).0,
            3
        );
    }
}

#[test]
fn preparation_can_recover_after_the_original_output_is_lost() {
    let f = Fixture::new();
    let original = operation_command(&f, "synthetic-lost-output")
        .stdout(std::process::Stdio::null())
        .output()
        .unwrap();
    assert!(original.status.success());
    assert!(original.stdout.is_empty());
    let (code, recovered) = decode(
        operation_command(&f, "synthetic-lost-output")
            .output()
            .unwrap(),
    );
    assert_eq!(code, 0);
    assert_eq!(recovered["result"]["already_prepared"], true);
    assert_eq!(
        f.call(&["list"]).1["result"]["packages"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn concurrent_prepare_calls_publish_one_durable_operation() {
    let f = Fixture::new();
    let mut children: Vec<_> = (0..6)
        .map(|_| {
            operation_command(&f, "synthetic-concurrent")
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    let results: Vec<_> = children
        .drain(..)
        .map(|child| {
            let (code, value) = decode(child.wait_with_output().unwrap());
            assert_eq!(code, 0);
            value["result"].clone()
        })
        .collect();
    assert_eq!(
        results
            .iter()
            .filter(|r| r["already_prepared"] == false)
            .count(),
        1
    );
    assert!(
        results
            .iter()
            .all(|r| r["package_id"] == results[0]["package_id"]
                && r["manifest_sha256"] == results[0]["manifest_sha256"])
    );
    let db = rusqlite::Connection::open(f.dir.path().join("store/mail.sqlite3")).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM prepare_operations", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert!(db.execute("DELETE FROM prepare_operations", []).is_err());
    assert!(
        db.execute("UPDATE prepare_operations SET input_sha256='changed'", [])
            .is_err()
    );
}

#[test]
fn concurrent_conflicting_operations_do_not_publish_two_packages() {
    let f = Fixture::new();
    fs::write(
        f.dir.path().join("alternative.html"),
        "<p>Different input</p>",
    )
    .unwrap();
    let first = operation_command(&f, "synthetic-conflict")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let second = operation_command(&f, "synthetic-conflict")
        .arg("--html-file")
        .arg(f.dir.path().join("alternative.html"))
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut codes = vec![
        decode(first.wait_with_output().unwrap()).0,
        decode(second.wait_with_output().unwrap()).0,
    ];
    codes.sort();
    assert_eq!(codes, vec![0, 5]);
    assert_eq!(
        f.call(&["list"]).1["result"]["packages"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn export_preserves_frozen_mime_without_configuration_or_attempts() {
    let f = Fixture::new();
    let id = f.prepare();
    let manifest = f.call(&["inspect", &id]).1["result"]["manifest"].clone();
    fs::remove_file(f.dir.path().join("config.toml")).unwrap();
    fs::write(f.dir.path().join("body.txt"), "Changed source").unwrap();
    let output = f.dir.path().join("review.eml");
    let (code, value) = decode(
        f.cmd(&["export-eml", &id, "--output"])
            .arg(&output)
            .output()
            .unwrap(),
    );
    assert_eq!(code, 0);
    let bytes = fs::read(&output).unwrap();
    let db = rusqlite::Connection::open(f.dir.path().join("store/mail.sqlite3")).unwrap();
    let frozen: Vec<u8> = db
        .query_row("SELECT eml FROM packages WHERE id=?1", [&id], |r| r.get(0))
        .unwrap();
    assert_eq!(bytes, frozen);
    assert_eq!(
        value["result"]["eml_sha256"],
        format!("{:x}", Sha256::digest(&bytes))
    );
    assert_eq!(value["result"]["eml_sha256"], manifest["eml_sha256"]);
    assert_eq!(value["result"]["eml_bytes"], bytes.len());
    assert_eq!(value["result"]["network_used"], false);
    assert!(!value.to_string().contains("Synthetic letter"));
    assert_eq!(f.call(&["status", &id]).1["result"]["attempts"], json!([]));
    let (code, value) = decode(
        f.cmd(&["export-eml", &id, "--output"])
            .arg(&output)
            .output()
            .unwrap(),
    );
    assert_eq!(code, 3);
    assert_eq!(value["error"]["code"], "output_exists");
    assert_eq!(fs::read(&output).unwrap(), frozen);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&output).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    let missing = f.dir.path().join("missing/review.eml");
    assert_eq!(
        decode(
            f.cmd(&["export-eml", &id, "--output"])
                .arg(missing)
                .output()
                .unwrap()
        )
        .0,
        7
    );
}

#[cfg(unix)]
#[test]
fn export_never_follows_destination_symlink_or_overwrites_linked_file() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let id = f.prepare();
    let target = f.dir.path().join("private");
    fs::write(&target, "unchanged").unwrap();
    let link = f.dir.path().join("link.eml");
    symlink(&target, &link).unwrap();
    let hard = f.dir.path().join("hard.eml");
    fs::hard_link(&target, &hard).unwrap();
    let absent = f.dir.path().join("absent");
    let dangling = f.dir.path().join("dangling.eml");
    symlink(&absent, &dangling).unwrap();
    for path in [&link, &hard, &dangling] {
        let (code, value) = decode(
            f.cmd(&["export-eml", &id, "--output"])
                .arg(path)
                .output()
                .unwrap(),
        );
        assert_eq!(code, 3);
        assert_eq!(value["error"]["code"], "output_exists");
    }
    assert_eq!(fs::read_to_string(target).unwrap(), "unchanged");
    assert!(!absent.exists());
}

#[test]
fn concurrent_exports_create_one_complete_file() {
    let f = Fixture::new();
    let id = f.prepare();
    let output = f.dir.path().join("shared.eml");
    let spawn = || {
        f.cmd(&["export-eml", &id, "--output"])
            .arg(&output)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap()
    };
    let a = spawn();
    let b = spawn();
    let mut codes = [
        decode(a.wait_with_output().unwrap()).0,
        decode(b.wait_with_output().unwrap()).0,
    ];
    codes.sort();
    assert_eq!(codes, [0, 3]);
    let manifest = f.call(&["inspect", &id]).1["result"]["manifest"].clone();
    let bytes = fs::read(output).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        manifest["eml_sha256"]
    );
}

#[test]
fn recipient_reports_preserve_smtp_history_and_never_enable_retry() {
    for outcome in ["unknown", "accepted", "rejected"] {
        let f = Fixture::new();
        let id = f.prepare();
        let mut evidence = unknown_attempt(&f, &id);
        let db = rusqlite::Connection::open(f.dir.path().join("store/mail.sqlite3")).unwrap();
        if outcome != "unknown" {
            db.execute(
                "UPDATE attempts SET outcome=?1,finished_at='2026-01-01T01:00:00+00:00'",
                [outcome],
            )
            .unwrap();
        }
        evidence["source"] = json!("recipient_confirmation");
        evidence["conclusion"] = json!("received");
        let path = evidence_file(&f, &evidence);
        let before = f.call(&["status", &id]).1["result"]["attempts"].clone();
        assert_eq!(f.call(&["reconcile", &id, "--evidence-file", &path]).0, 3);
        let (code, result) = f.call(&["record-receipt", &id, "--evidence-file", &path]);
        if outcome == "rejected" {
            assert_eq!(code, 5);
            assert_eq!(
                f.call(&["status", &id]).1["result"]["recipient_reported_received"],
                false
            );
            continue;
        }
        assert_eq!(code, 0, "{result}");
        assert_eq!(result["result"]["state"], outcome);
        let record = result["result"]["record"]["record_id"].as_str().unwrap();
        assert_eq!(
            f.call(&["inspect-reconciliation", record]).1["result"]["evidence"],
            evidence
        );
        assert_eq!(
            f.call(&["record-receipt", &id, "--evidence-file", &path]).1["result"]["already_recorded"],
            true
        );
        let status = f.call(&["status", &id]).1["result"].clone();
        assert_eq!(status["attempts"], before);
        assert_eq!(status["state"], outcome);
        assert_eq!(status["recipient_reported_received"], true);
        assert_eq!(status["delivery_confirmed"], false);
        assert_eq!(f.call(&["send", &id]).0, 5);
        assert!(db.execute("DELETE FROM reconciliations", []).is_err());
        assert!(
            db.execute("UPDATE reconciliations SET evidence=x'00'", [])
                .is_err()
        );
    }
}

#[test]
fn recipient_reports_require_exact_binding_and_recipient_source() {
    let f = Fixture::new();
    let id = f.prepare();
    let mut base = unknown_attempt(&f, &id);
    base["source"] = json!("recipient_confirmation");
    base["conclusion"] = json!("received");
    for (field, value, exit) in [
        ("source", "owner_review", 3),
        ("conclusion", "accepted", 3),
        ("attempt_id", "wrong", 5),
        ("manifest_sha256", "wrong", 5),
        ("message_id", "wrong", 5),
        ("package_id", "wrong", 5),
        ("observed_at", "2026-01-02", 3),
    ] {
        let mut evidence = base.clone();
        evidence[field] = json!(value);
        let path = evidence_file(&f, &evidence);
        assert_eq!(
            f.call(&["record-receipt", &id, "--evidence-file", &path]).0,
            exit
        );
    }
    let status = f.call(&["status", &id]).1;
    assert_eq!(status["result"]["reconciliations"], json!([]));
    assert_eq!(status["result"]["recipient_reported_received"], false);
}

fn prepare_html(f: &Fixture, html: &[u8]) -> (i32, Value) {
    let path = f.dir.path().join("body.html");
    fs::write(&path, html).unwrap();
    decode(
        f.cmd(&["prepare", "--to", "recipient@example.org", "--subject-file"])
            .arg(f.dir.path().join("subject.txt"))
            .arg("--body-file")
            .arg(f.dir.path().join("body.txt"))
            .arg("--html-file")
            .arg(path)
            .arg("--attach")
            .arg(f.dir.path().join("resume.pdf"))
            .output()
            .unwrap(),
    )
}

#[test]
fn html_alternative_is_frozen_hashed_and_exported_with_plain_fallback() {
    use base64::Engine;
    let f = Fixture::new();
    let html =
        "<p>Письмо</p><p><a href=\"https://example.org/contact?a=1&amp;b=2\">Contact</a></p>\n";
    let (code, prepared) = prepare_html(&f, html.as_bytes());
    assert_eq!(code, 0, "{prepared}");
    let id = prepared["result"]["package_id"].as_str().unwrap();
    let manifest = &prepared["result"]["manifest"];
    assert_eq!(manifest["schema_version"], 2);
    assert_eq!(manifest["html"]["content"], html);
    assert_eq!(manifest["html"]["bytes"], html.len());
    assert_eq!(
        manifest["html"]["sha256"],
        format!("{:x}", Sha256::digest(html.as_bytes()))
    );
    fs::write(f.dir.path().join("body.html"), "Changed source").unwrap();
    assert_eq!(f.call(&["inspect", id]).1["result"]["manifest"], *manifest);
    let output = f.dir.path().join("review.eml");
    assert_eq!(
        decode(
            f.cmd(&["export-eml", id, "--output"])
                .arg(&output)
                .output()
                .unwrap()
        )
        .0,
        0
    );
    let mime = fs::read_to_string(&output).unwrap();
    assert!(mime.contains("multipart/alternative"));
    assert!(mime.contains("Content-Type: text/html"));
    let parts: Vec<Vec<u8>> = mime
        .split("Content-Transfer-Encoding: base64\r\n\r\n")
        .skip(1)
        .map(|p| {
            base64::engine::general_purpose::STANDARD
                .decode(p.split("\r\n--").next().unwrap().replace("\r\n", ""))
                .unwrap()
        })
        .collect();
    assert_eq!(
        parts,
        vec![
            fs::read(f.dir.path().join("body.txt")).unwrap(),
            html.as_bytes().to_vec(),
            fs::read(f.dir.path().join("resume.pdf")).unwrap()
        ]
    );
    // Simulate corruption with the outer digest recomputed; component hash still detects it.
    let db = rusqlite::Connection::open(f.dir.path().join("store/mail.sqlite3")).unwrap();
    let trigger: String = db
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE name='packages_no_update'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    db.execute_batch("DROP TRIGGER packages_no_update").unwrap();
    let mut corrupt = manifest.clone();
    corrupt["html"]["content"] = json!("altered");
    let raw = serde_json::to_vec(&corrupt).unwrap();
    db.execute(
        "UPDATE packages SET manifest=?1,digest=?2 WHERE id=?3",
        rusqlite::params![raw, format!("{:x}", Sha256::digest(&raw)), id],
    )
    .unwrap();
    db.execute_batch(&trigger).unwrap();
    assert_eq!(f.call(&["inspect", id]).0, 4);
    assert_eq!(f.call(&["send", id]).0, 4);
}

#[test]
fn invalid_html_creates_no_package_and_plain_manifest_stays_v1() {
    let f = Fixture::new();
    for html in [
        vec![],
        b" \n ".to_vec(),
        b"<p>\0</p>".to_vec(),
        vec![0xff],
        vec![b'x'; 1024 * 1024 + 1],
    ] {
        let (code, result) = prepare_html(&f, &html);
        assert_eq!(code, 3, "{result}");
    }
    assert_eq!(f.call(&["list"]).1["result"]["packages"], json!([]));
    let id = f.prepare();
    let m = f.call(&["inspect", &id]).1["result"]["manifest"].clone();
    assert_eq!(m["schema_version"], 1);
    assert!(m.get("html").is_none());
}

#[test]
fn version_json_identifies_binary_without_accessing_storage() {
    let f = Fixture::new();
    let (code, result) = f.call(&["--version"]);
    assert_eq!(code, 0);
    assert_eq!(result["result"]["version"], env!("CARGO_PKG_VERSION"));
    assert!(!f.dir.path().join("store").exists());
}

#[test]
fn json_subcommand_help_describes_requested_arguments_without_state() {
    let mut command = Command::new(binary());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.arg0("synthetic-private-launcher");
    }
    let output = command
        .args(["prepare", "--help", "--json"])
        .env_remove("HOME")
        .env_remove("AGENT_MAIL_HOME")
        .output()
        .unwrap();
    let (code, value) = decode(output);
    assert_eq!(code, 0);
    assert_eq!(value["ok"], true);
    let help = value["result"]["help"].as_str().unwrap();
    assert!(help.contains("--subject-file"));
    assert!(help.contains("--operation-key"));
    assert!(help.contains("Usage: agent-mail prepare"));
    assert!(!help.contains("Commands:"));
}

#[cfg(unix)]
#[test]
fn json_help_output_failure_returns_storage_exit_without_panic() {
    let f = Fixture::new();
    for args in [["--help"], ["invalid-secret-argument"], ["--version"]] {
        let output = f
            .cmd(&args)
            .stdout(
                fs::OpenOptions::new()
                    .write(true)
                    .open("/dev/full")
                    .unwrap(),
            )
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(7));
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn invalid_authentication_identifiers_fail_before_attempt_creation() {
    let f = Fixture::new();
    let id = f.prepare();
    for (user, variable) in [
        (" ", "SYNTHETIC"),
        ("synthetic", ""),
        ("synthetic", "BAD=NAME"),
    ] {
        fs::write(f.dir.path().join("config.toml"), format!("sender='sender@example.org'\n[smtp]\nhost='smtp.example.invalid'\nport=465\nsecurity='tls'\nusername='{user}'\npassword_env='{variable}'\n")).unwrap();
        assert_eq!(f.call(&["doctor"]).1["result"]["smtp_issue"], "validation");
        assert_eq!(f.call(&["send", &id]).0, 3);
        assert_eq!(f.call(&["status", &id]).1["result"]["attempts"], json!([]));
    }
}

#[test]
fn worker_deadline_before_attempt_leaves_prepared_package() {
    let f = Fixture::new();
    let id = f.prepare();
    let db = rusqlite::Connection::open(f.dir.path().join("store/mail.sqlite3")).unwrap();
    db.execute_batch("BEGIN IMMEDIATE").unwrap();
    let started = std::time::Instant::now();
    let output = f.cmd(&["smtp-worker", &id, "1"]).output().unwrap();
    assert_eq!(output.status.code(), Some(8));
    assert!(output.stdout.is_empty());
    assert!(started.elapsed() < std::time::Duration::from_secs(3));
    db.execute_batch("ROLLBACK").unwrap();
    assert_eq!(f.call(&["status", &id]).1["result"]["attempts"], json!([]));
}

#[test]
fn invalid_total_deadline_creates_no_attempts() {
    let f = Fixture::new();
    let id = f.prepare();
    for seconds in [0, 601] {
        fs::write(f.dir.path().join("config.toml"),format!("sender='sender@example.org'\n[smtp]\nhost='smtp.example.invalid'\nport=465\nsecurity='tls'\ntotal_timeout_seconds={seconds}\n")).unwrap();
        assert_eq!(f.call(&["send", &id]).0, 3);
        assert_eq!(f.call(&["status", &id]).1["result"]["attempts"], json!([]));
    }
}

fn dsn_raw(package: &str, attempt: &str, recipient: &str, action: &str, status: &str) -> String {
    format!(
        "MIME-Version: 1.0\r\nContent-Type: multipart/report; report-type=delivery-status; boundary=dsn-test\r\n\r\n--dsn-test\r\nContent-Type: text/plain\r\n\r\nSynthetic report\r\n--dsn-test\r\nContent-Type: message/delivery-status\r\n\r\nReporting-MTA: dns; synthetic.example.org\r\nOriginal-Envelope-Id: {package}.{attempt}\r\n\r\nFinal-Recipient: rfc822; {recipient}\r\nAction: {action}\r\nStatus: {status}\r\n\r\n--dsn-test--\r\n"
    )
}
#[test]
fn dsn_policy_is_frozen_and_invalid_report_binding_is_rejected() {
    let f = Fixture::new();
    let id = f.prepare_dsn("required");
    let manifest = f.call(&["inspect", &id]).1["result"]["manifest"].clone();
    assert_eq!(manifest["schema_version"], 3);
    assert_eq!(manifest["dsn"]["ret"], "HDRS");
    let db = rusqlite::Connection::open(f.dir.path().join("store/mail.sqlite3")).unwrap();
    db.execute("INSERT INTO attempts VALUES('synthetic',?1,'2026-01-01T00:00:00+00:00',NULL,'unknown',?2,NULL)",rusqlite::params![id,manifest["message_id"].as_str().unwrap()]).unwrap();
    let payload = br#"{"dsn_parameters":true}"#;
    db.execute("INSERT INTO dsn_events VALUES('event','synthetic','parameters_about_to_send','2026-01-01T00:00:00+00:00',?1,?2)",rusqlite::params![payload,format!("{:x}",Sha256::digest(payload))]).unwrap();
    for (package, attempt, recipient, action, enhanced, expected) in [
        (
            id.as_str(),
            "synthetic",
            "wrong@example.org",
            "delivered",
            "2.0.0",
            5,
        ),
        (
            "wrong",
            "synthetic",
            "recipient@example.org",
            "delivered",
            "2.0.0",
            5,
        ),
        (
            id.as_str(),
            "wrong",
            "recipient@example.org",
            "delivered",
            "2.0.0",
            5,
        ),
        (
            id.as_str(),
            "synthetic",
            "recipient@example.org",
            "failed",
            "2.0.0",
            3,
        ),
        (
            id.as_str(),
            "synthetic",
            "recipient@example.org",
            "unknown",
            "2.0.0",
            3,
        ),
    ] {
        let path = f.dir.path().join("report.eml");
        fs::write(
            &path,
            dsn_raw(package, attempt, recipient, action, enhanced),
        )
        .unwrap();
        assert_eq!(
            f.call(&[
                "import-dsn",
                &id,
                "--attempt-id",
                "synthetic",
                "--file",
                path.to_str().unwrap()
            ])
            .0,
            expected
        );
    }
    let path = f.dir.path().join("report.eml");
    fs::write(
        &path,
        dsn_raw(
            &id,
            "synthetic",
            "recipient@example.org",
            "delivered",
            "2.0.0",
        ),
    )
    .unwrap();
    let (code, result) = f.call(&[
        "import-dsn",
        &id,
        "--attempt-id",
        "synthetic",
        "--file",
        path.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{result}");
    assert_eq!(f.call(&["status", &id]).1["result"]["state"], "unknown");
    assert_eq!(f.call(&["send", &id]).0, 5);
    assert!(db.execute("DELETE FROM dsn_reports", []).is_err());
    assert!(
        db.execute("UPDATE dsn_events SET payload=x'00'", [])
            .is_err()
    );
}

#[test]
fn unsolicited_bounce_requires_explicit_mode_and_preserves_smtp_acceptance() {
    let f = Fixture::new();
    let id = f.prepare();
    let (_, original) = f.call(&["inspect", &id]);
    let mid = original["result"]["manifest"]["message_id"]
        .as_str()
        .unwrap();
    let db = rusqlite::Connection::open(f.dir.path().join("store/mail.sqlite3")).unwrap();
    db.execute("INSERT INTO attempts VALUES('selected',?1,'2026-01-01T00:00:00+00:00','2026-01-01T00:00:01+00:00','accepted',?2,'250')",rusqlite::params![id,mid]).unwrap();
    drop(db);
    let raw = format!(
        "Content-Type: multipart/report; report-type=delivery-status; boundary=x\r\n\r\n--x\r\nContent-Type: text/plain\r\n\r\nSynthetic\r\n--x\r\nContent-Type: message/delivery-status\r\n\r\nReporting-MTA: dns; example.org\r\n\r\nFinal-Recipient: rfc822; recipient@example.org\r\nAction: failed\r\nStatus: 5.0.0\r\n\r\n--x\r\nContent-Type: text/rfc822-headers\r\n\r\nMessage-ID: {mid}\r\nFrom: sender@example.org\r\nTo: recipient@example.org\r\n\r\n--x--\r\n"
    );
    let path = f.dir.path().join("bounce.eml");
    fs::write(&path, raw).unwrap();
    assert_eq!(
        f.call(&[
            "import-dsn",
            &id,
            "--attempt-id",
            "selected",
            "--file",
            path.to_str().unwrap()
        ])
        .0,
        5
    );
    let args = [
        "import-dsn",
        &id,
        "--attempt-id",
        "selected",
        "--file",
        path.to_str().unwrap(),
        "--correlate-by-headers",
    ];
    let (code, first) = f.call(&args);
    assert_eq!(code, 0);
    assert_eq!(first["result"]["correlation"], "original_headers");
    let (code, replay) = f.call(&args);
    assert_eq!(code, 0);
    assert_eq!(replay["result"]["already_recorded"], true);
    let (_, status) = f.call(&["status", &id]);
    assert_eq!(status["result"]["state"], "accepted");
    assert_eq!(
        status["result"]["delivery_observation"]["outcome"],
        "failed_reported"
    );
    assert_eq!(status["next"][0]["action"], "review_delivery_report");
    assert_eq!(status["result"]["delivery_confirmed"], false);
    assert_eq!(f.call(&["send", &id]).0, 5);
}

#[path = "cases/archive.rs"]
mod archive;

#[path = "cases/preflight.rs"]
mod preflight;

#[path = "cases/rename.rs"]
mod rename;
