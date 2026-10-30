# Comparison: local email CLIs

Reviewed on 2026-10-30 from primary documentation. The task is an agent preparing a reviewed message and submitting through an existing mail account. msmtp is a relevant small submission component; Himalaya is a relevant full mailbox CLI. Neither requires adopting a hosted email API merely to send through an existing account.

| Project / reviewed reference | Documented workflow | Reason to choose it |
|---|---|---|
| Agent Mail, current 0.7.1 source | Frozen package/MIME, explicit review, durable attempt journal, accepted/unknown blocks; optional selected-DSN reader | Agents that need a persisted preparation/submission boundary and conservative retry behavior |
| msmtp, official project overview (unversioned) | Sendmail-compatible submission to an SMTP server; TLS, multiple accounts and DSN support | Existing mail composers/pipelines that already produce messages and manage their own workflow |
| Himalaya, master documentation and Cargo version 2.2.1 | Mailboxes/messages/attachments; IMAP, SMTP and other backends; JSON output | Broader CLI mailbox operations beyond selected delivery-notification import |

Sources: [msmtp overview](https://marlam.de/msmtp/), [Himalaya README](https://github.com/pimalaya/himalaya#readme), [Himalaya Cargo configuration](https://github.com/pimalaya/himalaya/blob/master/Cargo.toml). These are dated documentation observations, not a frozen release test or interoperability certification. Recheck the linked moving references before a new adoption decision.

No competitor binary was installed, no mailbox was accessed and no cross-tool throughput, safety ranking or absence-of-feature claim was measured. Equivalent durable-journal/retry guarantees in alternatives are **not verified**, rather than asserted unsupported. Provider/account compatibility also needs direct validation.

There is no automatic migration from these tools' configurations or mailboxes. Agent Mail's single-package legacy importer targets its separately documented manifest format; raw Sent messages are not a substitute for attempt history. Conversely, use an external submission client only with separate owner authorization and record its outcome rather than assuming the CLI journal observed it.

The comparison changes if full inbox management, a sendmail interface, other providers/platforms or synchronized Sent storage becomes the primary requirement. See [supported scope](production-readiness.md) and [maintenance priorities](maintenance.md). [Offline measurements](benchmarks.md) cover Agent Mail only and support no superiority claim.
