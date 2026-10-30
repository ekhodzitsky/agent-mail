# Agent workflow: discovery, preparation identity and DSN retrieval

Discovery, idempotent preparation and selected DSN retrieval preserve immutable package bytes and conservative SMTP submission. The core remains a standalone Rust binary. Owner consent is an external instruction, not something these features verify.

## Discover the installed interface

```sh
agent-mailer describe --json
```

Discovery works without HOME, configuration or a database and creates no files. It reports discovery schema 1, CLI/JSON/storage versions, supported manifest versions, input limits, commands and their parser-derived arguments/choices. Hidden worker plumbing is excluded. Side-effect/authorization classifications distinguish local work, submission, and the offline `send --dry-run` exception. `test_smtp_enabled` identifies a development-feature build.

JSON results include optional top-level `next` guidance. Guidance schema 1 uses `action`, fixed `instruction`, `argv` (an array or null), and `requires_owner_instruction`. Help/version/usage envelopes may omit it. Current actions:

| Action | Meaning |
|---|---|
| `review_frozen_package` | Inspect the existing exact package |
| `obtain_owner_instruction` | Wait for explicit consent; no executable send suggestion |
| `investigate_submission` | Inspect history/evidence; never automatically resend |
| `await_delivery_observation` | SMTP acceptance is not delivery or reading |
| `review_refusal` | Review refusal and obtain a new instruction before retry |
| `review_delivery_report` | Review unverified evidence without changing SMTP state or retry permission |
| `resolve_error` | Resolve the error without inferring retry permission |

Runnable hints retain the invocation's explicit home/config flags (including a parsed legacy home environment override). Non-UTF-8 paths suppress runnable hints rather than generating lossy commands. Execute argv through an argument-array API, not a shell-joined string. No hint includes a send invocation. Guidance is invocation context, not archived correspondence: backup/upgrade comparisons check archived data independently of hints.

The portable [Agent Mailer skill](../skills/agent-mailer/SKILL.md) is included in source. Install/select it using the agent environment's normal mechanism if desired; this release does not modify global agent configuration.

## Idempotent preparation

```sh
agent-mailer prepare --to recipient@example.org \
  --subject-file subject.txt --body-file letter.txt --attach resume.pdf \
  --operation-key synthetic/job-42 --json
```

Keys are optional, 1–128 ASCII letters/digits or `-._:/`. Use an opaque stable job reference, not a credential or letter text. The raw key is not retained in the manifest/database or echoed by operational errors; its SHA-256 selects the mapping. Keep using that same key when retrying the same logical preparation after lost output.

Input-fingerprint schema 1 covers exact sender/recipient strings, subject and final plain/HTML bytes, normalized DSN mode, ordered attachment names/byte lengths/hashes and metadata. Input paths and SMTP authentication/settings are not message content. MIME randomness, Message-ID, preparation time and the CLI version are excluded so an identical request can recover the original frozen material rather than regenerate it.

An IMMEDIATE SQLite transaction checks the key and publishes a new package plus its immutable mapping atomically. Identical input returns the same package ID, manifest/hash and `already_prepared:true`. A new package returns false. The returned state reflects existing attempt history; it can be accepted, unknown or rejected rather than prepared. Reusing a key with changed input returns `operation_conflict`, exit 5, without another package. Before reuse the stored package and its fingerprint binding are verified. A failed commit publishes neither the package nor its mapping.

Mappings have no automatic expiry, overwrite, delete or reset command. They remain with the local archive. A different key or no key creates an independent package; this is not global content deduplication or recipient-side exactly-once delivery. Never choose another key to evade an accepted/unknown block. Recovered packages still require review and a separate owner instruction; preparation identity is not send approval.

## Storage upgrade

Schema 5 adds only `prepare_operations` and immutable-row triggers. Recognized schemas 0–4 migrate transactionally, without changing existing manifest/MIME/PDF/attempt/evidence rows. Old preparation mappings do not exist for packages created without a key. JSON envelope schema 1 and manifest schemas 1/2/3 remain supported.

Before updating an existing installation, keep a private consistent offline or SQLite-backup-API copy outside Git and stop conflicting old writers. Current storage schema 6 adds header-correlated reports; older binaries cannot open it; downgrade is unsupported. Do not restore a stale copy as an independent sending instance or clear attempts. The default state path, `AGENT_MAILER_HOME`, keyring identities and configured sender/password source remain unchanged.

## Selected DSN retrieval

The separate [IMAP adapter](../integrations/imap-dsn/README.md) retrieves selected notifications through verified implicit TLS and passes original bytes to the existing strict importer. It has its own Python runtime/configuration; installing the Rust core never enables mailbox access or guesses a provider/login.

It uses EXAMINE/read-only selection, UID/UIDVALIDITY identities, size checks and BODY.PEEK[]. A local private immutable cache records remote-identity/raw-byte hashes and core report IDs. The core importer remains the authority for package/attempt/recipient binding and raw-byte duplicate import. Reports never change SMTP attempts, delivery_confirmed or retry permission. Cache/output failure after a successful core import can leave partial progress; replay verifies/deduplicates the evidence.

IMAP dry-run is offline. Real retrieval needs separately configured credentials and owner-authorized read scope. No real mailbox setup/retrieval is part of development verification, and no SMTP or mailbox-mutating command is available in the adapter. No automatic replies, generalized inbox client, Sent-folder APPEND or tracking pixels are added.

## Discover packages and protect state (0.6.0)

Use `list --limit 25 --summary --json` for a bounded index of IDs, then inspect/status on a selected package. Summary explicitly reports integrity:not_checked and gives no state/retry permission. Continue using pagination.next_after with --after; each page may observe newer additions/statuses. Legacy unparameterized list remains a full verified archive read.

For an explicitly selected private backup destination, use `backup --output NEW_DIRECTORY --json`, then `verify-backup DIRECTORY --json`. [Snapshot rules](backup.md) require trusted current-schema sources and explain cache pairing, lock duration and freshness limits. A successful verification is not send authorization; never activate a stale backup to evade accepted/unknown history.

## Diagnose setup without secrets (0.7.0)

Use `agent-mailer preflight --json` for configuration/credential-environment availability without opening state. For the fixed desktop deployment, use `agent-mailer-keyring preflight --json` to check binding, core installation and Keyring metadata without loading or injecting its secret. A completed diagnostic has ok:true even when result.ready is false; inspect checks, not only the process exit. Follow the [preflight status reference](preflight.md). No readiness result grants mail authorization or proves provider authentication. Existing doctor/run remain available when explicitly needed.
