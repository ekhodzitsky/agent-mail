# Documentation

Agent Mail is the project; `agent-mailer` is the native command and Cargo package. Runtime identifiers remain compatible so a repository rename cannot separate a sender from its history.

## Start and configure

| Task | Guide |
|---|---|
| Prepare/review a first synthetic package without sending | [Getting started](getting-started.md) |
| Install the native binary or build from source | [Installation and release assets](releases.md) |
| Select state, sender, SMTP, credential injection and timeouts | [Configuration](configuration.md) |
| Diagnose local setup without opening state/loading a Keyring secret | [Preflight](preflight.md) |
| Configure a provider-specific example | [Yandex](yandex-setup.md), [provider scope](mailbox-target.md) |

## Submit and interpret

- [Agent workflow](agent-workflow.md): discovery → prepare → review → dry-run → explicit instruction → send → status.
- [Commands](cli.md): command inventory, local effects, options and bounded pages.
- [JSON contract](json-contract.md): envelopes, fields, errors, schemas and guidance.
- [Unknown outcome](reconciliation.md) and [delivery reports](dsn.md): investigation/observations, never automatic retry.
- [Legacy import](import.md): read one explicitly selected prepared archive.

## Operate and extend

- [Snapshots](backup.md), [upgrade/recovery](upgrade-and-recovery.md) and [storage migrations](storage-migrations.md).
- Optional [Keyring launcher](../integrations/gnome-keyring/README.md), [selected read-only IMAP reader](../integrations/imap-dsn/README.md) and [portable agent skill](../skills/agent-mailer/SKILL.md).
- [Supported scope](production-readiness.md), [support](../SUPPORT.md) and [security reporting](../SECURITY.md).

## Evaluate and develop

- [Comparison](comparison.md), [measurements](benchmarks.md), [local correctness checks](validation.md), [capture-only SMTP walkthrough](local-testing.md).
- [Architecture](architecture.md), [maintained findings/tasks](maintenance.md), [publication checklist](public-opening-checklist.md).
- [Contributing](../CONTRIBUTING.md), [agent instructions](../AGENTS.md), [managed development references](knowledge-base.md), [versioning](../VERSIONING.md), [changelog](../CHANGELOG.md).

## Documentation ownership

README explains the task/benefit, first useful result, alternatives, evidence and limits. Getting started owns the complete synthetic recipe; releases owns installation/artifacts; configuration owns TOML/path selection. Agent workflow owns authorization and outcome handling, cli owns command inventory, json-contract owns serialized response/error fields. Feature guides own operational details; schema history belongs in storage-migrations rather than introductory recipes.

Update the canonical page first, then summaries/links. Runtime consent/SMTP ambiguity/privacy warnings belong beside affected operations, without copying the entire security policy onto every page. Keep released assets distinct from Unreleased source documentation. Run `python3 -B scripts/check-docs.py --binary /path/to/agent-mailer`, verify changed examples on synthetic data and review rendered Markdown. External sources need dated primary-source review; the local checker cannot certify every sentence or website.
