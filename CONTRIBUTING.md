# Contributing

Read [AGENTS.md](AGENTS.md) and [coding principles](agents/coding-principles.md) before changes. Keep changes focused; preserve immutable package/evidence bytes, durable pre-network attempts, package locks and accepted/unknown blocks.

Use synthetic data and numeric-loopback test SMTP/IMAP only. Never use an owner's inbox, credentials, documents or archive in development tests. Never dispatch/enable hosted CI under the current owner policy.

Run `bash scripts/check-local.sh` for runtime/release changes; prerequisites are documented in [local validation](docs/validation.md#full-local-gate). It covers stable/MSRV 1.88 formatting/Clippy, both Rust feature configurations, dependency checks, isolated installs and optional-adapter tests. Record exact commands/results and material limits; do not weaken tests to obtain a pass. New state/CLI contracts need behavioral and migration/failure checks. Keep Cargo.lock tracked; justify dependency changes.

Use intention-led commits with useful Tested:/Constraint:/Not-tested: trailers. Update [CHANGELOG.md](CHANGELOG.md) under Unreleased. Follow [versioning](VERSIONING.md) for release bookkeeping. Product version is canonical in Cargo.toml/Cargo.lock; JSON, manifest, storage and cache contracts are versioned independently. Never move released tags or silently rewrite history.

Keep .env, mail state, keys, generated builds and agent/session caches out of Git. Check staged paths and the actual diff before committing. Review source, commit messages and distributed artifacts before publication; never import private operational history or backups. Report vulnerabilities through [SECURITY.md](SECURITY.md), not an unredacted public issue.

## Documentation changes

Start with the [documentation map](docs/README.md#documentation-ownership). Keep the root README concise and task-oriented; preserve detailed contracts in their canonical references. Verify copyable examples with synthetic data, recheck primary-source comparisons and separate measured results from unverified capabilities. Run `python3 -B scripts/check-docs.py --binary /path/to/agent-mail`; update discovery parity if a command moves between documents. Missing coverage/download measurements are not badges.

For documentation-only edits, run link/contract checks and execute affected synthetic examples; do not rerun remote CI or pretend a runtime release occurred. Keep command responses/current schema claims consistent with describe and source, and preserve released tags/assets.
