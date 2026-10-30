# Storage schema 6

The product version and JSON/manifest schema are independent of the SQLite schema. Current storage has `PRAGMA application_id = 0x414d4c52` (AMLR) and `PRAGMA user_version = 6`. `doctor --json` reports `storage_schema_version:6`.

## Opening and migration

Every stateful CLI operation opens the selected store, including doctor, inspect, status and dry-run. Stateless describe/help/version do not. Opening a new empty database creates schema 6. Opening the exact initial unversioned development schema (application_id/user_version both zero) atomically upgrades it to schema 6. The exact schema-1, schema-2, schema-3, schema-4 and schema-5 layouts are also supported. Upgrading installs missing completed-attempt protections, reconciliation/DSN/operation tables, header-correlated reports and the current database identity; package and attempt rows are not rewritten, rehashed, regenerated or marked sent.

Recognition compares the tables, indexes and triggers in sqlite_schema to the known canonical layout, including SQL definitions. A consistent read transaction checks identity/layout before schema or journal changes. After acquiring an immediate write transaction, the opener checks again, so concurrent openers can observe a completed migration safely. Migration validates SQLite quick_check and foreign-key consistency, installs the additional triggers and sets both identity fields in the same transaction. Commit failure rolls back schema additions and version stamps. Normal durability remains DELETE journal plus synchronous EXTRA.

Version 1 preserved all original columns. Version 2 adds reconciliations and their integrity/identity constraints; existing package and attempt rows stay unchanged. Additional triggers prevent deleting attempts, changing completed attempts or altering attempt identity/start fields during finalization. The sender may finalize only an unfinished unknown attempt with a completion timestamp; rejected, accepted and finished-unknown records are historical evidence.

Future versions, foreign application IDs, partial layouts, missing safety objects and unexpected schema additions return `storage_schema`, exit 9, before a submission. They are not automatically repaired. A same-version database with manually reformatted/recreated SQL definitions may also be rejected: this deliberately strict first migration recognizes only application-created layouts.

## Operating safely

Use a dedicated local state directory. Before an upgrade, stop all CLI processes and make an offline backup of the complete state directory. Keep the backup private. If a process ended abnormally, let the current compatible SQLite reader recover its journal before taking a clean backup; do not delete journal files manually.

Run the new binary's doctor, inspect selected packages and compare status/attempt history with the pre-upgrade evidence. These commands may perform the migration but do not send mail. Do not use an old binary to write a migrated database; no downgrade workflow is supported.

If migration fails, preserve the original database and error, restore disk capacity/permissions where applicable, and investigate with a compatible reader. Do not reset user_version, remove triggers or delete attempts to force an upgrade. A failed migration is not permission to resend.

A backup restored from before an attempt may have forgotten a real submission. Never send from a restored copy until its attempt history is reconciled against external evidence. Schema versioning does not solve this fundamental stale-backup problem. Copying the database to another independently sending instance is unsupported.

Version 3 replaces the reconciliation insertion guard to admit accepted attempts as well as unknown attempts. Application validation permits accepted-attempt evidence only for recipient_confirmation/received. Existing evidence, packages and attempts are preserved byte for byte. Old binaries reject schema 3; keep a private pre-upgrade backup and do not downgrade an active sender.

Schema 4 adds append-only dsn_events and dsn_reports tables with foreign keys, immutable triggers and exact-byte/digest report idempotency. Existing package/attachment/attempt/reconciliation rows are not rewritten. Report insertion requires a matching accepted/unknown attempt. Request parameters/capability observations are separate immutable events; the application additionally binds ENVID and recipient and verifies digests. Migration commit failure rolls back to the original schema.

## Agent Mailer 0.3: schema 5

Recognized exact layouts 0–4 migrate to schema 5 by adding prepare_operations plus immutable update/delete triggers. The table binds key SHA-256 to fingerprint schema-1 input SHA-256, package ID and preparation timestamp; no existing package, attempt or evidence bytes are rewritten. Migration recognition/commit failure and schema-4 history preservation have regression tests. Old readers reject schema 5, and no downgrade is supported. Back up privately before upgrading; never treat a restored independent copy as permission to resend. New preparation and operation publication share one IMMEDIATE transaction. See [operation contract](agent-workflow.md).

## Schema 6: header-correlated reports

Adds only dsn_header_reports with immutable raw report bytes, SHA-256, size, package/attempt/report IDs and recording timestamp. Foreign-key/insert binding requires the selected accepted/unknown attempt in the same package; update/delete are blocked. Exact layouts 0–5 are recognized and migrated under the existing IMMEDIATE transaction, quick/FK checks and atomic identity commit. Schema-5 migration rollback/preservation is tested explicitly. Existing package, attempt, strict DSN, reconciliation and operation rows remain unchanged. Old binaries fail closed on schema 6; no downgrade is supported. Stop using old binaries and take a private consistent backup before the first new invocation. Never submit from a stale backup or use restore as a retry bypass.
