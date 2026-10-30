#!/usr/bin/env bash
# Local release gate; no hosted jobs and no real SMTP connections.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
unset AGENT_MAILER_TEST_BIN
msrv="${AGENT_MAILER_MSRV_TOOLCHAIN:-1.88}"
run() { printf '\n+ '; printf '%q ' "$@"; printf '\n'; "$@"; }
run /usr/bin/python3 - "$msrv" <<'PYTHON'
import subprocess
import sys
import tomllib
with open("Cargo.toml", "rb") as source:
    declared = tomllib.load(source)["package"]["rust-version"].split(".")
expected = ".".join(declared + ["0"] * (3 - len(declared)))
actual = subprocess.check_output(["rustc", "+" + sys.argv[1], "--version"], text=True).split()[1]
if actual != expected:
    raise SystemExit("Selected MSRV toolchain does not match Cargo.toml rust-version")
print("Verified MSRV:", actual)
PYTHON
source_has_git=false
if git_root="$(git rev-parse --show-toplevel 2>/dev/null)" && [[ "$git_root" == "$(pwd -P)" ]]; then
    source_has_git=true
    if git rev-parse --verify HEAD >/dev/null 2>&1; then
        run git rev-parse HEAD
    else
        printf 'Initial source repository: no commit yet; running all code checks.\n'
    fi
    run git status --short
else
    printf 'Source archive: Git revision/status unavailable; running all code checks.\n'
fi
run cargo +stable fmt --check
for toolchain in stable "$msrv"; do
    run rustc +"$toolchain" -Vv
    run cargo +"$toolchain" clippy --locked --all-targets -- -D warnings
    run cargo +"$toolchain" clippy --locked --all-targets --features test-smtp -- -D warnings
    run cargo +"$toolchain" test --locked
    run cargo +"$toolchain" test --locked --features test-smtp
done
run /usr/bin/python3 -B -m unittest discover -s integrations/gnome-keyring -p 'test_*.py'
run /usr/bin/python3 -B -m unittest discover -s integrations/imap-dsn -p 'test_*.py'
run /usr/bin/python3 -B -m unittest discover -s scripts -p 'test_*.py'
run cargo audit
run cargo deny --locked --all-features check advisories licenses sources
install_root="$(mktemp -d)"
trap 'rm -rf -- "$install_root"' EXIT
# Separate install roots ensure the test transport never becomes a shipped artifact.
run cargo +stable install --path . --locked --features test-smtp --root "$install_root/test" --target-dir target
run env AGENT_MAILER_TEST_BIN="$install_root/test/bin/agent-mailer" cargo +stable test --locked --features test-smtp --test cli smtp::vertical_scenario_and_duplicate_block -- --exact
run env AGENT_MAILER_TEST_BIN="$install_root/test/bin/agent-mailer" cargo +stable test --locked --features test-smtp --test cli smtp::dsn
run cargo +stable install --path . --locked --root "$install_root/production" --target-dir target
run env -i "$install_root/production/bin/agent-mailer" --version
run env -i "$install_root/production/bin/agent-mailer" --help
run env AGENT_MAILER_TEST_BIN="$install_root/production/bin/agent-mailer" cargo +stable test --locked --test cli
run env AGENT_MAILER_TEST_BIN="$install_root/production/bin/agent-mailer" /usr/bin/python3 -B -m unittest discover -s integrations/imap-dsn -p 'test_*.py'
run env AGENT_MAILER_TEST_BIN="$install_root/production/bin/agent-mailer" /usr/bin/python3 -B -m unittest discover -s scripts -p test_backup_failures.py
run /usr/bin/python3 -B scripts/check-docs.py --binary "$install_root/production/bin/agent-mailer"
if "$source_has_git"; then run git diff --check; fi
printf '\nLocal verification passed. Production binary: target/release/agent-mailer\n'
