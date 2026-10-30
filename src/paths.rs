//! Select existing history before opening storage; never reset resend protection.
use crate::{Result, err, invalid};
use std::path::PathBuf;

pub(crate) fn home(explicit: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(path);
    }
    let current = std::env::var_os("AGENT_MAIL_HOME");
    // Compatibility identifiers are intentionally stable, never product branding.
    let legacy = std::env::var_os("AGENT_MAILER_HOME");
    match (current, legacy) {
        (Some(a), Some(b)) if a != b => return Err(ambiguous()),
        (Some(path), _) | (_, Some(path)) if !path.is_empty() => return Ok(path.into()),
        (Some(_), _) | (_, Some(_)) => return Err(invalid()),
        _ => {}
    }
    let base = PathBuf::from(std::env::var_os("HOME").ok_or_else(invalid)?).join(".local/share");
    let current = base.join("agent-mail");
    let legacy = base.join("agent-mailer");
    // symlink_metadata also sees dangling links: never hide an existing path.
    let exists = |path: &PathBuf| match path.symlink_metadata() {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(invalid()),
    };
    match (exists(&current)?, exists(&legacy)?) {
        (true, true) => Err(ambiguous()),
        (false, true) => Ok(legacy),
        _ => Ok(current),
    }
}

fn ambiguous() -> crate::Error {
    err(
        "home_ambiguous",
        "Multiple state locations selected; specify --home explicitly",
        3,
    )
}
