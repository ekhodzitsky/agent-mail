#!/usr/bin/python3
"""Run under a fresh dbus-run-session with a temporary HOME, never owner keyring."""
import importlib.machinery
import importlib.util
import os
from pathlib import Path
import subprocess
import sys

root = Path.home()
assert (root / '.synthetic-keyring-test').exists(), 'Requires an explicitly isolated synthetic home'
assert os.environ.get('DBUS_SESSION_BUS_ADDRESS')
result = subprocess.run(['gnome-keyring-daemon','--unlock','--components=secrets'],input=b'synthetic-test-only\n',capture_output=True,timeout=10,check=False)
assert result.returncode == 0
loader = importlib.machinery.SourceFileLoader('adapter', str(Path(sys.argv[1]).resolve()))
spec = importlib.util.spec_from_loader(loader.name, loader)
adapter = importlib.util.module_from_spec(spec)
loader.exec_module(adapter)
Secret, schema = adapter.secret_api()
assert Secret.password_store_sync(schema, adapter.ATTRS, Secret.COLLECTION_DEFAULT, 'Synthetic isolated test', 'synthetic-password', None)
assert adapter.retrieve(Secret,schema) == 'synthetic-password'
service = Secret.Service.get_sync(Secret.ServiceFlags.OPEN_SESSION,None)
collection = Secret.Collection.for_alias_sync(service,'default',Secret.CollectionFlags.NONE,None)
assert collection is not None
service.lock_sync([collection],None)
try:
    adapter.retrieve(Secret,schema)
except ValueError:
    pass
else:
    raise AssertionError('Locked keyring must fail without prompting')
print('Isolated real Secret Service: unlocked lookup and locked refusal passed; no owner data accessed.')
