# Local SMTP walkthrough

```sh
cargo test --locked --features test-smtp smtp::vertical_scenario_and_duplicate_block -- --exact --nocapture
```

This runs the actual binary as separate processes with a temporary state directory, synthetic sender/recipient (`example.org`), synthetic letter/PDF, and a Rust fixture SMTP server bound to `127.0.0.1` on an ephemeral port. The test:

1. Executes prepare and records its package ID.
2. Executes inspect and verifies integrity.
3. Executes dry-run, asserting no DATA and no attempt.
4. Executes send through the loopback SMTP fixture.
5. Checks state accepted, compares received DATA to the frozen MIME bytes plus lettre's SMTP CRLF framing, and checks Message-ID.
6. Executes send again and verifies it is blocked with only one attempt.

No owner mailbox, credentials or real recipient is used. The fixture speaks only the minimum test-server protocol; production SMTP/TLS is implemented by lettre.

For exploratory manual use with your own local capture-only server, build `cargo build --features test-smtp`, then place this config in a temporary --home directory:

```toml
sender = "sender@example.org"
[smtp]
host = "127.0.0.1"
port = 2525
security = "loopback_test"
timeout_seconds = 5
```

Use `target/debug/agent-mail --home /your/temp/home …` for each command. Inspect the selected package before send. The server must be capture-only and must not relay mail. Prefer the included automated fixture; it cannot relay. Never use this development feature to configure a real mailbox.

The normal `cargo install --path . --locked` build excludes plaintext SMTP. Dry-run does not test a socket and will succeed with a syntactically valid configuration even when no server is listening.

TLS and storage-failure tests also run in the default build's unit suite:

```sh
cargo test --locked --lib tls_tests
cargo test --locked --lib storage_tests
```

These use ephemeral in-memory certificates and temporary databases. No real provider configuration, credentials, external recipients or OS certificate-store changes are required.
