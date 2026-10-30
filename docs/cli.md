# Commands and agent contract

| Command | Result |
|---|---|
| `describe --json` | Parser-derived commands, arguments, safety rules and exit codes; no storage/config access |
| `record-receipt ID --evidence-file PATH --json` | Record recipient feedback without changing SMTP state |
| `export-eml ID --output PATH --json` | Export verified frozen MIME to a new private file; no sending |
| `import-dsn ID --attempt-id ATTEMPT --file report.eml --json` | Import one selected DSN with exact binding |
| `inspect-dsn REPORT_ID --json` | Verify and inspect report metadata |
| `preflight --json` | Local configuration/credential-environment readiness; no storage/network |
| `doctor --json` | Local storage/config checks; no network |
| `prepare … --json` | Frozen package and reviewable contents |
| `inspect ID --json` | Verified manifest, complete letter and hashes |
| `send ID --dry-run --json` | Integrity, resend policy and configuration checks; no connection |
| `send ID --json` | One explicitly authorized submission attempt |
| `status ID --json`, `list --json` | State and separate attempt records; list supports opt-in bounded pages |
| `backup --output NEW_DIRECTORY --json` | Private core/cache snapshot without configuration or credentials |
| `verify-backup DIRECTORY --json` | Verify selected snapshot; do not open operational state |
| `reconcile ID --evidence-file PATH --json` | Append an observation; unknown remains blocked |
| `inspect-reconciliation RECORD_ID --json` | Verify and view private evidence |
| `import PATH --to ADDRESS --json` | Read-only import of one prepared legacy archive |


Use `describe --json` for parser-derived discovery. JSON stdout contains exactly one versioned result; diagnostics use stderr and never expose raw server prose or letter bodies. Commands never prompt in agent mode. Global `--home`/`--config` select local TOML configuration/state; company/vacancy references are optional opaque metadata. See [JSON contracts](json-contract.md) and [agent workflow](agent-workflow.md).

A stable `prepare --operation-key` recovers the same exact package after lost output and refuses changed input. Structured `next.argv` guidance never grants send approval. An optional [portable agent skill](../skills/agent-mailer/SKILL.md) is included; installing the core does not alter global agent configuration.

For labeled clickable links, provide final HTML with `prepare --html-file letter.html` plus a plain-text `--body-file` fallback. Both are frozen and reviewed. Use export-eml for private mail-client review; external submission bypasses the CLI's attempt journal and needs its own owner authorization.


For configuration, adapters and recovery, start with the [documentation index](README.md).

## Archive pages (0.6.0)

`list --limit 25 --json` returns at most 25 verified status entries, plus pagination. Continue with `list --limit 25 --after PACKAGE_ID --json`. `--limit` accepts 1–100; `--after` requires an existing ID and `--limit`. Ordering is preparation insertion order. Each invocation is one SQLite read snapshot; pages are not a persistent global snapshot, so later additions can appear and statuses can change. Unknown continuation IDs return not_found/exit 3. End-of-archive returns an empty page; next_after is null.

`list --limit 25 --summary --json` returns only package_id and integrity:not_checked, avoiding material/evidence reads. It cannot establish state, integrity or retry permission; call inspect/status for the selected ID. `--summary` also requires `--limit`. Without parameters, list preserves the original full packages shape and full verification. Full status pages bound package count, not the size/history of each package. See [contracts](json-contract.md), [measurements](benchmarks.md) and [snapshots](backup.md).

Offline setup checks: [preflight](preflight.md). Core preflight and the optional Keyring metadata preflight distinguish local setup failures without passwords or state creation; readiness is not authentication or consent.
