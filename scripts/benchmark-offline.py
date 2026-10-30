#!/usr/bin/env python3
"""Measure synthetic prepare/inspect/dry-run subprocesses without sending mail."""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import statistics
import subprocess
import tempfile
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--packages', type=int, default=20, choices=range(1, 201), metavar='1..200')
    parser.add_argument('--runs', type=int, default=3, choices=range(2, 11), metavar='2..10')
    args = parser.parse_args()
    binary = str(args.binary.resolve(strict=True))
    env = {'PATH': '/usr/bin:/bin', 'LANG': 'C'}
    version = subprocess.check_output([binary, '--version'], env=env, text=True, timeout=15).strip()
    samples = {name: [] for name in ('prepare', 'inspect', 'dry_run')}
    pdf = b'%PDF-1.4\n' + b'Synthetic fixture\n' * 240 + b'%%EOF\n'
    body = 'Synthetic application.\nRegards, Test\n'

    def call(home, *argv):
        result = subprocess.run([binary, '--home', str(home), *argv, '--json'], env=env,
                                capture_output=True, timeout=30, check=True)
        output = json.loads(result.stdout)
        if result.stderr or not output.get('ok'):
            raise RuntimeError('Synthetic workload failed')
        return output['result']

    for _ in range(args.runs):
        with tempfile.TemporaryDirectory(prefix='agent-mail-benchmark-') as directory:
            home = Path(directory)
            (home / 'config.toml').write_text('sender = "sender@example.org"\n[smtp]\nhost = "127.0.0.1"\nport = 465\nsecurity = "tls"\n')
            (home / 'subject.txt').write_text('Synthetic application')
            (home / 'letter.txt').write_text(body)
            (home / 'resume.pdf').write_bytes(pdf)
            ids = []
            start = time.perf_counter()
            for _ in range(args.packages):
                prepared = call(home, 'prepare', '--to', 'recipient@example.org',
                                '--subject-file', str(home / 'subject.txt'), '--body-file', str(home / 'letter.txt'),
                                '--attach', str(home / 'resume.pdf'))
                ids.append(prepared['package_id'])
            samples['prepare'].append((time.perf_counter() - start) * 1000 / args.packages)
            start = time.perf_counter()
            for package in ids:
                call(home, 'inspect', package)
            samples['inspect'].append((time.perf_counter() - start) * 1000 / args.packages)
            before = hashlib.sha256((home / 'mail.sqlite3').read_bytes()).digest()
            start = time.perf_counter()
            for package in ids:
                result = call(home, 'send', package, '--dry-run')
                if result.get('network_used') is not False:
                    raise RuntimeError('Dry-run contract failed')
            samples['dry_run'].append((time.perf_counter() - start) * 1000 / args.packages)
            if before != hashlib.sha256((home / 'mail.sqlite3').read_bytes()).digest():
                raise RuntimeError('Dry-run changed the synthetic archive')
    print(json.dumps({'schema_version': 1, 'version': version, 'binary_sha256': hashlib.sha256(Path(binary).read_bytes()).hexdigest(), 'platform': platform.system(),
                      'architecture': platform.machine(), 'python': platform.python_version(),
                      'packages_per_run': args.packages, 'runs': args.runs, 'pdf_bytes': len(pdf),
                      'body_bytes': len(body.encode()), 'dry_run_archive_unchanged': True,
                      'milliseconds_per_command': {name: {'samples': values, 'median': statistics.median(values),
                                                         'min': min(values), 'max': max(values)}
                                                   for name, values in samples.items()}}, indent=2))


if __name__ == '__main__':
    main()
