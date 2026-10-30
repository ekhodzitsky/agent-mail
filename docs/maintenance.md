# Maintenance and audit findings

This living summary records bounded maintainer review, not independent certification. Keep implementation checks in [validation](validation.md), performance evidence in [benchmarks](benchmarks.md), runtime scope in [support boundaries](production-readiness.md), and release-specific proof with its artifacts. Personal operational material stays outside Git.

## Documentation audit — current source

Reviewed the root README/AGENTS/CONTRIBUTING/SECURITY/SUPPORT/VERSIONING/CHANGELOG, documentation navigation, command/JSON/configuration guides, storage/submission/recovery, DSN/import/evidence, installation/validation/benchmarks and optional adapter instructions. Claims were compared with current source, parser-derived describe and synthetic execution. This revision changes documentation/examples only; runtime behavior and published tags/assets are preserved.

| Priority | Confirmed problem | Refactor/disposition |
|---|---|---|
| P2 | Root README's largest initial block was an installer, hiding prepare/review/dry-run and giving no compact actual result | Put a short source/example workflow and fixed JSON projection in the root; move native download/checksum details to installation; add a complete tutorial with synthetic committed inputs |
| P2 | A pasted native install sequence could continue after curl/checksum failure | Put native retrieval/check/install inside a fail-fast subshell; test that injected download/hash failures never reach install |
| P2 | Fresh source installation assumed an existing checkout; configuration path/default/credential rules were scattered | Provide explicit clone/install prerequisites and one canonical configuration reference; keep a native alternative and adapter fixed-path distinction |
| P2 | JSON reference said v1 had no pagination, then documented implemented pagination later | Remove contradiction; separate legacy full list from bounded pages and unverified IDs, preserving exact shape/options |
| P2 | Storage-opening prose ignored config-only preflight and read-only backup/selected-snapshot verification | Add command-effects matrix and update migration/diagnostic links; current schema remains 6 |
| P2 | Exit table omitted backup_mode/backup_binding and agent guide mixed feature chronology with submission decisions | Complete error list and lead the workflow with review/authorization/outcome actions; keep fingerprint/guidance details after main steps |
| P2 | Comparison named our source 0.5.x; generic DSN prose sounded like no-ENVID mode was impossible; an old account-specific operational anecdote remained | Correct current version, qualify strict/header modes, remove deployment narrative and keep primary technical evidence |
| P2 | Introductory pages repeated policy/current-feature summaries and had no clear canonical ownership | Route by task; separate setup, operation, fields, compatibility and maintainer evidence; link to canonical facts rather than repeat feature history |
| P3 | Old benchmark rows refer to versions not distributed by this public repository | Identify historical observations and reproduction prerequisites; current 0.7.1 recipe/raw samples remain reproducible |

## Root README v1 acceptance

The installed Root README standard v1/skeleton remains the reference, adapted for a native CLI rather than copied literally.

| Check | Result / evidence |
|---|---|
| Purpose | Pass: AI-agent SMTP workflow, saved final inputs and distinct structured outcomes |
| Hook | Pass: real prepare/inspect/dry-run commands plus exact projected JSON result; SMTP/delivery distinction remains visible |
| First useful result | Synthetic fixtures and commands are executed in an isolated home; source install/prepare/inspect/dry-run passed from the reviewed working tree with cached locked dependencies before the documentation commit |
| Alternatives | Pass for dated primary-source msmtp/Himalaya documentation, not competitor runtime or retry certification |
| Measurements | Pass for the named repeated current-version small workload; no broad performance superiority or persistent-disk preparation claim |
| Badges | Partial by explicit scope: version/license and locally verified source MSRV, native-asset download count; hosted CI disabled and coverage unmeasured, so no fabricated indicators |
| Constraints | Pass: native/adapter prerequisites, consent, unknown outcomes, private state and unsupported capabilities are explicit |
| Presentation | Concise English root, short functional example, task routes and checked headings/links; rendered HTML structure reviewed, not a browser/pixel-level certification |

Public release download behavior is separate from core correctness: direct native transfer in this environment hit a timeout, while the GitHub CLI retrieved the full published asset and its checksum matched. Installation documents bounded requests/source fallback; no partial file is treated as installed. Do not label a failed download test as a passed first-run check.

## Maintained runtime regressions

The source baseline already includes worker process-group deadline cleanup, pre-PRAGMA cache schema validation, canonical selected-command JSON help, fixed configuration-before-Keyring-store setup and local document/contract checks. Their synthetic regressions remain in the normal gate. Matching release adapters must be installed with the core; changing source docs does not update an existing executable automatically.

## Safety conclusions and limits

Reviewed paths preserve the package lock, durable unknown attempt before network I/O, accepted/unknown resend block, final-DATA acceptance distinction, verified TLS and fixed safe errors. The locked lettre source drops/aborts the standard connection after the submission result; no post-acceptance QUIT refusal is turned into a retryable outcome. DSN and reconciliation remain assertions, never authorization or a delivery guarantee. Existing tests exercise crash/concurrency/TLS/SQL failure, migrations, immutable bytes and restoration. No new defect was established in those reviewed submission safeguards by this audit; this is bounded evidence, not complete certification.

Opening effects are command-specific: preflight is configuration-only, backup reads existing current-schema state, and verify-backup reads a selected snapshot. Ordinary stateful commands can initialize/migrate history; use the [effects matrix](json-contract.md#state-opening-effects). Trusted local users/ancestors/binaries/backups are prerequisites. Hashes detect corruption, not a same-user attacker who can rewrite data and hashes. Exactly-once SMTP delivery, broad providers/platforms, physical power-loss guarantees and report authenticity remain outside the promise. See [supported scope](production-readiness.md) and [validation](validation.md).

## Prioritized next tasks

1. **P1 — Maintain matching verified releases.** For each patch, run the full local gate, verify prior-binary upgrade/restore, build normalized native/source assets, verify checksums and install only the reviewed matching set after private executable backup. Preserve published tags/history; never import private backup refs. No real-provider check is implied.
2. **P1 — Maintain publication hygiene and private reporting.** Recheck tracked source, commit messages, tagged archives and release assets before each publication. Keep security reporting private and verified, exclude operational/private development material, and keep hosted CI disabled under the current project policy. Follow the [publication checklist](public-opening-checklist.md).
3. **P2 — Maintain snapshot/recovery coverage.** 0.6.0 implements private database-only backup/verification, core-first pinned DELETE-journal reads, SQLite Backup API, pair validation and partial/skew/stale-copy regressions. No automatic restore or freshness/retry claim. 0.7.0 adds real partial-copy child death, post-death writer/unknown preservation, SQLITE_FULL/quota faults and repeated ext4 measurements. Physical power-loss/ENOSPC and report-heavy workloads remain untested; configuration remains separately protected. See [snapshot guide](backup.md).
4. **P2 — Expand measured archive workloads.** 0.6.0 adds opt-in 1–100-package pages and ID-only summaries, preserving legacy shape and send/export checks. Synthetic measurements record latency/RSS and distinguish reduced work from full verification. Persistent-disk, many-report and larger-attachment scenarios remain to measure; no global memory/latency bound is promised.
5. **P2 — Maintain agent-safe onboarding.** 0.7.0 adds core preflight without state/network and adapter preflight without secret loading/unlock. Missing bindings/binaries, invalid SMTP settings, credential availability and desktop item/backend/session/timeouts have fixed statuses. Readiness is local only; keep interactive storage owner-operated and preserve existing run/doctor behavior. See [preflight](preflight.md).
6. **P3 — Provider integrations after core maintenance.** Sent-folder synchronization and OAuth refresh may help deployments that need them. Keep write-capable mailbox operations in separately configured/authorized integrations; read-only DSN retrieval must remain read-only. Test partial success and idempotency before expanding scope.

Do not begin a general refactor or add a distributed service solely to make the repository look more complex. Refactor at demonstrated boundaries; the supported runtime remains a standalone local Rust binary.
