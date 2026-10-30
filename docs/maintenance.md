# Maintenance and next work

This is the maintained quality/backlog summary, updated in place. It records a maintainer audit of core preparation/storage/submission, DSN parsing/evidence, optional adapters, local checks and documentation. It is not an independent external audit or a proof against all failure schedules. Operational material and unrelated private development history stay outside this repository.

## Confirmed findings and current disposition

| Priority | Finding | Evidence and disposition |
|---|---|---|
| P2 | An IMAP timeout could leave a core status/import descendant alive after the worker ended | A blocked synthetic core survived the old total deadline. 0.5.2: the worker owns a separate session; parent timeout/error cleanup and the worker watchdog terminate its process group. Deadline and supervisor-death regressions verify no live synthetic core remains; no mail server is contacted. |
| P2 | The IMAP cache opener changed a foreign/future/altered SQLite database's journal mode before refusing its schema | Three quiescent synthetic WAL fixtures failed byte-equality before the fix and pass afterward. 0.5.2: consistent read validation before PRAGMAs, followed by revalidation under the initialization lock. No core/cache schema or retry-policy change. Normal SQLite hot-journal recovery is a separate boundary. |
| P2 | JSON subcommand help returned root help, hiding the selected command's options | A stateless prepare-help regression failed before the fix and passes afterward. 0.5.2: use the selected parser help; keep safe fixed usage errors and version behavior. Canonicalize the usage name; the regression also supplies a different argv[0], and the normalized release artifact is exercised directly. |
| P2 | Operational documentation contradicted current storage 6 with current-schema 4/5 statements | Current declarations, migration/write effects and guidance corrected. A local checker compares declarations and CLI commands to the actual binary's stateless discovery. |
| P2 | Keyring first-install instructions invoked store before required configuration and did not distinguish Cargo's default bin directory from the launcher's fixed path | Setup order/path/permissions clarified; preserve working lookup identities. Remove stale account-specific narrative and an implied separate password-manager requirement. |
| P2 | Local checks did not validate documentation file/heading links or the displayed command/schema contract | Add a no-network checker and synthetic broken-link/schema-drift fixtures to the local gate. It does not certify every sentence or external website. |

Changes above are included in the 0.5.2 maintenance patch. Install its matching core and adapters; an unchanged older installation does not receive fixes from a source update. Verify exact artifact hashes and upgrade/restore checks in release evidence.

## Root and reference documentation audit

The installed handbook’s Root README standard v1 and skeleton were reviewed, including the root README, AGENTS, CONTRIBUTING, SUPPORT, SECURITY, VERSIONING and CHANGELOG. The canonical documentation map now defines ownership and routes; moving command details did not remove them. Setup and recovery references retain their separate safety boundaries.

| Standard check | Result and evidence |
|---|---|
| Purpose / hook | Pass: factual package/review/submission boundary and visible unknown/accepted limits |
| First run | Pass for the package workflow: documented synthetic prepare/inspect and configured dry-run succeed without mail network or attempts. The README includes native GitHub download/checksum/install commands; those are checked against public assets as part of publication. |
| Alternatives | Pass for dated documentation comparison: primary msmtp/Himalaya sources; competitor runtime/retry guarantees explicitly not verified |
| Measurements | Pass for the declared 0.7.1 small offline workload only: reproducible repeated samples and unchanged dry-run archive. Persistent-disk, large-archive and competitor measurements remain absent. |
| Badges | Partial: release/license/MSRV link to actual sources; local MSRV gate replaces hosted verification under owner policy. CI and coverage are unconfigured, so no fabricated indicators. The download badge reads cumulative GitHub downloads of the 0.7.1 native asset only; it is not a user/installation count. |
| Constraints | Pass: consent, SMTP ambiguity, unencrypted trusted storage and adapter/runtime scope visible |
| Presentation / links | Pass for source review and local checks: concise English root, detailed reference links, command/schema parity. No browser-rendered visual review was available. |

The README is not a complete external standard certification: missing coverage/hosted indicators and broader measurements remain explicit. Published assets must carry their own exact-version evidence; unchanged older assets are not implicitly fixed.

## Safety conclusions and limits

Reviewed paths preserve the package lock, durable unknown attempt before network I/O, accepted/unknown resend block, final-DATA acceptance distinction, verified TLS and fixed safe errors. The locked lettre source drops/aborts the standard connection after the submission result; no post-acceptance QUIT refusal is turned into a retryable outcome. DSN and reconciliation remain assertions, never authorization or a delivery guarantee. Existing tests exercise crash/concurrency/TLS/SQL failure, migrations, immutable bytes and restoration. No new defect was established in those reviewed submission safeguards by this audit; this is bounded evidence, not complete certification.

Stateful commands can create/open/migrate local state; only describe/help/version are stateless. Trusted local users/ancestors/binaries/backups are prerequisites. Hashes detect corruption, not a same-user attacker who can rewrite data and hashes. Exactly-once SMTP delivery, broad providers/platforms, physical power-loss guarantees and report authenticity remain outside the promise. See [supported scope](production-readiness.md) and [validation](validation.md).



## Prioritized next tasks

1. **P1 — Maintain matching verified releases.** For each patch, run the full local gate, verify prior-binary upgrade/restore, build normalized native/source assets, verify checksums and install only the reviewed matching set after private executable backup. Preserve published tags/history; never import private backup refs. No real-provider check is implied.
2. **P1 — Maintain publication hygiene and private reporting.** Recheck tracked source, commit messages, tagged archives and release assets before each publication. Keep security reporting private and verified, exclude operational/private development material, and keep hosted CI disabled under the current project policy. Follow the [publication checklist](public-opening-checklist.md).
3. **P2 — Maintain snapshot/recovery coverage.** 0.6.0 implements private database-only backup/verification, core-first pinned DELETE-journal reads, SQLite Backup API, pair validation and partial/skew/stale-copy regressions. No automatic restore or freshness/retry claim. 0.7.0 adds real partial-copy child death, post-death writer/unknown preservation, SQLITE_FULL/quota faults and repeated ext4 measurements. Physical power-loss/ENOSPC and report-heavy workloads remain untested; configuration remains separately protected. See [snapshot guide](backup.md).
4. **P2 — Expand measured archive workloads.** 0.6.0 adds opt-in 1–100-package pages and ID-only summaries, preserving legacy shape and send/export checks. Synthetic measurements record latency/RSS and distinguish reduced work from full verification. Persistent-disk, many-report and larger-attachment scenarios remain to measure; no global memory/latency bound is promised.
5. **P2 — Maintain agent-safe onboarding.** 0.7.0 adds core preflight without state/network and adapter preflight without secret loading/unlock. Missing bindings/binaries, invalid SMTP settings, credential availability and desktop item/backend/session/timeouts have fixed statuses. Readiness is local only; keep interactive storage owner-operated and preserve existing run/doctor behavior. See [preflight](preflight.md).
6. **P3 — Provider integrations after core maintenance.** Sent-folder synchronization and OAuth refresh may help deployments that need them. Keep write-capable mailbox operations in separately configured/authorized integrations; read-only DSN retrieval must remain read-only. Test partial success and idempotency before expanding scope.

Do not begin a general refactor or add a distributed service solely to make the repository look more complex. Refactor at demonstrated boundaries; the supported runtime remains a standalone local Rust binary.
