# Installation and release assets

Current public release: **0.7.1**. Agent Mail is the repository/project; the Cargo package, executable and optional adapter names remain `agent-mailer` for runtime compatibility. Use matching release assets. Supported native target: Linux x86_64/glibc; see [scope](production-readiness.md).

For an existing installation, first stop active workers and follow [upgrade/recovery](upgrade-and-recovery.md). Replacing source/binaries must not create a new operational store or change credential lookup identities.

## Install the native Linux asset

The core needs no Rust, Node.js, Bun or Python runtime. Download the native binary and SHA256SUMS from the [0.7.1 release](https://github.com/ekhodzitsky/agent-mail/releases/tag/v0.7.1), or use curl/GNU coreutils:

```sh
export PATH="$HOME/.local/bin:$PATH"
(
set -eu
umask 077
download_dir="$(mktemp -d)"
release_url="https://github.com/ekhodzitsky/agent-mail/releases/download/v0.7.1"
curl -fLsS --connect-timeout 15 --max-time 300 \
  "$release_url/agent-mailer-0.7.1-x86_64-unknown-linux-gnu" \
  -o "$download_dir/agent-mailer-0.7.1-x86_64-unknown-linux-gnu"
curl -fLsS --connect-timeout 15 --max-time 300 \
  "$release_url/SHA256SUMS" -o "$download_dir/SHA256SUMS"
(cd "$download_dir" && sha256sum --ignore-missing --check SHA256SUMS)
install -Dm755 "$download_dir/agent-mailer-0.7.1-x86_64-unknown-linux-gnu" \
  "$HOME/.local/bin/agent-mailer"
agent-mailer --version
)
```

Expect `agent-mailer 0.7.1` and a successful checksum line for the native file. --ignore-missing allows checking just the downloaded subset; it does not replace the hash check. Checksums detect corruption, not independently signed provenance. Keep the downloaded checksum/verification records with the installation evidence.

The isolated subshell stops on download/checksum errors before install; it does not enable errexit in your interactive shell. On a download timeout/failure, do not install a partial file. On slow connections use a suitable finite transfer budget, retry only the download or build from source below; no mail operation is involved. The browser release page and GitHub CLI `gh release download` provide alternative retrieval paths. Never disable TLS verification to work around a transfer problem.

## Install from source

Requires Git, Rust 1.88+, a C compiler and access to GitHub/locked Cargo dependencies. For the published version:

```sh
git clone --branch v0.7.1 --depth 1 https://github.com/ekhodzitsky/agent-mail.git
cd agent-mail
cargo install --path . --locked --root "$HOME/.local"
export PATH="$HOME/.local/bin:$PATH"
agent-mailer --version
```

A development checkout can include Unreleased documentation/code; check its Cargo version. Do not add --features test-smtp to a production install. The installation root above matches the desktop launcher's fixed core path. If you use ordinary Cargo's default ~/.cargo/bin instead, the optional Keyring launcher will still require a matching binary at ~/.local/bin/agent-mailer.

The [first-package tutorial](getting-started.md) uses the current source examples, or run the [capture-only SMTP fixture](local-testing.md) for a local submission test. Neither configures a real mailbox.

## What each asset contains

| Asset | Contents |
|---|---|
| agent-mailer-0.7.1-x86_64-unknown-linux-gnu | Standalone native core; production features, no plaintext test SMTP |
| agent-mailer-keyring | Optional system-Python/GNOME credential launcher and metadata preflight |
| agent-mailer-imap-dsn | Optional system-Python selected read-only notification reader |
| agent-mail-0.7.1-source.tar.gz | Full tracked source from the reviewed tag; no Git database, generated state or operational data |
| VERIFICATION.txt | Revision/toolchain/check evidence and limitations |
| SHA256SUMS | Hashes of the five other assets |

Adapters are separate installations with [Keyring](../integrations/gnome-keyring/README.md) / [IMAP](../integrations/imap-dsn/README.md) prerequisites. Installing only the core does not update an already installed adapter. GitHub's automatic tag archives represent tagged source; main documentation can evolve independently of immutable release files.

## Maintainers

Use the [version/release procedure](../VERSIONING.md#release-procedure), [local validation](validation.md) and [publication checklist](public-opening-checklist.md). `bash scripts/check-local.sh` runs local gates without hosted jobs. `scripts/build-distribution.py` creates normalized/stripped native artifacts with path/feature checks; verify tagged archive member names/bytes before publication. Exact tool prerequisites belong in validation, not the end-user setup path.
