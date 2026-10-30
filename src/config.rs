//! Configuration parsing and envelope-address validation.
use crate::{Result, err, files::text_file, invalid};
use serde::Deserialize;
use std::path::Path;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Config {
    pub(super) sender: String,
    pub(super) smtp: Option<Smtp>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Smtp {
    pub(super) host: String,
    pub(super) port: u16,
    pub(super) security: String,
    pub(super) username: Option<String>,
    pub(super) password_env: Option<String>,
    #[serde(default = "timeout")]
    pub(super) timeout_seconds: u64,
    #[serde(default = "total_timeout")]
    pub(super) total_timeout_seconds: u64,
}
fn total_timeout() -> u64 {
    60
}
pub(super) fn timeout() -> u64 {
    30
}
pub(super) fn config(path: &Path) -> Result<Config> {
    let text =
        text_file(path).map_err(|_| err("configuration", "Configuration file unavailable", 3))?;
    let cfg: Config = toml::from_str(&text).map_err(|_| invalid())?;
    address(&cfg.sender)?;
    Ok(cfg)
}
pub(super) fn address(value: &str) -> Result<lettre::Address> {
    if value.trim() != value || value.contains(['\r', '\n', '<', '>']) {
        return Err(invalid());
    }
    value.parse().map_err(|_| invalid())
}
