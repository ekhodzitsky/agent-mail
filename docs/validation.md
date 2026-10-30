# Local validation

## Full local gate

Run `bash scripts/check-local.sh` with Bash, Git, system Python 3.11+, OpenSSL CLI, Rust stable/MSRV 1.88 with rustfmt/Clippy, cargo-audit 0.22.2 and cargo-deny 0.20.2. The script does not install tools or dispatch hosted jobs. It can download Cargo/advisory metadata; no real mail server is used.

The gate runs formatting and Clippy with production/test-smtp features on stable and MSRV 1.88, both Rust suites, optional adapter/scanner tests, dependency audit/deny and isolated test/production installations. It never dispatches hosted CI or connects to a real mailbox. Exact source revision, toolchain, commands/results and artifact hashes belong in release VERIFICATION.txt and SHA256SUMS.

The current 0.8.0 source checks have 40 Rust unit tests (including an internal child fixture), 44 default/54 test-smtp CLI tests, 32 keyring adapter tests, 35 IMAP adapter tests, 2 private-marker input tests, 3 documentation-check tests, 2 documented-installer failure tests and 1 real CLI quota-fault test. Test counts describe this source branch, not retroactive changes to an immutable release; each release VERIFICATION lists its tagged checks. Counts are exercised cases, not security certification or coverage percentage.

## Exercised boundaries

| Area | Synthetic checks |
|---|---|
| Frozen materials | Exact letter/PDF/MIME bytes, hashes, tampering refusal, HTML/plain alternatives, private exclusive EML export |
| Submission | Separate CLI processes, complete loopback scenario, exclusive same-package lock, repeat refusal, explicit rejection, timeout/disconnect/process death after DATA |
| TLS and secrets | Implicit TLS/mandatory STARTTLS, untrusted/wrong-host/expired certificates, no credentials before required TLS, safe errors when replies echo a synthetic secret, unsafe credential-destination refusal before lookup/exec |
| Rename compatibility | Existing accepted/unknown history and exact package bytes, fresh defaults, legacy/new environment selection, ambiguity refusal and unchanged credential identity |
| Storage | Durable unknown before network, commit veto and real SQLITE_FULL, migration preservation/concurrent openers, schema identity/index/column refusal, symlink/hardlink/FIFO and permissions checks |
| Observations | Immutable reconciliation/receipt/DSN bytes and binding, idempotent replay, corruption/lock/commit failure, conflicting reports, deterministic report truncation/byte mutations, unchanged resend blocks |
| Agent/IMAP | Parser-derived discovery, operation-key conflicts/concurrency, verified loopback TLS, read-only selected UID/epoch/Message-ID, PEEK limits/deadlines, immutable cache provenance, cache failure ordering and paired core/cache restoration/skew refusal |

Fixtures use synthetic example.org identities/documents and numeric-loopback servers that cannot relay. Ephemeral test certificates are not installed in system trust stores. Test seams never become production flags or certificate bypasses. See [local SMTP walkthrough](local-testing.md).

The source-install/first-use walkthrough was also exercised under empty HOME/Cargo configuration with explicit Rust/C-compiler prerequisites: doctor, prepare, list, inspect/status and dry-run with/without synthetic SMTP configuration. Dry-run without SMTP returns configuration/exit 3; configured dry-run uses no network and creates no attempt. A native release binary was checksum-verified and exercised in clean Ubuntu userspace without Rust/Node/Python at runtime. PRoot userspace checks establish ABI behavior, not VM/kernel isolation.

Optional `scripts/check-upgrade.py OLD_BINARY NEW_BINARY --reports` compares real compatible prior-binary package/status/report bytes and restored-copy resend blocks using synthetic state. Older comparison binaries are not bundled or downloaded automatically. No restore test proves that a real backup is current; see [recovery](upgrade-and-recovery.md).

No throughput, coverage percentage, physical power-loss, broad provider interoperability, unattended consent or independent-audit claim is made. Provider examples and unverified capabilities are listed in [supported scope](production-readiness.md); development scripts never contact a real provider. Keep logs, fixtures generated during testing and all operational evidence outside tracked files.

The local gate also checks Markdown file/heading links and compares the current storage declarations and CLI command table to offline discovery from the installed production binary. Run it directly with `python3 -B scripts/check-docs.py --binary /path/to/agent-mail`. This is a bounded Markdown/contract check, not a complete prose fact checker or external-link validator.

IMAP fault injection also blocks a synthetic core subprocess, then exercises adapter deadline and supervisor death. The managed worker session must leave no live core descendant. Process-group cleanup is separate from SQLite atomicity: interrupted core imports can still have committed evidence, so inspect/replay by exact bytes rather than treating timeout as proof of no local progress.

Snapshot/list checks cover pinned rollback locks through copy/hashing with a separate-process SQLite writer, refusal of WAL cache, original list compatibility, bounded selection/corruption, cache/core skew, real adapter schema, incomplete/tampered copies and explicit stale-copy limits. No backup power-loss guarantee or offline proof of freshness is claimed. GNU time is needed only for the optional listing benchmark, not the core or normal gate.

Version 0.7.0 adds actual child death during stepped copying, competing-process attempt commit/recovery, destination SQLITE_FULL and per-child filesystem-size quota/EFBIG faults; source hashes remain unchanged on failure and no success manifest is published. These are not physical ENOSPC/power-loss tests. Preflight tests verify config/SMTP/env distinctions, no state creation, secretless core invocation, metadata-only flags/no password getters, binary safety, backend/session/item/timeout reporting and safe fixed output. See [preflight](preflight.md) and [backup evidence](backup.md#failure-evidence-added-in-070).

Documentation installer tests execute the published shell recipe with synthetic failing transfer/hash tools and an install spy. They intentionally use bash without global errexit: the recipe must itself prevent installation after verification failure. The tutorial is separately exercised with committed synthetic fixtures; no owner files or real mail are involved.
