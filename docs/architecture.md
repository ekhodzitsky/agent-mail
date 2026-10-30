# Local storage and submission safety

## Decision: SQLite, one local state directory

A single SQLite database stores immutable manifest JSON, its SHA-256, exact attachment BLOBs, and the prepared MIME bytes. A preparation transaction makes these visible together; no directory rename/manifest/attachment partial-commit protocol is needed. SQLite is bundled into the binary through rusqlite. `journal_mode=DELETE` and `synchronous=EXTRA` establish the local commit barrier, including syncing the directory after rollback-journal deletion. Keep the database on a durable local filesystem that honors fsync and locks. NFS, cloud-synced live databases, independent database copies and multi-host submission are unsupported.

SQLite is opened with an absolute filesystem path to the checked file, not a URI. Ambiguous relative homes starting with `file:` are rejected before creation; ordinary relative paths and literal absolute paths remain supported. This prevents SQLite URI semantics from selecting another database outside path/permission checks.

Database identity and migrations are described in [storage migrations](storage-migrations.md). Completed attempts cannot be updated or deleted through ordinary SQL; finalization can only close an unfinished unknown attempt while preserving its identity and start fields.

Packages have UUIDs. Triggers disallow UPDATE/DELETE of package and attachment rows through ordinary SQL. Every inspect/send/status verifies the manifest digest, component hashes, sizes, schema and attachment count. SHA-256 detects corruption, not malicious rewriting by someone who can modify the database and its hashes. This is not a cryptographic signature or an authorization system. Store permissions are 0700/0600 on Unix; protect existing configuration and backups too. Database contents are unencrypted private data.

Preparation reads bounded regular files: at most five PDFs, 25 MiB per PDF, 1 MiB per text/config/import manifest, 998 UTF-8 bytes per single-line subject. PDF validation checks the `%PDF-` prefix, not document validity or rendering. Letter and subject are UTF-8. Original PDF filenames, exact source bytes, sender/recipient, full final body including signature, UTC RFC3339 preparation time, hashes and sizes are retained. Letter and PDF MIME parts use explicit base64 to preserve source bytes. MIME boundaries, Date and Message-ID are fixed at preparation. The SMTP library adds protocol framing/dot-stuffing; no application content is regenerated during send.

## State transitions

Current storage schema 6 includes optional preparation-operation identity. The key hash, versioned exact-input fingerprint and package are published in the same IMMEDIATE transaction. Repeated identical preparation verifies/reuses the original package and current state; changed input conflicts. No key expiry/reset is provided, and another key is not a safe workaround for accepted/unknown state. Existing package/attempt/evidence rows are unchanged. See the [agent workflow and upgrade contract](agent-workflow.md).

1. `prepared`: immutable package exists; no attempt exists.
2. Before any network operation, acquire an exclusive OS lock for that package. Validate package, config and resend policy.
3. Commit a separate attempt with UUID, UTC start time, frozen Message-ID and `unknown` outcome. This intentionally means “may be in progress, or interrupted”; it never implies non-delivery.
4. Only after durable commit, call lettre once with the saved envelope and MIME bytes.
5. Store UTC completion time and `accepted`, `rejected` or `unknown`.

`accepted` means the SMTP server returned a successful response to submission. `rejected` means a 4xx/5xx SMTP rejection (at connection, authentication, envelope or DATA stage). Other errors conservatively mean `unknown`, even when failure probably occurred before DATA. An unresolved attempt after process death has a null completion time. Error responses direct agents to `status` for the full record.

An OS lock held until the attempt finishes prevents simultaneous submissions of the same package; a partial unique SQLite index independently permits at most one accepted/unresolved attempt. Locks are released by the OS on process death, while the durable unknown record remains. Lock files must not be removed while a process is running. The lock spans synchronous network I/O deliberately: it protects the one package, not the entire database.

Accepted and unknown attempts block subsequent `send`, including dry-run. Explicitly rejected attempts may be retried with a **new owner instruction**; each attempt gets a new UUID and the package's same Message-ID. No automatic retry loop, force-resend or reset command exists. Preparing/importing a new package creates a new identity: this mechanism cannot prevent a human or agent from deliberately recreating the same letter under a different UUID.

## Failure boundaries

- Crash before attempt commit: no network call has begun.
- Crash after commit but before connection: unknown, conservatively blocked.
- Timeout/disconnect after DATA: unknown, blocked.
- Acceptance followed by process death or failed local final-state write: unknown, blocked.
- Restoring an old backup can erase attempt evidence and defeat deduplication. Never submit from restored/copied state without reconciling external history.

SMTP cannot guarantee exactly-once delivery across arbitrary failures. The server may accept mail but its reply may be lost. A repeated Message-ID does not oblige it or the recipient to discard duplicates. After unknown outcomes, investigate provider logs with Message-ID and timestamps; do not edit the database to clear the block. The [reconciliation workflow](reconciliation.md) records external observations without rewriting history or enabling retries.

## SMTP, TLS and diagnostics

lettre 0.11 supplies SMTP, MIME, authentication and rustls transport. `tls` uses certificate-verified implicit TLS; `starttls` requires the upgrade and verification. There is no insecure TLS switch. The optional development feature `test-smtp` enables `loopback_test` only for numeric loopback IPs without credentials. Do not ship that feature in production builds.

The journal records the available three-digit SMTP reply code, or null when unavailable. Free-form reply text is deliberately not retained: a server may echo credentials, recipient addresses or private letter fragments. `server_response.text_retained=false` makes this reduction explicit. This loses provider queue IDs and enhanced-status prose; Message-ID and timestamps remain the safe correlation fields. No transport debug/tracing feature is enabled.

Credentials are read from a named environment variable, never embedded in an archive, error, command argument or configuration value. Current authentication supports lettre's username/password mechanisms. Provider-specific OAuth refresh is future work; optional desktop Keyring credential injection is already available separately from the core. `doctor` and dry-run construct/validate configuration without DNS or connections; they cannot certify login, certificates, server reachability or provider limits.

SMTP may not save Sent-folder copies. IMAP APPEND/provider API synchronization and Markdown writeback are separate integrations, not responsibilities of the core sender.

## Dependency choices

- lettre/rustls: maintained SMTP/MIME/TLS rather than custom protocols or crypto.
- rusqlite (bundled SQLite): transactional durability and a single-file local archive without a service.
- fs2: portable exclusive file locking; tested here on Linux.
- rustix (Unix): safe no-follow/nonblocking descriptor opens and regular-file handling.
- sha2, uuid, chrono: package checksums, random identifiers and timezone-aware timestamps.
- clap, serde/serde_json, toml: explicit noninteractive CLI and versioned structured contracts.
- tempfile: test isolation only.
- rcgen and direct rustls dev-dependencies: ephemeral certificate generation and loopback TLS test servers. SQLite hooks are enabled only for tests to veto commits. These add no production configuration or certificate-verification bypass.

Cargo.lock fixes resolved versions. Application code is safe Rust; dependencies may contain unsafe code. Dependency checks and exercised safety boundaries are described in [validation](validation.md). No independent audit, power-loss laboratory test, Windows/macOS validation or broad real-provider certification is claimed.

Sources: [lettre transport API](https://docs.rs/lettre/0.11.23/lettre/transport/smtp/struct.SmtpTransport.html), [SQLite atomic commit](https://www.sqlite.org/atomiccommit.html), [SQLite synchronous](https://www.sqlite.org/pragma.html#pragma_synchronous). Exact crash tests and scope are in [validation](validation.md).

On Unix, file inputs must be regular files with no final-component symlink; copy the selected bytes into a regular file if necessary. State/lock files also require a single hard link and get mode 0600 on each open. The final home component must be a directory, not a symlink. Trusted ancestors and protection from same-user file replacement remain deployment assumptions.

## Module boundaries and audit follow-up

CLI dispatch is in lib.rs; config.rs validates configuration/addresses; files.rs handles bounded/private files; package.rs builds and exports immutable MIME; store.rs verifies SQL data; smtp.rs owns transport construction and submission. Legacy import, reconciliation and migrations remain separate modules. DSN report interpretation, transport, persistence and tests have separate modules; operational limits are in [supported scope](production-readiness.md).

Only final DATA code 250 counts as accepted. A positive but unexpected code is unknown and blocks resubmission. timeout_seconds is a socket I/O setting, not a strict wall-clock deadline for the entire DNS/SMTP session.

## Total network deadline

Normal `send` validates offline and launches a network worker in the same standalone binary. The worker repeats package/policy validation under the package lock, commits unknown durably, then calls lettre once. The parent starts its monotonic budget before spawning; on expiry it kills and reaps the worker before returning outcome_unknown. The worker also starts an independent process-exit watchdog before opening SQLite, so it cannot continue indefinitely after supervisor death. This safely interrupts blocked synchronous DNS and SMTP/TLS calls without leaving a live sending thread behind.

`total_timeout_seconds` defaults to 60 (1–600). `timeout_seconds` remains the socket I/O timeout (1–300). Dry-run is offline and creates no worker/attempt. A timeout before attempt creation leaves prepared; after commit it leaves unknown, unless a completed result already committed. Always inspect status after error/absent output. An accepted commit can win the deadline race; never resend on the basis of exit 8 alone. Ordinary OS scheduling/process-launch and uninterruptible kernel/filesystem failures are not hard realtime guarantees. The deadline bounds the network worker, not every preparation/SQLite/hash operation in the parent.

The hidden smtp-worker command is internal subprocess plumbing, not a public API. It still enforces the durable attempt, package lock, resend policy and its own required 1–600-second watchdog. It receives paths/IDs only through arguments; credentials remain in the inherited environment. Its output is a small fixed envelope; raw SMTP data/status/secret text is never sent over that pipe. The parent reads the pipe only after exit; no live writer can strand a receive. A worker watchdog can terminate without JSON; the public supervisor converts that to a safe outcome_unknown envelope.
