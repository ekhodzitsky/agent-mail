//! Typed SMTP DSN submission; no report parsing or report persistence.
use super::{DsnMode, envid, event};
use crate::{
    Result,
    config::{Config, address},
    err, invalid,
    package::Package,
    store::Store,
};
use lettre::transport::smtp::{
    authentication::{Credentials, Mechanism},
    client::{SmtpConnection, TlsParameters},
    commands::{Data, Ehlo, Mail, Rcpt},
    extension::{ClientId, Extension, MailBodyParameter, MailParameter, RcptParameter},
    response::Response,
};
use serde_json::json;
use std::time::Duration;

pub(crate) fn submit(
    store: &Store,
    cfg: &Config,
    p: &Package,
    attempt: &str,
) -> Result<std::result::Result<Response, lettre::transport::smtp::Error>> {
    submit_with_tls(
        store,
        cfg,
        p,
        attempt,
        |host| TlsParameters::new(host.to_string()),
        |variable| {
            std::env::var(variable)
                .map_err(|_| err("configuration", "Credential environment unavailable", 3))
        },
    )
}
pub(crate) fn submit_with_tls(
    store: &Store,
    cfg: &Config,
    p: &Package,
    attempt: &str,
    make_tls: impl FnOnce(&str) -> std::result::Result<TlsParameters, lettre::transport::smtp::Error>,
    password_from_env: impl FnOnce(&str) -> Result<String>,
) -> Result<std::result::Result<Response, lettre::transport::smtp::Error>> {
    let policy = p.manifest.dsn.as_ref().ok_or_else(invalid)?;
    let s = cfg.smtp.as_ref().ok_or_else(invalid)?;
    let envelope_id = envid(&p.manifest.id, attempt);
    event(
        store,
        attempt,
        "policy",
        json!({"mode":policy.mode,"envid":envelope_id,"notify":policy.notify,"ret":policy.ret}),
    )?;
    let hello = ClientId::Domain("localhost".into());
    let tls = make_tls(&s.host).map_err(|_| invalid())?;
    macro_rules! net {
        ($call:expr) => {
            match $call {
                Ok(v) => v,
                Err(e) => return Ok(Err(e)),
            }
        };
    }
    let mut conn = net!(SmtpConnection::connect(
        (s.host.as_str(), s.port),
        Some(Duration::from_secs(s.timeout_seconds)),
        &hello,
        if s.security == "tls" {
            Some(&tls)
        } else {
            None
        },
        None
    ));
    if s.security == "starttls" {
        net!(conn.starttls(&tls, &hello));
    }
    // Only a test-feature numeric loopback path can reach plaintext (validated by transport()).
    let capabilities = net!(conn.command(Ehlo::new(hello.clone())));
    if capabilities.code().to_string() != "250" {
        return Err(err(
            "outcome_unknown",
            "Unexpected EHLO response; inspect status",
            8,
        ));
    }
    let capable = capabilities.message().skip(1).any(|line| {
        line.split_whitespace()
            .next()
            .is_some_and(|word| word.eq_ignore_ascii_case("DSN"))
    });
    event(store, attempt, "capability", json!({"supported":capable}))?;
    if !capable && policy.mode == DsnMode::Required {
        return Err(err(
            "dsn_unsupported",
            "SMTP server did not advertise required DSN; inspect status",
            5,
        ));
    }
    if let (Some(user), Some(variable)) = (&s.username, &s.password_env) {
        let password = password_from_env(variable)?;
        net!(conn.auth(
            &[Mechanism::Plain, Mechanism::Login],
            &Credentials::new(user.clone(), password)
        ));
    }
    let mut mail = Vec::new();
    if !p.eml.is_ascii() {
        if !conn.server_info().supports_feature(Extension::EightBitMime) {
            return Err(err(
                "outcome_unknown",
                "SMTP does not support required message encoding; inspect status",
                8,
            ));
        }
        mail.push(MailParameter::Body(MailBodyParameter::EightBitMime));
    }
    let mut rcpt = Vec::new();
    if capable {
        mail.push(MailParameter::Other {
            keyword: "RET".into(),
            value: Some(policy.ret.clone()),
        });
        mail.push(MailParameter::Other {
            keyword: "ENVID".into(),
            value: Some(envelope_id),
        });
        rcpt.push(RcptParameter::Other {
            keyword: "NOTIFY".into(),
            value: Some(policy.notify.clone()),
        });
    }
    event(
        store,
        attempt,
        "parameters_about_to_send",
        json!({"dsn_parameters":capable}),
    )?;
    let response = net!(conn.command(Mail::new(Some(address(&p.manifest.sender)?), mail)));
    if response.code().to_string() != "250" {
        return Err(err(
            "outcome_unknown",
            "Unexpected MAIL response; inspect status",
            8,
        ));
    }
    let response = net!(conn.command(Rcpt::new(address(&p.manifest.recipient)?, rcpt)));
    if !matches!(response.code().to_string().as_str(), "250" | "251") {
        return Err(err(
            "outcome_unknown",
            "Unexpected RCPT response; inspect status",
            8,
        ));
    }
    event(
        store,
        attempt,
        "parameters_accepted",
        json!({"dsn_parameters":capable}),
    )?;
    let response = net!(conn.command(Data));
    if response.code().to_string() != "354" {
        return Err(err(
            "outcome_unknown",
            "Unexpected DATA response; inspect status",
            8,
        ));
    }
    let result = conn.message(&p.eml);
    // Drop the connection; no post-acceptance command can turn a sent message into a retryable rejection.
    Ok(result)
}
