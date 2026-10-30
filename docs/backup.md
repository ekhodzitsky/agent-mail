# Private database snapshots

`backup` copies the selected core database and, if present, its IMAP cache into a **new** private directory. It uses SQLite Backup API, not a raw live file copy. It never connects to SMTP/IMAP, retrieves credentials or restores a running installation.

```sh
agent-mail backup --output /private/backups/snapshot-001 --json
agent-mail verify-backup /private/backups/snapshot-001 --json
```

Use a trusted private parent outside Git and retain another protected backup. The output contains correspondence, PDFs, attempts and evidence: it is unencrypted private data. Existing output paths are refused. Directory/file modes are 0700/0600 on supported Linux. Symlink/nonregular/multilink database or manifest files are refused. Trusted directory ancestors and local storage remain prerequisites.

## Contents and verification

`mail.sqlite3`, optional `imap-dsn.sqlite3`, and `backup.json` are the only accepted files. Configuration, environment, Keyring data, executable backups and arbitrary neighboring files are **not** copied. Preserve private configuration/lookup identities separately using your deployment's protected configuration procedure; never add credentials to a mail snapshot.

Manifest schema 1 contains `schema_version`, RFC3339 `created_at` and ordered `files` entries (`name`, `bytes`, `sha256`). Names are fixed; paths from a manifest are never accepted. `backup.json` is written last, after copying, validating and syncing both databases. Files and directories are synced before successful return. Failed/killed operations can leave partial or complete output; verify it explicitly, preserve originals, and use a new output path for a new attempt. Missing/truncated manifests, unexpected files and hash mismatches refuse verification.

Verification checks exact current core/cache identities/layouts, SQLite quick_check/foreign keys, stored package/status/evidence integrity, and every cache reference's report/package/attempt/hash against core. It reads only the selected snapshot; `--home`/`--config` do not open or initialize an operational store. Returned metadata includes:

```json
{"backup_schema_version":1,"integrity":"verified","cache_included":true,"configuration_included":false,"credentials_included":false,"network_used":false,"freshness":"not_established","retry_authorized":false}
```

This is `result` inside the ordinary schema-1 envelope. `cache_included` reflects presence at snapshot selection. Hashes detect corruption, not a same-user attacker who can rewrite files and manifest. Verification does not prove the snapshot is recent or corresponds to every external SMTP action. Extra operational files created after activating a snapshot make it unsuitable as a pristine backup directory; restore database files into a separate private working directory instead.

## Pair consistency and concurrency

Core is pinned in a read transaction first, then an existing cache is pinned. Both must use rollback **DELETE** journal mode. Those SHARED SQLite locks hold commits until copying/verification completes. A cache row must refer to an already verified core report; a newer-cache/older-core pair fails with `backup_binding`, exit 5. A lagging/empty cache is permitted because core evidence is authoritative. An absent cache is omitted, not created by backup.

SQLite Backup API copies each pinned connection. This is not two independent live snapshots: core stays fixed while the cache snapshot is established, so included references cannot outrun core. WAL cache mode refuses with `backup_mode`, exit 5; unsupported/altered layouts refuse with exit 9. Backup opens existing sources read-only and requires current core schema 6; it refuses older/foreign layouts without creating or migrating operational state. For pre-upgrade older layouts, use the quiescent manual backup procedure before installing/running migrating commands. Copies preserve stored material/history, not a promise of byte-identical SQLite physical layout.

Prefer stopping CLI/adapters/workers before backup. Validation scans the entire archive and can be slow. Holding read locks can delay other commands' writes beyond their busy deadlines; an in-flight SMTP operation may finish remotely while its local final-state update cannot commit, leaving a conservative unknown. A captured unknown remains blocked. Backup never waits for network completion or invents a final outcome. This mechanism coordinates SQLite commits, not provider delivery or multiple independent installations.

Do not open/close the raw **source** SQLite inode while its transaction holds POSIX locks: close can release process-wide file locks. Source reads remain inside SQLite; hashing/fsync operates on closed destination databases. A separate-process writer regression checks that copying/destination hashing do not release the source lock. Synthetic tests also cover incomplete/corrupt snapshots, WAL refusal, actual adapter cache schema, skew, preserved attempts and stale-copy limits.

## Restore boundary

There is intentionally no automatic restore/overwrite command. Follow [upgrade/recovery](upgrade-and-recovery.md): stop all writers, preserve current state, verify the snapshot, compare against external submission history and activate only one reconciled instance. Never replace current accepted/unknown history with an older prepared state to permit resend. A valid stale snapshot can predate an SMTP attempt; offline verification cannot discover that absent event.

Foundation: [SQLite Backup API](https://www.sqlite.org/backup.html), [rollback file locks](https://www.sqlite.org/lockingv3.html), [unsafe raw-copy conditions](https://www.sqlite.org/howtocorrupt.html#_backup_or_restore_while_a_transaction_is_active). Physical power-loss/filesystem guarantees are bounded by the same [supported scope](production-readiness.md).

## Failure evidence added in 0.7.0

The production copy routine advances in 128-page SQLite Backup API steps while retaining the same pinned read snapshots. Test-only internal synchronization stops a child after a real partial step; SIGKILL leaves a rejected incomplete output and releases the source writer lock. A blocked separate-process attempt writer can then commit; the next verified snapshot retains that unknown attempt. There are no production fault flags or automatic retries.

A destination max_page_count budget exercises actual SQLITE_FULL. A separate real CLI subprocess inherits a small RLIMIT_FSIZE and ignored SIGXFSZ; writing the new destination fails with EFBIG/SQLite I/O error, returns storage/exit 7, leaves no success manifest and preserves the source. This is a file-size quota fault, **not** a physical ENOSPC disk-fill experiment. Tests never exhaust a shared filesystem or alter parent limits. Physical power loss and storage firmware behavior remain untested. Repeated synthetic [ext4 measurements](benchmarks.md#persistent-filesystem-backup-070) report backup/verify latency and RSS separately from tmpfs listing results.
