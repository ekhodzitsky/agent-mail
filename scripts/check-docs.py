#!/usr/bin/env python3
"""Check local Markdown links and documented contracts against offline discovery."""
import argparse
import json
from pathlib import Path
import re
import subprocess
import sys
from urllib.parse import unquote, urlsplit


def markdown(text):
    return re.sub(r'^```[^\n]*\n.*?^```\s*$', '', text, flags=re.MULTILINE | re.DOTALL)


def anchors(text):
    seen = {}
    result = set()
    for title in re.findall(r'^#{1,6}\s+(.+?)\s*#*\s*$', markdown(text), re.MULTILINE):
        base = re.sub(r'[^\w\s-]', '', title.lower()).replace(' ', '-')
        count = seen.get(base, 0)
        seen[base] = count + 1
        result.add(base + (f'-{count}' if count else ''))
    return result


def check_links(root):
    errors = []
    paths = [root / name for name in ('README.md', 'AGENTS.md', 'CHANGELOG.md', 'VERSIONING.md',
                                     'CONTRIBUTING.md', 'SECURITY.md', 'SUPPORT.md')]
    for folder in ('docs', 'agents', 'integrations', 'skills'):
        paths.extend((root / folder).rglob('*.md'))
    for path in paths:
        if not path.is_file():
            errors.append(f'{path.relative_to(root)}: missing document')
            continue
        for target in re.findall(r'\]\(([^)\s]+)\)', markdown(path.read_text())):
            value = urlsplit(target)
            if value.scheme or value.netloc:
                continue
            destination = path.parent / unquote(value.path) if value.path else path
            if not destination.exists():
                errors.append(f'{path.relative_to(root)}: missing local link target')
            elif value.fragment and destination.suffix == '.md' and unquote(value.fragment) not in anchors(destination.read_text()):
                errors.append(f'{path.relative_to(root)}: missing local heading target')
    return errors


def check_contracts(root, description):
    errors = []
    version = description['storage_schema_version']
    storage = (root / 'docs/storage-migrations.md').read_text()
    contract = (root / 'docs/json-contract.md').read_text()
    if not storage.startswith(f'# Storage schema {version}\n'):
        errors.append('docs/storage-migrations.md: current storage heading differs from describe')
    if f'`PRAGMA user_version = {version}`' not in storage:
        errors.append('docs/storage-migrations.md: storage identity differs from describe')
    if f'Current storage schema: **{version}**.' not in contract:
        errors.append('docs/json-contract.md: current storage schema differs from describe')
    documented = set()
    for row in (root / 'docs/cli.md').read_text().splitlines():
        if row.startswith('| `'):
            cell = row.split('|')[1]
            documented.update(value.split()[0] for value in re.findall(r'`([^`]+)`', cell))
    commands = {command['name'] for command in description['commands']} - {'help'}
    if documented != commands:
        errors.append('docs/cli.md: command table differs from describe')
    return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    # describe is stateless; a minimal environment avoids owner configuration.
    result = subprocess.run([str(args.binary.resolve()), 'describe', '--json'],
                            env={'PATH': '/usr/bin:/bin'}, capture_output=True, timeout=15, check=True)
    value = json.loads(result.stdout)
    if result.stderr or not value.get('ok') or value.get('schema_version') != 1:
        raise SystemExit('Offline discovery failed')
    errors = check_links(root) + check_contracts(root, value['result'])
    if errors:
        print('\n'.join(errors), file=sys.stderr)
        return 1
    print('Documentation links/headings, current storage schema and command table match offline discovery.')
    return 0


if __name__ == '__main__':
    sys.exit(main())
