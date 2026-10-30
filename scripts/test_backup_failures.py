"""POSIX quota fault on synthetic state; never fill the shared filesystem."""
import hashlib
import json
import os
from pathlib import Path
import resource
import signal
import subprocess
import tempfile
import unittest


class BackupQuotaTests(unittest.TestCase):
    def test_real_file_size_quota_fails_without_manifest_and_preserves_source(self):
        binary = str(Path(os.environ.get('AGENT_MAIL_TEST_BIN', 'target/debug/agent-mail')).resolve())
        with tempfile.TemporaryDirectory(prefix='agent-mail-backup-quota-') as directory:
            root = Path(directory)
            (root / 'config.toml').write_text('sender="sender@example.org"\n')
            (root / 'subject').write_text('Synthetic quota fault')
            (root / 'body').write_text('Synthetic letter')
            (root / 'resume.pdf').write_bytes(b'%PDF-1.4\n' + b'x' * (2 * 1024 * 1024))
            def call(*args, **kwargs):
                p = subprocess.run([binary, '--home', directory, '--json', *args],
                                   env={'PATH': '/usr/bin:/bin'}, capture_output=True, timeout=20, **kwargs)
                self.assertFalse(p.stderr)
                return p.returncode, json.loads(p.stdout)
            code, value = call('prepare', '--to', 'recipient@example.org', '--subject-file', str(root/'subject'),
                               '--body-file', str(root/'body'), '--attach', str(root/'resume.pdf'))
            self.assertEqual(code, 0)
            package = value['result']['package_id']
            before = hashlib.sha256((root / 'mail.sqlite3').read_bytes()).digest()
            def quota():
                # Test-only child setup; EFBIG/SQLITE_IOERR is distinct from
                # ENOSPC. Never change the parent's quota or fill a real disk.
                signal.signal(signal.SIGXFSZ, signal.SIG_IGN)
                resource.setrlimit(resource.RLIMIT_FSIZE, (262144, 262144))
            output = root / 'partial'
            code, value = call('backup', '--output', str(output), preexec_fn=quota)
            self.assertEqual(code, 7)
            self.assertEqual(value['error']['code'], 'storage')
            self.assertFalse((output/'backup.json').exists())
            self.assertEqual(hashlib.sha256((root/'mail.sqlite3').read_bytes()).digest(), before)
            self.assertEqual(call('status', package)[1]['result']['state'], 'prepared')
            self.assertNotEqual(call('verify-backup', str(output))[0], 0)
            # An independent new output succeeds after the constrained child
            # exits; leftover partial files are never silently overwritten.
            complete = root/'complete'
            self.assertEqual(call('backup', '--output', str(complete))[0], 0)
            self.assertEqual(call('verify-backup', str(complete))[0], 0)


if __name__ == '__main__':
    unittest.main()
