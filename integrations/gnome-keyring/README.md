# Optional GNOME Keyring adapter

This optional desktop integration is separate from the standalone Rust CLI. It uses the system Python 3.11+, PyGObject/libsecret, a running Secret Service (GNOME Keyring here), and Zenity for initial entry only. Nothing is installed from pip. The core binary still runs without Python and still accepts environment credentials.

## Initial setup order

1. Install the default-feature Rust binary at the fixed `~/.local/bin/agent-mailer` path. A native release install already uses that path. For a source install intended for this launcher, use `cargo install --path . --locked --root "$HOME/.local"`; the ordinary Cargo default is ~/.cargo/bin and is not this launcher's destination.
2. Configure private `~/.local/share/agent-mailer/config.toml` with the actual sender and SMTP settings, then `keyring.toml` using the [binding template](../../examples/keyring.toml). Both files must be private regular single-link files, mode 0600, inside a mode-0700 directory. Both store and run require the binding **before** any secret operation. Never overwrite working configuration/bindings with templates.
3. Set sender, username, host, port, security (tls/starttls), password_env and account in the binding. The first six must match config.toml; account is a stable lookup key, not necessarily the SMTP login. Preserve an existing account value to reuse its credential. Do not rename it or delete older items automatically.
4. Install the launcher and, for a new credential only, have the owner run the interactive store command:

```sh
install -Dm755 integrations/gnome-keyring/agent-mailer-keyring "$HOME/.local/bin/agent-mailer-keyring"
"$HOME/.local/bin/agent-mailer-keyring" store
"$HOME/.local/bin/agent-mailer-keyring" run doctor --json
```

The final command is offline. Existing working installations need matching executable updates, not fresh configuration, a new state directory or re-entry of the password.

`store` opens a password dialog and saves the value in the default persistent collection; matching adapter attributes are replaced. It may ask to unlock/create the collection. A timeout does not prove that the service failed to save the item; check with run doctor after unlocking rather than assuming the operation had no effect. Set a password on the collection; do not choose an unencrypted/passwordless keyring. Storage protection and unlocking are managed by the desktop, not this adapter. No password is written to a file, stdout or command arguments by the adapter.

`run` looks up exactly one item for application=agent-mailer and the private binding's account attribute using schema org.agent_mailer.SMTP. It refuses missing, ambiguous or locked items without requesting unlock. It injects the secret into the child environment and execs the fixed ~/.local/bin/agent-mailer binary. An unlocked desktop session is required; this is not a headless secret-service deployment. Libsecret lookup runs in a separate process with a 30-second parent deadline; store uses 60 seconds. Stalled workers are terminated and reaped; this does not rely on interrupting a native GLib call with a Python signal. Safe errors use a JSON envelope with keyring_unavailable and exit 3. Raw OS/library errors are suppressed. Output failure returns exit 7. Config reads reject symlinks/FIFOs, group/world-readable files and inputs above 1 MiB.

This adapter binds the local default configuration to the sender, authentication identity, SMTP endpoint and environment-variable name in a private keyring.toml file. Config/home overrides are rejected. The trusted user must protect the local config, binary and parent directories. Running as that user grants access to its unlocked secrets; this is not a consent mechanism or protection against same-user compromise. Python does not promise secret-memory zeroization. Environment credentials remain visible to sufficiently privileged processes.

All CLI commands can follow `run`; real send still requires an explicit owner instruction for the reviewed package. `run doctor --json` does not connect to SMTP. Plain `agent-mailer` does not read the keyring. Agents must use the adapter when they need the stored credential. Do not print/look up the secret directly for troubleshooting.

Use the desktop Passwords and Keys application to manage the matching application/account item. Newly stored items are labeled Agent Mailer SMTP; existing labels can differ because lookup uses attributes rather than the label. Revoking the application password in Yandex is a separate action. The item is not included in Git or mail archive backups.

Synthetic checks (no keyring/mail access):

```sh
/usr/bin/python3 -B -m unittest discover -s integrations/gnome-keyring -p 'test_*.py'
```

API references: [libsecret Python examples](https://gnome.pages.gitlab.gnome.org/libsecret/libsecret-python-examples.html), [password storage](https://gnome.pages.gitlab.gnome.org/libsecret/func.password_store_sync.html). Readiness is only local: it does not validate authentication, sender permissions or delivery.

Worker IPC results and entered/returned secrets are bounded to 2048 UTF-8 bytes. Workers must exit within their deadline before the parent receives the bounded result; partial frames cannot extend the timeout. For upgrades preserve the fixed installation/state/credential identities; see [upgrade/recovery](../../docs/upgrade-and-recovery.md).

## Optional IMAP DSN credential launch

`agent-mailer-keyring imap-dsn --package-id PACKAGE_UUID --attempt-id ATTEMPT_UUID --dry-run --json`
passes the existing application password to the fixed `~/.local/bin/agent-mailer-imap-dsn` executable. Remove `--dry-run` only within an explicit owner-authorized read scope. This mode never invokes SMTP. A provider may require enabling IMAP/app-password access separately; SMTP authentication alone does not prove IMAP access.

The owner must explicitly configure private `~/.local/share/agent-mailer/imap.toml` as described in the [IMAP adapter](../imap-dsn/README.md), and add a matching destination binding to the existing private `keyring.toml` (do not replace its existing account attribute):

```toml
[imap]
host = "imap.example.org"
port = 993
mailbox = "INBOX"
```

The IMAP login and `password_env` must match the existing top-level keyring binding. This explicitly opts into reuse of that stored credential; it does not discover providers or silently reuse SMTP settings. The IMAP config must select the default state directory and fixed core binary. Both config files must be private regular files, not symlinks or hardlinks. Configured destination, identity and executable paths are validated before secret lookup. Only canonical package/attempt UUIDs, up to 100 `--uid` values or one bounded `--report-message-id`, `--report-search header|text|sender-date|recent-uids`, the required paired `--report-date`/`--report-from` for sender-date, `--dry-run` and `--json` are accepted; config/home overrides, worker mode and abbreviated options are refused. Ordinary failures never prompt for a password or print it.

For Yandex, the [official configuration instructions](https://yandex.ru/support/yandex-360/customers/mail/ru/mail-clients/others.html) specify `imap.yandex.ru:993`, implicit TLS and an application password. Use the owner's primary account login rather than inferring it from a sender alias. No real mailbox configuration is shipped in this repository.

For an explicitly authorized no-ENVID IMAP lookup, the launcher also accepts `--correlate-by-headers` only with `--report-message-id`. It does not change the fixed destination/configuration or authorize SMTP. The separate adapter/core validate attempt binding and selected report contents.

For a known UID, `--expected-uidvalidity` is accepted only with `--uid` and is required when combining UID and notification Message-ID. UID cannot be mixed with a search selector. Invalid/misplaced/duplicate epoch values are refused before credential lookup; the IMAP adapter compares the remote epoch before any fetch.

Credential variable names must be application-specific. Version 0.5.1 rejects loader/interpreter/locale/process-control destinations before credential injection; use names such as AGENT_MAILER_SMTP_PASSWORD or AGENT_MAILER_IMAP_PASSWORD. This fixes misconfiguration-driven pre-startup diagnostic leakage; it does not protect a process started with an already dangerous external environment.

## Metadata-only preflight (0.7.0)

Use `agent-mailer-keyring preflight --json` before secret-loading commands when diagnosing setup. It distinguishes binding/configuration/binary problems and missing/locked/ambiguous/unavailable Keyring items, without loading or injecting their values, requesting unlock, opening mail state or contacting SMTP/IMAP. It uses the fixed matching core's stateless preflight plus a bounded metadata worker. ok:true means diagnostics completed; inspect result.ready. An available item is not proof of a nonempty/valid password. See [status vocabulary and deadlines](../../docs/preflight.md). Existing run/store behavior and stable lookup identities remain unchanged.
