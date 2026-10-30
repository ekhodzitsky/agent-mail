#!/usr/bin/env python3
"""Build a native Linux artifact with normalized source paths; never access mail."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tomllib

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--target-dir', type=Path)
args = parser.parse_args()
project = Path(__file__).resolve().parents[1]
output = args.output.absolute()
target = (args.target_dir or project / 'target/distribution').resolve()
if not sys.platform.startswith('linux'):
    parser.error('Distribution builds currently support native Linux only')
if output.exists() or output.is_symlink():
    parser.error('Output must not already exist')
if any(os.environ.get(name) for name in ('RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'CARGO_BUILD_TARGET')):
    parser.error('Use an explicit clean native build environment without Rust flag/target overrides')
home = Path.home().resolve()
roots = [(home, '/build-home'),
         (Path(os.environ.get('CARGO_HOME', home / '.cargo')).resolve(), '/cargo'),
         (Path(os.environ.get('RUSTUP_HOME', home / '.rustup')).resolve(), '/rustup'),
         (target, '/target'), (project, '/agent-mail')]
env = os.environ.copy()
env['CARGO_ENCODED_RUSTFLAGS'] = '\x1f'.join(f'--remap-path-prefix={path}={replacement}' for path, replacement in roots)
env['CARGO_PROFILE_RELEASE_STRIP'] = 'symbols'
env['CARGO_PROFILE_RELEASE_DEBUG'] = '0'
env['CARGO_PROFILE_RELEASE_INCREMENTAL'] = 'false'
version_info = subprocess.check_output(['rustc', '-vV'], cwd=project, env=env, text=True)
host = next((line.removeprefix('host: ') for line in version_info.splitlines() if line.startswith('host: ')), '')
if host != 'x86_64-unknown-linux-gnu':
    parser.error('Official distribution currently supports native x86_64-unknown-linux-gnu only')
subprocess.run(['cargo', 'build', '--release', '--locked', '--target', host, '--target-dir', str(target)],
               cwd=project, env=env, check=True)
package = tomllib.loads((project / 'Cargo.toml').read_text())['package']
binary = target / host / 'release' / package['name']
raw = binary.read_bytes()
# Fail rather than patch binary bytes or silently ship local path metadata.
markers = {os.fsencode(path) for path, _ in roots}
if any(marker in raw for marker in markers):
    raise SystemExit('Distribution artifact still contains a local build-path marker')
if subprocess.check_output([str(binary), '--version'], env={'PATH': '/usr/bin:/bin'}, text=True).strip() != f"{package['name']} {package['version']}":
    raise SystemExit('Artifact version does not match source')
description = json.loads(subprocess.check_output([str(binary), 'describe', '--json'], env={'PATH': '/usr/bin:/bin'}))
if description['result']['test_smtp_enabled'] is not False:
    raise SystemExit('Test SMTP transport must not be distributed')
output.parent.mkdir(parents=True, exist_ok=True)
# Exclusive publication avoids accidentally replacing an installed binary/state.
fd = os.open(output, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o755)
try:
    with os.fdopen(fd, 'wb') as destination, binary.open('rb') as source:
        shutil.copyfileobj(source, destination)
        destination.flush()
        os.fsync(destination.fileno())
except BaseException:
    output.unlink()
    raise
fd = os.open(output.parent, os.O_RDONLY | os.O_DIRECTORY)
try:
    os.fsync(fd)
finally:
    os.close(fd)
print(json.dumps({'version': package['version'], 'sha256': hashlib.sha256(raw).hexdigest(),
                  'bytes': len(raw), 'target': host, 'local_path_markers': False, 'symbols_stripped': True}))
