//! Parser-derived discovery and structured guidance; neither grants consent.
use crate::{Cli, Command, schema};
use clap::CommandFactory;
use serde_json::{Value, json};
use uuid::Uuid;

fn effect(name: &str) -> (&'static str, bool) {
    match name {
        "describe" | "help" => ("none", false),
        "prepare" | "import" => ("local_package_write", false),
        "export-eml" | "backup" => ("local_export_write", false),
        "verify-backup" => ("selected_backup_read", false),
        "preflight" => ("configuration_read_only", false),
        "reconcile" | "record-receipt" | "import-dsn" => ("local_evidence_write", false),
        "send" => ("smtp_submission_unless_dry_run", true),
        "doctor" | "inspect" | "status" | "list" | "inspect-dsn" | "inspect-reconciliation" => {
            ("local_read_or_schema_migration", false)
        }
        _ => ("unknown_requires_review", true),
    }
}

pub(super) fn describe() -> Value {
    let mut parser = Cli::command();
    parser.build();
    let commands: Vec<_> = parser
        .get_subcommands()
        .filter(|c| !c.is_hide_set())
        .map(|c| {
            let (side_effect, requires_owner_instruction) = effect(c.get_name());
            let arguments: Vec<_> = c
                .get_arguments()
                .filter(|a| !a.is_hide_set())
                .map(|a| {
                    let choices: Vec<_> = a
                        .get_value_parser()
                        .possible_values()
                        .into_iter()
                        .flatten()
                        .filter(|v| !v.is_hide_set())
                        .map(|v| v.get_name().to_string())
                        .collect();
                    json!({"id":a.get_id().as_str(),"long":a.get_long(),
                "short":a.get_short().map(|v|v.to_string()),"positional":a.is_positional(),
                "required":a.is_required_set(),"takes_value":a.get_action().takes_values(),
                "repeatable":matches!(a.get_action(),clap::ArgAction::Append),
                "choices":choices,"help":a.get_help().map(ToString::to_string)})
                })
                .collect();
            json!({"name":c.get_name(),"description":c.get_about().map(ToString::to_string),
            "arguments":arguments,"side_effect":side_effect,
            "requires_owner_instruction":requires_owner_instruction,
            "dry_run_exception":if c.get_name()=="send" {
                json!({"flag":"dry-run","requires_owner_instruction":false,"network_used":false,"attempt_created":false})
            } else {Value::Null}})
        })
        .collect();
    json!({"discovery_schema_version":1,"cli_version":env!("CARGO_PKG_VERSION"),
        "json_schema_version":1,"guidance_schema_version":1,"storage_schema_version":schema::VERSION,
        "test_smtp_enabled":cfg!(feature="test-smtp"),
        "manifest_schema_versions":[1,2,3],"commands":commands,
        "input_limits":{"text_bytes":crate::files::MAX_TEXT,"pdf_bytes":crate::files::MAX_PDF,
            "attachments":5,"subject_bytes":998,"operation_key_bytes":crate::package::MAX_OPERATION_KEY,
            "operation_key_characters":"ASCII letters/digits and -._:/"},
        "envelope":{"type":"object","required":["schema_version","ok","result","error"],
            "properties":{"schema_version":{"const":1},"ok":{"type":"boolean"},
                "result":{"type":["object","null"]},"error":{"type":["object","null"],
                    "required":["code","message"]},"next":{"type":"array"}}},
        "exit_codes":{"0":"completed; inspect result","2":"usage","3":"input/configuration",
            "4":"integrity","5":"safety block or operation conflict","6":"SMTP rejection",
            "7":"storage/output failure","8":"unknown submission outcome","9":"storage schema"},
        "authorization":{"prepare_is_not_send_approval":true,"send_requires_explicit_owner_instruction":true,
            "cli_verifies_owner_consent":false},
        "rules":{"json_stdout":"one JSON envelope","diagnostics":"stderr",
            "dry_run":"offline; no attempt","accepted":"SMTP acceptance, not delivery",
            "unknown":"investigate; never automatically resend","message_id":"correlation, not deduplication"},
        "network_used":false})
}

pub struct Guidance {
    prefix: Option<Vec<String>>,
    id: Option<String>,
}
impl Guidance {
    pub fn new(cli: &Cli) -> Self {
        let mut prefix = Some(vec!["agent-mail".to_string()]);
        for (flag, path) in [
            ("--home", cli.home.as_ref()),
            ("--config", cli.config.as_ref()),
        ] {
            if let Some(path) = path {
                prefix = prefix.and_then(|mut args| {
                    args.extend([flag.to_string(), path.to_str()?.to_string()]);
                    Some(args)
                });
            }
        }
        let id = match &cli.command {
            Command::Inspect { package_id }
            | Command::Status { package_id }
            | Command::Send { package_id, .. }
            | Command::ExportEml { package_id, .. }
            | Command::ImportDsn { package_id, .. }
            | Command::Reconcile { package_id, .. }
            | Command::RecordReceipt { package_id, .. } => Some(package_id),
            _ => None,
        }
        .filter(|id| Uuid::parse_str(id).is_ok())
        .cloned();
        Self { prefix, id }
    }
    fn step(&self, action: &str, command: Option<(&str, &str)>, requires: bool) -> Value {
        let argv = command.and_then(|(name, id)| {
            self.prefix.as_ref().map(|prefix| {
                let mut argv = prefix.clone();
                argv.extend([name.to_string(), id.to_string(), "--json".into()]);
                argv
            })
        });
        let instruction = match action {
            "investigate_submission" => {
                "Inspect durable history and external evidence; do not resend automatically"
            }
            "review_frozen_package" => {
                "Review exact sender, recipient, alternatives and attachments"
            }
            "obtain_owner_instruction" => {
                "Wait for explicit owner instruction for this reviewed package"
            }
            "review_delivery_report" => {
                "Review unverified delivery evidence; SMTP outcome and resend blocks remain unchanged"
            }
            "await_delivery_observation" => "SMTP acceptance is not recipient delivery or reading",
            "review_refusal" => "Review refusal and obtain a new owner instruction before retry",
            _ => "Resolve the error; absent output or errors do not authorize retry",
        };
        json!({"action":action,"instruction":instruction,"argv":argv,"requires_owner_instruction":requires})
    }
    pub fn next(&self, envelope: &Value) -> Vec<Value> {
        let result = &envelope["result"];
        let id = result["package_id"].as_str().or(self.id.as_deref());
        let state = result["state"].as_str();
        let error = envelope["error"]["code"].as_str();
        if matches!(error, Some("outcome_unknown" | "send_blocked")) || state == Some("unknown") {
            return vec![self.step("investigate_submission", id.map(|id| ("status", id)), false)];
        }
        if error.is_some() {
            return vec![self.step("resolve_error", None, false)];
        }
        if result["delivery_observation"]["outcome"]
            .as_str()
            .is_some_and(|s| s != "not_reported")
        {
            return vec![self.step("review_delivery_report", None, false)];
        }
        match (state, id) {
            (Some("prepared"), Some(id)) => vec![
                self.step("review_frozen_package", Some(("inspect", id)), false),
                self.step("obtain_owner_instruction", None, true),
            ],
            (Some("accepted"), _) => vec![self.step("await_delivery_observation", None, false)],
            (Some("rejected"), _) => vec![self.step("review_refusal", None, true)],
            _ => Vec::new(),
        }
    }
}
