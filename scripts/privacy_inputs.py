"""Generic disclosure heuristics; owner-specific markers stay outside Git."""
import os
from pathlib import Path
import re
import stat


def load_markers(path):
    if path is None:
        return ()
    fd = os.open(path, os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW)
    with os.fdopen(fd, 'rb') as source:
        info = os.fstat(source.fileno())
        if not stat.S_ISREG(info.st_mode) or info.st_mode & 0o077 or info.st_nlink != 1:
            raise ValueError('Private marker file must be a private regular single-link file')
        raw = source.read(65537)
        if len(raw) > 65536:
            raise ValueError('Private marker file exceeds limit')
    return tuple(line.strip().lower() for line in raw.splitlines() if line.strip())


def default_marker_file():
    value = os.environ.get('AGENT_MAILER_AUDIT_MARKERS_FILE')
    if value:
        return Path(value)
    path = Path.home() / '.local/share/agent-mailer-audit/private-markers.txt'
    return path if path.exists() else None


def private_identifier_content(raw, markers=()):
    lowered = raw.lower()
    if any(marker in lowered for marker in markers):
        return True
    if re.search(rb'["\x27]/home/[^/\s]+/', raw):
        return True
    domains = re.findall(rb"[A-Za-z0-9.!#$%&'*+/=?^_`{|}~-]+@([A-Za-z0-9.-]+)", raw)
    examples = {b'example.org', b'example.com', b'example.net', b'example', b'invalid', b'localhost'}
    return any(domain.lower().rstrip(b'.') not in examples
               and not domain.lower().rstrip(b'.').endswith((b'.invalid', b'.example', b'.test', b'.localhost'))
               for domain in domains)
