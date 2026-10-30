# Changelog

User-visible changes are recorded here. Follow [versioning](VERSIONING.md); product and serialized contract versions are independent.

## Unreleased

### Documentation

- Refocus the root README on sending from an existing mailbox, interrupted-agent recovery and a short prepare/review/send example with selected real JSON fields. Prioritize native installation, add agent onboarding, distinguish caller consent from CLI behavior and separate use cases from limitations.
- Remove the native binary download-count badge and its explanatory text from the root README.

## 0.8.0 — 2026-10-30

### Changed

- Rename the Cargo package, Rust crate, binary, adapters and portable skill to agent-mail. Update current examples, discovery, distribution tooling and documentation. This is an incompatible command-name change; update scripts and explicit adapter binary paths.
- Preserve existing package/attempt bytes, storage schemas, legacy default-state selection, legacy home environment fallback and Keyring lookup identities. Refuse ambiguous histories before opening state or loading credentials; new installations use the agent-mail directory.
- Rewrite the root README around the agent's prepare/review/authorized-send workflow, a short command example, practical benefits and focused limitations, following ryf's direct introduction/example/capabilities structure.

### Documentation

- Rework the root README around a visible prepare/inspect/offline-validation workflow and actual JSON result; move detailed native installation into the installation guide and add a complete synthetic tutorial with committed byte fixtures.
- Add canonical configuration/path/SMTP/timeout reference, task-oriented documentation index and ordered command/agent workflow. Keep diagnostics, consent and outcome recovery separate from serialized reference details.
- Correct stale no-pagination/schema-opening claims, missing backup error codes and outdated comparison version; document read-only preflight/backup exceptions and strict-versus-header DSN mode clearly. Remove account-specific operational narrative and qualify historical benchmark reproducibility.
- Audit root/support/security/versioning, operational guides, contracts and adapter documentation against current source/discovery. Runtime behavior, release tags, state/credentials and compiled artifacts are unchanged.

## 0.7.1 — 2026-10-30

- Align the root README with Root README standard v1: complete native installation/prepare/inspect/dry-run, explicit prerequisites and expected success, primary-source alternatives, current-version repeated measurements, accurate per-asset download badge and visible limitations.
- Publish matching native/source/adapters from the independent reviewed source tree. Preserve runtime behavior, state/credential identities, dependencies and serialized contracts. Hosted CI remains disabled; no personal operational data is included.

## 0.7.0 — 2026-10-30

### Initial public source baseline

- Publish Agent Mail with a new source-only Git history, MIT license, installation/support/security/contribution guides and locally verified release assets. Runtime executables retain agent-mailer/agent-mailer-keyring/agent-mailer-imap-dsn names for compatibility. Configuration, credentials, correspondence, operational databases and private development history are excluded.
- Provide immutable reviewed packages with exact sender/recipient, final plain/HTML letters, PDF bytes, hashes, MIME and Message-ID. Support optional operation keys and one-package legacy import; JSON envelope 1, manifest 1–3 and core storage 6.
- Submit through maintained lettre/rustls with verified TLS, durable pre-network attempts, per-package locking and accepted/unknown resend blocks. No automatic retry, consent inference or delivery guarantee.
- Record immutable reconciliation, recipient observations and selected DSN assertions without granting retries. Optional read-only IMAP retrieval uses selected UID/Message-ID/epoch binding, bounded PEEK and cache 1; incoming messages are never agent instructions.
- Provide opt-in bounded list pages, explicitly unverified ID summaries, private database-only snapshot/verification and backup manifest 1. Integrity does not establish snapshot freshness or retry authorization.
- Include offline core/Keyring preflight with preflight result schema 1: configuration/environment or metadata-only checks, no state initialization, password loading in desktop preflight, unlock prompts or mail network.
- Verify corruption/concurrency/crash/TLS/schema/recovery boundaries using synthetic fixtures, including mid-copy SIGKILL, actual SQLite page exhaustion and per-child file-size quota failures. Publish reproducible scoped latency/RSS measurements; physical ENOSPC/power-loss and broad providers/platforms are not certified.

Checks run locally; hosted CI and Cargo registry publication are disabled. Real mail always needs explicit owner authorization for the reviewed package.
