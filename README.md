# agent-mail

**Email for AI agents. Prepare. Review. Send.**

[![Version: 0.8.0](docs/badges/release.svg)](docs/releases.md)
[![Native binary downloads for v0.8.0](https://img.shields.io/github/downloads/ekhodzitsky/agent-mail/v0.8.0/agent-mail-0.8.0-x86_64-unknown-linux-gnu?label=binary%20downloads)](https://github.com/ekhodzitsky/agent-mail/releases/tag/v0.8.0)
[![Source MSRV: Rust 1.88](docs/badges/msrv.svg)](Cargo.toml)
[![License: MIT](docs/badges/license.svg)](LICENSE)

Let your agent prepare an application, attach a PDF, and send it through
**your existing email account** after your approval. You review the exact
letter and attachments it will send. The agent gets JSON and a durable
attempt history, so a lost response does not become a blind retry.

One Rust binary. No Node.js, Python, hosted account or agent framework
required by the core. Works with any agent that can run a shell command.

## Example

With a [configured sender](docs/configuration.md), your final files and jq:

```sh
package_id="$(agent-mail prepare --to recipient@example.org \
  --subject-file subject.txt --body-file letter.txt --attach resume.pdf \
  --operation-key application/example --json | jq -er '.result.package_id')"
agent-mail inspect "$package_id" --json
agent-mail send "$package_id" --dry-run --json
```

Prepare freezes the final plain/HTML letter, sender, recipient and PDF bytes.
Inspect shows that saved package. Dry-run checks it **without connecting
to a mail server**. After the owner explicitly authorizes this package:

```sh
agent-mail send "$package_id" --json
agent-mail status "$package_id" --json
```

The result distinguishes `accepted`, `rejected` and `unknown`.
Accepted and unknown packages block ordinary resends. SMTP acceptance is
not proof of delivery; an uncertain result needs investigation, not an
automatic retry. [Agent workflow and result handling](docs/agent-workflow.md).

Want to try it without an email account? The [five-minute local walkthrough](docs/getting-started.md)
uses synthetic files and never sends mail.

## Built for agent workflows

| Need | agent-mail provides |
|---|---|
| Review what will actually leave your account | Immutable packages, hashes and saved MIME; edits require a new package |
| Recover when an agent loses command output | Stable preparation keys return the original package; changed inputs conflict |
| Avoid blind duplicate submissions | Exclusive package locks, durable pre-network attempts and resend blocks |
| Parse outcomes without scraping terminal text | Versioned JSON, documented errors, exit codes and `describe --json` discovery |
| Investigate a delivery problem | Attempt history, Message-ID, optional DSN import and a separate read-only IMAP adapter |
| Keep control of your mail | Local state, your SMTP provider, credentials outside message packages |

Use it for job applications, PDF handoffs and other owner-approved agent
correspondence. It is a focused sending tool; mailbox management, provider
OAuth refresh and Sent-folder synchronization are outside the core.

## Install

Build with Rust **1.88+**, Git and a C compiler:

```sh
git clone https://github.com/ekhodzitsky/agent-mail.git
cd agent-mail
cargo install --path . --locked
agent-mail --version
```

Or install the [standalone Linux x86_64 binary](docs/releases.md).
[Upgrading an existing installation](docs/upgrade-and-recovery.md) preserves
its attempt history and credential binding. Optional desktop/IMAP adapters
have separate dependencies.

## Fit and limits

Choose agent-mail when an agent needs reviewable messages and conservative
submission recovery. For a sendmail-style pipeline, consider
[msmtp](https://marlam.de/msmtp/); for broader mailbox operations,
[Himalaya](https://github.com/pimalaya/himalaya).
[Detailed comparison and scope](docs/comparison.md).

TLS certificate verification is mandatory. Local state is unencrypted;
protect it as private correspondence. No SMTP tool can guarantee exactly-once
delivery through every network failure. DSNs are external assertions, not
authenticated proof. Authorization remains the owner's decision.

[Local tests](docs/validation.md) exercise corruption, concurrent sends,
refusals, TLS and process crashes. [Measured performance](docs/benchmarks.md)
includes its hardware and storage limits. Hosted CI is disabled; coverage
is unmeasured. Version/download badges refer to release artifacts, not users.

[Documentation](docs/README.md) · [Agent skill](skills/agent-mail/SKILL.md) ·
[Changelog](CHANGELOG.md) · [Contributing](CONTRIBUTING.md) ·
[Help](SUPPORT.md) · [Security](SECURITY.md) · [MIT](LICENSE)
