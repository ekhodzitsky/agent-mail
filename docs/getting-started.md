# Your first package

This tutorial prepares and reviews a synthetic email, then validates it offline. It never submits mail. Use a trusted local filesystem and keep generated state private. The core binary is `agent-mail`; the project/repository is Agent Mail.

## Install and obtain the examples

For a source checkout, use Git, Rust 1.88+, a C compiler and network access to GitHub/Cargo:

```sh
git clone https://github.com/ekhodzitsky/agent-mail.git
cd agent-mail
cargo install --path . --locked --root "$HOME/.local"
export PATH="$HOME/.local/bin:$PATH"
agent-mail --version
```

The installed core does not need Rust, Node.js, Bun or Python at runtime. If you prefer a prebuilt binary, follow [installation](releases.md); clone the source only to obtain the example files below and skip cargo install. The demonstration uses a shell and jq to extract JSON fields. Optional Keyring/IMAP adapters are not needed.

## Create an isolated demonstration store

Run from the repository root:

```sh
umask 077
demo_home="$(mktemp -d)"
cp examples/demo/config.toml "$demo_home/config.toml"
cp examples/demo/resume.fixture "$demo_home/resume.pdf"
agent-mail --home "$demo_home" preflight --json
```

Preflight should return `ok:true` and `result.ready:true`. The example config contains an example.org sender, verified TLS and numeric loopback SMTP settings with no authentication. This is a **syntactic offline fixture**, not a configured real mailbox. No server must be running, and no connection is made. Explicit --home leaves your ordinary state selection unchanged.

`resume.fixture` begins with a PDF prefix so it exercises the CLI's byte-preservation checks. It is not a valid/rendered resume and must never be used as a real application attachment.

## Prepare once, then inspect

```sh
package_id="$(agent-mail --home "$demo_home" prepare \
  --to recipient@example.org --subject-file examples/demo/subject.txt \
  --body-file examples/demo/letter.txt --attach "$demo_home/resume.pdf" \
  --operation-key demo/application --json | jq -er '.result.package_id')"
agent-mail --home "$demo_home" inspect "$package_id" --json
```

Inspect returns the saved sender/recipient, exact subject/body, original attachment filename, sizes, hashes and frozen MIME digest with `integrity:"verified"`. Review this output privately. The operation key lets an identical prepare recover the same ID after lost output; changed inputs conflict. It does not authorize sending.

Edit the source letter only to create a new package. An existing package's text, signature, PDF and MIME never change when it is submitted. [HTML alternatives](json-contract.md#html-alternatives) must also be finalized before preparation.

## Validate without submission

```sh
agent-mail --home "$demo_home" send "$package_id" --dry-run --json \
  | jq '{ok, state: .result.state, dry_run: .result.dry_run, network_used: .result.network_used}'
agent-mail --home "$demo_home" status "$package_id" --json
```

Expected dry-run projection:

```json
{"ok":true,"state":"prepared","dry_run":true,"network_used":false}
```

Status still has an empty attempts array. There is no transmission to loopback or any real recipient. For a capture-only submission test, run the [local SMTP fixture](local-testing.md). Keep/delete only the synthetic directory you created; do not reuse it as your operational sending history.

## Connect a real account separately

Use a separate private operational configuration/state directory and the [configuration reference](configuration.md). Obtain SMTP/TLS/login requirements from the provider; an alias sender may differ from the primary login. [Preflight](preflight.md) checks local readiness; [doctor](cli.md) additionally opens/validates storage. Neither proves authentication or delivery.

For actual correspondence, prepare the real finalized inputs, inspect that package, run dry-run and obtain an explicit owner instruction for its ID. Only then follow the [authorized submission workflow](agent-workflow.md#submit-and-interpret-the-result). Do not send this demonstration package.
