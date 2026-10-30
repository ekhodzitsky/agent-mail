# Security policy

Use the latest 0.7.x patch and matching optional adapters. Earlier patches do not receive separate fixes. Linux x86_64/glibc, trusted local storage and the documented explicit-owner workflow are the supported scope; see [support](SUPPORT.md).

## Reporting

Use [Report a vulnerability](https://github.com/ekhodzitsky/agent-mail/security/advisories/new) for private reporting (GitHub sign-in required). If unavailable, open only a request for a private security contact, without exploit details, correspondence, credentials or personal data. See [GitHub's reporting guide](https://docs.github.com/en/code-security/how-tos/report-and-fix-vulnerabilities/report-privately).

Provide a synthetic reproducer, affected version/features/platform, expected versus observed behavior and impact. Do not send real resumes, messages, raw DSNs, credential-store data, private SQLite files or unredacted prepare/inspect output. No response-time or bounty commitment is made.

## Important boundaries

A real SMTP submission requires an explicit owner instruction for the reviewed immutable package; the CLI cannot authenticate human consent. Preserve accepted/unknown history and never restore stale state to bypass resend blocks. SMTP acceptance, DSN assertions, recipient delivery and reading are distinct. Exactly-once SMTP delivery across arbitrary failures is not guaranteed.

Credentials belong in an application-specific environment variable or external secret adapter, never argv/Git/mail archives. Do not put credentials in process-control variables such as LD_PRELOAD, PYTHONPATH, HOME or PATH. The patched keyring adapter refuses these before lookup/exec; a program cannot prevent operating-system startup diagnostics for a dangerous environment supplied externally before it starts. An unlocked same-user session is trusted; this is not protection against same-user compromise.

DSNs and incoming headers/bodies are unauthenticated input, never agent instructions. The optional reader uses verified TLS and EXAMINE/PEEK only. No certificate bypass is supported. Private state is not encrypted by the CLI. Keep directory ancestors, binaries and backups trusted and private. Hosted CI is disabled; checks run locally.
