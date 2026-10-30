# JSON contract v1

Current storage schema: **6**. JSON envelope 1, package manifests 1–3 and optional IMAP cache 1 are independent contracts.

With `--json`, stdout contains one UTF-8 JSON object followed by LF, including usage errors and help. No prompts or progress output appear there. Diagnostics use stderr; operational errors are represented in JSON with fixed safe messages. Without `--json`, operational results currently use the same JSON object; help remains human-readable.

```json
{"schema_version":1,"ok":true,"result":{"package_id":"UUID","state":"prepared"},"error":null}
```

```json
{"schema_version":1,"ok":false,"result":null,"error":{"code":"send_blocked","message":"An accepted or unresolved attempt prevents resubmission"}}
```

The examples above show envelope shape; `prepare` additionally returns the complete `manifest` and `manifest_sha256`. `inspect` returns `manifest`, `manifest_sha256` and `integrity:"verified"`. Manifest schema 1 includes id, sender, recipient, subject, body, text hashes and byte counts, prepared_at, message_id, attachments (original name, sha256, bytes), eml_sha256, eml_bytes, metadata. Contents returned for review are private; do not publish captured stdout.

`status` returns package_id, state, delivery_confirmed (always false), attempts in insertion order, and reconciliations in insertion order. Each attempt has attempt_id, started_at, finished_at (nullable), outcome, message_id, and server_response `{code: string|null, text_retained:false}`. Timestamps are UTC RFC3339 with `+00:00`. `unknown` with null finished_at may still be in progress or may have crashed. `list` returns `{packages:[<status>,…]}` in preparation order and verifies all listed packages. v1 has no pagination; intended for a modest local application archive.

Dry-run returns package_id, state, dry_run:true, network_used:false and manifest_sha256. It creates no attempt. SMTP configuration must be complete; accepted/unknown packages remain blocked. `doctor` returns storage, storage_schema_version (see current storage declaration above), sender, smtp_ready, smtp_issue (safe error code or null), network_used:false and test_smtp_enabled. A missing SMTP section is a successful diagnostic with smtp_ready:false; an absent/malformed config file is an error. Consumers must inspect readiness separately from envelope ok.

| Exit | Meaning | Error codes |
|---|---|---|
| 0 | Operation completed; check result fields | none |
| 2 | CLI usage error | usage |
| 3 | Input/configuration unavailable or invalid | validation, configuration, not_found, input_limit, output_exists |
| 4 | Corrupt or altered package/import source | integrity |
| 5 | Submission/import/reconciliation/operation safety block | busy, send_blocked, import_blocked, reconciliation_binding, dsn_unsupported, dsn_binding, operation_conflict |
| 6 | Explicit SMTP rejection | smtp_rejected |
| 7 | Local storage/output failure | storage (output failure may prevent JSON) |
| 8 | Indeterminate SMTP outcome | outcome_unknown |
| 9 | Unsupported, foreign or altered database schema | storage_schema |

Exit 7 during submission does not establish non-delivery: consult status; the durable attempt may be unknown. External termination, broken stdout or OS failure can prevent any JSON result. Never infer a retry permission from absent output. CLI usage errors do not echo untrusted argument values. Help/version with --json uses the same envelope. Help text is in result.help; version additionally returns result.version and version text in result.help. Output failure exits 7 without a panic.

Relative state paths beginning with `file:` return `configuration`, exit 3, before any files are created. Use an ordinary relative path or an absolute filesystem path; see [upgrade/recovery](upgrade-and-recovery.md) before reusing a previously affected URI-like location.

Within schema 1, consumers should allow additive fields and use error.code/state, not English messages. Breaking changes require a new schema version and explicit migration. No automatic modification API for prepared contents exists.

Stateful commands may create the selected state directory/database or atomically migrate the recognized legacy database layout to schema 6. Describe/help/version do not open state. No network call or new submission attempt is involved in opening/migration. Unknown layouts are not repaired automatically. See [storage migration](storage-migrations.md).

`reconcile ID --evidence-file PATH` returns `{record:<metadata>,already_recorded:bool,state:"unknown",retry_authorized:false}`. Exact repeated evidence returns the same record. `inspect-reconciliation RECORD_ID` returns `{record:<metadata>,evidence:<parsed private input>,integrity:"verified"}`. Status adds a reconciliations array without changing attempt state or delivery_confirmed. Metadata fields: record_id, package_id, attempt_id, recorded_at, observed_at, source, asserted_conclusion, evidence_sha256, evidence_bytes, independently_verified:false, retry_authorized:false. The input contract and operational limits are in [reconciliation](reconciliation.md). These are additive schema-1 JSON fields and commands; evidence input has its own schema_version:1.

## MIME export

`export-eml ID --output PATH --json` verifies the entire package and writes the exact frozen MIME bytes; it does not rebuild MIME, load SMTP configuration, connect to a server or record an attempt. It works for prepared and previously attempted packages. Result fields: package_id, eml_sha256, eml_bytes, integrity:"verified", network_used:false. JSON contains neither exported bytes nor the destination path.

The destination must be new and its parent must already exist. Existing files, symlinks (including dangling links) and hard links are never overwritten (`output_exists`, exit 3). Unix files are created with mode 0600 (subject to a stricter umask); parent directories must be trusted. Concurrent exporters to one path allow only one creator. A missing parent or write/sync failure returns `storage`, exit 7. On failure or interruption a partial destination can remain: do not use it; choose a new path and verify the returned hash after a successful export. Successful file sync does not promise survival of the directory entry across physical power loss. No automatic cleanup or overwrite is attempted.

The EML contains private recipient, body and attachment data. Keep it out of repositories and logs. Treat it as a review copy: sending it through another client bypasses this CLI's attempt history and repeat-send safeguards. An export never confirms or authorizes submission or delivery.

## Recipient-reported receipt

`record-receipt ID --evidence-file PATH --json` uses evidence schema 1 with source `recipient_confirmation` and conclusion `received`. It accepts only a bound accepted or unknown attempt; a prepared package or rejected attempt cannot receive such a record. The return shape matches reconcile, but state is the unchanged package SMTP state (accepted or unknown). `reconcile` retains its original unknown-attempt conclusions and rejects received. Use inspect-reconciliation for the private receipt details.

Status adds `recipient_reported_received`: true when at least one verified stored record asserts receipt, false otherwise. This is an externally asserted observation, not independent delivery verification. `delivery_confirmed` remains false for compatibility and to avoid conflating assertions with transport guarantees. Attempt rows, state and resend policy never change. observed_at describes when the report was observed, not an inferred delivery time; omit unknown arrival times from the summary. Existing evidence hash, size, privacy, lock and idempotency rules apply.

## HTML alternatives

`prepare --html-file PATH` optionally reads a final UTF-8 HTML body (up to 1 MiB, nonempty, no NUL) in addition to the required --body-file plain text. The MIME is multipart/mixed containing multipart/alternative (plain first, HTML second), followed by attachments. Both alternatives are base64 encoded and frozen before submission. No later signature insertion or conversion occurs.

HTML packages without DSN use manifest schema 2 and an html object with content, sha256 and bytes. Plain-only packages without DSN retain manifest schema 1 and omit html. Readers accept both; schema 2 requires a valid HTML object, schema 1 requires its absence. DSN packages use schema 3 and may also include HTML. HTML does not itself require a separate database migration; use the current storage schema declared above. Older CLI versions may reject newer manifests/storage, so use a compatible version for all operations on the store (including list). JSON envelope remains schema 1.

HTML is owner/agent-provided final content, not a template language or sanitized web page. The CLI does not render HTML, fetch links/images, validate link safety or guarantee plain/HTML semantic equivalence. Inspect both alternatives before approval. Use simple static paragraphs and explicit HTTPS anchors; avoid scripts, remote images, tracking, forms and hidden content. Rendering/links depend on the recipient client. Exported EML retains exact alternatives.

Normal send enforces the configured total network deadline. Exit 8/outcome_unknown also covers worker termination or missing worker result. It is not proof that an attempt was created or mail was not accepted; status distinguishes prepared/unknown/accepted. Internal worker commands are excluded from the public CLI/JSON contract. See architecture for the deadline boundary and races.

## DSN contracts

`prepare --dsn off|optional|required` defaults off. Non-off creates manifest schema 3 with dsn mode, notify SUCCESS,FAILURE,DELAY, ret HDRS, and envid_prefix equal to the package UUID. HTML remains optional. Old schemas 1/2 omit dsn. Status adds `dsn:{events:[],reports:[]}`; event metadata includes attempt_id, kind, recorded_at and details. Report metadata includes report_id/package_id/attempt_id/recorded_at/sha256/bytes and a report object with action/status/independently_verified:false/retry_authorized:false. It does not contain raw report text, diagnostics or the original letter.

`import-dsn ID --attempt-id ATTEMPT --file PATH --json` returns report_id, report, already_recorded and network_used:false. Exact-byte duplicate imports return the existing ID. `inspect-dsn REPORT_ID` returns record metadata and integrity:verified. Raw source bytes remain stored privately, hashed and immutable. DSN import does not change state, attempts, delivery_confirmed, recipient_reported_received or retry policy. See dsn.md for formats and capability/error semantics.

## Discovery, operation keys and guidance

`describe --json` is stateless and reports parser-derived command/argument metadata, safety/authorization classifications, envelope shape, versions and limits. Discovery and guidance contracts start at version 1. Normal operational envelopes add optional top-level `next` with structured `action`, `instruction`, `argv` (array/null) and `requires_owner_instruction`; it is invocation guidance, never consent or archived message data. No next-step command sends mail.

Preparation adds `already_prepared` and optional `--operation-key`; matching input recovers the original package and its current state, changed input returns operation_conflict/exit 5. Recognized schemas 0–5 migrate to SQLite 6; stored packages/attempts/evidence are unchanged and older binaries refuse the new store. Detailed fingerprint, retention, argv and authorization rules: [agent workflow](agent-workflow.md). The optional IMAP adapter has a separate schema-1 envelope/ledger contract in its README and cannot change core SMTP state.

## Header-correlated report additions

JSON envelope version remains 1. import-dsn adds an explicit --correlate-by-headers flag and correlation provenance. inspect-dsn/status reports identify envid/original_headers correlation, with an optional numeric smtp_reply_code; no raw server prose is exposed. status adds delivery_observation {outcome, report_ids, independently_verified:false, retry_authorized:false}. Outcomes and precedence are documented in dsn.md. SMTP state/attempts, delivery_confirmed:false and retry policy are unchanged. next may contain additive action review_delivery_report with no argv; unknown SMTP history still takes investigation priority. Discovery is parser-derived and reports storage schema 6.

## Preparation input details

Subject/body/HTML inputs are exact UTF-8 bytes. The subject must be nonempty, at most 998 bytes and contain no CR, LF or NUL: use printf rather than an echo command that appends a newline. Body and optional HTML are nonempty, at most 1 MiB each and contain no NUL. Include the final signature before preparation. Up to five attachments of at most 25 MiB each require the `%PDF-` prefix; this is not a full PDF syntax/rendering check. Attachment filenames and ordering are preserved. Input/config files are bounded regular files without final symlinks; see [filesystem boundary](architecture.md).

JSON help honors the selected subcommand, for example `agent-mailer prepare --help --json` returns prepare's arguments in result.help. Released 0.5.1 returns root help in this case; the correction is included in 0.5.2. Usage text retains the canonical agent-mailer name even when the executable is renamed. Treat `next` as invocation guidance only and allow additive actions such as review_delivery_report.

## Archive pagination and snapshots (0.6.0)

Unparameterized list remains `{packages:[...]}` with full verified statuses. With `--limit N` (1–100), add `pagination:{limit:N,has_more:BOOLEAN,next_after:PACKAGE_ID_OR_NULL,mode:"verified_status"}`. Read at most N statuses; the SQL selector reads N+1 IDs to detect continuation. `--after ID` advances strictly beyond its insertion row. Each command is a consistent read snapshot; later pages may observe appended packages and changed states. There is no cursor expiry or frozen multi-page archive. `next_after` is the last returned ID only if more IDs exist. Unknown after IDs return not_found/exit 3.

With `--summary`, packages entries are only `{package_id:ID,integrity:"not_checked"}` and pagination.mode is ids_only. This is an ID index, not verified history or send guidance. Summary does not read letter/MIME/PDF or attempt/report payloads; schema validation still occurs when opening state. after/summary without limit return usage/exit 2. Existing inspect/send/export integrity and retry rules are unchanged.

Backup/verify-backup return snapshot integrity/presence/freshness metadata under the same JSON envelope 1; backup manifest schema 1 is independent of core storage 6. Canonical fields, pairing/error rules and recovery limits: [snapshots](backup.md). Backup opens an existing current-schema source read-only and never initializes/migrates it. verify-backup is a selected-file read exception: it never opens/creates/migrates the operational home, even when --home is supplied. It cannot establish freshness or authorize retries.

## Offline preflight (0.7.0)

Core `preflight --json` adds a preflight-schema-1 result containing ready/checks, network_used:false, storage_opened:false, credential_content_validated:false and remote_authentication_verified:false. It reads selected configuration without opening operational state; invalid/missing setup returns ok:true/exit 0 with ready:false. `checks` contains fixed configuration/smtp/credential_environment statuses; no sender, login, path or credential values. The optional desktop preflight has a separate metadata-only result. Fields, status vocabularies, deadlines and readiness limitations: [preflight reference](preflight.md). Existing doctor behavior and serialized storage/package/cache formats are unchanged.
