#!/usr/bin/env python3
"""Inventory reachable Git blobs without printing personal identifiers/secrets."""
import argparse
import json
from pathlib import Path
import re
import subprocess

from privacy_inputs import default_marker_file, load_markers, private_identifier_content

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--private-markers',type=Path,default=default_marker_file())
options=parser.parse_args()
markers=load_markers(options.private_markers)
seen=set()
personal_paths=set()
forbidden=set()
binary=set()
for entry in subprocess.check_output(['git','rev-list','--objects','--all'],text=True).splitlines():
    oid, _, path = entry.partition(' ')
    if not path or oid in seen:
        continue
    if subprocess.check_output(['git','cat-file','-t',oid],text=True).strip() != 'blob':
        continue
    seen.add(oid)
    raw=subprocess.check_output(['git','cat-file','blob',oid])
    if private_identifier_content(raw,markers):
        personal_paths.add(path)
    if raw.startswith((b'%PDF-', b'PK\x03\x04', b'SQLite format', b'\x7fELF')):
        binary.add(path)
    if re.search(r'(^|/)(\.env($|\.)|\.local/|\.serena/|\.codex/|mail\.sqlite|resume\.pdf)',path):
        forbidden.add(path)
print(json.dumps({'unique_blobs':len(seen),'personal_identifier_paths':sorted(personal_paths),
                  'binary_document_state_paths':sorted(binary),'forbidden_paths':sorted(forbidden)},indent=2))
