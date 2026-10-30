# Investigating unknown submissions

An unknown attempt means the client cannot establish whether the SMTP server accepted the mail. It may represent a crashed process, lost reply or conservative pre-DATA failure classification. Never resend automatically, delete the attempt, change its outcome, copy an old database into service or prepare the same message again as a workaround.

## Operator procedure

1. Run `status PACKAGE_ID --json` and `inspect PACKAGE_ID --json`. Privately retain the package ID, attempt ID, Message-ID, manifest SHA-256, start/completion times, sender and recipient. Check whether the original process is still running; do not kill it merely to obtain the lock.
2. Obtain external evidence through an authorized channel: provider logs/support or an explicit recipient confirmation. Message-ID is a correlation hint, not guaranteed recipient deduplication. Search using time, envelope and identifiers together. Missing mail in Sent is not proof of non-acceptance; absence from one search is not definitive evidence.
3. Write a sanitized UTF-8 JSON evidence file using the schema below. Do not put passwords, tokens, full mail transcripts or irrelevant personal data in it. Reference retained evidence by ticket/log identifier. The CLI does not contact the referenced source or certify its authenticity.
4. Record the observation with `reconcile`. Inspect it explicitly if needed. Contradictory or later observations are new records; earlier records are never replaced.
5. Keep the package blocked even if an observation claims not_accepted. Further action requires owner review; this version deliberately has no force-send/reset/unlock operation. An accepted observation is also not a delivery receipt.

## Evidence schema 1

Replace the identifiers with values from the selected synthetic or authorized package. This example is illustrative and cannot be attached to a different package.

```json
{
  "schema_version": 1,
  "package_id": "UUID_FROM_PREPARE",
  "attempt_id": "UUID_FROM_STATUS",
  "message_id": "<UUID_FROM_INSPECT@example.org>",
  "manifest_sha256": "SHA256_FROM_INSPECT",
  "observed_at": "2026-10-28T12:00:00+03:00",
  "checked_by": "owner",
  "source": "provider_log",
  "conclusion": "inconclusive",
  "evidence_ref": "synthetic-support-ticket",
  "summary": "No definitive acceptance or rejection established."
}
```

`source`: provider_log, provider_support, recipient_confirmation or owner_review. `conclusion`: accepted, not_accepted or inconclusive. These are **asserted observations**, not transport state. observed_at requires RFC3339 with an explicit offset. Unknown fields, empty reviewer/reference/summary, embedded NUL and text fields above 8192 bytes are rejected. Total evidence size is at most 64 KiB. IDs, Message-ID and manifest digest must bind to the selected package's unknown attempt.

```sh
agent-mailer reconcile PACKAGE_ID --evidence-file evidence.json --json
agent-mailer inspect-reconciliation RECORD_ID --json
agent-mailer status PACKAGE_ID --json
```

Recording requires the same exclusive package lock used by send, so a live send returns busy. The exact input bytes, their SHA-256/size, record UUID and UTC recording timestamp are saved atomically in an append-only table. Repeating identical bytes for the same attempt returns the existing record with already_recorded:true. Whitespace changes or new observations produce different records. Failed commits do not leave a partial observation. Reconciliation never changes the original attempt, Message-ID, package state or retry policy, and never connects to a mail server.

`status` exposes record metadata including asserted_conclusion, source, observed_at, evidence digest/size, independently_verified:false and retry_authorized:false. It omits checked_by, evidence_ref and summary. `inspect-reconciliation` intentionally returns the private parsed evidence after integrity/binding verification. Keep that output private. The stored raw bytes remain the authoritative hashed input, even though inspection renders parsed JSON.

## Backup and unresolved evidence

After all writers exit cleanly, back up the complete private state directory. A synthetic offline-copy test verifies that a complete database copy preserves evidence and the resend block. It does not certify stale backups: a snapshot taken before an attempt can lose submission history. Reconcile external events before allowing any restored state to submit mail. Do not run original and restored databases as independent senders.

## Record recipient feedback separately

For an explicit recipient report after accepted or unknown submission, use `record-receipt PACKAGE_ID --evidence-file evidence.json --json`. Use the same identity/hash fields as above with source `recipient_confirmation` and conclusion `received`. Record when the report was observed with a timezone; do not invent the time the mail arrived. This appends evidence and sets the derived status flag recipient_reported_received, while delivery_confirmed remains false and SMTP state is unchanged. The assertion is not independently verified. Unknown packages remain blocked. The command rejects rejected attempts and non-recipient sources; inspect-reconciliation displays its saved evidence.
