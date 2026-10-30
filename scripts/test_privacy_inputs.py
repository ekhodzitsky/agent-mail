"""Synthetic disclosure-scanner boundary checks; no owner identifiers."""
import os
from pathlib import Path
import tempfile
import unittest

from privacy_inputs import load_markers, private_identifier_content


class PrivacyInputsTests(unittest.TestCase):
    def test_private_marker_permissions_links_and_bound(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'markers'
            path.write_bytes(b' Synthetic-Private-Marker \n\n')
            path.chmod(0o600)
            self.assertEqual(load_markers(path), (b'synthetic-private-marker',))
            self.assertTrue(private_identifier_content(b'SYNTHETIC-PRIVATE-MARKER', load_markers(path)))
            path.chmod(0o644)
            with self.assertRaises(ValueError):
                load_markers(path)
            path.chmod(0o600)
            link = Path(directory) / 'link'
            link.symlink_to(path)
            with self.assertRaises(OSError):
                load_markers(link)
            link.unlink()
            os.link(path, link)
            with self.assertRaises(ValueError):
                load_markers(path)
            link.unlink()
            path.write_bytes(b'x' * 65537)
            with self.assertRaises(ValueError):
                load_markers(path)

    def test_examples_are_allowed_other_addresses_and_home_paths_flagged(self):
        for raw in (b'a@example.org', b'a@example.com', b'a@example.net', b'a@fixture.invalid', b'a@local.test'):
            self.assertFalse(private_identifier_content(raw), raw)
        # Assemble synthetic non-example values to avoid including them as scanner findings.
        self.assertTrue(private_identifier_content(b'a@' + b'mail-provider' + b'.org'))
        self.assertTrue(private_identifier_content(b'"' + b'/home/' + b'synthetic-user/project'))
        self.assertFalse(private_identifier_content(b'ordinary source code'))


if __name__ == '__main__':
    unittest.main()
