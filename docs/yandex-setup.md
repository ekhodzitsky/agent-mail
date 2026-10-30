# Yandex setup

Owner-authorized scope: prepare local configuration and offline checks. Real submission still requires a separate instruction for a reviewed package. No account password, app password or OAuth token belongs in Git, chat, package data or command arguments.

## Provider settings

The [official Yandex instructions](https://yandex.ru/support/yandex-360/customers/mail/ru/mail-clients/others.html), checked on 2026-10-30, specify smtp.yandex.ru, port 465 with implicit TLS and a mail application password. Use the primary account login appropriate for that account type; an alias/custom-looking sender address does not establish a business account or its login. Their SMTP-only setup also asks users to enable IMAP access and application-password/OAuth access in Mail settings. Configure these in the intended account; organization policy may restrict access.

This CLI currently uses an application password. Yandex recommends OAuth where supported; OAuth refresh is a future integration. TLS certificate verification remains enabled. We do not configure IMAP synchronization, change mailbox contents or assume Sent-folder copies.

## Local configuration

The sanitized [Yandex example](../examples/yandex.toml) uses reserved example.org sender and login addresses and references AGENT_MAIL_SMTP_PASSWORD. Copy it to a private state directory only if no configuration already exists. Use mode 0600 for ~/.local/share/agent-mail/config.toml and mode 0700 for its directory. Keep the actual sender/login in that private file; store no secret there.

The alias remains the frozen envelope/From address; only SMTP authentication uses the primary account. Do not infer successful authentication or alias sender permission from an offline check.

## Owner step: app password and local diagnostic

Open the intended Yandex account's Mail settings → Mail programs, enable the access options described above, then create a dedicated mail application password named agent-mail in Yandex ID. Keep it in an owner-controlled private secret mechanism, such as the optional GNOME Keyring integration below. A separate password-manager product is not required by the CLI. Enter it only in a private local Bash terminal without session recording. Do not send it to the assistant.

```bash
(
  set +x
  IFS= read -r -s -p 'Yandex mail application password: ' AGENT_MAIL_SMTP_PASSWORD
  printf '\n'
  export AGENT_MAIL_SMTP_PASSWORD
  "$HOME/.local/bin/agent-mail" doctor --json
)
```

The prompt is a manual setup step outside the noninteractive CLI. The subshell keeps the variable out of its parent; no secret literal enters shell history. The environment is still visible to sufficiently privileged local processes. This is session-only secret handling, not persistent keychain integration. Subsequent agent processes do not automatically inherit this terminal's variable: use your controlled environment/secret injection when running the CLI, without revealing the value in logs.

`doctor` never connects. Without the environment secret it reports smtp_ready:false and smtp_issue:configuration. With a nonempty value it validates local configuration only; smtp_ready:true does not prove the password or account works. `send ID --dry-run --json` is also offline and needs the configured environment variable. Do not remove authentication fields to bypass a missing secret.

## Controlled submission later

Select an owner-controlled recipient and prepare a synthetic package. Inspect its full contents and obtain a separate explicit send instruction. Real submission is not part of setup. Check status after any error; unknown blocks retries. Confirm arrival independently with the recipient and investigate Sent-folder behavior separately.

Offline smoke tests may use a clearly synthetic placeholder credential in an isolated process, never for a network command. Such a test proves configuration parsing, not authentication. No placeholder is persisted in the real configuration.

## Optional persistent desktop storage

For optional GNOME Keyring integration, use `agent-mail-keyring store` for private graphical entry and `agent-mail-keyring run doctor --json` for offline readiness. The adapter reads an unlocked keyring without an interactive prompt in agent mode; plain agent-mail continues to use only environment credentials. See [integration details](../integrations/gnome-keyring/README.md).

Yandex documents the beautiful address as an additional address of the existing mailbox: [official alias documentation](https://yandex.ru/support/yandex-360/customers/mail/ru/web/preferences/about-sender/additional-addresses). A custom domain alone does not establish a separate business-mail login.
