#![forbid(unsafe_code)]
#![deny(unused_must_use, clippy::unwrap_used, clippy::expect_used)]

use clap::Parser;
use serde_json::json;
use std::io::Write;

fn main() {
    let json_mode = std::env::args_os().any(|arg| arg == "--json");
    let args = match agent_mail::Cli::try_parse() {
        Ok(args) => args,
        Err(error) => {
            let help = matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            );
            if json_mode {
                let result = if error.kind() == clap::error::ErrorKind::DisplayVersion {
                    json!({"version": env!("CARGO_PKG_VERSION"), "help": format!("agent-mail {}\n", env!("CARGO_PKG_VERSION"))})
                } else if help {
                    json!({"help": error.to_string()})
                } else {
                    serde_json::Value::Null
                };
                if writeln!(
                    std::io::stdout().lock(), "{}",
                    json!({"schema_version":1,"ok":help,"result":result,"error":if help {serde_json::Value::Null} else {json!({"code":"usage","message":"Invalid command arguments; use --help"})}})
                ).is_err() {
                    std::process::exit(7);
                }
            } else if help {
                if error.print().is_err() {
                    std::process::exit(7);
                }
            } else if writeln!(
                std::io::stderr().lock(),
                "Invalid command arguments; use --help"
            )
            .is_err()
            {
                std::process::exit(7);
            }
            std::process::exit(if help { 0 } else { 2 });
        }
    };
    let guidance = agent_mail::Guidance::new(&args);
    let (mut value, code) = match agent_mail::run(args) {
        Ok(value) => (
            json!({"schema_version":1,"ok":true,"result":value,"error":null}),
            0,
        ),
        Err(error) => (
            json!({"schema_version":1,"ok":false,"result":null,"error":{"code":error.code,"message":error.message}}),
            error.exit,
        ),
    };
    value["next"] = serde_json::Value::Array(guidance.next(&value));
    if writeln!(std::io::stdout().lock(), "{value}").is_err() {
        std::process::exit(7);
    }
    std::process::exit(code);
}
