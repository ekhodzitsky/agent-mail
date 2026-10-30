"""Synthetic, verified-TLS IMAP tests. Never connect to a real mailbox."""
import contextlib
import hashlib
import importlib.machinery
import importlib.util
import json
import os
import signal
from pathlib import Path
import socket
import shutil
import sqlite3
import ssl
import subprocess
import sys
import tempfile
import threading
import time
import unittest
import uuid
from unittest.mock import patch, Mock

SCRIPT = Path(__file__).with_name("agent-mail-imap-dsn").resolve()
ROOT = SCRIPT.parents[2]
loader = importlib.machinery.SourceFileLoader("imap_adapter", str(SCRIPT))
spec = importlib.util.spec_from_loader(loader.name, loader)
adapter = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = adapter
loader.exec_module(adapter)


def setUpModule():
    global isolated_home, home_patch
    isolated_home = tempfile.TemporaryDirectory()
    home_patch = patch.object(Path, 'home', return_value=Path(isolated_home.name))
    home_patch.start()


def tearDownModule():
    home_patch.stop()
    isolated_home.cleanup()


class Server:
    def __init__(self, cert, key, raw, *, validity="77", wrong_uid=False,
                 oversized=False, huge_literal=False, stall=False, refuse_login=False, refuse_search=False, report_header=None, search_uids=b'42', uidnext=b'43'):
        self.raw, self.validity = raw, validity
        self.wrong_uid, self.oversized = wrong_uid, oversized
        self.huge_literal, self.stall = huge_literal, stall
        self.refuse_login = refuse_login
        self.refuse_search = refuse_search
        self.report_header = report_header if report_header is not None else b'Message-ID: <synthetic-report@example.org>\r\n\r\n'
        self.search_uids = search_uids
        self.uidnext = uidnext
        self.commands, self.errors = [], []
        self.stop = threading.Event()
        self.listener = socket.socket()
        self.listener.bind(("127.0.0.1", 0))
        self.listener.listen()
        self.listener.settimeout(0.1)
        self.port = self.listener.getsockname()[1]
        self.context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        self.context.load_cert_chain(cert, key)
        self.thread = threading.Thread(target=self.serve, daemon=True)
        self.thread.start()

    def serve(self):
        while not self.stop.is_set():
            try:
                raw, _ = self.listener.accept()
            except socket.timeout:
                continue
            except OSError:
                break
            try:
                with raw:
                    raw.settimeout(3)
                    with self.context.wrap_socket(raw, server_side=True) as wire:
                        if self.stall:
                            self.stop.wait(3)
                            continue
                        wire.sendall(b"* OK synthetic IMAP\r\n")
                        reader = wire.makefile("rb")
                        with reader:
                            while True:
                                line = reader.readline()
                                if not line:
                                    break
                                self.commands.append(line)
                                tag, command = line.rstrip(b"\r\n").split(b" ", 1)
                                if command == b"CAPABILITY":
                                    wire.sendall(b"* CAPABILITY IMAP4rev1\r\n")
                                elif command.startswith(b"LOGIN "):
                                    if self.refuse_login:
                                        wire.sendall(tag + b" NO synthetic-secret-only refused\r\n")
                                        continue
                                elif command.startswith(b"EXAMINE "):
                                    wire.sendall(b"* 1 EXISTS\r\n* OK [UIDVALIDITY " + self.validity.encode() + b"] synthetic\r\n* OK [UIDNEXT " + self.uidnext + b"] synthetic\r\n")
                                elif command.startswith(b"UID SEARCH "):
                                    if self.refuse_search:
                                        wire.sendall(tag + b" NO synthetic-secret-only search refused\r\n")
                                        continue
                                    wire.sendall(b"* SEARCH " + self.search_uids + b"\r\n")
                                elif command.startswith(b"UID FETCH "):
                                    uid = b"99" if self.wrong_uid else b"42"
                                    if b"RFC822.SIZE" in command:
                                        size = adapter.MAX_REPORT + 1 if self.oversized else len(self.raw)
                                        wire.sendall(b"* 1 FETCH (UID " + uid + b" RFC822.SIZE " + str(size).encode() + b")\r\n")
                                    elif b"BODY.PEEK[HEADER.FIELDS (MESSAGE-ID)]" in command:
                                        wire.sendall(b"* 1 FETCH (UID " + uid + b" BODY[HEADER.FIELDS (MESSAGE-ID)] {" + str(len(self.report_header)).encode() + b"}\r\n" + self.report_header + b")\r\n")
                                    elif b"BODY.PEEK[]" in command:
                                        size = adapter.MAX_REPORT + 1 if self.huge_literal else len(self.raw)
                                        wire.sendall(b"* 1 FETCH (UID " + uid + b" BODY[] {" + str(size).encode() + b"}\r\n")
                                        if self.huge_literal:
                                            break
                                        wire.sendall(self.raw + b")\r\n")
                                    else:
                                        raise AssertionError("Expected read-only PEEK fetch")
                                else:
                                    raise AssertionError("Unexpected mailbox operation: " + repr(command))
                                wire.sendall(tag + b" OK synthetic\r\n")
            except (OSError, ssl.SSLError):
                pass
            except Exception as error:
                self.errors.append(error)

    def close(self):
        self.stop.set()
        self.listener.close()
        self.thread.join(4)
        if self.thread.is_alive() or self.errors:
            raise AssertionError(self.errors or "Server did not exit")


class AdapterTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.certs = tempfile.TemporaryDirectory(prefix="agent-mail-imap-tls-")
        cls.cert = Path(cls.certs.name) / "ca.pem"
        cls.key = Path(cls.certs.name) / "key.pem"
        subprocess.run(["openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1",
                        "-subj", "/CN=synthetic", "-addext", "subjectAltName=IP:127.0.0.1",
                        "-keyout", str(cls.key), "-out", str(cls.cert)],
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=True, timeout=15)

    @classmethod
    def tearDownClass(cls):
        cls.certs.cleanup()

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="agent-mail-imap-case-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.home = self.root / "state"
        self.home.mkdir(mode=0o700)
        self.binary = os.environ.get("AGENT_MAIL_TEST_BIN", str(ROOT / "target/debug/agent-mail"))
        (self.home / "config.toml").write_text("sender='sender@example.org'\n")
        (self.root / "subject").write_text("Synthetic")
        (self.root / "body").write_text("Synthetic letter")
        (self.root / "resume.pdf").write_bytes(b"%PDF-1.4 synthetic")
        prepared = self.core("prepare", "--to", "recipient@example.org", "--dsn", "required",
                             "--subject-file", str(self.root / "subject"), "--body-file", str(self.root / "body"),
                             "--attach", str(self.root / "resume.pdf"))
        self.package, self.attempt = prepared["package_id"], str(uuid.uuid4())
        payload = b'{"dsn_parameters":true}'
        with contextlib.closing(sqlite3.connect(self.home / "mail.sqlite3")) as db, db:
            db.execute("INSERT INTO attempts VALUES(?,?,'2026-01-01T00:00:00+00:00','2026-01-01T00:00:01+00:00','accepted',?,'250')",
                       (self.attempt, self.package, prepared["manifest"]["message_id"]))
            db.execute("INSERT INTO dsn_events VALUES(?,?,'parameters_about_to_send','2026-01-01T00:00:00+00:00',?,?)",
                       (str(uuid.uuid4()), self.attempt, payload, hashlib.sha256(payload).hexdigest()))
        self.before = self.core("status", self.package)["attempts"]
        self.raw = ("Message-ID: <synthetic-report@example.org>\r\nContent-Type: multipart/report; report-type=delivery-status; boundary=x\r\n\r\n"
                    "--x\r\nContent-Type: text/plain\r\n\r\nSynthetic notification\r\n"
                    "--x\r\nContent-Type: message/delivery-status\r\n\r\nReporting-MTA: dns; example.org\r\n"
                    f"Original-Envelope-Id: {self.package}.{self.attempt}\r\n\r\n"
                    "Final-Recipient: rfc822; recipient@example.org\r\nAction: relayed\r\nStatus: 2.0.0\r\n\r\n--x--\r\n").encode()

    def core(self, *args):
        result = subprocess.run([self.binary, "--home", str(self.home), "--json", *args],
                                capture_output=True, check=False, timeout=10)
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stderr, b"")
        return json.loads(result.stdout)["result"]

    def run_adapter(self, server, *, trust=True, deadline=8, uid=None, dry_run=False, host="127.0.0.1", password=True, message_id=None, report_search=None, report_date=None, report_from=None, by_headers=False, expected_uidvalidity=None):
        cfg = self.root / "imap.toml"
        cfg.write_text(f'host={json.dumps(host)}\nport={server.port}\nusername="synthetic@example.org"\npassword_env="SYNTHETIC_IMAP_PASSWORD"\nhome={json.dumps(str(self.home))}\nbinary={json.dumps(self.binary)}\ntotal_timeout_seconds={deadline}\n')
        cfg.chmod(0o600)
        env = os.environ.copy()
        env.pop("SSL_CERT_FILE", None)
        env.pop("SSL_CERT_DIR", None)
        if trust:
            env["SSL_CERT_FILE"] = str(self.cert)
        env.pop("SYNTHETIC_IMAP_PASSWORD", None)
        if password:
            env["SYNTHETIC_IMAP_PASSWORD"] = "synthetic-secret-only"
        args = [sys.executable, "-B", str(SCRIPT), "--config", str(cfg), "--package-id", self.package,
                "--attempt-id", self.attempt, "--json"]
        if uid:
            args.extend(["--uid", uid])
        if expected_uidvalidity is not None:
            args.extend(["--expected-uidvalidity", expected_uidvalidity])
        if message_id:
            args.extend(["--report-message-id", message_id])
        if report_search:
            args.extend(["--report-search", report_search])
        if report_date:
            args.extend(["--report-date", report_date])
        if report_from:
            args.extend(["--report-from", report_from])
        if by_headers:
            args.append("--correlate-by-headers")
        if dry_run:
            args.append("--dry-run")
        result = subprocess.run(args, capture_output=True, env=env, timeout=deadline + 4, check=False)
        self.assertEqual(result.stderr, b"")
        self.assertNotIn(b"synthetic-secret-only", result.stdout)
        self.assertNotIn(b"Synthetic notification", result.stdout)
        return result.returncode, json.loads(result.stdout)

    @contextlib.contextmanager
    def server(self, **kwargs):
        server = Server(self.cert, self.key, self.raw, **kwargs)
        try:
            yield server
        finally:
            server.close()

    def unsolicited(self):
        prepared=self.core("prepare","--to","recipient@example.org","--subject-file",str(self.root/"subject"),"--body-file",str(self.root/"body"),"--attach",str(self.root/"resume.pdf"))
        self.package,self.attempt=prepared["package_id"],str(uuid.uuid4())
        mid=prepared["manifest"]["message_id"]
        with contextlib.closing(sqlite3.connect(self.home/"mail.sqlite3")) as db, db:
            db.execute("INSERT INTO attempts VALUES(?,?,'2026-01-01T00:00:00+00:00','2026-01-01T00:00:01+00:00','accepted',?,'250')",(self.attempt,self.package,mid))
        self.before=self.core("status",self.package)["attempts"]
        self.raw=("Message-ID: <synthetic-report@example.org>\r\nContent-Type: multipart/report; report-type=delivery-status; boundary=x\r\n\r\n"
                  "--x\r\nContent-Type: text/plain\r\n\r\nSynthetic notification\r\n"
                  "--x\r\nContent-Type: message/delivery-status\r\n\r\nReporting-MTA: dns; example.org\r\n\r\n"
                  "Final-Recipient: rfc822; recipient@example.org\r\nAction: failed\r\nStatus: 5.0.0\r\n\r\n"
                  "--x\r\nContent-Type: text/rfc822-headers\r\n\r\n"
                  f"Message-ID: {mid}\r\nFrom: sender@example.org\r\nTo: recipient@example.org\r\n\r\n--x--\r\n").encode()

    def test_unsolicited_selected_report_import_replay_and_default_refusal(self):
        self.unsolicited()
        params=dict(message_id='<synthetic-report@example.org>',report_search='sender-date',report_date='2026-01-01',report_from='mailer-daemon@example.org')
        with self.server() as server:
            code,_=self.run_adapter(server,**params)
            self.assertEqual(code,5);self.assertEqual(server.commands,[])
            for repeated in (False,True):
                code,value=self.run_adapter(server,**params,by_headers=True)
                self.assertEqual(code,0)
                self.assertEqual(value['result']['correlation'],'original_headers')
                self.assertEqual(value['result']['cached'],int(repeated))
                self.assertEqual(sum(b'BODY.PEEK[]' in line for line in server.commands),1)
        status=self.core('status',self.package)
        self.assertEqual(status['attempts'],self.before)
        self.assertEqual(status['delivery_observation']['outcome'],'failed_reported')
        self.assertEqual(len(status['dsn']['reports']),1)

    def test_unsolicited_mode_rejects_unselected_or_ambiguous_attempt_before_network(self):
        self.unsolicited()
        with self.server() as server:
            code,_=self.run_adapter(server,by_headers=True)
            self.assertEqual(code,3);self.assertEqual(server.commands,[])
        with contextlib.closing(sqlite3.connect(self.home/'mail.sqlite3')) as db, db:
            db.execute("INSERT INTO attempts VALUES(?,?,'2025-01-01T00:00:00+00:00','2025-01-01T00:00:01+00:00','rejected',?,'550')",(str(uuid.uuid4()),self.package,self.before[0]['message_id']))
        with self.server() as server:
            code,_=self.run_adapter(server,by_headers=True,message_id='<synthetic-report@example.org>')
            self.assertEqual(code,5);self.assertEqual(server.commands,[])

    def test_paired_core_cache_restore_preserves_reports_operations_and_resend_blocks(self):
        self.unsolicited()
        original=self.home
        unknown=self.core('prepare','--to','recipient@example.org','--subject-file',str(self.root/'subject'),
                          '--body-file',str(self.root/'body'),'--attach',str(self.root/'resume.pdf'),
                          '--operation-key','synthetic-restore')
        unknown_id=unknown['package_id']
        with contextlib.closing(sqlite3.connect(original/'mail.sqlite3')) as db, db:
            db.execute("INSERT INTO attempts VALUES(?,?,'2026-01-01T00:00:00+00:00',NULL,'unknown',?,NULL)",
                       (str(uuid.uuid4()),unknown_id,unknown['manifest']['message_id']))

        def backup(source,target):
            fd=os.open(target,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600);os.close(fd)
            with contextlib.closing(sqlite3.connect(source.as_uri()+'?mode=ro',uri=True)) as src, \
                 contextlib.closing(sqlite3.connect(target)) as dst:
                src.backup(dst)
                self.assertEqual(dst.execute('PRAGMA quick_check').fetchone()[0],'ok')

        baseline=self.root/'before-report.sqlite3'
        backup(original/'mail.sqlite3',baseline)
        params=dict(uid='42',message_id='<synthetic-report@example.org>',by_headers=True,expected_uidvalidity='77')
        with self.server() as server:
            code,value=self.run_adapter(server,**params);self.assertEqual(code,0)
            report_id=value['result']['records'][0]['report_id']
            expected={pid:(self.core('inspect',pid),self.core('status',pid)) for pid in (self.package,unknown_id)}
            restored=self.root/'restored';restored.mkdir(mode=0o700)
            # Exercise the core's snapshot against the actual adapter cache schema.
            snapshot=self.root/'snapshot'
            result=self.core('backup','--output',str(snapshot))
            self.assertTrue(result['cache_included'])
            self.assertFalse(result['retry_authorized'])
            self.assertEqual(self.core('verify-backup',str(snapshot))['integrity'],'verified')
            backup(snapshot/'mail.sqlite3',restored/'mail.sqlite3')
            backup(snapshot/'imap-dsn.sqlite3',restored/'imap-dsn.sqlite3')
            shutil.copyfile(original/'config.toml',restored/'config.toml');(restored/'config.toml').chmod(0o600)
            self.home=restored
            for pid in (self.package,unknown_id):
                self.assertEqual((self.core('inspect',pid),self.core('status',pid)),expected[pid])
                result=subprocess.run([self.binary,'--home',str(restored),'send',pid,'--json'],capture_output=True,timeout=10)
                self.assertEqual(result.returncode,5)
                self.assertEqual(json.loads(result.stdout)['error']['code'],'send_blocked')
            recovered=self.core('prepare','--to','recipient@example.org','--subject-file',str(self.root/'subject'),
                                '--body-file',str(self.root/'body'),'--attach',str(self.root/'resume.pdf'),
                                '--operation-key','synthetic-restore')
            self.assertEqual(recovered['package_id'],unknown_id)
            self.assertTrue(recovered['already_prepared'])
            self.assertEqual(recovered['state'],'unknown')
            bodies=sum(b'BODY.PEEK[]' in line for line in server.commands)
            code,value=self.run_adapter(server,**params);self.assertEqual(code,0)
            self.assertEqual(value['result']['cached'],1)
            self.assertEqual(sum(b'BODY.PEEK[]' in line for line in server.commands),bodies)
            self.assertEqual(self.core('inspect-dsn',report_id)['record']['sha256'],hashlib.sha256(self.raw).hexdigest())
            # A newer cache with an older core must fail closed before any fetch.
            skewed=self.root/'skewed';skewed.mkdir(mode=0o700)
            backup(baseline,skewed/'mail.sqlite3')
            backup(restored/'imap-dsn.sqlite3',skewed/'imap-dsn.sqlite3')
            shutil.copyfile(original/'config.toml',skewed/'config.toml');(skewed/'config.toml').chmod(0o600)
            self.home=skewed
            refused=self.root/'refused-snapshot'
            result=subprocess.run([self.binary,'--home',str(skewed),'backup','--output',str(refused),'--json'],capture_output=True,timeout=10)
            self.assertEqual(result.returncode,5)
            self.assertEqual(json.loads(result.stdout)['error']['code'],'backup_binding')
            self.assertFalse(refused.exists())
            start=len(server.commands)
            code,value=self.run_adapter(server,**params)
            self.assertEqual(code,3);self.assertEqual(value['error']['code'],'not_found')
            self.assertFalse(any(b'UID FETCH' in line for line in server.commands[start:]))
            self.assertEqual(self.core('status',self.package)['dsn']['reports'],[])
            self.assertEqual(self.core('status',unknown_id)['state'],'unknown')
            # The matched restoration and original remain intact after refusal.
            self.home=restored
            self.assertEqual(self.core('status',self.package),expected[self.package][1])
        self.home=original
        self.assertEqual(self.core('status',self.package),expected[self.package][1])

    def test_known_uid_with_message_id_is_guarded_and_replayed_without_search(self):
        self.unsolicited()
        params=dict(uid='42',message_id='<synthetic-report@example.org>',by_headers=True,expected_uidvalidity='77')
        with self.server() as server:
            for repeat in (False,True):
                code,value=self.run_adapter(server,**params)
                self.assertEqual(code,0)
                self.assertEqual(value['result']['selection'],'uid')
                self.assertEqual(value['result']['cached'],int(repeat))
                self.assertEqual(sum(b'BODY.PEEK[]' in line for line in server.commands),1)
            self.assertFalse(any(b'UID SEARCH' in line or b'UID FETCH 23:42' in line for line in server.commands))
            server.validity='78';before=len(server.commands)
            code,value=self.run_adapter(server,**params)
            self.assertEqual(code,5);self.assertEqual(value['error']['code'],'imap_uidvalidity')
            self.assertFalse(any(b'UID FETCH' in line for line in server.commands[before:]))
            code,value=self.run_adapter(server,**(params|dict(expected_uidvalidity='78')))
            self.assertEqual(code,0)
            self.assertTrue(value['result']['records'][0]['already_recorded'])
        self.assertEqual(self.core('status',self.package)['attempts'],self.before)
        self.assertEqual(len(self.core('status',self.package)['dsn']['reports']),1)

    def test_uid_epoch_validation_and_header_mismatch_never_fetch_body(self):
        self.unsolicited()
        base=dict(uid='42',message_id='<synthetic-report@example.org>',by_headers=True)
        for changes in [{},dict(expected_uidvalidity='0'),dict(expected_uidvalidity='4294967296'),
                        dict(expected_uidvalidity='bad'),dict(expected_uidvalidity='77',uid=None),
                        dict(expected_uidvalidity='77',report_search='recent-uids')]:
            with self.server() as server:
                code,_=self.run_adapter(server,**(base|changes))
                self.assertEqual(code,3);self.assertEqual(server.commands,[])
        with self.server(report_header=b'Message-ID: <different@example.org>\r\n\r\n') as server:
            code,value=self.run_adapter(server,**base,expected_uidvalidity='77')
            self.assertEqual(code,5);self.assertEqual(value['error']['code'],'imap_report_identity')
            self.assertFalse(any(b'BODY.PEEK[]' in line for line in server.commands))
        self.assertEqual(len(self.core('status',self.package)['dsn']['reports']),0)

    def test_recent_uid_window_matches_only_selected_header_without_search(self):
        self.unsolicited()
        with self.server() as server:
            code,value=self.run_adapter(server,by_headers=True,message_id='<synthetic-report@example.org>',report_search='recent-uids')
            self.assertEqual(code,0)
            self.assertEqual(value['result']['scanned_headers'],1)
            self.assertEqual(value['result']['mailbox_messages'],1)
            self.assertTrue(any(b'UID FETCH 23:42 (UID BODY.PEEK[HEADER.FIELDS (MESSAGE-ID)])' in line for line in server.commands))
            self.assertFalse(any(b'UID SEARCH' in line for line in server.commands))
            code,value=self.run_adapter(server,by_headers=True,message_id='<synthetic-report@example.org>',report_search='recent-uids')
            self.assertEqual(code,0);self.assertEqual(value['result']['cached'],1)
            self.assertEqual(sum(b'BODY.PEEK[]' in line for line in server.commands),1)
        self.assertEqual(self.core('status',self.package)['attempts'],self.before)

    def test_recent_uid_header_mismatch_or_out_of_window_never_fetches_body(self):
        self.unsolicited()
        for options,expected in [(dict(report_header=b'Message-ID: <other@example.org>\r\n\r\n'),0),
                                 (dict(uidnext=b'100'),5), (dict(uidnext=b'1'),0), (dict(uidnext=b'bad'),8)]:
            with self.subTest(options=options), self.server(**options) as server:
                code,value=self.run_adapter(server,by_headers=True,message_id='<synthetic-report@example.org>',report_search='recent-uids')
                self.assertEqual(code,expected)
                self.assertFalse(any(b'BODY.PEEK[]' in line for line in server.commands))
                if code==0:self.assertEqual(value['result']['matched_candidates'],0)

    def test_recent_fetch_uid_after_literal_and_duplicate_identity(self):
        session=Mock()
        session.response.return_value=('UIDNEXT',[b'43'])
        raw=b'Message-ID: <synthetic-report@example.org>\r\n\r\n'
        session.uid.return_value=('OK',[(b'1 (BODY[HEADER.FIELDS (MESSAGE-ID)] {53}',raw),b' UID 42)'])
        self.assertEqual(adapter.recent_report_uids(session,'<synthetic-report@example.org>'),(['42'],1))
        for tail in [b' UID 42 UID 42)',b' UID 100)']:
            session.uid.return_value=('OK',[(b'1 (BODY[HEADER.FIELDS (MESSAGE-ID)] {53}',raw),tail])
            with self.assertRaises(adapter.Failure):adapter.recent_report_uids(session,'<synthetic-report@example.org>')

    def test_recent_header_response_limits_are_enforced(self):
        session=Mock();session.response.return_value=('UIDNEXT',[b'43'])
        for data in [[(b'x'*4097,b'')],[(b'1 (UID 42)',b'x'*8193)],
                     [(b'1 (UID 42)',b'')]+[b')']*8,
                     [(b'1 (UID 42)',b'')]*21]:
            session.uid.return_value=('OK',data)
            with self.assertRaises(adapter.Failure) as raised:adapter.recent_report_uids(session,'<synthetic-report@example.org>')
            self.assertEqual(raised.exception.code,'input_limit')

    def test_cached_envid_report_cannot_be_relabelled_as_header_correlated(self):
        with self.server() as server:
            code,_=self.run_adapter(server)
            self.assertEqual(code,0)
            before=sum(b'BODY.PEEK[]' in line for line in server.commands)
            code,value=self.run_adapter(server,by_headers=True,message_id='<synthetic-report@example.org>')
            self.assertEqual(code,5);self.assertEqual(value['error']['code'],'ledger_binding')
            self.assertEqual(sum(b'BODY.PEEK[]' in line for line in server.commands),before)

    def test_verified_tls_fetch_import_and_cached_replay_preserve_history(self):
        with self.server() as server:
            code, result = self.run_adapter(server)
            self.assertEqual(code, 0)
            self.assertEqual(len(result["result"]["records"]), 1)
            first = result["result"]["records"][0]
            self.assertEqual(first["sha256"], hashlib.sha256(self.raw).hexdigest())
            fetches = sum(b"BODY.PEEK[]" in line for line in server.commands)
            code, repeated = self.run_adapter(server)
            self.assertEqual(code, 0)
            self.assertEqual(repeated["result"]["cached"], 1)
            self.assertEqual(sum(b"BODY.PEEK[]" in line for line in server.commands), fetches)
            self.assertTrue(any(b"EXAMINE" in line for line in server.commands))
            self.assertFalse(any(b" STORE " in line or b" APPEND " in line or b" CLOSE" in line for line in server.commands))
        status = self.core("status", self.package)
        self.assertEqual(status["attempts"], self.before)
        self.assertEqual(len(status["dsn"]["reports"]), 1)
        self.assertFalse(status["delivery_confirmed"])

    def test_selected_notification_header_search_and_validation(self):
        with self.server() as server:
            code, value = self.run_adapter(server, message_id='<synthetic-report@example.org>')
            self.assertEqual(code, 0)
            self.assertEqual(len(value['result']['records']), 1)
            self.assertTrue(any(b'UID SEARCH HEADER "Message-ID" "<synthetic-report@example.org>"' in line for line in server.commands))
            self.assertFalse(any(b'UID SEARCH BODY' in line for line in server.commands))
            for invalid in ['bad', '<bad@example.org>\r\nSTORE', '<"bad"@example.org>']:
                count = len(server.commands)
                code, _ = self.run_adapter(server, message_id=invalid)
                self.assertEqual(code, 3)
                self.assertEqual(len(server.commands), count)
            code, _ = self.run_adapter(server, uid='42', message_id='<synthetic@example.org>')
            self.assertEqual(code, 3)

    def test_search_refusal_is_safe_and_does_not_fetch_or_change_history(self):
        with self.server(refuse_search=True) as server:
            code, value = self.run_adapter(server, message_id='<synthetic@example.org>')
            self.assertEqual(code, 8)
            self.assertEqual(value['error']['code'], 'imap_search')
            self.assertFalse(any(b'UID FETCH' in line for line in server.commands))
            self.assertEqual(self.core('status', self.package)['attempts'], self.before)
        self.assertEqual(len(self.core("status", self.package)["dsn"]["reports"]), 0)

    def test_explicit_text_search_verifies_header_and_replay_without_body_refetch(self):
        with self.server() as server:
            for repeat in (False, True):
                code, value = self.run_adapter(server, message_id='<synthetic-report@example.org>', report_search='text')
                self.assertEqual(code, 0)
                self.assertEqual(value['result']['cached'], int(repeat))
                self.assertEqual(sum(b'BODY.PEEK[]' in line for line in server.commands), 1)
                self.assertEqual(sum(b'BODY.PEEK[HEADER.FIELDS' in line for line in server.commands), 1 + int(repeat))
            self.assertTrue(any(b'UID SEARCH TEXT "<synthetic-report@example.org>"' in line for line in server.commands))
            self.assertFalse(any(b'UID SEARCH HEADER' in line for line in server.commands))
            server.report_header = b'Message-ID: <other@example.org>\r\n\r\n'
            code, value = self.run_adapter(server, message_id='<synthetic-report@example.org>', report_search='text')
            self.assertEqual(code, 5)
            self.assertEqual(value['error']['code'], 'imap_report_identity')
            self.assertEqual(sum(b'BODY.PEEK[]' in line for line in server.commands), 1)
        self.assertEqual(self.core('status', self.package)['attempts'], self.before)

    def test_header_mismatch_duplicate_missing_and_oversize_never_fetch_body(self):
        for header in [b'Message-ID: <other@example.org>\r\n\r\n', b'\r\n',
                       b'Message-ID: <synthetic-report@example.org>\r\nMessage-ID: <synthetic-report@example.org>\r\n\r\n',
                       b'Message-ID: '+b'x'*8192]:
            with self.subTest(header_size=len(header)), self.server(report_header=header) as server:
                code, _ = self.run_adapter(server, message_id='<synthetic-report@example.org>', report_search='text')
                self.assertIn(code, (3, 5))
                self.assertFalse(any(b'BODY.PEEK[]' in line for line in server.commands))
        with self.server() as server:
            code, _ = self.run_adapter(server, report_search='text')
            self.assertEqual(code, 3)
            self.assertEqual(server.commands, [])
        self.assertEqual(len(self.core('status', self.package)['dsn']['reports']), 0)

    def test_sender_date_selection_exact_header_and_bounded_candidates(self):
        params = dict(message_id='<synthetic-report@example.org>', report_search='sender-date',
                      report_date='2026-01-01', report_from='mailer-daemon@example.org')
        with self.server() as server:
            code, _ = self.run_adapter(server, **params)
            self.assertEqual(code, 0)
            self.assertTrue(any(b'UID SEARCH SENTON 01-Jan-2026 FROM "mailer-daemon@example.org"' in line for line in server.commands))
        with self.server(search_uids=b' '.join(str(n).encode() for n in range(1, 22))) as server:
            code, value = self.run_adapter(server, **params)
            self.assertEqual(code, 3)
            self.assertEqual(value['error']['code'], 'candidate_limit')
            self.assertFalse(any(b'UID FETCH' in line for line in server.commands))
        with self.server(search_uids=b'') as server:
            code, value = self.run_adapter(server, **params)
            self.assertEqual(code, 0)
            self.assertEqual(value['result']['matched_candidates'], 0)
            self.assertEqual(value['result']['selection'], 'message-id-sender-date')
            self.assertFalse(any(b'UID FETCH' in line for line in server.commands))
        for changes in [dict(report_date='2026-02-30'), dict(report_from='bad\r\nSTORE'),
                        dict(report_date=None), dict(report_search='text')]:
            with self.server() as server:
                code, _ = self.run_adapter(server, **(params | changes))
                self.assertEqual(code, 3)
                self.assertEqual(server.commands, [])

    def test_full_body_identity_must_match_header_before_import(self):
        self.raw = self.raw.replace(b'<synthetic-report@example.org>', b'<different@example.org>')
        with self.server() as server:
            code, value = self.run_adapter(server, message_id='<synthetic-report@example.org>', report_search='text')
            self.assertEqual(code, 5)
            self.assertEqual(value['error']['code'], 'imap_report_identity')
        self.assertEqual(len(self.core('status', self.package)['dsn']['reports']), 0)

    def test_safe_search_failure_classification_never_exposes_prose(self):
        for raw, code in [(b'[UNAVAILABLE] synthetic-secret-only', 'unavailable'),
                          (b'not supported synthetic-secret-only', 'unsupported'),
                          (b'[BADCHARSET] synthetic-secret-only', 'charset'),
                          (b'[LIMIT] synthetic-secret-only', 'limited'),
                          (b'synthetic-secret-only', None)]:
            with self.assertRaises(adapter.Failure) as raised:
                adapter.ok(('NO', [raw]), 'imap_search')
            self.assertEqual(raised.exception.code, 'imap_search' + ('_' + code if code else ''))
            self.assertNotIn('synthetic-secret-only', str(raised.exception))

    def test_uidvalidity_change_refetches_but_core_deduplicates_bytes(self):
        with self.server() as server:
            for validity in ("77", "78"):
                server.validity = validity
                code, result = self.run_adapter(server, uid="42")
                self.assertEqual(code, 0)
                self.assertEqual(result["result"]["records"][0]["already_recorded"], validity == "78")
        self.assertEqual(len(self.core("status", self.package)["dsn"]["reports"]), 1)

    def test_untrusted_certificate_never_receives_login(self):
        with self.server() as server:
            code, result = self.run_adapter(server, trust=False)
            self.assertNotEqual(code, 0)
            self.assertFalse(result["ok"])
            self.assertFalse(any(b"LOGIN" in line for line in server.commands))

    def test_wrong_hostname_never_receives_login(self):
        with self.server() as server:
            code, _ = self.run_adapter(server, host="localhost")
            self.assertNotEqual(code, 0)
            self.assertFalse(any(b"LOGIN" in line for line in server.commands))

    def test_authentication_refusal_never_echoes_server_secret_prose(self):
        with self.server(refuse_login=True) as server:
            code, _ = self.run_adapter(server)
            self.assertNotEqual(code, 0)
        self.assertEqual(self.core("status", self.package)["dsn"]["reports"], [])

    def test_cache_write_failure_after_core_import_is_safe_to_replay(self):
        class FailedWriter:
            def __init__(self, db):
                self.db = db
            def execute(self, sql, *args):
                if sql.startswith("INSERT"):
                    raise sqlite3.OperationalError("synthetic cache failure")
                return self.db.execute(sql, *args)
            def __enter__(self):
                self.db.__enter__()
                return self
            def __exit__(self, *args):
                return self.db.__exit__(*args)
            def close(self):
                self.db.close()
        with self.server() as server:
            code, _ = self.run_adapter(server, dry_run=True)
            self.assertEqual(code, 0)
            cfg = adapter.config(self.root / "imap.toml")
            failed = FailedWriter(adapter.ledger(str(self.home)))
            with patch.object(adapter, "ledger", return_value=failed), patch.dict(os.environ, {
                "SYNTHETIC_IMAP_PASSWORD": "synthetic-secret-only", "SSL_CERT_FILE": str(self.cert)
            }):
                with self.assertRaises(sqlite3.OperationalError):
                    adapter.worker(cfg, adapter.Retrieval(package=self.package, attempt=self.attempt))
            self.assertEqual(len(self.core("status", self.package)["dsn"]["reports"]), 1)
            code, result = self.run_adapter(server)
            self.assertEqual(code, 0)
            self.assertTrue(result["result"]["records"][0]["already_recorded"])
        status = self.core("status", self.package)
        self.assertEqual(len(status["dsn"]["reports"]), 1)
        self.assertEqual(status["attempts"], self.before)

    def test_offline_dry_run_and_missing_secret_do_not_connect_or_create_cache(self):
        with self.server() as server:
            code, result = self.run_adapter(server, dry_run=True)
            self.assertEqual(code, 0)
            self.assertFalse(result["result"]["network_used"])
            code, _ = self.run_adapter(server, password=False)
            self.assertEqual(code, 3)
            self.assertEqual(server.commands, [])
            self.assertFalse((self.home / "imap-dsn.sqlite3").exists())

    def test_wrong_uid_oversize_and_excessive_literal_do_not_import(self):
        for options in ({"wrong_uid": True}, {"oversized": True}, {"huge_literal": True}):
            with self.server(**options) as server:
                code, _ = self.run_adapter(server)
                self.assertNotEqual(code, 0)
                if options.get("wrong_uid") or options.get("oversized"):
                    self.assertFalse(any(b"BODY.PEEK[]" in line for line in server.commands))
        self.assertEqual(self.core("status", self.package)["dsn"]["reports"], [])

        self.assertEqual(self.core("status", self.package)["attempts"], self.before)

    def test_stalled_server_is_bounded_by_total_deadline(self):
        with self.server(stall=True) as server:
            code, result = self.run_adapter(server, deadline=1)
            self.assertEqual(code, 8)
            self.assertEqual(result["error"]["code"], "integration_timeout")

    def test_mismatched_report_and_private_config_validation(self):
        self.raw = self.raw.replace(self.attempt.encode(), str(uuid.uuid4()).encode())
        with self.server() as server:
            code, result = self.run_adapter(server)
            self.assertEqual(code, 5)
            self.assertEqual(result["error"]["code"], "dsn_binding")
        cfg = self.root / "imap.toml"
        cfg.chmod(0o644)
        with self.assertRaises(adapter.Failure):
            adapter.config(cfg)
        self.assertEqual(self.core("status", self.package)["dsn"]["reports"], [])

    def test_standalone_adapters_share_process_control_environment_policy(self):
        name='keyring_env_policy'
        loader=importlib.machinery.SourceFileLoader(name,str(ROOT/'integrations/gnome-keyring/agent-mail-keyring'))
        spec=importlib.util.spec_from_loader(loader.name,loader);keyring=importlib.util.module_from_spec(spec)
        loader.exec_module(keyring)
        for variable in ['AGENT_MAIL_SMTP_PASSWORD','SMTP_TOKEN','mail_password','MAIL-PASSWORD',
                         'LD_PRELOAD','LD_AUDIT','PYTHONHOME','PATH','HOME','LC_ALL','SSL_CERT_FILE',
                         'GLIBC_TUNABLES','RUST_BACKTRACE','AGENT_MAIL_HOME','bad=name','bad\x00name']:
            self.assertEqual(adapter.credential_env_name(variable),keyring.credential_env_name(variable))

    def test_process_control_credential_environment_is_rejected(self):
        with self.server() as server:
            code,_=self.run_adapter(server,dry_run=True)
            self.assertEqual(code,0)
            path=self.root/'imap.toml';good=path.read_text()
            for name in ['LD_AUDIT','LD_DEBUG','DYLD_INSERT_LIBRARIES','PYTHONINSPECT','LANG','LC_ALL',
                         'SSL_CERT_FILE','OPENSSL_CONF','OPENSSL_MODULES','SSLKEYLOGFILE','GCONV_PATH','GLIBC_TUNABLES','RUST_BACKTRACE','AGENT_MAIL_HOME']:
                path.write_text(good.replace('SYNTHETIC_IMAP_PASSWORD',name));path.chmod(0o600)
                with self.assertRaises(adapter.Failure) as raised:adapter.config(path)
                self.assertEqual(raised.exception.code,'configuration')
            self.assertEqual(server.commands,[])



class LocalBoundaryTests(unittest.TestCase):
    def test_ledger_rows_are_immutable_and_unknown_schema_is_not_repaired(self):
        with tempfile.TemporaryDirectory() as home:
            db=adapter.ledger(home)
            db.execute("INSERT INTO reports VALUES('remote','package','attempt','hash','report')")
            db.commit()
            with self.assertRaises(sqlite3.IntegrityError):
                db.execute("DELETE FROM reports")
            with self.assertRaises(sqlite3.IntegrityError):
                db.execute("UPDATE reports SET report_id='different'")
            db.rollback()
            db.execute("ALTER TABLE reports ADD COLUMN unexpected TEXT")
            db.commit()
            db.close()
            with self.assertRaises(adapter.Failure) as failure:
                adapter.ledger(home)
            self.assertEqual(failure.exception.code,"ledger_schema")
            with contextlib.closing(sqlite3.connect(Path(home)/"imap-dsn.sqlite3")) as check, check:
                self.assertEqual(check.execute("SELECT report_id FROM reports").fetchone(),("report",))

    def test_rejected_cache_schema_preserves_wal_database_bytes(self):
        for kind in ("foreign", "future", "altered"):
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as home:
                path = Path(home) / "imap-dsn.sqlite3"
                with contextlib.closing(sqlite3.connect(path)) as db:
                    self.assertEqual(db.execute("PRAGMA journal_mode=WAL").fetchone()[0], "wal")
                    if kind == "foreign":
                        db.execute("CREATE TABLE unrelated(value TEXT)")
                        db.execute("INSERT INTO unrelated VALUES('synthetic')")
                    else:
                        db.executescript(adapter.LEDGER_SQL)
                        db.execute("PRAGMA application_id=1296974931")
                        db.execute("PRAGMA user_version=" + ("99" if kind == "future" else "1"))
                        if kind == "altered":
                            db.execute("ALTER TABLE reports ADD COLUMN unexpected TEXT")
                    db.commit()
                path.chmod(0o600)
                before = path.read_bytes()
                self.assertEqual(before[18:20], b"\x02\x02")
                with self.assertRaises(adapter.Failure) as failure:
                    adapter.ledger(home)
                self.assertEqual(failure.exception.code, "ledger_schema")
                self.assertEqual(failure.exception.exit_code, 9)
                self.assertEqual(path.read_bytes(), before)

    def test_total_deadline_stops_blocked_core_descendant(self):
        self.blocked_core_deadline_case(parent_dies=False)

    def test_worker_watchdog_stops_core_after_supervisor_death(self):
        self.blocked_core_deadline_case(parent_dies=True)

    def blocked_core_deadline_case(self, parent_dies):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pid_file = root / "core.pid"
            fake = root / "synthetic-core"
            fake.write_text(f"#!/usr/bin/python3\nimport os,time\nfrom pathlib import Path\nPath({str(pid_file)!r}).write_text(str(os.getpid()))\ntime.sleep(30)\n")
            fake.chmod(0o700)
            config = root / "imap.toml"
            config.write_text(f"host='imap.example.invalid'\nusername='login@example.org'\npassword_env='SYNTHETIC_PASSWORD'\nhome={json.dumps(str(root))}\nbinary={json.dumps(str(fake))}\ntotal_timeout_seconds=2\n")
            config.chmod(0o600)
            def live(pid):
                try:
                    return (Path('/proc') / str(pid) / 'stat').read_text().split()[2] != 'Z'
                except FileNotFoundError:
                    return False
            pid = None
            process = subprocess.Popen([sys.executable, '-B', str(SCRIPT), '--config', str(config),
                                        '--package-id', '11111111-1111-4111-8111-111111111111',
                                        '--attempt-id', '22222222-2222-4222-8222-222222222222', '--json'],
                                       stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            try:
                started = time.monotonic() + 1.5
                while not pid_file.exists() and time.monotonic() < started:
                    time.sleep(0.01)
                self.assertTrue(pid_file.exists())
                pid = int(pid_file.read_text())
                if parent_dies:
                    process.kill()
                raw, diagnostics = process.communicate(timeout=10)
                self.assertFalse(diagnostics)
                if parent_dies:
                    self.assertEqual(process.returncode, -signal.SIGKILL)
                else:
                    self.assertEqual(process.returncode, 8)
                    self.assertEqual(json.loads(raw)['error']['code'], 'integration_timeout')
                deadline = time.monotonic() + 3
                while live(pid) and time.monotonic() < deadline:
                    time.sleep(0.01)
                self.assertFalse(live(pid), 'A core child survived the adapter total deadline')
            finally:
                if process.poll() is None:
                    process.kill()
                    process.communicate()
                if pid is None and pid_file.exists():
                    pid = int(pid_file.read_text())
                if pid is not None and live(pid):
                    # Only clean up this fixture's synthetic executable.
                    command = (Path('/proc') / str(pid) / 'cmdline').read_bytes()
                    if str(fake).encode() in command:
                        os.kill(pid, signal.SIGKILL)

    def test_configuration_rejects_symlinks_and_fifo_without_blocking(self):
        with tempfile.TemporaryDirectory() as home:
            path=Path(home)/"cfg"
            path.write_text('host="example.org"')
            path.chmod(0o600)
            link=Path(home)/"link"
            link.symlink_to(path)
            with self.assertRaises(adapter.Failure):
                adapter.config(link)
            fifo=Path(home)/"fifo"
            os.mkfifo(fifo,0o600)
            with self.assertRaises(adapter.Failure):
                adapter.config(fifo)


if __name__ == "__main__":
    import sys
    unittest.main()


class RenameTests(unittest.TestCase):
    def test_default_history_selection_never_hides_legacy_store(self):
        with tempfile.TemporaryDirectory() as root, patch.object(Path, 'home', return_value=Path(root)):
            current = Path(root) / '.local/share/agent-mail'
            legacy = Path(root) / '.local/share/agent-mailer'
            self.assertEqual(adapter.default_home(), current)
            legacy.mkdir(parents=True)
            self.assertEqual(adapter.default_home(), legacy)
            current.mkdir()
            with self.assertRaises(ValueError):
                adapter.default_home()
            current.rmdir()
            legacy.rmdir()
            legacy.symlink_to(Path(root) / 'missing')
            self.assertEqual(adapter.default_home(), legacy)


if __name__ == "__main__":
    unittest.main()
