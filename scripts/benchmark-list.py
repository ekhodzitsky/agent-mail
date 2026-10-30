#!/usr/bin/env python3
"""Compare synthetic whole-archive listing and opt-in bounded pages; no mail."""
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
    parser.add_argument('--old', required=True, type=Path)
    parser.add_argument('--new', required=True, type=Path)
    parser.add_argument('--packages', type=int, default=300, choices=range(25, 1001), metavar='25..1000')
    parser.add_argument('--runs', type=int, default=3, choices=range(2, 11), metavar='2..10')
    args = parser.parse_args()
    old, new = [str(p.resolve(strict=True)) for p in (args.old, args.new)]
    env = {'PATH': '/usr/bin:/bin', 'LANG': 'C'}
    versions = {name: subprocess.check_output([binary, '--version'], env=env, text=True).strip()
                for name, binary in [('old', old), ('new', new)]}
    results = {name: [] for name in ('old_full', 'new_full', 'new_page_25', 'new_ids_25')}
    with tempfile.TemporaryDirectory(prefix='agent-mailer-list-benchmark-') as directory:
        root = Path(directory)
        (root / 'config.toml').write_text('sender="sender@example.org"\n')
        (root / 'subject').write_text('Synthetic application')
        (root / 'body').write_text('Synthetic letter')
        (root / 'resume.pdf').write_bytes(b'%PDF-1.4\n' + b's' * (262144 - 15) + b'%%EOF\n')
        for _ in range(args.packages):
            p = subprocess.run([old, '--home', directory, '--json', 'prepare', '--to', 'recipient@example.org',
                                '--subject-file', str(root / 'subject'), '--body-file', str(root / 'body'),
                                '--attach', str(root / 'resume.pdf')], env=env, capture_output=True, timeout=30, check=True)
            if p.stderr or not json.loads(p.stdout)['ok']:
                raise RuntimeError('Synthetic preparation failed')
        before = hashlib.sha256((root / 'mail.sqlite3').read_bytes()).digest()
        expected = None
        for _ in range(args.runs):
            for name, binary, flags in [('old_full', old, []), ('new_full', new, []),
                                        ('new_page_25', new, ['--limit', '25']),
                                        ('new_ids_25', new, ['--limit', '25', '--summary'])]:
                start = time.perf_counter()
                p = subprocess.run(['/usr/bin/time', '-f', '%M', '-o', str(root / 'rss'),
                                    binary, '--home', directory, '--json', 'list', *flags],
                                   env=env, capture_output=True, timeout=120, check=True)
                elapsed = (time.perf_counter() - start) * 1000
                value = json.loads(p.stdout)
                if p.stderr or not value['ok']:
                    raise RuntimeError('Synthetic listing failed')
                packages = value['result']['packages']
                if name == 'old_full':
                    if expected is not None and expected != packages:
                        raise RuntimeError('Unstable baseline')
                    expected = packages
                elif name == 'new_full' and packages != expected:
                    raise RuntimeError('Whole-archive output differs')
                elif name == 'new_page_25' and packages != expected[:25]:
                    raise RuntimeError('Verified page differs')
                elif name == 'new_ids_25' and packages != [
                        {'package_id': p['package_id'], 'integrity': 'not_checked'} for p in expected[:25]]:
                    raise RuntimeError('ID page differs')
                results[name].append({'ms': elapsed, 'peak_rss_kib': int((root / 'rss').read_text())})
        if before != hashlib.sha256((root / 'mail.sqlite3').read_bytes()).digest():
            raise RuntimeError('Listing changed synthetic archive')
    print(json.dumps({'schema_version': 1, 'versions': versions, 'platform': platform.system(),
                      'architecture': platform.machine(), 'packages': args.packages, 'pdf_bytes': 262144,
                      'runs': args.runs, 'archive_unchanged': True, 'full_results_equivalent': True,
                      'binary_sha256': {name: hashlib.sha256(Path(b).read_bytes()).hexdigest()
                                        for name, b in [('old', old), ('new', new)]},
                      'results': {name: {'samples': samples,
                                         'median_ms': statistics.median(p['ms'] for p in samples),
                                         'median_peak_rss_kib': statistics.median(p['peak_rss_kib'] for p in samples)}
                                  for name, samples in results.items()}}, indent=2))


if __name__ == '__main__':
    main()
