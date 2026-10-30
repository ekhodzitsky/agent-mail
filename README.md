# agent-mail

**A CLI for AI agents to send email from your own mailbox.**

[![Version: 0.8.0](docs/badges/release.svg)](docs/releases.md)
[![Source MSRV: Rust 1.88](docs/badges/msrv.svg)](Cargo.toml)
[![License: MIT](docs/badges/license.svg)](LICENSE)

Your agent prepares a letter and PDFs. Review the saved package, then send
those exact materials through your SMTP account. Each attempt has a local
record and a JSON result.

If the agent stops mid-send or loses the response, it can check the recorded outcome
before acting again. Accepted and unknown attempts block ordinary resends.

## Prepare, review, send

With a [configured sender](docs/configuration.md) and your final files:

```sh
agent-mail prepare --to recipient@example.org \
  --subject-file subject.txt --body-file letter.txt --attach resume.pdf --json
```

Use the returned `result.package_id` as `PACKAGE_ID` below. Inspect shows
the saved contents; dry-run validates them without connecting:

```sh
agent-mail inspect PACKAGE_ID --json
agent-mail send PACKAGE_ID --dry-run --json
```

Selected dry-run response fields (additional fields omitted):

```json
{
  "schema_version": 1,
  "ok": true,
  "result": {"state": "prepared", "dry_run": true, "network_used": false},
  "error": null
}
```

When the owner explicitly instructs the agent to send this reviewed package:

```sh
agent-mail send PACKAGE_ID --json
agent-mail status PACKAGE_ID --json
```

Results distinguish `accepted`, `rejected` and `unknown`. SMTP acceptance
does not prove recipient delivery. Investigate unknown outcomes before any retry.
**The calling agent must obtain permission; the CLI does not verify consent.**

[Try the complete offline example](docs/getting-started.md) with synthetic
files and no email account. [Submission and recovery details](docs/agent-workflow.md).

## Built for agent workflows

| Need | What agent-mail does |
|---|---|
| Review the actual outgoing materials | Saves the final letter and PDFs; changing them requires a new package |
| Recover preparation after lost output | An optional stable operation key returns the same package and refuses changed inputs |
| Keep submission attempts accounted for | Records an attempt before network access, locks the package and blocks accepted/unknown resends |
| Read results programmatically | Returns versioned JSON with documented errors, exit codes and command discovery |

## Install

**Linux x86_64/glibc:** [download the standalone binary](https://github.com/ekhodzitsky/agent-mail/releases/tag/v0.8.0)
and follow the [checksum and installation steps](docs/releases.md#install-the-native-linux-asset).
The native core needs no Rust, Node.js or Python runtime.
Existing users: [upgrade without losing history](docs/upgrade-and-recovery.md).

<details>
<summary>Build from source — Rust 1.88+, Git and a C compiler</summary>

```sh
git clone --branch v0.8.0 --depth 1 https://github.com/ekhodzitsky/agent-mail.git
cd agent-mail
cargo install --path . --locked --root "$HOME/.local"
export PATH="$HOME/.local/bin:$PATH"
agent-mail --version
```

</details>

## Connect your agent

Give your agent the [portable skill](skills/agent-mail/SKILL.md) for preparation,
authorization and recovery rules. Discover installed commands with:

```sh
agent-mail describe --json
```

Keep the same state/configuration throughout a workflow. Reuse a stable
`prepare --operation-key` after lost output; it never authorizes sending.
[Agent integration guide](docs/agent-workflow.md).

## When to use it

Use it for job applications, PDF handoffs and other individually reviewed
agent correspondence. Optional [desktop credentials](integrations/gnome-keyring/README.md)
and [read-only delivery-notification retrieval](integrations/imap-dsn/README.md)
are separate adapters with their own dependencies.

For alternatives focused on submission pipelines or mailbox management,
see the [comparison](docs/comparison.md).

## Limits

- SMTP cannot guarantee exactly-once delivery through every network failure.
- Local correspondence is unencrypted; protect its storage and backups.
- TLS certificate verification is required. Credentials stay outside message packages.
- Delivery notifications are external assertions, not authenticated proof.
- General inbox management, OAuth refresh and Sent-folder synchronization are outside the core.

[Local validation](docs/validation.md) covers corruption, concurrent sends,
refusals, TLS and process crashes. [Performance measurements](docs/benchmarks.md)
include their test conditions and limits.

[Documentation](docs/README.md) · [Changelog](CHANGELOG.md) ·
[Contributing](CONTRIBUTING.md) · [Help](SUPPORT.md) ·
[Security](SECURITY.md) · [MIT](LICENSE)
