# Documentation

Agent Mailer is a local Rust CLI. Start with a synthetic package; configure a real account only for a separately authorized deployment. Instructions that inspect private letters or import notifications require private output handling.

## Use the tool

- [Offline setup diagnostics](preflight.md).
- [First package without sending](../README.md#quick-start) and [installation/releases](releases.md).
- [Commands](cli.md), [agent workflow and operation keys](agent-workflow.md), [JSON fields and exit codes](json-contract.md).
- [Synthetic local SMTP](local-testing.md); [provider scope](mailbox-target.md) and [Yandex setup](yandex-setup.md).
- Optional [GNOME Keyring launcher](../integrations/gnome-keyring/README.md), [selected read-only IMAP DSN reader](../integrations/imap-dsn/README.md) and [portable agent skill](../skills/agent-mailer/SKILL.md).
- [Delivery reports](dsn.md), [unknown-outcome reconciliation](reconciliation.md) and [one-package legacy import](import.md).

## Operate safely

- [Supported production scope](production-readiness.md), [snapshot creation/verification](backup.md), [upgrade and paired-state recovery](upgrade-and-recovery.md), [storage migrations](storage-migrations.md).
- [Support](../SUPPORT.md), [security reporting](../SECURITY.md), [publication checklist](public-opening-checklist.md).

## Evaluate and develop

- [Alternatives and source verification](comparison.md), [offline measurements](benchmarks.md), [local validation and its limits](validation.md).
- [Architecture](architecture.md), [maintained audit/backlog](maintenance.md).
- [Contribution workflow](../CONTRIBUTING.md), [agent instructions](../AGENTS.md), [managed knowledge-base references](knowledge-base.md).
- [Versioning](../VERSIONING.md) and [changelog](../CHANGELOG.md). Product, JSON, package, storage and cache versions are independent.

## Documentation ownership

The root README explains the task, first result, alternatives and limits. This index routes to guides; `cli.md` inventories commands and `json-contract.md` owns serialized fields/errors. `production-readiness.md` owns support boundaries, `validation.md` owns check coverage, and `maintenance.md` owns current findings/tasks. Release assets carry revision-specific verification and hashes; development documentation must not imply unreleased fixes are already installed.

Update the canonical page first, then its summaries and links. Preserve useful migration and failure guidance. Run `python3 -B scripts/check-docs.py --binary /path/to/agent-mailer` for local links/headings and discovery parity. External sources need dated review; this checker does not verify their contents.
