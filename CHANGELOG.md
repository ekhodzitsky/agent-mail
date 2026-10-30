# Changelog

User-visible changes are recorded here. Follow [versioning](VERSIONING.md); product and serialized contract versions are independent.

## Unreleased

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
