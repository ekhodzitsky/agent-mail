# Agent Mail

Rust email CLI for AI agents. The executable is `agent-mailer`; existing runtime names and storage formats remain compatible. Freeze a letter and attachments into a reviewable package, then submit that exact package on the owner's instruction. Durable attempt history blocks ordinary resends after SMTP acceptance or an unknown outcome. SMTP acceptance does **not** confirm recipient delivery.

[![Release](docs/badges/release.svg)](docs/releases.md)
[![MSRV: Rust 1.88](docs/badges/msrv.svg)](Cargo.toml)
[![License: MIT](docs/badges/license.svg)](LICENSE)

[Documentation](docs/README.md) · [Agent workflow](docs/agent-workflow.md) · [Supported scope](docs/production-readiness.md)

```text
prepare → inspect → send --dry-run → owner instruction → send → status
                                                            accepted ≠ delivered
                                                            unknown → retry blocked
```

## Try without sending

Use Linux x86_64/glibc and trusted local storage. Source installation needs Rust 1.88+, a C compiler and access to Cargo dependencies. The installed core needs no Rust, Node.js, Bun or Python runtime. This example uses `jq` only to extract the package ID; it requires no mail account, credential or server.

```sh
export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
cargo install --path . --locked
export AGENT_MAILER_HOME="$(mktemp -d)"
printf 'sender = "sender@example.org"\n' > "$AGENT_MAILER_HOME/config.toml"
printf 'Synthetic application' > "$AGENT_MAILER_HOME/subject.txt"
printf 'Synthetic letter.\nRegards, Test\n' > "$AGENT_MAILER_HOME/letter.txt"
printf '%%PDF-1.4\nSynthetic fixture only\n%%%%EOF\n' > "$AGENT_MAILER_HOME/resume.pdf"
agent-mailer prepare --to recipient@example.org \
  --subject-file "$AGENT_MAILER_HOME/subject.txt" \
  --body-file "$AGENT_MAILER_HOME/letter.txt" \
  --attach "$AGENT_MAILER_HOME/resume.pdf" --company-ref demo --json \
  > "$AGENT_MAILER_HOME/prepared.json"
package_id="$(jq -er '.result.package_id' "$AGENT_MAILER_HOME/prepared.json")"
agent-mailer inspect "$package_id" --json
agent-mailer send "$package_id" --dry-run --json
```

Inspect returns the frozen manifest, complete letter and hashes. Dry-run returns `configuration` (exit 3) because SMTP is deliberately unset; it opens no connection and records no attempt. The PDF is a byte-preservation fixture, not a rendering example. Keep the temporary directory private. For a successful offline dry-run and local submission, use the [capture-only SMTP walkthrough](docs/local-testing.md).

## When to use it

Compare CLI workflows for preparing and submitting email, not service throughput:

| Tool | Primary workflow | Adoption consideration |
|---|---|---|
| Agent Mailer | Immutable package, review, journaled SMTP attempt | Standalone core; optional Keyring/selected-DSN adapters |
| [msmtp](https://marlam.de/msmtp/) | Sendmail-compatible SMTP submission | Fits existing mail composers and Unix pipelines |
| [Himalaya](https://github.com/pimalaya/himalaya) | CLI mailbox and message management | Broader mailbox backends and JSON interface |

These tools are not drop-in replacements for the package journal. Alternative retry and audit guarantees were not tested. See [comparison sources and scope](docs/comparison.md).

## Evidence and limits

Local checks exercise corruption, parallel sends, refusal and unknown outcomes with synthetic servers. A preparation baseline (20 packages × 3 runs, 4.3 KB PDF, tmpfs) measured:

| Production binary | Prepare | Inspect | Dry-run |
|---|---:|---:|---:|
| 0.5.2, median run mean, ms ↓ | 3.18 | 2.91 | 3.51 |

[Method and raw samples](docs/benchmarks.md). Process startup is included; persistent-disk latency, large archives and competitors are not measured. No cross-tool advantage is claimed. The same guide records 0.6.0 full/page/ID listing latency and RSS, distinguishing reduced work from full verification.

Use [offline preflight](docs/preflight.md) to diagnose setup without loading a Keyring secret or creating a database. Use the latest [release](docs/releases.md) with matching optional adapters. JSON stdout contains one versioned result; commands do not prompt in agent mode. [Discovery and commands](docs/cli.md) describe HTML/plain alternatives, operation keys and private EML export. Inspect output contains private content.

Credentials stay outside packages and Git. Verified TLS is mandatory. A real send requires a separate owner instruction; the CLI cannot verify consent. DSNs are unverified assertions, never retry permission. SMTP may omit a Sent-folder copy. Exactly-once delivery through arbitrary failures cannot be guaranteed.

State is unencrypted and assumes trusted local users, directories and backups. Follow [upgrade/recovery](docs/upgrade-and-recovery.md); never activate a stale copy to evade send history. Use [bounded list pages](docs/cli.md#archive-pages-060) and [private snapshots](docs/backup.md) for archive work. Sent synchronization and OAuth refresh remain future work.

Development and release checks run locally; hosted CI is disabled. Coverage and public download measurements are not available. Cargo registry publication is not enabled. See [maintenance](docs/maintenance.md) and [changelog](CHANGELOG.md).

[Help](SUPPORT.md) · [Contributing](CONTRIBUTING.md) · [Security](SECURITY.md) · [MIT license](LICENSE)
