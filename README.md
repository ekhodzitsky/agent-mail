# Agent Mail

Agent Mail is a Rust email CLI for AI agents. It freezes a reviewable letter and PDF attachments before submission, records attempts durably, and blocks ordinary resends after SMTP acceptance or an unknown outcome.

[![Version: 0.7.1](docs/badges/release.svg)](docs/releases.md)
[![Native binary downloads for v0.7.1](https://img.shields.io/github/downloads/ekhodzitsky/agent-mail/v0.7.1/agent-mailer-0.7.1-x86_64-unknown-linux-gnu?label=binary%20downloads)](https://github.com/ekhodzitsky/agent-mail/releases/tag/v0.7.1)
[![Source MSRV: Rust 1.88](docs/badges/msrv.svg)](Cargo.toml)
[![License: MIT](docs/badges/license.svg)](LICENSE)

[Documentation](docs/README.md) · [Agent workflow](docs/agent-workflow.md) · [Supported scope](docs/production-readiness.md)

```text
prepare → inspect → send --dry-run → owner instruction → send → status
                                                            accepted ≠ delivered
                                                            unknown → retry blocked
```

SMTP acceptance does **not** confirm delivery. The CLI cannot verify human consent; a real send requires the owner's separate instruction for the reviewed package.

## Quick start

Use Linux x86_64/glibc and trusted local storage. The native core needs no Rust, Node.js, Bun or Python runtime. This example needs curl, GNU coreutils, a shell and jq; downloads contact GitHub, while package preparation/review/dry-run use no mail network, credentials or account.

```sh
umask 077
download_dir="$(mktemp -d)"
release_url="https://github.com/ekhodzitsky/agent-mail/releases/download/v0.7.1"
curl -fLsS "$release_url/agent-mailer-0.7.1-x86_64-unknown-linux-gnu" \
  -o "$download_dir/agent-mailer-0.7.1-x86_64-unknown-linux-gnu"
curl -fLsS "$release_url/SHA256SUMS" -o "$download_dir/SHA256SUMS"
(cd "$download_dir" && sha256sum --ignore-missing --check SHA256SUMS)
install -Dm755 "$download_dir/agent-mailer-0.7.1-x86_64-unknown-linux-gnu" \
  "$HOME/.local/bin/agent-mailer"
export PATH="$HOME/.local/bin:$PATH"
demo_home="$(mktemp -d)"
printf 'sender="sender@example.org"\n[smtp]\nhost="127.0.0.1"\nport=465\nsecurity="tls"\n' \
  > "$demo_home/config.toml"
printf 'Synthetic application' > "$demo_home/subject.txt"
printf 'Synthetic letter.\nRegards, Test\n' > "$demo_home/letter.txt"
printf '%%PDF-1.4\nSynthetic fixture only\n%%%%EOF\n' > "$demo_home/resume.pdf"
package_id="$(agent-mailer --home "$demo_home" prepare --to recipient@example.org \
  --subject-file "$demo_home/subject.txt" \
  --body-file "$demo_home/letter.txt" \
  --attach "$demo_home/resume.pdf" --json | jq -er '.result.package_id')"
agent-mailer --home "$demo_home" inspect "$package_id" --json
agent-mailer --home "$demo_home" send "$package_id" --dry-run --json
```

Inspect shows frozen contents and hashes. The last JSON must have `ok:true`, `result.dry_run:true`, `result.network_used:false` and state `prepared`; no attempt is recorded. No local server needs to be running. The PDF is a byte-preservation fixture, not a rendering example. Keep generated state private. Checksums detect corruption, not signed provenance.

The executable/package remains `agent-mailer` for compatibility. [Source installation](docs/releases.md) requires Rust 1.88+ and a C compiler; MSRV describes source builds and is verified locally. [Commands and configuration](docs/cli.md) cover real setup and optional adapters. [Local SMTP tests](docs/local-testing.md) exercise actual submission without external recipients.

## Comparison

For agents using an existing mail account:

| Tool | Workflow | Choose when |
|---|---|---|
| Agent Mail | Frozen package, review, conservative SMTP journal | Persisting preparation and unknown outcomes matters |
| [msmtp](https://marlam.de/msmtp/) | Sendmail-compatible SMTP submission | An existing composer/pipeline already manages messages |
| [Himalaya](https://github.com/pimalaya/himalaya) | CLI mailbox/message management with JSON | Broader mailbox backends and inbox operations are needed |

These tools are not drop-in journal replacements. Alternative retry guarantees are **not verified**. [Versions, primary sources and comparison limits](docs/comparison.md).

## Benchmarks

Local freeze/review/offline-validation workload: 20 synthetic packages per run, one 4.3 KB PDF each, three runs. Median run means; lower latency is smaller:

| Native version | Prepare, ms | Inspect, ms | Dry-run, ms |
|---|---:|---:|---:|
| 0.7.1 | 3.43 | 3.22 | 3.30 |

Linux x86_64, Ryzen AI 9 HX 370, tmpfs, warm caches; process startup included. No competitor speedup, persistent-disk preparation or large-archive guarantee is claimed. [Raw samples, reproduction and separate ext4 backup/listing results](docs/benchmarks.md). Failure/concurrency/TLS correctness is exercised separately by [local validation](docs/validation.md).

## Limitations

Credentials stay outside packages/Git; TLS verification is mandatory. JSON results are versioned and agent commands do not prompt. Review output contains private content. State is unencrypted; trust local users/directories and follow [recovery](docs/upgrade-and-recovery.md). Snapshot integrity cannot prove freshness or authorize retries.

Exactly-once SMTP delivery cannot be guaranteed. DSNs are unverified assertions. Sent-folder synchronization and OAuth refresh are not implemented. Optional Keyring/IMAP adapters have separate Python/desktop requirements. [Preflight](docs/preflight.md) checks local readiness, not authentication.

Badges describe release 0.7.1; downloads count that native asset's cumulative GitHub downloads, not users/installations. Hosted CI is disabled by project policy; coverage is unmeasured, so neither has a fabricated badge. Cargo registry publication is disabled. See [maintenance](docs/maintenance.md) and [changelog](CHANGELOG.md).

[Help](SUPPORT.md) · [Contributing](CONTRIBUTING.md) · [Security](SECURITY.md) · [MIT license](LICENSE)
