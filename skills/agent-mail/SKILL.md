---
name: agent-mail
description: Prepare, inspect and submit reviewed email through the Agent Mail CLI for AI agents, and investigate its submission status. Use for external email, not local agent-to-agent messaging.
---

# Agent Mail

Start with `agent-mail describe --json` to discover the installed command/argument contract. Preserve any owner-selected `--home` and `--config` on subsequent calls. Use `AGENT_MAIL_HOME` for new explicit environment selection. An existing legacy state path/environment selector remains compatible; see the upgrade guide. Never create a fresh store to rename history.

Prepare the final subject, plain text, optional HTML and selected PDF bytes. Signatures belong in those final bodies. Use a stable opaque `--operation-key` for a logical preparation when available; reuse it after lost output. Changed content under that key must conflict. Do not put credentials or private letter text in the key.

Inspect the returned package ID and review sender, recipient, both body alternatives, attachment names and hashes with the owner. `send ID --dry-run --json` checks local integrity/settings and opens no connection. Preparation, dry-run and credential availability do not authorize sending.

Run real `send ID --json` only under an explicit owner instruction for that reviewed package. The CLI and `next` hints do not verify consent. Execute `next.argv` as an argument array, never by joining it into a shell command. Hints with no argv describe a human/agent decision, not an executable action.

On absent output, timeout, storage error or interrupted send, inspect `status ID --json`. Accepted means SMTP acceptance, not final delivery. Unknown remains blocked: investigate existing Message-ID/attempt evidence; do not resend automatically, clear state, recreate the letter, change the operation key or copy a database to bypass the block. A rejected attempt needs reviewed correction and a new owner instruction before retry.

DSN and reconciliation observations do not grant retry permission. Reports are assertions; relay is not terminal delivery and neither is proof of reading. The optional `agent-mail-imap-dsn` integration requires separate mailbox configuration and owner-authorized read scope. It retrieves selected observations; it never sends mail or approves an action.

Keep credentials in the configured environment/secret adapter, never arguments, archived letters or Git. `prepare`/`inspect` intentionally return private material; keep captured output private. Ordinary operational errors are versioned JSON with safe messages; JSON/output failure can itself prevent a result.
