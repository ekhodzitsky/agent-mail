"""Synthetic documentation drift checks; no network or mailbox access."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('check_docs', Path(__file__).with_name('check-docs.py'))
docs = importlib.util.module_from_spec(spec)
spec.loader.exec_module(docs)


class DocumentationTests(unittest.TestCase):
    def test_current_contract_drift_is_reported(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'docs').mkdir()
            (root / 'docs/cli.md').write_text('| `describe --json` | offline |\n')
            (root / 'docs/storage-migrations.md').write_text('# Storage schema 6\n`PRAGMA user_version = 6`\n')
            (root / 'docs/json-contract.md').write_text('Current storage schema: **6**.\n')
            current = {'storage_schema_version': 6, 'commands': [{'name': 'describe'}, {'name': 'help'}]}
            self.assertEqual(docs.check_contracts(root, current), [])
            (root / 'docs/json-contract.md').write_text('Current storage schema: **4**.\n')
            self.assertTrue(any('json-contract' in error for error in docs.check_contracts(root, current)))
            (root / 'docs/cli.md').write_text('| `send ID` | submit |\n')
            self.assertTrue(any('command table' in error for error in docs.check_contracts(root, current)))

    def test_missing_file_and_heading_are_detected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name in ('README.md', 'AGENTS.md', 'CHANGELOG.md', 'VERSIONING.md',
                         'CONTRIBUTING.md', 'SECURITY.md', 'SUPPORT.md'):
                (root / name).write_text('# Guide\n')
            (root / 'README.md').write_text('[good](AGENTS.md#guide)\n[bad](missing.md)\n[bad heading](SUPPORT.md#absent)\n')
            errors = docs.check_links(root)
            self.assertEqual(len(errors), 2)
            self.assertTrue(any('heading' in error for error in errors))

    def test_fenced_examples_are_not_document_headings_or_links(self):
        self.assertEqual(docs.anchors('# Actual\n```md\n# Example\n```\n'), {'actual'})


if __name__ == '__main__':
    unittest.main()
