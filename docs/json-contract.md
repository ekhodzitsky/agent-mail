# JSON contract v1

Current storage schema: **6**. This page owns the wire format; [commands](cli.md) and [agent workflow](agent-workflow.md) describe how to use it.

## Envelope and output

With --json, stdout is one UTF-8 JSON object followed by LF, including usage/help/version output. There are no prompts/progress lines. Diagnostics go to stderr; operational errors use fixed safe messages. Without --json, operational results currently use the same JSON format, while help is human-readable.

The following examples show envelope shape, not complete prepare output:

```json
{"schema_version":1,"ok":true,"result":{"package_id":"UUID","state":"prepared"},"error":null}
```

```json
{"schema_version":1,"ok":false,"result":null,"error":{"code":"send_blocked","message":"An accepted or unresolved attempt prevents resubmission"}}
```

Operational envelopes can also contain top-level next guidance. Consumers must allow additive fields/actions and compare error.code and result.state rather than English messages. Breaking serialized changes need a new schema/migration. Output/OS failure or external termination can prevent a JSON result; missing output never authorizes retry.

## Independent versions

| Contract | Current |
|---|---|
| Envelope | 1 |
| Package manifest | 1 plain; 2 HTML; 3 DSN with optional HTML |
| Core storage | 6 |
| Discovery / guidance / prepare-input fingerprint | 1 each |
| Preflight result / backup manifest / reconciliation input | 1 each |
| Optional IMAP cache/envelope | 1 each; separate adapter contract |

Product version comes from --version/describe. It is not a storage or manifest version. Existing prepared contents have no modification API.

## Preparation and inspection

prepare returns package_id, state, manifest, manifest_sha256 and already_prepared. Inspect returns manifest, manifest_sha256 and integrity:verified. Review output contains private subject/body/recipient information; do not publish it.

Manifest fields include schema_version, id, sender, recipient, subject, body, prepared_at, message_id, subject/body hashes and byte counts, ordered attachments (name/sha256/bytes), eml_sha256/eml_bytes and metadata. HTML/DSN fields depend on manifest version. Optional company/vacancy references are opaque metadata, not repository bindings. Exact original attachment names and bytes are retained.

Subject/body/HTML inputs are exact UTF-8 bytes. The subject must not be empty/whitespace-only, at most 998 bytes and contain no CR, LF or NUL: use printf rather than an echo command that appends a newline. Body and optional HTML are not empty/whitespace-only, at most 1 MiB each and contain no NUL. Include the final signature before preparation. Up to five attachments of at most 25 MiB each require the `%PDF-` prefix; this is not a full PDF syntax/rendering check. Attachment filenames and ordering are preserved. Input/config files are bounded regular files without final symlinks; see [filesystem boundary](architecture.md).

## Submission, status and diagnostics

| Command | Result |
|---|---|
| send ID --dry-run | package_id, state:prepared, dry_run:true, network_used:false, manifest_sha256; complete SMTP config required, no attempt |
| send ID | On acceptance, the same verified status shape as status; otherwise an error envelope/exit code and separately retained history |
| status ID | package_id, state, delivery_confirmed:false, delivery_observation, recipient_reported_received, attempts, reconciliations, dsn |
| doctor | storage, storage_schema_version, private sender, smtp_ready, smtp_issue (safe code/null), network_used:false, test_smtp_enabled |
| preflight | preflight_schema_version:1, ready, fixed checks, network_used:false, storage_opened:false, credential_content_validated:false, remote_authentication_verified:false |

State is prepared before any attempt; otherwise it reflects the latest attempt: accepted, rejected or unknown. Only final DATA 250 is acceptance, not delivery. An unknown attempt can still be running, have crashed or have lost a reply. Accepted/unknown blocks include dry-run. A failed send never implies that no network effect occurred; inspect status.

Attempts are ordered by insertion and contain attempt_id, started_at, finished_at (nullable), outcome, message_id and server_response `{code:string|null,text_retained:false}`. Stored times use UTC RFC3339 +00:00; precision is not fixed. Unknown/null completion may be in progress or interrupted. SMTP free prose is omitted because it may echo private content/credentials.

Doctor succeeds with smtp_ready:false when SMTP is absent, but unavailable/malformed config is an error. Preflight instead reports unavailable/invalid local setup under ok:true/exit 0 with ready:false. Neither verifies authentication or delivery. See [preflight fields/statuses](preflight.md).

## Errors and exit codes

| Exit | Meaning | Error codes |
|---|---|---|
| 0 | Operation completed; check result fields | none |
| 2 | CLI usage error | usage |
| 3 | Input/configuration unavailable or invalid | validation, configuration, not_found, input_limit, output_exists |
| 4 | Corrupt or altered package/import source | integrity |
| 5 | Submission/import/reconciliation/operation safety block | busy, send_blocked, import_blocked, reconciliation_binding, dsn_unsupported, dsn_binding, operation_conflict, backup_mode, backup_binding |
| 6 | Explicit SMTP rejection | smtp_rejected |
| 7 | Local storage/output failure | storage (output failure may prevent JSON) |
| 8 | Indeterminate SMTP outcome | outcome_unknown |
| 9 | Unsupported, foreign or altered database schema | storage_schema |

Exit 7 during submission does not establish non-delivery; inspect durable history. Exit 8 also covers worker termination/missing results and does not prove that an attempt exists: status may still be prepared or already accepted. Explicit rejection is exit 6; any new attempt needs a fresh owner instruction. Never infer permission from a diagnostic code or absent output.

Usage errors omit untrusted argv values. JSON help uses result.help for the selected subcommand and the canonical agent-mailer usage name; version additionally returns result.version. Help/version/usage may omit next. Output failure exits 7 without a guaranteed envelope.

## State-opening effects

| Commands | Local effect |
|---|---|
| describe/help/version | No config/storage access |
| preflight | Selected config/environment readiness only; no operational home initialization/migration |
| backup | Read-only existing current-schema core/cache; new private snapshot destination |
| verify-backup | Selected snapshot only; ignores operational home/config |
| Other stateful commands | Open selected store, possibly create it or migrate recognized layouts 0–5 to 6, before completing the command |

Opening never submits mail. Unknown/altered/future layouts fail closed rather than being repaired; [migration details](storage-migrations.md) are canonical. Relative operational homes starting with file: are refused with configuration/exit 3 before creation; use a normal relative or absolute filesystem path and follow [recovery](upgrade-and-recovery.md) for an older affected location.

## Archive pagination and snapshots

Unparameterized list remains `{packages:[...]}` with full verified statuses. With `--limit N` (1–100), add `pagination:{limit:N,has_more:BOOLEAN,next_after:PACKAGE_ID_OR_NULL,mode:"verified_status"}`. Read at most N statuses; the SQL selector reads N+1 IDs to detect continuation. `--after ID` advances strictly beyond its insertion row. Each command is a consistent read snapshot; later pages may observe appended packages and changed states. There is no cursor expiry or frozen multi-page archive. `next_after` is the last returned ID only if more IDs exist. Unknown after IDs return not_found/exit 3.

With `--summary`, packages entries are only `{package_id:ID,integrity:"not_checked"}` and pagination.mode is ids_only. This is an ID index, not verified history or send guidance. Summary does not read letter/MIME/PDF or attempt/report payloads; schema validation still occurs when opening state. after/summary without limit return usage/exit 2. Existing inspect/send/export integrity and retry rules are unchanged.

Backup/verify-backup return snapshot integrity/presence/freshness metadata under the same JSON envelope 1; backup manifest schema 1 is independent of core storage 6. Canonical fields, pairing/error rules and recovery limits: [snapshots](backup.md). Backup opens an existing current-schema source read-only and never initializes/migrates it. verify-backup is a selected-file read exception: it never opens/creates/migrates the operational home, even when --home is supplied. It cannot establish freshness or authorize retries.

## Discovery, operation keys and guidance

`describe --json` is stateless and reports parser-derived command/argument metadata, safety/authorization classifications, envelope shape, versions and limits. Discovery and guidance contracts start at version 1. Normal operational envelopes add optional top-level `next` with structured `action`, `instruction`, `argv` (array/null) and `requires_owner_instruction`; it is invocation guidance, never consent or archived message data. No next-step command sends mail.

Preparation adds `already_prepared` and optional `--operation-key`; matching input recovers the original package and its current state, changed input returns operation_conflict/exit 5. Recognized schemas 0–5 migrate to SQLite 6; stored packages/attempts/evidence are unchanged and older binaries refuse the new store. Detailed fingerprint, retention, argv and authorization rules: [agent workflow](agent-workflow.md). The optional IMAP adapter has a separate schema-1 envelope/ledger contract in its README and cannot change core SMTP state.

Guidance schema 1 actions are review_frozen_package, obtain_owner_instruction, investigate_submission, await_delivery_observation, review_refusal, review_delivery_report and resolve_error. Execute argv as an argument array, never shell-joined text. Runnable hints retain explicit home/config and suppress lossy non-UTF-8 paths; no hint executes send. Guidance is not archived correspondence or consent.

## HTML alternatives

`prepare --html-file PATH` optionally reads a final UTF-8 HTML body (up to 1 MiB, nonempty, no NUL) in addition to the required --body-file plain text. The MIME is multipart/mixed containing multipart/alternative (plain first, HTML second), followed by attachments. Both alternatives are base64 encoded and frozen before submission. No later signature insertion or conversion occurs.

HTML packages without DSN use manifest schema 2 and an html object with content, sha256 and bytes. Plain-only packages without DSN retain manifest schema 1 and omit html. Readers accept both; schema 2 requires a valid HTML object, schema 1 requires its absence. DSN packages use schema 3 and may also include HTML. HTML does not itself require a separate database migration; use the current storage schema declared above. Older CLI versions may reject newer manifests/storage, so use a compatible version for all operations on the store (including list). JSON envelope remains schema 1.

HTML is owner/agent-provided final content, not a template language or sanitized web page. The CLI does not render HTML, fetch links/images, validate link safety or guarantee plain/HTML semantic equivalence. Inspect both alternatives before approval. Use simple static paragraphs and explicit HTTPS anchors; avoid scripts, remote images, tracking, forms and hidden content. Rendering/links depend on the recipient client. Exported EML retains exact alternatives.

Normal send enforces the configured total network deadline. Exit 8/outcome_unknown also covers worker termination or missing worker result. It is not proof that an attempt was created or mail was not accepted; status distinguishes prepared/unknown/accepted. Internal worker commands are excluded from the public CLI/JSON contract. See architecture for the deadline boundary and races.

## MIME export

`export-eml ID --output PATH --json` verifies the entire package and writes the exact frozen MIME bytes; it does not rebuild MIME, load SMTP configuration, connect to a server or record an attempt. It works for prepared and previously attempted packages. Result fields: package_id, eml_sha256, eml_bytes, integrity:"verified", network_used:false. JSON contains neither exported bytes nor the destination path.

The destination must be new and its parent must already exist. Existing files, symlinks (including dangling links) and hard links are never overwritten (`output_exists`, exit 3). Unix files are created with mode 0600 (subject to a stricter umask); parent directories must be trusted. Concurrent exporters to one path allow only one creator. A missing parent or write/sync failure returns `storage`, exit 7. On failure or interruption a partial destination can remain: do not use it; choose a new path and verify the returned hash after a successful export. Successful file sync does not promise survival of the directory entry across physical power loss. No automatic cleanup or overwrite is attempted.

The EML contains private recipient, body and attachment data. Keep it out of repositories and logs. Treat it as a review copy: sending it through another client bypasses this CLI's attempt history and repeat-send safeguards. An export never confirms or authorizes submission or delivery.

## Reconciliation

`reconcile ID --evidence-file PATH` returns `{record:<metadata>,already_recorded:bool,state:"unknown",retry_authorized:false}`. Exact repeated evidence returns the same record. `inspect-reconciliation RECORD_ID` returns `{record:<metadata>,evidence:<parsed private input>,integrity:"verified"}`. Status adds a reconciliations array without changing attempt state or delivery_confirmed. Metadata fields: record_id, package_id, attempt_id, recorded_at, observed_at, source, asserted_conclusion, evidence_sha256, evidence_bytes, independently_verified:false, retry_authorized:false. The input contract and operational limits are in [reconciliation](reconciliation.md). These are additive schema-1 JSON fields and commands; evidence input has its own schema_version:1.

## Recipient-reported receipt

`record-receipt ID --evidence-file PATH --json` uses evidence schema 1 with source `recipient_confirmation` and conclusion `received`. It accepts only a bound accepted or unknown attempt; a prepared package or rejected attempt cannot receive such a record. The return shape matches reconcile, but state is the unchanged package SMTP state (accepted or unknown). `reconcile` retains its original unknown-attempt conclusions and rejects received. Use inspect-reconciliation for the private receipt details.

Status adds `recipient_reported_received`: true when at least one verified stored record asserts receipt, false otherwise. This is an externally asserted observation, not independent delivery verification. `delivery_confirmed` remains false for compatibility and to avoid conflating assertions with transport guarantees. Attempt rows, state and resend policy never change. observed_at describes when the report was observed, not an inferred delivery time; omit unknown arrival times from the summary. Existing evidence hash, size, privacy, lock and idempotency rules apply.

## DSN reports

`prepare --dsn off|optional|required` defaults off. Non-off creates manifest schema 3 with dsn mode, notify SUCCESS,FAILURE,DELAY, ret HDRS, and envid_prefix equal to the package UUID. HTML remains optional. Old schemas 1/2 omit dsn. Status adds `dsn:{events:[],reports:[]}`; event metadata includes attempt_id, kind, recorded_at and details. Report metadata includes report_id/package_id/attempt_id/recorded_at/sha256/bytes and a report object with action/status/independently_verified:false/retry_authorized:false. It does not contain raw report text, diagnostics or the original letter.

`import-dsn ID --attempt-id ATTEMPT --file PATH --json` returns report_id, report, already_recorded and network_used:false. Exact-byte duplicate imports return the existing ID. `inspect-dsn REPORT_ID` returns record metadata and integrity:verified. Raw source bytes remain stored privately, hashed and immutable. DSN import does not change state, attempts, delivery_confirmed, recipient_reported_received or retry policy. See dsn.md for formats and capability/error semantics.

JSON envelope version remains 1. import-dsn adds an explicit --correlate-by-headers flag and correlation provenance. inspect-dsn/status reports identify envid/original_headers correlation, with an optional numeric smtp_reply_code; no raw server prose is exposed. status adds delivery_observation {outcome, report_ids, independently_verified:false, retry_authorized:false}. Outcomes and precedence are documented in dsn.md. SMTP state/attempts, delivery_confirmed:false and retry policy are unchanged. next may contain additive action review_delivery_report with no argv; unknown SMTP history still takes investigation priority. Discovery is parser-derived and reports storage schema 6.

## Optional integrations

The [IMAP adapter](../integrations/imap-dsn/README.md) and [desktop adapter](../integrations/gnome-keyring/README.md) have separate contracts/runtime/configuration. Neither an incoming report nor a local metadata preflight changes core SMTP retry permission. Core preflight status vocabularies and adapter metadata deadlines are in [preflight](preflight.md).
