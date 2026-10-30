# Command reference

All commands below are from the native `agent-mailer` binary. Use `describe --json` for the installed parser contract, `COMMAND --help --json` for its options, and the [JSON reference](json-contract.md) for response fields/errors. [Configuration](configuration.md) defines global --home/--config resolution. Commands do not prompt.

## Command map

| Command | Network | Purpose |
|---|---|---|
| `describe --json` | None | Discover versions, arguments, limits, authorization classifications and exit codes without state/config access |
| `preflight --json` | None | Diagnose configuration/environment availability without opening state |
| `doctor --json` | None | Open/validate local storage and inspect SMTP readiness |
| `prepare … --json` | None | Save immutable sender/recipient, final letter/PDF/MIME and optional metadata/operation identity |
| `inspect ID --json` | None | Verify the package and return full private review contents/hashes |
| `send ID --dry-run --json` | None | Verify integrity, configuration and resend policy; create no attempt |
| `send ID --json` | SMTP | Submit once on an explicit owner instruction for this reviewed ID |
| `status ID --json` | None | Verify saved materials and return separate attempt/evidence records |
| `list --json` | None | Verify/return the complete archive; use bounded options below for growing stores |
| `export-eml ID --output NEW_FILE --json` | None | Export exact verified MIME to a new private review file |
| `import PATH --to ADDRESS --json` | None | Read one selected prepared legacy archive and create a new package |
| `reconcile ID --evidence-file PATH --json` | None | Append an external observation for an unknown attempt; keep retry block |
| `record-receipt ID --evidence-file PATH --json` | None | Record recipient feedback for an accepted/unknown attempt |
| `inspect-reconciliation RECORD_ID --json` | None | Verify and return private evidence |
| `import-dsn ID --attempt-id ATTEMPT --file report.eml --json` | None | Correlate/store one selected notification; do not alter SMTP history |
| `inspect-dsn REPORT_ID --json` | None | Verify and inspect saved notification metadata |
| `backup --output NEW_DIRECTORY --json` | None | Read existing current-schema core/cache and create a private snapshot |
| `verify-backup DIRECTORY --json` | None | Verify only the selected snapshot; do not open operational home |

Network:none does not mean no local writes. Ordinary stateful commands can initialize/migrate a store; [state-opening effects](json-contract.md#state-opening-effects) list exceptions. Preparation, dry-run, configuration and incoming reports never authorize real submission.

## Prepare final inputs

Provide a single recipient, subject/body files and one to five PDF attachments. The signature belongs in the final body. Optional `--html-file` adds a separately finalized HTML alternative; review both plain and HTML. Optional company/vacancy references are opaque metadata. [Input limits](json-contract.md#preparation-and-inspection) and [operation keys](agent-workflow.md#recover-the-same-preparation) apply before a package is reused.

No later signature/template expansion occurs. `export-eml` is useful for private mail-client review; submitting that file through another client bypasses this journal and needs separate authorization. [EML export contract](json-contract.md#mime-export).

## Archive pages

`list --limit 25 --json` returns at most 25 verified status entries with pagination. Continue using `list --limit 25 --after PACKAGE_ID --json`. Limit is 1–100; after requires limit and an existing ID. Order is preparation insertion order. An unknown continuation returns not_found/exit 3. End-of-archive has next_after:null; continuing after the last ID returns an empty page.

`list --limit 25 --summary --json` returns only package_id/integrity:not_checked. It avoids letter/PDF/MIME/evidence reads and cannot establish state, integrity or retry permission; inspect/status the selected ID. Summary also requires limit.

Each invocation is one SQLite read snapshot, not a persistent multi-page snapshot. Later pages can include appended packages or changed states. Full pages bound package count, not each package's history/byte size. Legacy list without options retains its full shape/verification. See [pagination fields](json-contract.md#archive-pagination-and-snapshots) and [measurements](benchmarks.md).

## Optional adapters

The [Keyring launcher](../integrations/gnome-keyring/README.md) and [selected read-only IMAP reader](../integrations/imap-dsn/README.md) are separate executables with separate prerequisites and contracts. Core installation never enables inbox access. [Preflight](preflight.md) distinguishes local setup from actual authentication; [DSN](dsn.md) distinguishes correlation from delivery truth.
