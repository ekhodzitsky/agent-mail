#!/usr/bin/env python3
"""Synthetic old/new runtime check in an unpacked rootfs via local PRoot.
Not a security sandbox: /proc and /dev/null expose the Linux runtime ABI.
"""
import contextlib
import json
import os
from pathlib import Path
import shutil
import sqlite3
import subprocess
import sys
import tempfile

proot, root = [str(Path(p).resolve()) for p in sys.argv[1:]]
base = Path(root)/'work'
work = Path(tempfile.mkdtemp(prefix='case-',dir=base))
guest='/work/'+work.name
for name in ['old','new']:
    shutil.copy2(base/name,work/name)
state = work/'state'
state.mkdir(exist_ok=False)
(state/'config.toml').write_text("sender='sender@example.org'\n")
(work/'subject').write_text('Synthetic clean Linux check')
(work/'body').write_text('Synthetic letter')
(work/'resume.pdf').write_bytes(b'%PDF-1.4\nsynthetic\n%%EOF')
env={'PATH':'/usr/bin:/bin','PROOT_NO_SECCOMP':'1'}
def call(binary, home, *args, code=0):
    command=[proot,'-r',root,'-b','/proc','-b','/dev/null','-w','/', '/usr/bin/env','-i','PATH=/usr/bin:/bin','HOME=/root',binary,'--home',home,'--json',*args]
    result=subprocess.run(command,env=env,capture_output=True,timeout=15)
    assert result.returncode==code,(result.returncode,result.stderr.decode())
    assert not result.stderr,result.stderr.decode()
    value = json.loads(result.stdout)
    value.pop("next", None)  # Invocation guidance is not archived package/history data.
    return value
# Install a prior release then replace it with the candidate in the clean home.
installed=Path(root)/'usr/local/bin/agent-mailer'
shutil.copy2(work/'old',installed)
ids=[]
for outcome in ['accepted','unknown']:
    p=call('/usr/local/bin/agent-mailer',guest+'/state','prepare','--to','recipient@example.org','--subject-file',guest+'/subject','--body-file',guest+'/body','--attach',guest+'/resume.pdf')['result']
    ids.append(p['package_id'])
    with contextlib.closing(sqlite3.connect(state/'mail.sqlite3')) as db, db:
        db.execute('INSERT INTO attempts VALUES(?,?,?,?,?,?,?)',(outcome,p['package_id'],'2026-01-01T00:00:00+00:00',None if outcome=='unknown' else '2026-01-01T00:01:00+00:00',outcome,p['manifest']['message_id'],None))
before=[(call('/usr/local/bin/agent-mailer',guest+'/state','inspect',i),call('/usr/local/bin/agent-mailer',guest+'/state','status',i)) for i in ids]
shutil.copytree(state,work/'restored')
shutil.copy2(work/'new',installed)
for home in ['state','restored']:
    for pid,(inspect,status) in zip(ids,before):
        assert call('/usr/local/bin/agent-mailer',guest+'/'+home,'inspect',pid)==inspect
        after=call('/usr/local/bin/agent-mailer',guest+'/'+home,'status',pid)
        if 'delivery_observation' not in status['result']: after['result'].pop('delivery_observation',None)
        if 'dsn' not in status['result']: after['result'].pop('dsn',None)
        assert after==status
        (work/home/'config.toml').write_text("sender='sender@example.org'\n[smtp]\nhost='smtp.example.invalid'\nport=465\nsecurity='tls'\n")
        assert call('/usr/local/bin/agent-mailer',guest+'/'+home,'send',pid,code=5)['error']['code']=='send_blocked'
print('Clean Ubuntu rootfs install/upgrade/restore passed. Synthetic accepted/unknown packages remain blocked; no network.')
