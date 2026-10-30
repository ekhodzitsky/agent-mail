"""Verify a pasted native-install recipe cannot install after transfer/hash errors."""
from pathlib import Path
import os
import re
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class DocumentedInstallTests(unittest.TestCase):
    def check_failure(self, checksum_failure):
        text = (ROOT/'docs/releases.md').read_text().split('## Install the native Linux asset', 1)[1]
        code = re.search(r'```sh\n(.*?)\n```', text, re.S)[1]
        name = re.search(r'-o "\$download_dir/(agent-mail-[^"]+)"', code)[1]
        with tempfile.TemporaryDirectory(prefix='agent-mail-install-doc-') as directory:
            home = Path(directory); tools = home/'tools'; tools.mkdir()
            if checksum_failure:
                fake_curl = ('#!/usr/bin/python3\nimport sys\nfrom pathlib import Path\n'
                             'p=Path(sys.argv[sys.argv.index("-o")+1])\n'
                             f'p.write_text("0"*64+"  {name}\\n" if p.name=="SHA256SUMS" else "synthetic wrong bytes")\n')
            else:
                fake_curl = '#!/bin/sh\nexit 28\n'
            (tools/'curl').write_text(fake_curl);(tools/'curl').chmod(0o700)
            (tools/'install').write_text('#!/bin/sh\ntouch "$INSTALL_CALLED"\n');(tools/'install').chmod(0o700)
            marker=home/'install-was-called'
            # Intentionally no `bash -e`: simulate pasting the whole recipe in
            # an interactive shell. Its own subshell must stop unsafe progress.
            p=subprocess.run(['bash','-c',code],env={'HOME':directory,'TMPDIR':directory,
                             'PATH':str(tools)+':/usr/bin:/bin','INSTALL_CALLED':str(marker)},
                             capture_output=True,timeout=10)
            self.assertNotEqual(p.returncode,0)
            self.assertFalse(marker.exists(),'Documented install proceeded after verification failure')
            self.assertFalse((home/'.local/bin/agent-mail').exists())

    def test_transfer_failure_never_reaches_install(self):
        self.check_failure(False)

    def test_hash_mismatch_never_reaches_install(self):
        self.check_failure(True)


if __name__ == '__main__':
    unittest.main()
