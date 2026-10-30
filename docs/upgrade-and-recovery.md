# Upgrade and recovery

Install the default-feature core and matching optional adapters. Keep `~/.local/share/agent-mailer`, `AGENT_MAILER_HOME`, private configuration and stable keyring/schema/application/account identifiers. An earlier executable name is not a reason to create a fresh store. If maintaining an old command alias, point it at the current installation rather than running separate old executables. The configured core/adapter paths must match the fixed destinations documented by the keyring adapter.

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
