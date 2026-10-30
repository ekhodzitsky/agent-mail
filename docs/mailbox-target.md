# Provider scope and sender configuration

The core CLI is provider-neutral. Configure the actual sender and login privately; tracked examples use reserved example.org addresses. A sender alias and the primary authentication account can differ. Freeze the configured sender in each package and use the account required by the provider for authentication.

## Configuration requirements

- Verify current official provider instructions for the selected account type: SMTP hostname, port, required TLS mode, login format, supported authentication and account/admin settings.
- Confirm how the selected account handles Sent-folder copies rather than assuming SMTP creates one.
- Keep credentials outside Git and packages. Configuration references an environment variable; the secret is supplied privately through the intended injection mechanism.
- Provider identification, local setup, doctor and dry-run do not authorize a mailbox connection or SMTP submission. Real sending requires a separate instruction for a reviewed package.

The [Yandex example](../examples/yandex.toml) and [setup guide](yandex-setup.md) document provider settings and account/alias distinctions. An alias sender may require the primary account as username; do not infer authentication success or sender permission from an offline check.

Provider examples are not account-specific interoperability certification. Published checks use synthetic local servers; verify an actual account separately within an explicitly authorized deployment.
