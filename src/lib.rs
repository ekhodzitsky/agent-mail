#![forbid(unsafe_code)]
#![deny(unused_must_use, clippy::unwrap_used, clippy::expect_used)]

mod backup;
mod config;
mod diagnostics;
mod discovery;
mod dsn;
mod files;
mod legacy;
mod listing;
mod package;
mod reconciliation;
mod schema;
mod smtp;
#[cfg(test)]
mod storage_tests;
mod store;
#[cfg(test)]
mod tls_tests;

use chrono::Utc;
use clap::{CommandFactory, Parser, Subcommand};
use config::config;
pub use discovery::Guidance;
use files::{MAX_PDF, read_bounded, text_file};
use package::{Content, export_eml, prepare_content};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use smtp::{send, supervised_send, transport};
use std::path::PathBuf;
use store::Store;

#[derive(Debug)]
pub struct Error {
    pub code: &'static str,
    pub message: &'static str,
    pub exit: i32,
}
type Result<T> = std::result::Result<T, Error>;
fn err(code: &'static str, message: &'static str, exit: i32) -> Error {
    Error {
        code,
        message,
        exit,
    }
}
fn invalid() -> Error {
    err("validation", "Invalid input or configuration", 3)
}
fn storage() -> Error {
    err("storage", "Local storage operation failed", 7)
}
fn integrity() -> Error {
    err("integrity", "Package integrity verification failed", 4)
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn now() -> String {
    Utc::now().to_rfc3339()
}

#[derive(Parser)]
#[command(
    bin_name = "agent-mailer",
    version,
    about = "Email CLI for AI agents: prepare immutable packages and submit explicitly authorized mail"
)]
pub struct Cli {
    #[arg(long, global = true)]
    pub json: bool,
    /// State directory; retains the legacy AGENT_MAILER_HOME environment variable.
    #[arg(long, global = true, env = "AGENT_MAILER_HOME")]
    pub home: Option<PathBuf>,
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Describe the agent interface without opening configuration or storage.
    Describe,
    Doctor,
    /// Inspect local configuration readiness without opening storage or networking.
    Preflight,
    Prepare {
        #[arg(long)]
        to: String,
        #[arg(long)]
        subject_file: PathBuf,
        #[arg(long)]
        body_file: PathBuf,
        /// Optional final HTML alternative, frozen together with plain text.
        #[arg(long)]
        html_file: Option<PathBuf>,
        #[arg(long, value_enum, default_value = "off")]
        dsn: dsn::DsnMode,
        #[arg(long, required = true)]
        attach: Vec<PathBuf>,
        #[arg(long)]
        company_ref: Option<String>,
        #[arg(long)]
        vacancy_ref: Option<String>,
        /// Stable opaque caller key; identical retries recover the same package.
        #[arg(long)]
        operation_key: Option<String>,
    },
    Inspect {
        package_id: String,
    },
    Send {
        package_id: String,
        #[arg(long)]
        dry_run: bool,
    },
    #[command(hide = true)]
    SmtpWorker {
        package_id: String,
        deadline_seconds: u64,
    },
    ImportDsn {
        /// Explicitly correlate a no-ENVID report using exact original headers.
        #[arg(long)]
        correlate_by_headers: bool,
        package_id: String,
        #[arg(long)]
        attempt_id: String,
        #[arg(long)]
        file: PathBuf,
    },
    InspectDsn {
        report_id: String,
    },
    /// Export verified frozen MIME bytes without sending or overwriting files.
    ExportEml {
        package_id: String,
        #[arg(long)]
        output: PathBuf,
    },
    /// Create a private verified database snapshot; excludes configuration/secrets.
    Backup {
        #[arg(long)]
        output: PathBuf,
    },
    /// Verify a selected snapshot without opening the operational store.
    VerifyBackup {
        source: PathBuf,
    },
    Status {
        package_id: String,
    },
    List {
        /// Bound a page to 1–100 packages; omitted keeps the original full listing.
        #[arg(long, value_parser = clap::value_parser!(u16).range(1..=100))]
        limit: Option<u16>,
        /// Continue after an existing package ID in preparation order.
        #[arg(long, requires = "limit")]
        after: Option<String>,
        /// Return IDs only, without reading/verifying package or evidence bytes.
        #[arg(long, requires = "limit")]
        summary: bool,
    },
    /// Append external evidence about an unknown attempt; never enables retry.
    Reconcile {
        package_id: String,
        #[arg(long)]
        evidence_file: PathBuf,
    },
    /// Record an external recipient report without changing SMTP history.
    RecordReceipt {
        package_id: String,
        #[arg(long)]
        evidence_file: PathBuf,
    },
    /// Explicitly inspect private reconciliation evidence.
    InspectReconciliation {
        record_id: String,
    },
    /// Read one legacy prepared application package; never modifies the source.
    Import {
        source: PathBuf,
        #[arg(long)]
        to: String,
        #[arg(long)]
        subject_file: Option<PathBuf>,
    },
}
pub fn help() -> String {
    Cli::command().render_long_help().to_string()
}
pub fn run(cli: Cli) -> Result<Value> {
    if matches!(cli.command, Command::Describe) {
        return Ok(discovery::describe());
    }
    if let Command::VerifyBackup { source } = &cli.command {
        return backup::verify(source);
    }
    if matches!(cli.command, Command::Preflight) {
        let path = cli
            .config
            .or_else(|| {
                cli.home
                    .or_else(|| {
                        std::env::var_os("HOME")
                            .map(|h| PathBuf::from(h).join(".local/share/agent-mailer"))
                    })
                    .map(|h| h.join("config.toml"))
            })
            .ok_or_else(invalid)?;
        return diagnostics::preflight(&path);
    }
    if let Command::SmtpWorker {
        deadline_seconds, ..
    } = &cli.command
    {
        if !(1..=600).contains(deadline_seconds) {
            return Err(invalid());
        }
        let deadline = std::time::Duration::from_secs(*deadline_seconds);
        // Terminate the whole network process, including blocked DNS/native I/O.
        // Durable attempt state and the OS lock survive/release as on a crash.
        std::thread::spawn(move || {
            std::thread::sleep(deadline);
            std::process::exit(8);
        });
    }
    // Preserve the existing journal/config location across the Agent Mailer rename.
    // Selecting a fresh store would hide prior accepted/unknown resend blocks.
    let home = cli
        .home
        .or_else(|| {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share/agent-mailer"))
        })
        .ok_or_else(invalid)?;
    if let Command::Backup { output } = &cli.command {
        return backup::create_from_home(&home, output);
    }
    let cfg_path = cli.config.unwrap_or_else(|| home.join("config.toml"));
    let mut store = Store::open(home)?;
    match cli.command {
        Command::Describe => Ok(discovery::describe()),
        Command::Backup { output } => backup::create(&store, &output),
        Command::VerifyBackup { source } => backup::verify(&source),
        Command::Reconcile {
            package_id,
            evidence_file,
        } => reconciliation::record(&store, &package_id, &evidence_file),
        Command::RecordReceipt {
            package_id,
            evidence_file,
        } => reconciliation::record_receipt(&store, &package_id, &evidence_file),
        Command::InspectReconciliation { record_id } => reconciliation::inspect(&store, &record_id),
        Command::Preflight => diagnostics::preflight(&cfg_path),
        Command::Doctor => {
            let cfg = config(&cfg_path)?;
            let ready = transport(&cfg);
            Ok(
                json!({"storage":"ok","storage_schema_version":schema::VERSION,"sender":cfg.sender,"smtp_ready":ready.is_ok(),"smtp_issue":ready.err().map(|e|e.code),"network_used":false,"test_smtp_enabled":cfg!(feature="test-smtp")}),
            )
        }
        Command::Prepare {
            to,
            subject_file,
            body_file,
            html_file,
            dsn,
            attach,
            company_ref,
            vacancy_ref,
            operation_key,
        } => {
            let cfg = config(&cfg_path)?;
            if attach.len() > 5 {
                return Err(invalid());
            }
            let mut files = Vec::new();
            for path in attach {
                files.push((
                    path.file_name()
                        .and_then(|n| n.to_str())
                        .ok_or_else(invalid)?
                        .to_string(),
                    read_bounded(&path, MAX_PDF)?,
                ));
            }
            prepare_content(
                &mut store,
                &cfg,
                to,
                text_file(&subject_file)?,
                Content {
                    body: text_file(&body_file)?,
                    html: html_file.as_deref().map(text_file).transpose()?,
                    dsn: if matches!(dsn, dsn::DsnMode::Off) {
                        None
                    } else {
                        Some(dsn)
                    },
                    operation_key,
                },
                files,
                json!({"company_ref":company_ref,"vacancy_ref":vacancy_ref}),
            )
        }
        Command::Inspect { package_id } => {
            let p = store.load(&package_id)?;
            Ok(
                json!({"manifest":p.manifest,"manifest_sha256":p.manifest_sha256,"integrity":"verified"}),
            )
        }
        Command::ImportDsn {
            correlate_by_headers,
            package_id,
            attempt_id,
            file,
        } => dsn::import_mode(
            &store,
            &package_id,
            &attempt_id,
            &file,
            correlate_by_headers,
        ),
        Command::InspectDsn { report_id } => dsn::inspect(&store, &report_id),
        Command::ExportEml { package_id, output } => export_eml(&store, &package_id, &output),
        Command::Status { package_id } => store.status(&package_id),
        Command::SmtpWorker { package_id, .. } => {
            send(&store, &config(&cfg_path)?, &package_id, false)?;
            Ok(json!({"submitted":true}))
        }
        Command::Send {
            package_id,
            dry_run,
        } => {
            let cfg = config(&cfg_path)?;
            if dry_run {
                send(&store, &cfg, &package_id, true)
            } else {
                supervised_send(&store, &cfg, &cfg_path, &package_id)
            }
        }
        Command::List {
            limit,
            after,
            summary,
        } => listing::list(&store, limit, after.as_deref(), summary),
        Command::Import {
            source,
            to,
            subject_file,
        } => legacy::import(&mut store, &config(&cfg_path)?, &source, to, subject_file),
    }
}
