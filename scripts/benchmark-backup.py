#!/usr/bin/env python3
"""Bounded synthetic backup/verify measurement on an explicitly chosen filesystem."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import statistics
import subprocess
import tempfile
import time


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--work-parent', required=True, type=Path)
    parser.add_argument('--packages', type=int, default=500, choices=range(25, 1001), metavar='25..1000')
    parser.add_argument('--runs', type=int, default=3, choices=range(2, 6), metavar='2..5')
    args = parser.parse_args()
    binary = str(args.binary.resolve(strict=True))
    if shutil.disk_usage(args.work_parent).free < 4 * args.packages * 262144 + 512 * 1024 * 1024:
        raise SystemExit('Insufficient free space for bounded synthetic workload')
    env = {'PATH': '/usr/bin:/bin', 'LANG': 'C'}
    results = {'backup': [], 'verify': []}
    with tempfile.TemporaryDirectory(prefix='agent-mailer-backup-benchmark-', dir=args.work_parent.resolve()) as directory:
        root = Path(directory); home = root/'source'; home.mkdir(mode=0o700)
        (home/'config.toml').write_text('sender="sender@example.org"\n')
        (root/'subject').write_text('Synthetic disk backup')
        (root/'body').write_text('Synthetic letter')
        (root/'resume.pdf').write_bytes(b'%PDF-1.4\n' + b'x' * (262144 - 9))
        filesystem = subprocess.check_output(['findmnt', '-n', '-o', 'FSTYPE', '--target', directory], env=env, text=True).strip()
        version = subprocess.check_output([binary, '--version'], env=env, text=True).strip()
        def call(*argv, measured=None):
            command = [binary, '--home', str(home), '--json', *argv]
            if measured:
                command = ['/usr/bin/time', '-f', '%M', '-o', str(root/'rss'), *command]
            start = time.perf_counter()
            p = subprocess.run(command, env=env, capture_output=True, check=True, timeout=120)
            duration = (time.perf_counter() - start) * 1000
            value = json.loads(p.stdout)
            if p.stderr or not value['ok']:
                raise RuntimeError('Synthetic backup benchmark failed')
            if measured:
                results[measured].append({'ms': duration, 'peak_rss_kib': int((root/'rss').read_text())})
            return value['result']
        for _ in range(args.packages):
            call('prepare', '--to', 'recipient@example.org', '--subject-file', str(root/'subject'),
                 '--body-file', str(root/'body'), '--attach', str(root/'resume.pdf'))
        before = digest(home/'mail.sqlite3'); source_bytes = (home/'mail.sqlite3').stat().st_size
        for number in range(args.runs):
            output = root/f'snapshot-{number}'
            created = call('backup', '--output', str(output), measured='backup')
            verified = call('verify-backup', str(output), measured='verify')
            for value in (created, verified):
                if value['integrity'] != 'verified' or value['network_used'] or value['retry_authorized'] or value['freshness'] != 'not_established':
                    raise RuntimeError('Snapshot contract mismatch')
            shutil.rmtree(output)  # Own synthetic snapshot only; one copy at a time.
        if before != digest(home/'mail.sqlite3'):
            raise RuntimeError('Benchmark changed source archive')
    print(json.dumps({'schema_version': 1, 'version': version, 'binary_sha256': digest(Path(binary)),
                      'filesystem': filesystem, 'packages': args.packages, 'pdf_bytes': 262144,
                      'source_database_bytes': source_bytes, 'runs': args.runs,
                      'source_unchanged': True, 'network_used': False,
                      'results': {name: {'samples': values, 'median_ms': statistics.median(v['ms'] for v in values),
                                         'median_peak_rss_kib': statistics.median(v['peak_rss_kib'] for v in values)}
                                  for name, values in results.items()}}, indent=2))


if __name__ == '__main__':
    main()
