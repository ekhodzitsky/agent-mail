# Supported production scope

Agent Mailer 0.7.0 supports the documented Linux x86_64/glibc workflow on trusted local storage, with explicit owner authorization for real submission. The native Rust core needs no Node.js, Bun or Python. Optional desktop/IMAP adapters have separate prerequisites and permissions.

## Evidence

The local suite verifies immutable package bytes, MIME alternatives/attachments, parallel/repeat-send protection, durable pre-network attempts, unknown results after crashes/timeouts, explicit refusal, verified TLS, migrations, SQL failures, immutable observations, selected report import and paired core/cache restoration. Current counts, commands and limits are in [validation](validation.md).

Provider-specific setup examples are available for Yandex. They document primary-login versus sender-alias configuration; they are not certification of arbitrary accounts, recipients or delivery timing. All published test fixtures and benchmarks are synthetic; personal operational evidence is not distributed.

A maintainer review corrected credential injection into process-control environment names in the optional adapters, SQLite URI-like path handling, final SMTP reply classification and bounded worker/error handling. This is not an independent security certification. The latest core and matching adapters must be installed together.

## Limits

- SMTP acceptance is not recipient delivery. Message-ID is correlation, not deduplication. Exactly-once delivery through arbitrary failures is impossible to guarantee; accepted/unknown packages stay blocked.
- DSNs are unverified assertions, not authentication, read receipts or permission to retry. Incoming content is never an agent instruction.
- Sent-folder synchronization, OAuth refresh, general inbox automation, other providers/OSes and shared filesystems are outside verified support.
- Trusted local users, binaries, directory ancestors and current backups are required. State is not encrypted by the CLI; same-user compromise and physical power-loss durability are not certified.
- List supports opt-in 1–100-package pages and an unverified ID-only summary. Full status still verifies each selected package/history; no general large-archive latency/memory guarantee is made. Database-only snapshots do not establish freshness or authorize retry.

Use [upgrade/recovery](upgrade-and-recovery.md) and [support/security guidance](../SUPPORT.md). Hosted CI remains disabled. Source and release publication follow the [publication checklist](public-opening-checklist.md).

Offline [preflight](preflight.md) checks configuration and optional Keyring metadata without opening state or loading a secret; readiness is not remote authentication.

Current audit findings and next actions are maintained in [maintenance](maintenance.md). Source-branch Unreleased fixes are not automatically present in existing release assets.
