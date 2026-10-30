# Configuration and state selection

The CLI does not discover a mail provider or account. Configure a sender and provider-confirmed SMTP settings privately. A configuration file references the name of a credential environment variable; it never contains its value. See [Yandex](yandex-setup.md) for a provider-specific example or [Keyring](../integrations/gnome-keyring/README.md) for optional desktop injection.

## Select a configuration and history

| Selection | Resolution |
|---|---|
| State directory | `--home PATH`, otherwise `AGENT_MAILER_HOME`, otherwise `$HOME/.local/share/agent-mailer` |
| Configuration | `--config PATH`, otherwise `config.toml` in the selected state directory |
| Explicit config for preflight | Works without HOME or a state directory; preflight never opens state |

Always retain the intended state/config selection across preparation, inspection and submission. Global options work before or after subcommands. Paths may be ordinary relative paths or absolute filesystem paths; operational state homes beginning with the ambiguous relative prefix `file:` are refused. Never choose an empty/new store to evade accepted/unknown history. Repository names do not change default runtime paths.

## TOML fields

```toml
sender = "sender@example.org"
[smtp]
host = "smtp.example.org"
port = 465
security = "tls"
username = "login@example.org"
password_env = "AGENT_MAILER_SMTP_PASSWORD"
timeout_seconds = 30
total_timeout_seconds = 60
```

The host/login above are synthetic, not a provider recommendation. Copy only provider-confirmed values. The SMTP section is optional for preparation; it is needed for send/dry-run. Unknown fields and invalid types are rejected, so a `password` or `signature` field is not supported.

| Field | Contract |
|---|---|
| sender | Required single mailbox address, without display-name brackets, surrounding whitespace or line breaks; frozen at prepare |
| smtp.host / port | Required in an SMTP section; nonempty host and integer port 1–65535 |
| smtp.security | `tls` for implicit TLS, or `starttls` for mandatory upgrade; certificate checks are always enabled |
| smtp.username / password_env | Both present or both absent; username must be nonempty and single-line; variable name must be valid and value available/nonempty UTF-8 for actual transport construction |
| smtp.timeout_seconds | Default 30; integer 1–300; socket I/O timeout, not a session wall-clock budget |
| smtp.total_timeout_seconds | Default 60; integer 1–600; supervised network-worker budget including DNS |

No-auth settings can pass local validation; this does not establish that a real server permits them. Plaintext loopback settings exist only in the explicit test-smtp development feature and must not be used for production installation. No insecure TLS bypass is supported.

The sender is copied into the package; later config changes do not rewrite it. Submission checks the configured sender against the frozen package. A signature belongs in the final plain/HTML content before prepare. There are no late templates or automatic signature additions.

## Credentials and permissions

Supply credentials through your intended protected environment injection mechanism or the optional desktop adapter. Do not put their values in argv, shell history, Git, message packages or troubleshooting output. Adapter bindings require application-specific variable names and reject loader/interpreter/process-control names. OAuth refresh and separate provider APIs are not implemented.

Keep configuration private, for example mode 0600 under a trusted directory. Core input reads reject nonregular files and final-component symlinks; the desktop adapter additionally enforces private file mode and a single hard link. State directories/databases use 0700/0600 on supported Unix systems. Trusted ancestors/users/filesystems remain prerequisites; correspondence and backups are unencrypted.

## Diagnose in increasing scope

1. `preflight --json` reads local configuration/environment availability without state access, DNS or a connection. `ok:true` can accompany `ready:false`.
2. `doctor --json` opens/initializes/migrates recognized local state and checks SMTP configuration; it still does not connect. Inspect smtp_ready independently of ok.
3. `send ID --dry-run --json` verifies the package, resend policy and configuration. It makes no attempt/network call; accepted/unknown packages are refused.
4. Real submission is a separate authorized operation. It may be accepted, explicitly rejected or unknown; follow the [agent workflow](agent-workflow.md).

Desktop `agent-mailer-keyring preflight --json` inspects the matching item's metadata without loading its value. See [preflight statuses](preflight.md). Configuring a credential or obtaining local readiness never authorizes sending or mailbox access.
