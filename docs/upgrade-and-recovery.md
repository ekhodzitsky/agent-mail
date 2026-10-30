# Upgrade and recovery

## Rename to agent-mail (0.8)

The executable, Cargo package/crate, portable skill and adapters now use
`agent-mail`, `agent_mail`, `agent-mail-keyring` and `agent-mail-imap-dsn`.
Update scripts to these names. Previously released tags/assets keep their
original names. JSON, package and database schemas are unchanged.

For fresh installations the default state is `$HOME/.local/share/agent-mail`.
When only `$HOME/.local/share/agent-mailer` exists, both the core and optional
adapters keep using it. They never move or rewrite packages to rename a tool.
If both directories exist, selection fails; explicitly select the correct
core --home. Resolve an adapter's ambiguous defaults before using it.
`AGENT_MAIL_HOME` is canonical; `AGENT_MAILER_HOME` is still accepted.
Conflicting environment selectors fail unless --home is explicit.

Keyring's existing `application=agent-mailer` / `org.agent_mailer.SMTP`
identifiers and private configured password variable names remain stable.
Do not re-store credentials just to rename an executable. For the optional
IMAP adapter, update a private explicit binary path to `~/.local/bin/agent-mail`
without changing home/account settings. Keep old executables until all scripts
and explicit bindings have been updated; do not run old/new senders together
during an upgrade. Back up history first as described below.


Install the default-feature core and matching optional adapters. Keep `~/.local/share/agent-mail`, `AGENT_MAIL_HOME`, private configuration and stable keyring/schema/application/account identifiers. An earlier executable name is not a reason to create a fresh store. If maintaining an old command alias, point it at the current installation rather than running separate old executables. The configured core/adapter paths must match the fixed destinations documented by the keyring adapter.

Current contracts: JSON envelope 1, core storage 6, package manifests 1–3 and optional IMAP cache 1. Recognized older core layouts migrate atomically without rewriting frozen materials or attempts. Older readers cannot open schema 6; downgrade is unsupported. See [migration details](storage-migrations.md).

## Protected backup and restore

`backup --output NEW_DIRECTORY` and `verify-backup DIRECTORY` provide database-only snapshots; see [snapshot consistency and limits](backup.md). Snapshots exclude configuration/credentials and do not automate restoration or establish freshness. Prefer stopping writers before running it. The manual procedure below also applies to older compatible installations.

1. Stop all core CLI/adapters/workers before copying state. Separate SQLite backup calls are individually consistent, not a global live snapshot of core and cache; quiesce writers first.
2. Keep a mode-0700 backup outside Git containing private configuration, `mail.sqlite3` and optional `imap-dsn.sqlite3` with private file permissions. Credentials remain in their external secret store/environment, not in the mail archive. Preserve lookup identities and retain another protected copy before investigating damage.
3. Check SQLite quick_check and expected identities/schemas on the copies. A matching database pair does not prove a backup is current: compare it against subsequent provider/owner events before enabling submission.
4. Restore into a private destination with the original instance stopped. Use doctor/inspect/status/inspect-dsn to verify packages, attempts, operation bindings and evidence. Never run original/restored copies as independent senders, delete attempts or reset unknown state.
5. Cache references must resolve in the core. A newer cache referencing evidence absent from an older core fails closed before FETCH. An older/missing cache may require authorized selected evidence retrieval; exact-byte core deduplication is authoritative. Cache state never grants SMTP retry permission.

Accepted/unknown attempts must remain blocked. A stale backup can forget a real submission, which schema/version checks alone cannot detect. Real sending or IMAP retrieval requires a separate explicit owner instruction. The [local validation suite](validation.md) exercises paired restoration and mismatched cache/core refusal with synthetic data.

For an older rejected relative state path beginning with `file:`, do not create a fresh database to bypass the refusal. Stop writers, preserve the entire original private directory and investigate its actual SQLite file location and journal before moving it to a normal trusted path. Never guess that a newly empty store represents the original submission history.
