# Versioning and releases

## Product version

`[package].version` in Cargo.toml is the canonical product version; Cargo.lock must agree. Do not add a second VERSION file. CLI `--version` uses Cargo's package version. Release assets bind exact source revisions to toolchains/check results; [installation/releases](docs/releases.md) owns the current published version. Do not spread current feature summaries/version strings through policy files. See [JSON contracts](docs/json-contract.md#independent-versions) for independently versioned data formats and [supported scope](docs/production-readiness.md) for maturity limits.

Use semantic versions MAJOR.MINOR.PATCH. Before 1.0, incompatible CLI, configuration or behavior changes increment MINOR; compatible features increment MINOR and corrections increment PATCH. After 1.0, incompatible changes increment MAJOR, compatible features MINOR and fixes PATCH. State changes with data-loss or resend implications always require migration documentation regardless of version number. Prereleases use explicit suffixes such as `-alpha.1` or `-rc.1` when preparing an actual prerelease.

During development, record user-visible changes under `Unreleased`; do not increment the product version for every commit. Documentation-only changes do not by themselves require a release. When cutting a release, choose the appropriate version for the accumulated changes, update Cargo.toml/Cargo.lock together, and turn the applicable changelog section into a dated version entry. Never backdate an entry or imply a release occurred because a version appeared in Cargo.

## Independent contracts

Product version, JSON envelope schema, package manifest schema and database schema are distinct contracts. An additive compatible field need not bump the JSON schema. Breaking serialized changes require a new schema version, explicit reader compatibility and migration rules. Existing packages and attempt evidence must not be silently rewritten or discarded. The database uses application_id and user_version, currently schema 6. The exact unversioned, schema-1, schema-2, schema-3, schema-4 and schema-5 layouts are migrated atomically without changing package or attempt bytes. Future storage changes require an explicit migration and preservation tests; unknown versions fail closed. See [storage migrations](docs/storage-migrations.md).

The `agent-dev-kb` stamp records installed development instructions only. It is not the CLI version, a release tag or proof of tool readiness.

## Changelog discipline

Every user-visible behavior, safety, compatibility or dependency change needs a concise entry with its impact. Use Added, Changed, Fixed, Security, Deprecated or Removed as appropriate; omit empty categories. Preserve released entries. Document known limitations and migration instructions beside breaking changes. Do not copy raw commit logs, local agent state, credentials or correspondence into the changelog.

## Release procedure

1. Review the final diff and changelog; verify product/lockfile/schema consistency and migration compatibility.
2. Run the AGENTS.md gates for default and test-smtp, plus MSRV checks. Run any additional checks required by the affected safety boundary.
3. Record local verification for the exact release candidate: commit, clean working tree, toolchain versions, commands and results. Checks run locally; do not enable or run GitHub Actions without a new explicit instruction.
4. For a production designation, close the applicable gaps in [production readiness](docs/production-readiness.md). A prerelease must describe remaining gaps explicitly.
5. Build production artifacts without test-smtp using `python3 -B scripts/build-distribution.py --output <new-artifact-path>` (native Linux with path remapping and symbol stripping); record target, toolchain, commit and SHA-256. Verify the standalone binary and installation instructions.
6. Commit the version/changelog changes and create an annotated immutable `vX.Y.Z` tag (or full prerelease version). Never move or replace a published tag. Create the matching GitHub release with notes and checksums; mark prereleases accordingly.

Release bookkeeping never authorizes real-provider mail or publication of operational data. No tag or release is created merely to make the version history look complete.

## Public source baseline

Agent Mail starts from one reviewed source-only initial commit at product version 0.7.0. Version 0.8 renames the CLI, Cargo package and adapters to agent-mail. Compatibility with existing state and credential lookup identities is retained; serialized formats are unchanged. Earlier Git history and operational data are not part of this repository. Only new commits and immutable release tags are published here. Git author attribution is retained intentionally.
