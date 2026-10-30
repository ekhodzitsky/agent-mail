//! Offline configuration readiness; never open operational storage or Keyring.
use crate::{Result, config::config, smtp::transport_using_tls};
use lettre::transport::smtp::client::TlsParameters;
use serde_json::{Value, json};
use std::path::Path;

pub(super) fn preflight(path: &Path) -> Result<Value> {
    let mut checks =
        json!({"configuration":"not_checked","smtp":"blocked","credential_environment":"blocked"});
    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            checks["configuration"] = json!("missing")
        }
        Err(_) => checks["configuration"] = json!("unavailable"),
        Ok(_) => match config(path) {
            Err(error) => {
                checks["configuration"] = json!(if error.code == "configuration" {
                    "unavailable"
                } else {
                    "invalid"
                })
            }
            Ok(cfg) => {
                checks["configuration"] = json!("ready");
                if cfg.smtp.is_none() {
                    checks["smtp"] = json!("not_configured");
                } else if transport_using_tls(
                    &cfg,
                    |host| TlsParameters::new(host.to_owned()),
                    |_| Ok("preflight-placeholder".into()),
                )
                .is_err()
                {
                    checks["smtp"] = json!("invalid");
                } else {
                    checks["smtp"] = json!("ready");
                    checks["credential_environment"] =
                        json!(
                            match cfg.smtp.as_ref().and_then(|s| s.password_env.as_ref()) {
                                None => "not_required",
                                Some(name)
                                    if std::env::var_os(name).is_some_and(|value| value
                                        .to_str()
                                        .is_some_and(|text| !text.is_empty())) =>
                                    "available",
                                Some(_) => "unavailable",
                            }
                        );
                }
            }
        },
    }
    let ready = checks["configuration"] == "ready"
        && checks["smtp"] == "ready"
        && matches!(
            checks["credential_environment"].as_str(),
            Some("available" | "not_required")
        );
    Ok(
        json!({"preflight_schema_version":1,"ready":ready,"checks":checks,"network_used":false,"storage_opened":false,"credential_content_validated":false,"remote_authentication_verified":false}),
    )
}
