#!/usr/bin/env python3
"""Offline compatibility check against an actual prior release binary."""
import argparse
import contextlib
import hashlib
import json
from pathlib import Path
import shutil
import sqlite3
import subprocess
import tempfile

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('old',type=Path)
parser.add_argument('new',type=Path)
parser.add_argument('--reports',action='store_true',help='Also preserve strict/header-correlated evidence; requires an old reader supporting these commands.')
options=parser.parse_args()
old,new=str(options.old.resolve()),str(options.new.resolve())
with tempfile.TemporaryDirectory(prefix='agent-mail-upgrade-') as directory:
    root = Path(directory)
    home = root / 'state'
    home.mkdir()
    (home / 'config.toml').write_text("sender='sender@example.org'\n")
    (root / 'subject').write_text('Synthetic upgrade test')
    (root / 'body').write_text('Synthetic letter')
    (root / 'resume.pdf').write_bytes(b'%PDF-1.4\nsynthetic\n%%EOF')

    def call(binary, state, *args, code=0):
        result = subprocess.run([binary, '--home', str(state), '--json', *args], capture_output=True, check=False, timeout=15)
        assert result.returncode == code, result.returncode
        assert not result.stderr
        value = json.loads(result.stdout)
        value.pop("next", None)  # Invocation guidance is not archived package/history data.
        return value

    ids = []
    for outcome in ['unknown', 'accepted', 'rejected']:
        dsn=['--dsn','required'] if options.reports and outcome=='unknown' else []
        prepared = call(old, home, 'prepare',*dsn, '--to', 'recipient@example.org', '--subject-file', str(root/'subject'), '--body-file', str(root/'body'), '--attach', str(root/'resume.pdf'))['result']
        pid = prepared['package_id']
        ids.append(pid)
        with contextlib.closing(sqlite3.connect(home/'mail.sqlite3')) as db, db:
            db.execute("INSERT INTO attempts VALUES(?,?,?, ?,?,?,?)", (outcome, pid, '2026-01-01T00:00:00+00:00', None if outcome=='unknown' else '2026-01-01T00:01:00+00:00', outcome, prepared['manifest']['message_id'], None))
        if options.reports and outcome in ('accepted','unknown'):
            if outcome=='unknown':
                raw=b'{"dsn_parameters":true}'
                with contextlib.closing(sqlite3.connect(home/'mail.sqlite3')) as db, db:
                    db.execute("INSERT INTO dsn_events VALUES('synthetic-event',?,'parameters_about_to_send','2026-01-01T00:00:00+00:00',?,?)",(outcome,raw,hashlib.sha256(raw).hexdigest()))
            envid=f'Original-Envelope-Id: {pid}.{outcome}\r\n' if outcome=='unknown' else ''
            original=(f'--x\r\nContent-Type: text/rfc822-headers\r\n\r\nMessage-ID: {prepared["manifest"]["message_id"]}\r\nFrom: sender@example.org\r\nTo: recipient@example.org\r\n\r\n') if outcome=='accepted' else ''
            report=("Content-Type: multipart/report; report-type=delivery-status; boundary=x\r\n\r\n"
                    "--x\r\nContent-Type: text/plain\r\n\r\nSynthetic notification\r\n"
                    "--x\r\nContent-Type: message/delivery-status\r\n\r\nReporting-MTA: dns; example.org\r\n"+envid+
                    "\r\nFinal-Recipient: rfc822; recipient@example.org\r\nAction: failed\r\nStatus: 5.0.0\r\nDiagnostic-Code: smtp; 550 synthetic\r\n\r\n"+original+"--x--\r\n")
            path=root/(outcome+'.eml');path.write_bytes(report.encode())
            flags=['--correlate-by-headers'] if outcome=='accepted' else []
            saved=call(old,home,'import-dsn',pid,'--attempt-id',outcome,'--file',str(path),*flags)['result']
            assert saved['report']['action']=='failed'
    before = [(call(old, home, 'inspect', pid), call(old, home, 'status', pid)) for pid in ids]
    backup = root/'backup'
    shutil.copytree(home, backup)
    for state in [home, backup]:
        for pid, (package, status) in zip(ids, before):
            assert call(new, state, 'inspect', pid) == package
            after = call(new, state, 'status', pid)
            if 'dsn' not in status['result']: after['result'].pop('dsn', None)
            if 'delivery_observation' not in status['result']: after['result'].pop('delivery_observation', None)
            assert after == status
        # A safe config allows send to reach its policy check; no socket is used.
        (state/'config.toml').write_text("sender='sender@example.org'\n[smtp]\nhost='smtp.example.invalid'\nport=465\nsecurity='tls'\n")
        for pid in ids[:2]:
            result = call(new, state, 'send', pid, code=5)
            assert result['error']['code'] == 'send_blocked'
    print('Prior-release upgrade and restored-copy compatibility passed; no network or mail.')
