# Offline setup diagnostics

Available in 0.7.0. These commands never send mail, read an inbox or open/migrate the operational database. They do not prompt. Exit 0 means diagnostics completed; inspect `result.ready`, which may be false. Usage/unresolved configuration path and output failures retain documented exit codes. Do not infer remote authentication, delivery or owner send authorization from readiness.

## Native core

```sh
agent-mailer preflight --json
agent-mailer --config /private/config.toml preflight --json
```

Selects configuration using explicit `--config`, otherwise `--home`/`AGENT_MAILER_HOME`, otherwise the existing default home. Explicit config works without HOME. Unlike doctor, this does not create a state directory/database or expose sender/login/path values in the result.

`result` uses `preflight_schema_version:1`, `ready`, `checks`, `network_used:false`, `storage_opened:false`, `credential_content_validated:false` and `remote_authentication_verified:false`. The envelope remains JSON schema 1.

| checks field | States | Meaning / next local step |
|---|---|---|
| configuration | missing, unavailable, invalid, ready | Supply an existing readable bounded regular TOML file with a valid sender and known fields; errors do not echo its contents |
| smtp | blocked, not_configured, invalid, ready | Configure the provider-confirmed host/port/TLS/auth fields and valid timeouts; no provider is guessed |
| credential_environment | blocked, unavailable, available, not_required | A configured credential variable must already contain a nonempty UTF-8 value; do not paste it into chat/argv |

Core preflight constructs a validated transport with a placeholder, not the credential value; environment inspection checks availability only. It does not consult an external secret store, connect to a server, resolve DNS or validate the real password. `ready:true` is local configuration/environment availability, not account verification. No-auth SMTP can report not_required; this does not establish that a provider permits it.

## Optional desktop adapter

```sh
agent-mailer-keyring preflight --json
```

This exact invocation checks the existing fixed configuration/binary binding and the matching Keyring item's **metadata**. It uses the same private keyring.toml identities as run; it does not store, retrieve, print or inject a password. It contacts the desktop Secret Service over D-Bus. Search uses ALL without UNLOCK/LOAD_SECRETS; the proxy requests NONE rather than a secret-transfer session. Incoming metadata is reduced to fixed statuses, never item labels, account values or library errors.

| checks field | States / action |
|---|---|
| binding | missing, invalid_or_unavailable, ready — prepare the existing private binding according to the setup guide |
| core_binary | missing, unavailable, unsafe, not_executable, ready — install the matching core at the fixed path; reject symlink/nonregular/multilink, different-owner or group/world-writable files |
| smtp_configuration | blocked, missing, invalid_or_binding_mismatch, binding_matches, core_incompatible, core_preflight_failed, core_timeout, invalid, ready — fix binding/config or install a matching core |
| keyring_item | blocked, backend_unavailable, session_unavailable, missing, ambiguous, locked, available, timeout, unavailable — install the documented backend, open/unlock the desktop session manually, or manage the one selected item |

binding_matches is only a preliminary equality check; complete SMTP readiness requires the core's preflight. The adapter invokes that fixed binary with the bound config, no credential variable, a 10-second subprocess deadline and no stateful doctor. It probes metadata in an isolated worker with a 5-second deadline; ordinary failures never trigger unlock/storage dialogs. Backend import/initialization failure differs from an unavailable desktop service. Trusted binaries/ancestors/session remain prerequisites; metadata is not proof of credential validity or protection against same-user compromise.

Adapter `result` includes `preflight_schema_version:1`, `ready`, `checks`, `secret_loaded:false`, `interactive:false`, `network_used:false` (no SMTP/IMAP), `storage_opened:false`, `remote_authentication_verified:false`. `ready:true` means valid local binding/configuration/core plus exactly one accessible item; that item's password has **not** been loaded or tested. A locked/missing item returns ok:true, ready:false. Output failure returns exit 7. Existing run/store failure contracts remain unchanged.

Owner-operated setup/storage and unlock instructions: [Keyring adapter](../integrations/gnome-keyring/README.md). The [doctor/dry-run](cli.md) checks still cannot prove login or delivery. Real send always requires an explicit owner instruction for the reviewed package.
