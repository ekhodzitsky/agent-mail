# Delivery Status Notifications

DSN is an optional SMTP extension, not a read receipt. Support can vary across relays/providers. Absence of a notification proves neither delivery nor failure. Imported notifications can be forged; correlation is checked but authenticity is not certified. An asserted delivered action does not prove a human read the message or permit a retry.

## Freeze the request

```sh
agent-mailer prepare --to recipient@example.org --subject-file subject.txt \
  --body-file letter.txt --attach resume.pdf --dsn required --json
```

--dsn defaults to off. Optional requests notifications only if the secure SMTP session advertises DSN; required stops before MAIL/DATA if unsupported. Current fixed policy requests SUCCESS,FAILURE,DELAY and RET=HDRS (headers only), and freezes these in manifest schema 3. Older prepared packages keep their existing policy; enabling DSN requires a new package. ASCII envelope addresses are required for DSN in this version; SMTPUTF8/global-delivery-status is not supported here. Body and HTML text can still be UTF-8.

A committed attempt UUID yields ENVID=`package_uuid.attempt_uuid`. ENVID is distinct from Message-ID: retries after explicit rejection use a different ENVID even though their Message-ID is unchanged. ORCPT is optional and is not emitted in this version. All parameters are serialized/escaped by lettre; the application uses its typed SMTP commands, response parsing, TLS/authentication and DATA framing, with no raw socket protocol implementation.

## Observations, not inferred usage

Before any network I/O, the ordinary durable unknown attempt is saved, then its DSN policy event is committed. Subsequent immutable events are capability, parameters_about_to_send and parameters_accepted. The latter requires successful MAIL and RCPT replies; it is evidence that the server accepted the parameters, not that a report will arrive. A crash/write failure can leave only an earlier stage. No later stage is invented. Events store hashes of their payloads and omit SMTP free text/credentials.

Capability is checked after certificate-verified implicit TLS or mandatory STARTTLS. Missing required support returns dsn_unsupported (exit 5) with a conservatively blocked unknown attempt and no DATA. Optional unsupported mode may submit normally, records supported:false and dsn_parameters:false, and never claims DSN was requested. Lost replies/timeouts remain unknown. Explicit 4xx/5xx refusal remains rejected; accepted still means final SMTP 250 only. The total network-process deadline also covers DSN mode. Dry-run is offline and cannot test capabilities.

## Import one selected report

Save the chosen DSN email outside the source repository; no inbox is scanned automatically.

```sh
agent-mailer import-dsn PACKAGE_ID --attempt-id ATTEMPT_ID --file report.eml --json
agent-mailer inspect-dsn REPORT_ID --json
agent-mailer status PACKAGE_ID --json
```

Input is a regular file up to 1 MiB. Supported MIME: multipart/report with report-type=delivery-status, human-readable first part, exactly one message/delivery-status second part, and an optional returned-message third part. mailparse handles MIME/header parsing with its recursion limit. The strict importer accepts one recipient block only. Unsupported/malformed/duplicate critical fields, aggregate recipient blocks and ambiguous forwarding are rejected rather than guessed.

Original-Envelope-Id must match the selected package/attempt; Final-Recipient rfc822 address must exactly match the frozen recipient. The selected attempt must be accepted/unknown with recorded DSN parameter use; reports for prepared/rejected/non-DSN attempts are refused. Required reporting/recipient/action/enhanced-status fields are checked. Supported actions are delivered, failed, delayed, relayed and expanded (case-insensitive). Failed can carry 4.x.x when a temporary error was ultimately abandoned, as RFC 3464 specifies. Leading-zero numeric components are rejected. Missing ENVID and changed forwarded recipients are unsupported, not automatically correlated by Message-ID alone.

Exact raw bytes, SHA-256, length and UTC import time are stored atomically. The report's arrival time is not guessed. Re-importing identical bytes for the same attempt is idempotent; changed evidence is a new record. SQL triggers prevent report/event updates/deletes. Status/inspection verify stored hashes and binding before returning safe action/status metadata. Raw report text can contain private correspondence and is kept out of ordinary JSON output. No URLs, returned-message content or remote images are fetched. Keep reports credential-free and private.

Imported reports do not change the original SMTP outcome, delivery_confirmed, recipient_reported_received, attempt history or accepted/unknown resend blocks. They remain independently_verified:false. Selected-file import is separate from the optional IMAP retrieval integration and does not authenticate the report.

Standards: [RFC 3461](https://www.rfc-editor.org/rfc/rfc3461), [RFC 3464](https://www.rfc-editor.org/rfc/rfc3464). On 2026-10-29, an explicitly authorized real self-test observed DSN capability after verified TLS, successful MAIL/RCPT acceptance of the requested parameters, and final SMTP 250. The owner subsequently supplied the original notification and exact-file import succeeded. Automated release tests still use synthetic SMTP only.

## Explicit reports without ENVID

Use only for one selected original notification:

```sh
agent-mailer import-dsn PACKAGE_UUID --attempt-id ATTEMPT_UUID \
  --file selected-report.eml --correlate-by-headers --json
```

This mode requires an absent Original-Envelope-Id, exactly one original-header MIME part (`text/rfc822-headers` or `message/rfc822`), one exact Message-ID matching the frozen package, single From/To mailboxes matching its sender/recipient, and exact Final-Recipient. It permits packages without a frozen DSN request, but only one attempt across the package history, selected as accepted or unknown and carrying the frozen Message-ID. Multiple attempts make Message-ID correlation ambiguous and are refused. Address groups, multiple/duplicate identity headers, wrong recipients, missing original headers and present ENVID are refused. The normal command keeps its original frozen-policy/parameter-use/ENVID checks; there is no automatic fallback. Correlation is not authenticity: an attacker can forge DSN headers/content, and the saved assertion is independently_verified=false.

Exact original bytes/hash/size/time/IDs are stored in a separate immutable schema-6 table. Import uses the package lock and durable SQLite commit; identical attempt/raw-byte replay returns the existing report. Inspection/status revalidate hashes and binding. Old readers cannot open schema 6. No attempts, packages, send permissions or SMTP outcomes are rewritten. The optional IMAP adapter defaults to strict DSN-enabled preflight; the optional reader supports the same explicit flag with a mandatory selected-notification Message-ID and a single attempt. See its integration README.

Status adds `delivery_observation` with outcome not_reported, failed_reported, delivered_reported, delayed_reported, relayed_reported, expanded_reported or conflicting_reports. Failure plus delivered assertions is conflicting; otherwise terminal failed/delivered assertions take precedence over delayed/relayed/expanded. All report IDs remain available, so this summary never discards contradictory evidence. It is not a latest-event delivery truth or a human read receipt. `delivery_confirmed` stays false; SMTP `state`/attempts remain authoritative for duplicate protection. A failed report after accepted submission does not permit resending that package. Status guidance recommends reviewing the report, never a new send.

Report metadata includes correlation=envid/original_headers and an optional numeric smtp_reply_code extracted from one well-formed Diagnostic-Code. Ambiguous/malformed optional diagnostics remain only in raw evidence and do not invalidate otherwise supported legacy reports. Raw server prose, headers and message bodies stay in the private archived EML, not status/errors. Keep inspect/import output private. No parser authenticates provider assertions.

A provider may append a separate informational Help-only block after the single recipient block. The parser permits exactly that one extra block with one nonempty Help field whose value is uninterpreted text; mixed fields, duplicate Help, extra recipients and further blocks are refused. It is retained in raw evidence and never treated as delivery status or executable instructions.
