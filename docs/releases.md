# Installation and local releases

Current public product version: 0.7.0. This repository starts from a reviewed source-only baseline. Use matching core/adapters. The repository is named Agent Mail; the executable/package remains agent-mailer for compatibility, without changing existing state or credential identities.

## Install from source

With Rust 1.88+, a C compiler and access to locked Cargo dependencies:

```sh
export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
cargo install --path . --locked
agent-mailer --version
```

The installed binary needs no Node.js, Bun or Python. Do not add `--features test-smtp` to a production installation. See the synthetic first-use walkthrough in [README](../README.md).

## Install the native Linux asset

Download the native binary and SHA256SUMS from this repository’s current GitHub release. In their download directory:

```sh
sha256sum --ignore-missing --check SHA256SUMS
install -Dm755 agent-mailer-0.7.0-x86_64-unknown-linux-gnu "$HOME/.local/bin/agent-mailer"
export PATH="$HOME/.local/bin:$PATH"
agent-mailer --version
```

The checksum detects corruption; it is not independently signed build provenance. The binary targets Linux x86_64/glibc and excludes test-smtp. Optional keyring/IMAP adapters are separate matching assets with their own prerequisites. Preserve existing state/configuration/credential identities and follow [upgrade/recovery](upgrade-and-recovery.md). Installing only the core does not replace the optional adapter security fix.

## Local release gate

Run `bash scripts/check-local.sh`. Prerequisites: Bash, Git, system Python 3.11+, OpenSSL CLI for synthetic adapter tests, Rust stable/MSRV with rustfmt/Clippy, cargo-audit 0.22.2 and cargo-deny 0.20.2. The script does not install global tools or dispatch hosted jobs. It downloads Cargo/advisory metadata as needed, uses isolated install roots and finishes with a default-feature production build. It also checks local documentation links/headings and discovery-contract parity against the installed production binary. Details and limits: [validation](validation.md).

Build the distributed native artifact with normalized source/toolchain paths and stripped symbols:

```sh
python3 -B scripts/build-distribution.py --output /path/to/new-release-binary
```

The output must be new. The builder fails if local root markers remain, version disagrees or test SMTP is enabled. A normal cargo install remains suitable for local use but does not perform the distribution path checks.

For a source archive, use `git archive` from the reviewed current tag. Include tracked code/documentation only, never .git, ignored state, caches or private backups. Verify archive contents against that tree. GitHub automatic source archives contain the same maintained tagged source.

Record the exact source revision/toolchain/gate results and all asset SHA-256 values. Update version/changelog and immutable release tags under [VERSIONING](../VERSIONING.md). Real mailbox operations require explicit owner authorization; source publication never includes operational state. GitHub Actions remains disabled.
