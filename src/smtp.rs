//! Verified transport setup and durable conservative submission.
use crate::{
    Result,
    config::{Config, address},
    err,
    files::private_file,
    invalid, now, storage,
    store::Store,
};
use fs2::FileExt;
use lettre::{
    SmtpTransport, Transport,
    address::Envelope,
    transport::smtp::{
        authentication::Credentials,
        client::{Tls, TlsParameters},
    },
};
use rusqlite::params;
use serde_json::{Value, json};
use std::{fs::File, time::Duration};
use uuid::Uuid;

pub(super) fn transport(cfg: &Config) -> Result<SmtpTransport> {
    transport_using_tls(
        cfg,
        |host| TlsParameters::new(host.to_string()),
        |variable| {
            std::env::var(variable).map_err(|_| {
                err(
                    "configuration",
                    "SMTP credential environment variable unavailable",
                    3,
                )
            })
        },
    )
}
// The production caller always uses the mail library's default verified roots. Tests
// supply an ephemeral CA through this private seam; no configuration bypass exists.
pub(super) fn transport_using_tls(
    cfg: &Config,
    make_tls: impl FnOnce(&str) -> std::result::Result<TlsParameters, lettre::transport::smtp::Error>,
    password_from_env: impl FnOnce(&str) -> Result<String>,
) -> Result<SmtpTransport> {
    let s = cfg
        .smtp
        .as_ref()
        .ok_or_else(|| err("configuration", "SMTP is not configured", 3))?;
    if s.host.is_empty()
        || !(1..=600).contains(&s.total_timeout_seconds)
        || s.port == 0
        || !(1..=300).contains(&s.timeout_seconds)
        || s.username
            .as_ref()
            .is_some_and(|user| user.trim().is_empty() || user.contains(['\r', '\n', '\0']))
        || s.password_env
            .as_ref()
            .is_some_and(|name| name.is_empty() || name.contains(['=', '\0']))
        || s.username.is_some() != s.password_env.is_some()
    {
        return Err(invalid());
    }
    let mut builder = match s.security.as_str() {
        "tls" => SmtpTransport::builder_dangerous(&s.host)
            .tls(Tls::Wrapper(make_tls(&s.host).map_err(|_| invalid())?)),
        "starttls" => SmtpTransport::builder_dangerous(&s.host)
            .tls(Tls::Required(make_tls(&s.host).map_err(|_| invalid())?)),
        #[cfg(feature = "test-smtp")]
        "loopback_test" => {
            let ip: std::net::IpAddr = s.host.parse().map_err(|_| invalid())?;
            if !ip.is_loopback() || s.username.is_some() {
                return Err(invalid());
            }
            SmtpTransport::builder_dangerous(ip.to_string())
        }
        _ => return Err(invalid()),
    }
    .port(s.port)
    .timeout(Some(Duration::from_secs(s.timeout_seconds)));
    if let (Some(user), Some(variable)) = (&s.username, &s.password_env) {
        let password = password_from_env(variable)?;
        if password.is_empty() {
            return Err(invalid());
        }
        builder = builder.credentials(Credentials::new(user.clone(), password));
    }
    Ok(builder.build())
}
pub(super) fn package_lock(store: &Store, id: &str) -> Result<File> {
    let uuid = Uuid::parse_str(id).map_err(|_| invalid())?;
    let lock = private_file(&store.home.join(format!("{uuid}.lock")))?;
    lock.try_lock_exclusive()
        .map_err(|_| err("busy", "Another process is operating on this package", 5))?;
    Ok(lock)
}
pub(super) fn send(store: &Store, cfg: &Config, id: &str, dry_run: bool) -> Result<Value> {
    send_using(store, cfg, id, dry_run, transport)
}
pub(super) fn send_using<T>(
    store: &Store,
    cfg: &Config,
    id: &str,
    dry_run: bool,
    build: impl FnOnce(&Config) -> Result<T>,
) -> Result<Value>
where
    T: Transport<
            Ok = lettre::transport::smtp::response::Response,
            Error = lettre::transport::smtp::Error,
        >,
{
    let package = store.load(id)?;
    let _lock = package_lock(store, id)?;
    let blocked:bool=store.conn.query_row("SELECT EXISTS(SELECT 1 FROM attempts WHERE package_id=?1 AND outcome IN ('accepted','unknown'))",[id],|r|r.get(0)).map_err(|_|storage())?;
    if blocked {
        return Err(err(
            "send_blocked",
            "An accepted or unresolved attempt prevents resubmission",
            5,
        ));
    }
    if cfg.sender != package.manifest.sender {
        return Err(err(
            "configuration",
            "Configured sender differs from frozen package sender",
            3,
        ));
    }
    let smtp = build(cfg)?; // Construction validates settings, never connects.
    let envelope = Envelope::new(
        Some(address(&package.manifest.sender)?),
        vec![address(&package.manifest.recipient)?],
    )
    .map_err(|_| invalid())?;
    if dry_run {
        return Ok(
            json!({"package_id":id,"state":"prepared","dry_run":true,"network_used":false,"manifest_sha256":package.manifest_sha256}),
        );
    }
    let attempt = Uuid::new_v4().to_string();
    // Autocommit + synchronous EXTRA is the durable barrier BEFORE any network I/O.
    store.conn.execute("INSERT INTO attempts(id,package_id,started_at,outcome,message_id) VALUES(?1,?2,?3,'unknown',?4)",params![attempt,id,now(),package.manifest.message_id]).map_err(|_|storage())?;
    let response = if package.manifest.dsn.is_some() {
        crate::dsn::submit(store, cfg, &package, &attempt)?
    } else {
        smtp.send_raw(&envelope, &package.eml)
    };
    let (outcome, code) = match response {
        Ok(response) => {
            let code = response.code().to_string();
            // lettre accepts intermediate 3xx replies as positive. Only the
            // final DATA 250 is an acceptance; unexpected replies stay blocked.
            let outcome = if code == "250" { "accepted" } else { "unknown" };
            (outcome, Some(code))
        }
        Err(error) => {
            let code = error.status().map(|c| c.to_string());
            if error.is_permanent() || error.is_transient() {
                ("rejected", code)
            } else {
                ("unknown", code)
            }
        }
    };
    // Never expose SMTP free text: servers can echo passwords, recipients or message data.
    store
        .conn
        .execute(
            "UPDATE attempts SET finished_at=?1,outcome=?2,smtp_code=?3 WHERE id=?4",
            params![now(), outcome, code, attempt],
        )
        .map_err(|_| storage())?;
    match outcome {
        "accepted" => store.status(id),
        "rejected" => Err(err(
            "smtp_rejected",
            "SMTP explicitly rejected submission; inspect status for attempt and reply code",
            6,
        )),
        _ => Err(err(
            "outcome_unknown",
            "Submission outcome is unknown; automatic retry is blocked; inspect status",
            8,
        )),
    }
}

pub(super) fn supervised_send(
    store: &Store,
    cfg: &Config,
    config_path: &std::path::Path,
    id: &str,
) -> Result<Value> {
    supervised_send_with_executable(
        store,
        cfg,
        config_path,
        id,
        &std::env::current_exe().map_err(|_| storage())?,
    )
}
pub(super) fn supervised_send_with_executable(
    store: &Store,
    cfg: &Config,
    config_path: &std::path::Path,
    id: &str,
    executable: &std::path::Path,
) -> Result<Value> {
    store.load(id)?;
    {
        let _guard = package_lock(store, id)?;
        let blocked: bool = store.conn.query_row("SELECT EXISTS(SELECT 1 FROM attempts WHERE package_id=?1 AND outcome IN ('accepted','unknown'))", [id], |r|r.get(0)).map_err(|_|storage())?;
        if blocked {
            return Err(err(
                "send_blocked",
                "An accepted or unresolved attempt prevents resubmission",
                5,
            ));
        }
    }
    transport(cfg)?; // Offline validation before launching any network process.
    let seconds = cfg.smtp.as_ref().ok_or_else(invalid)?.total_timeout_seconds;
    let started = std::time::Instant::now();
    let mut child = std::process::Command::new(executable)
        .arg("--home")
        .arg(&store.home)
        .arg("--config")
        .arg(config_path)
        .arg("--json")
        .arg("smtp-worker")
        .arg(id)
        .arg(seconds.to_string())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|_| storage())?;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    return store.status(id);
                }
                // Worker replies are fixed small envelopes (no status/mail data).
                let mut raw = String::new();
                if let Some(stdout) = child.stdout.take() {
                    use std::io::Read;
                    stdout
                        .take(8192)
                        .read_to_string(&mut raw)
                        .map_err(|_| storage())?;
                }
                let parsed: Option<Value> = serde_json::from_str(&raw).ok();
                let code = parsed.as_ref().and_then(|v| v["error"]["code"].as_str());
                return Err(match code {
                    Some("dsn_unsupported") => err(
                        "dsn_unsupported",
                        "SMTP server did not advertise required DSN; inspect status",
                        5,
                    ),
                    Some("busy") => err("busy", "Another process is operating on this package", 5),
                    Some("send_blocked") => err(
                        "send_blocked",
                        "An accepted or unresolved attempt prevents resubmission",
                        5,
                    ),
                    Some("smtp_rejected") => err(
                        "smtp_rejected",
                        "SMTP explicitly rejected submission; inspect status",
                        6,
                    ),
                    Some("configuration" | "validation") => invalid(),
                    Some("integrity") => crate::integrity(),
                    Some("storage") => storage(),
                    _ => err(
                        "outcome_unknown",
                        "Network worker ended without a definitive result; inspect status",
                        8,
                    ),
                });
            }
            Ok(None) if started.elapsed() < Duration::from_secs(seconds) => {
                std::thread::sleep(Duration::from_millis(10))
            }
            other => {
                // Kill then reap before returning/releasing supervision. If a kill
                // races normal exit, wait still establishes worker termination.
                let killed = child.kill();
                let waited = child.wait();
                if waited.is_err()
                    || (killed.is_err() && child.try_wait().map_err(|_| storage())?.is_none())
                {
                    return Err(storage());
                }
                if other.is_err() {
                    return Err(storage());
                }
                return Err(err(
                    "outcome_unknown",
                    "Submission deadline exceeded; inspect durable status before further action",
                    8,
                ));
            }
        }
    }
}
