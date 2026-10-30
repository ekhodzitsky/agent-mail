# Optional IMAP DSN retrieval

Use the new agent-mail executable names. Existing legacy state and Keyring lookup identities remain compatible; follow [rename/upgrade instructions](../../docs/upgrade-and-recovery.md#rename-to-agent-mail-08) before replacing an installation. Do not create a fresh history to rename the tool.

`agent-mail-imap-dsn` is an on-demand, separate Python 3.11+ standard-library integration tested on Linux with Unix file-permission/no-follow semantics. Other operating systems are not supported by this adapter release. The installed Rust CLI remains standalone and has no Python dependency. This adapter requires an owner-selected IMAP server/account, explicit read scope, a verified TLS connection and the installed agent-mail binary. It does not configure a provider or reuse SMTP credentials automatically.

Install the script alongside the core binary:

```sh
install -Dm755 integrations/imap-dsn/agent-mail-imap-dsn "$HOME/.local/bin/agent-mail-imap-dsn"
```

Create a private mode-0600 TOML file outside Git. Synthetic example:

```toml
host = "imap.example.org"
port = 993
username = "login@example.org"
password_env = "AGENT_MAIL_IMAP_PASSWORD"
mailbox = "INBOX"
timeout_seconds = 10
total_timeout_seconds = 60
max_messages = 20
# Optional absolute paths; defaults use the existing local installation/state.
# home = "/absolute/private/state"
# binary = "/absolute/path/to/agent-mail"
```

The secret must already be supplied through the named environment variable by an owner-operated secret mechanism (the optional [GNOME Keyring launcher](../gnome-keyring/README.md#optional-imap-dsn-credential-launch) supports an explicit private IMAP binding); never pass it in argv or put it in this file. Sender aliases do not establish the IMAP login identity. This release supports implicit TLS with certificate/hostname verification and ASCII usernames/mailbox names. There is no plaintext, certificate bypass, STARTTLS or OAuth-refresh mode. Normal system TLS trust configuration applies.

Choose an existing DSN-enabled package and its accepted/unknown attempt:

```sh
agent-mail-imap-dsn --config /absolute/private/imap.toml \
  --package-id PACKAGE_UUID --attempt-id ATTEMPT_UUID --dry-run --json

# Only with the owner's read authorization:
agent-mail-imap-dsn --config /absolute/private/imap.toml \
  --package-id PACKAGE_UUID --attempt-id ATTEMPT_UUID --json
```

Dry-run checks local configuration, credential presence, TLS context and core attempt binding without a connection or retrieval-cache write. As with other core commands, reading status may apply a recognized local storage migration. It does not certify IMAP login or mailbox reachability.

Normal retrieval logs in over verified TLS, uses EXAMINE/read-only selection, and searches BODY for the exact `package.attempt` envelope identifier. Search is candidate selection, not authentication. Alternatively supply repeated `--uid 42` to select at most 100 known UIDs explicitly, or `--report-message-id '<notification-id@example.org>'` to search the selected notification’s Message-ID header. A known UID may be combined with Message-ID only under an explicit expected UIDVALIDITY guard; see below. Message-ID accepts a bounded ASCII subset without quoting/control characters; selection still requires exact core envelope/recipient binding. No sequence-number selection is used.

For a selected notification, `--report-search header` is the default. `--report-search text` searches the same Message-ID through the standard IMAP TEXT criterion; `--report-search sender-date --report-date 2026-01-01 --report-from mailer-daemon@example.org` searches the chosen Date-header day (`SENTON`) and sender (`FROM`). These are explicit alternatives, never automatic retries/fallbacks or other-folder scans. See [RFC 3501 search semantics](https://www.rfc-editor.org/rfc/rfc3501.html#section-6.4.4). Date must be canonical ISO; sender and Message-ID use bounded ASCII subsets. Date/from are required only for sender-date and are rejected with other methods.

Sender-date candidate count must not exceed configured `max_messages`; otherwise `candidate_limit` stops before any fetch, including header fetches. Narrow the scope or select known UIDs rather than silently widening access. For every Message-ID selection, fetch only `BODY.PEEK[HEADER.FIELDS (MESSAGE-ID)]` first and require exactly one matching header (maximum 8 KiB). Missing, mismatched or duplicate IDs stop before full-body retrieval. The full report must match the same ID again before core import. Cached matches recheck the current header but do not refetch the body. This protects selection; it does not authenticate server/DSN assertions beyond the existing verified TLS and core binding checks. The core importer verifies DSN format, exact envelope/recipient binding, hashes and prior parameter-use observations.

The adapter fetches RFC822.SIZE first (maximum 1 MiB), then BODY.PEEK[]; it checks returned UID and actual byte count. It bounds literal size, aggregate network reads, search results (64 KiB/10,000 UIDs) and each batch (1–100 new messages). Socket timeout is 1–60 seconds; a supervised subprocess and independent worker watchdog bound the complete operation to 1–300 seconds. It never runs STORE, CLOSE, APPEND, SMTP, automatic replies or remote deletion. It does not mark messages seen. The temporary original EML lives in a private directory/mode-0600 file and is removed after import; normal process death can leave a temporary file, so normal private temporary-directory hygiene still applies.

## Replay, provenance and failures

The local `imap-dsn.sqlite3` cache sits beside the core database, mode 0600, with its own recognized schema 1. Immutable rows record a hash of server/port/login/mailbox/UIDVALIDITY/UID, package/attempt IDs, raw-byte SHA-256 and core report ID. It contains no password or raw body. Do not copy it to another core state directory. Keep it with the same private backup scope.

Cached reports are reverified through core `inspect-dsn` before they are skipped. A UIDVALIDITY change selects a new remote identity; unchanged notification bytes still deduplicate in the core importer. The cache is an optimization/provenance record, not authority to change SMTP state. The core import commits before cache publication: lost output or cache failure may leave the report imported already. Repeating retrieval safely verifies/deduplicates it. A partial operation never implies that no reports were added. A cache referencing missing/altered core evidence fails closed; restore consistent state rather than deleting the core submission history.

JSON stdout uses a schema-1 envelope and contains report metadata only. Successful network results include the effective `selection` method and `matched_candidates` count, plus `records`, `cached`, `remaining`, `network_used`, `mailbox_modified:false`, `smtp_state_modified:false` and `retry_authorized:false`. `remaining` counts new candidates beyond the current batch; run again within the authorized scope to continue. A malformed, unrelated, oversize or conflicting selected report stops the batch; previously committed reports remain. Use explicit UIDs to investigate/select appropriate candidates.

Exit codes: 0 success, 2 usage, 3 input/configuration/limits, 4 core integrity failure, 5 binding/busy refusal, 7 local/integration/output failure, 8 IMAP/total deadline uncertainty, 9 unsupported core/cache schema. Explicit non-OK responses use safe stage codes such as `imap_mailbox`, `imap_search`, `imap_fetch_size` and `imap_fetch_body`; incomplete body sizes use `imap_body_size`. Library-raised authentication/protocol exceptions may still use the generic integration error. Search refusals may be classified as `imap_search_unavailable`, `imap_search_unsupported`, `imap_search_charset` or `imap_search_limited` using a fixed vocabulary; these are diagnostic hints, not retry permission. Exceptions/server prose never appear in errors. Output loss can prevent JSON. TLS/protocol/library failures may produce the safe generic integration error; inspect configuration/status without printing credentials. The adapter cannot authorize a send or reset an unknown attempt, and retrieved DSN assertions remain independently unverified.

No real mailbox is configured by installation. Provider-specific IMAP access/app-password requirements must be established separately by the owner. Sent-folder synchronization and inbound-message automation are outside this integration.

## Local tests

After building the CLI, run:

```sh
/usr/bin/python3 -B -m unittest discover -s integrations/imap-dsn -p 'test_*.py'
```

Tests generate a temporary certificate/key using the system OpenSSL executable and run numeric-loopback TLS servers. They exercise actual CLI import, replay, UIDVALIDITY, certificate/hostname refusal, size/literal limits, binding failures, total timeout and offline checks. They never use real credentials or external mail. OpenSSL is a test prerequisite, not an adapter runtime dependency.

## Explicit no-ENVID IMAP retrieval and bounded UID lookup

Use `--correlate-by-headers --report-message-id '<selected-notification@example.org>'` only for the owner-selected original notification. It requires one accepted/unknown attempt across the package history, supports packages without a frozen DSN request and invokes the core's explicit header-correlation importer. Core checks the original attached Message-ID, single From/To and Final-Recipient against frozen bytes. The adapter does not infer authenticity or retry permission. Default preflight/import still requires recorded DSN parameter use; there is no automatic downgrade. Correlation modes cannot relabel an existing cached report.

For providers whose SEARCH cannot find the selected report, explicitly choose `--report-search recent-uids`. This uses EXAMINE's UIDNEXT, then UID FETCH of only `BODY.PEEK[HEADER.FIELDS (MESSAGE-ID)]` for the last 20 UID **values**; UID gaps mean fewer actual messages. No date/from arguments, SEARCH ALL, subjects, addresses, flags or other bodies are fetched. Missing/invalid UIDNEXT fails closed. The lookup never advances to older ranges or other folders automatically. A report outside this window may need a provider-supported search/explicit selection; absence from the window does not prove absence from the account. RFC behavior: [UIDNEXT/EXAMINE](https://www.rfc-editor.org/rfc/rfc3501.html#section-6.3.2).

Each returned UID must be unique and inside the chosen range. Both UID-before-literal and UID-after-literal metadata are supported. At most 20 header responses, 8 KiB per header and bounded metadata fragments are accepted, within the existing aggregate byte/time budget. Nonmatching/missing/duplicate Message-ID headers select no body; malformed/out-of-window UID metadata fails. Exact selected matches still pass size/UID/header/full-body/core checks, and the normal max_messages body/import limit applies. Repeated cached lookup rechecks selected headers without downloading the full report.

Successful network results also include mailbox_messages, uidvalidity, scanned_headers and correlation=envid/original_headers; these are private diagnostic metadata, not evidence that any other mail was read or any recipient received a message. Dry-run remains offline. This mode reads up to 20 identifier headers, so use it only within the authorized selected-notification lookup scope, never as general inbox automation.

## Guarded retrieval of a known UID

A selected report outside the recent lookup window can be retrieved directly, without SEARCH or reading identifier headers of other messages:

```sh
agent-mail-imap-dsn --config /absolute/private/imap.toml \
  --package-id PACKAGE_UUID --attempt-id ATTEMPT_UUID \
  --uid 42 --expected-uidvalidity 77 \
  --report-message-id '<selected-notification@example.org>' \
  --correlate-by-headers --json
```

Use UID/UIDVALIDITY from a prior observation of that exact folder/account; UIDs alone are not globally stable. Only the explicitly selected UIDs (up to 100) are considered. Combining UID with Message-ID requires the guard; expected-uidvalidity is accepted only with UID, and UID cannot be mixed with report-search/date/from criteria. Legacy strict UID-only retrieval still works and may opt into an epoch guard. No-ENVID mode always requires the selected notification Message-ID.

After verified TLS login and EXAMINE, the adapter compares the observed epoch before any FETCH, including size/header reads or cached-header verification. A mismatch returns imap_uidvalidity/exit 5, never falls back to search or another epoch and never changes core history. Obtain a fresh selected identity through an authorized source rather than guessing/reusing the UID. Invalid/misplaced arguments fail locally before a connection; the keyring launcher validates them before secret lookup. Dry-run can validate the guard's syntax/presence but cannot verify its remote value.

With a matching epoch, the exact notification header/body and core original-header or ENVID checks still apply. Cached reports retain original provenance; same UID under a new, explicitly observed epoch is a different cache identity, while identical raw evidence still deduplicates in the core. This lookup authenticates neither the report contents nor recipient delivery and never grants send/retry permission.

Credential variable names must be application-specific. The current adapter rejects loader/interpreter/locale/process-control destinations before credential injection; use names such as AGENT_MAIL_SMTP_PASSWORD or AGENT_MAIL_IMAP_PASSWORD. This fixes misconfiguration-driven pre-startup diagnostic leakage; it does not protect a process started with an already dangerous external environment.

Unsupported/foreign/altered cache identities and layouts fail with ledger_schema/exit 9. The opener validates a consistent schema snapshot before changing persistent journal settings, then rechecks under its initialization lock. Synthetic quiescent WAL fixtures verify unchanged database bytes on refusal; normal SQLite recovery of an interrupted database is a separate boundary.

The public supervisor starts the worker in a separate Linux session. Parent timeout/error cleanup terminates that process group; the worker watchdog also terminates its own session group if its supervisor dies. This includes core status/import descendants, not just the network process. Direct internal worker invocation never signals an inherited terminal group. Ordinary OS scheduling/uninterruptible kernel failures are not hard real-time guarantees. An interrupted core import may already have committed evidence; inspect/replay exact bytes and preserve SMTP history.
