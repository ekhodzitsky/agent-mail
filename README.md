# Agent Mail

Agent Mail (`agent-mailer`) is a Rust CLI for AI agents that prepare and send email through an existing SMTP account. It saves the final letter and PDFs as a reviewable package, records each submission attempt, and returns JSON that distinguishes acceptance, refusal and uncertainty.

[![Version: 0.7.1](docs/badges/release.svg)](docs/releases.md)
[![Native binary downloads for v0.7.1](https://img.shields.io/github/downloads/ekhodzitsky/agent-mail/v0.7.1/agent-mailer-0.7.1-x86_64-unknown-linux-gnu?label=binary%20downloads)](https://github.com/ekhodzitsky/agent-mail/releases/tag/v0.7.1)
[![Source MSRV: Rust 1.88](docs/badges/msrv.svg)](Cargo.toml)
[![License: MIT](docs/badges/license.svg)](LICENSE)

[Start here](docs/getting-started.md) · [Agent workflow](docs/agent-workflow.md) · [Documentation](docs/README.md)

A package freezes the sender, recipient, subject, final plain/HTML letter, attachment bytes and MIME before any submission. The attempt journal records `accepted`, `rejected` or `unknown`. Accepted/unknown packages cannot be resent by an ordinary command; SMTP acceptance still does **not** confirm recipient delivery.

## Quick start

From a new working directory, use Git, Rust 1.88+, a C compiler, a shell and jq. Installation downloads GitHub/Cargo sources. The example then runs entirely locally: no mail account, credential or SMTP server is needed. A prebuilt binary is available through [native installation](docs/releases.md#install-the-native-linux-asset).

```sh
git clone https://github.com/ekhodzitsky/agent-mail.git
cd agent-mail
cargo install --path . --locked --root "$HOME/.local"
export PATH="$HOME/.local/bin:$PATH"
umask 077
demo_home="$(mktemp -d)"
cp examples/demo/config.toml "$demo_home/config.toml"
cp examples/demo/resume.fixture "$demo_home/resume.pdf"
package_id="$(agent-mailer --home "$demo_home" prepare \
  --to recipient@example.org --subject-file examples/demo/subject.txt \
  --body-file examples/demo/letter.txt --attach "$demo_home/resume.pdf" \
  --operation-key demo/application --json | jq -er '.result.package_id')"
agent-mailer --home "$demo_home" inspect "$package_id" --json
agent-mailer --home "$demo_home" send "$package_id" --dry-run --json \
  | jq '{ok, state: .result.state, dry_run: .result.dry_run, network_used: .result.network_used}'
```

Inspect returns the frozen contents and hashes. The final command projects the JSON result:

```json
{"ok":true,"state":"prepared","dry_run":true,"network_used":false}
```

No submission attempt is recorded. The example PDF is only a byte-preservation fixture. Keep the generated directory private; explicit --home leaves ordinary history selection unchanged. [The full walkthrough](docs/getting-started.md) explains each step. The [local SMTP test](docs/local-testing.md) covers actual capture-only submission.

## Working with an agent

Use `describe --json` for installed command/argument discovery. A stable prepare operation key recovers the same package after lost output and refuses changed inputs. Review the saved package, run dry-run, and obtain a separate owner instruction before real send. No interactive questions appear in agent mode.

After an unknown outcome, inspect history and investigate; do not retry or recreate the letter automatically. Message-ID helps correlation but does not guarantee recipient deduplication. [Submission and result handling](docs/agent-workflow.md) keeps consent, SMTP state and delivery observations separate.

## Comparison

For existing mail accounts, these tools serve different workflows:

| Tool | Primary task | Choose when |
|---|---|---|
| Agent Mail | Reviewed packages and journaled SMTP attempts | Agents need saved inputs and conservative recovery |
| [msmtp](https://marlam.de/msmtp/) | Sendmail-compatible SMTP submission | A composer or Unix pipeline manages the message lifecycle |
| [Himalaya](https://github.com/pimalaya/himalaya) | CLI mailbox/message management with JSON | Broader inbox operations/backends are needed |

They are not drop-in journal replacements; alternative retry guarantees are **not verified**. [Versions, primary sources and comparison limits](docs/comparison.md).

## Benchmarks

On Linux x86_64/Ryzen AI 9 HX 370, 0.7.1 prepared/reviewed/validated 20 synthetic packages per run, three times, with one 4.3 KB PDF each. Median run means, including process startup; lower is smaller:

| Prepare, ms | Inspect, ms | Dry-run, ms |
|---:|---:|---:|
| 3.43 | 3.22 | 3.30 |

State used tmpfs and warm/uncontrolled caches. These numbers establish no competitor advantage or persistent-disk preparation guarantee. [Raw samples and reproduction](docs/benchmarks.md) also separate ext4 backup and bounded-list work. [Correctness checks](docs/validation.md) exercise corruption, concurrency, refusal, TLS and crashes independently of timings.

## Limitations

The native core supports Linux x86_64/glibc without Node.js, Bun or Python. Rust/MSRV concerns source builds. Optional Keyring/IMAP adapters have separate runtimes. Credentials stay outside Git/packages; TLS verification is mandatory. State is unencrypted and assumes trusted local users/storage. [Configuration](docs/configuration.md), [readiness checks](docs/preflight.md) and [recovery](docs/upgrade-and-recovery.md) cover deployment.

Exactly-once SMTP delivery is not guaranteed. DSNs are unverified assertions; snapshot integrity cannot prove freshness or authorize retries. Sent synchronization and OAuth refresh are not implemented.

Version/download badges describe release 0.7.1; downloads count that binary asset, not users. MSRV is verified locally. Hosted CI is disabled by project policy and coverage is unmeasured; no fabricated indicators are shown. See [maintenance](docs/maintenance.md) and [changelog](CHANGELOG.md).

[Help](SUPPORT.md) · [Contributing](CONTRIBUTING.md) · [Security](SECURITY.md) · [MIT license](LICENSE)
