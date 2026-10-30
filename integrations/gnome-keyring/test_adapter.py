"""Synthetic adapter tests: never access the desktop keyring or SMTP."""
import importlib.machinery
import importlib.util
from pathlib import Path
import unittest
import tempfile
import time
import os
import multiprocessing
import subprocess
import sys
from unittest.mock import Mock, patch

loader = importlib.machinery.SourceFileLoader("adapter", str(Path(__file__).with_name("agent-mailer-keyring")))
spec = importlib.util.spec_from_loader(loader.name, loader)
adapter = importlib.util.module_from_spec(spec)
loader.exec_module(adapter)


class AdapterTests(unittest.TestCase):
    def service(self, items):
        secret = Mock()
        secret.Service.get_sync.return_value.search_sync.return_value = items
        return secret

    def test_locked_item_never_loads_secret(self):
        item = Mock()
        item.get_locked.return_value = True
        with self.assertRaises(ValueError):
            adapter.retrieve(self.service([item]), Mock())
        item.load_secret_sync.assert_not_called()

    def test_absent_or_ambiguous_item_fails(self):
        for items in [[], [Mock(), Mock()]]:
            with self.assertRaises(ValueError):
                adapter.retrieve(self.service(items), Mock())

    def test_unlocked_nonempty_item(self):
        item = Mock()
        item.get_locked.return_value = False
        item.get_secret.return_value.get_text.return_value = "synthetic"
        self.assertEqual(adapter.retrieve(self.service([item]), Mock()), "synthetic")
        item.load_secret_sync.assert_called_once_with(None)

    def test_empty_secret_fails(self):
        item = Mock()
        item.get_locked.return_value = False
        item.get_secret.return_value.get_text.return_value = ""
        with self.assertRaises(ValueError):
            adapter.retrieve(self.service([item]), Mock())


def stalled_worker(connection, operation, password):
    time.sleep(10)


def partial_frame_worker(connection, operation, password):
    import struct
    os.write(connection.fileno(), struct.pack("!i", 100) + b"x")
    time.sleep(10)


def successful_worker(connection, operation, password):
    connection.send((True, "synthetic-secret"))
    connection.close()


def failed_worker(connection, operation, password):
    connection.send((False, None))
    connection.close()


class BoundaryTests(unittest.TestCase):
    def test_native_call_deadline_terminates_and_reaps_worker(self):
        before = {p.pid for p in multiprocessing.active_children()}
        start = time.monotonic()
        with patch.object(adapter, "keyring_worker", stalled_worker):
            with self.assertRaises(TimeoutError):
                adapter.keyring_operation("lookup", timeout=0.05)
        self.assertLess(time.monotonic() - start, 3)
        self.assertEqual({p.pid for p in multiprocessing.active_children()}, before)

    def test_partial_frame_does_not_escape_deadline(self):
        start = time.monotonic()
        with patch.object(adapter, "keyring_worker", partial_frame_worker):
            with self.assertRaises(TimeoutError):
                adapter.keyring_operation("lookup", timeout=0.05)
        self.assertLess(time.monotonic() - start, 3)

    def test_private_worker_success_and_failure(self):
        with patch.object(adapter, "keyring_worker", successful_worker):
            self.assertEqual(adapter.keyring_operation("lookup"), "synthetic-secret")
        with patch.object(adapter, "keyring_worker", failed_worker):
            with self.assertRaises(ValueError):
                adapter.keyring_operation("lookup")

    def test_config_limits_types_and_private_permissions(self):
        with tempfile.TemporaryDirectory() as root:
            path = Path(root) / "config"
            path.write_text("sender='synthetic'")
            path.chmod(0o600)
            self.assertEqual(adapter.read_config(path)["sender"], "synthetic")
            path.chmod(0o644)
            with self.assertRaises(ValueError):
                adapter.read_config(path)
            path.chmod(0o600)
            path.write_bytes(b"x" * (1024 * 1024 + 1))
            with self.assertRaises(ValueError):
                adapter.read_config(path)
            link = Path(root) / "link"
            link.symlink_to(path)
            with self.assertRaises(OSError):
                adapter.read_config(link)
            fifo = Path(root) / "fifo"
            os.mkfifo(fifo, 0o600)
            with self.assertRaises(ValueError):
                adapter.read_config(fifo)

    def test_destination_overrides_rejected_before_read_or_secret(self):
        for args in [["--config=other"], ["--config", "other"], ["--home=other"], ["--home", "other"]]:
            with patch.object(adapter, "read_config") as reader:
                with self.assertRaises(ValueError):
                    adapter.validate_run(args, Path("/synthetic"))
                reader.assert_not_called()

    def test_exec_passes_secret_only_in_environment(self):
        config = {"sender": adapter.ACCOUNT, "smtp": {"username": adapter.LOGIN,
                  "host": "smtp.yandex.ru", "port": 465, "security": "tls",
                  "password_env": "AGENT_MAILER_SMTP_PASSWORD"}}
        with patch.object(adapter, "configure_binding"), patch.object(adapter, "read_config", return_value=config), \
             patch.object(adapter, "keyring_operation", return_value="synthetic-secret"), \
             patch.object(adapter.os, "execve") as execute:
            self.assertEqual(adapter.main(["run", "doctor", "--json"]), 0)
            binary, args, env = execute.call_args.args
            self.assertNotIn("synthetic-secret", repr(args))
            self.assertEqual(env["AGENT_MAILER_SMTP_PASSWORD"], "synthetic-secret")
            self.assertTrue(binary.endswith("/.local/bin/agent-mailer"))

    def test_output_failure_is_handled_without_traceback(self):
        with patch.object(adapter.os, "write", side_effect=BrokenPipeError):
            self.assertEqual(adapter.fail(), 7)

    def test_real_full_stdout_has_no_shutdown_traceback(self):
        with open("/dev/full", "wb") as output:
            result = subprocess.run([sys.executable, "-B", str(Path(__file__).with_name("agent-mailer-keyring"))], stdout=output, stderr=subprocess.PIPE, check=False)
        self.assertEqual(result.returncode, 7)
        self.assertEqual(result.stderr, b"")

    def test_wrong_smtp_host_rejected_before_secret_access(self):
        config = {"sender": adapter.ACCOUNT, "smtp": {"username": adapter.LOGIN,
                  "host": "attacker.example", "port": 465, "security": "tls",
                  "password_env": "AGENT_MAILER_SMTP_PASSWORD"}}
        with patch.object(adapter, "configure_binding"), patch.object(adapter, "read_config", return_value=config), \
             patch.object(adapter, "keyring_operation") as lookup:
            with self.assertRaises(ValueError):
                adapter.main(["run", "doctor", "--json"])
            lookup.assert_not_called()



class BindingTests(unittest.TestCase):
    def test_configurable_identity_and_destination(self):
        binding = {'sender':'alias@example.org','username':'primary@example.org',
                   'host':'smtp.example.org','port':587,'security':'starttls',
                   'password_env':'EXAMPLE_PASSWORD','account':'stable-key'}
        names = ['ACCOUNT','LOGIN','HOST','PORT','SECURITY','PASSWORD_ENV','ATTRS']
        with patch.multiple(adapter, **{n:getattr(adapter,n) for n in names}):
            with patch.object(adapter,'read_config',return_value=binding):
                adapter.configure_binding(Path('/synthetic'))
            self.assertEqual(adapter.LOGIN,'primary@example.org')
            self.assertEqual(adapter.ATTRS['account'],'stable-key')
            config={'sender':binding['sender'],'smtp':{k:binding[k] for k in ['username','host','port','security','password_env']}}
            with patch.object(adapter,'read_config',return_value=config):
                adapter.validate_run(['doctor'],Path('/synthetic'))
                config['smtp']['host']='other.example.org'
                with self.assertRaises(ValueError):
                    adapter.validate_run(['doctor'],Path('/synthetic'))


class CredentialEnvironmentTests(unittest.TestCase):
    def test_process_control_env_refused_before_secret_lookup_or_exec(self):
        names=['ACCOUNT','LOGIN','HOST','PORT','SECURITY','PASSWORD_ENV','ATTRS']
        blocked=['LD_PRELOAD','LD_AUDIT','LD_DEBUG','DYLD_INSERT_LIBRARIES','PYTHONPATH','PYTHONHOME',
                 'PYTHONINSPECT','SSL_CERT_FILE','SSL_CERT_DIR','PATH','HOME','LANG','LC_ALL',
                 'OPENSSL_CONF','OPENSSL_MODULES','SSLKEYLOGFILE','GCONV_PATH','GLIBC_TUNABLES','RUST_BACKTRACE','AGENT_MAILER_HOME']
        for variable in blocked:
            binding={'sender':'sender@example.org','username':'login@example.org','host':'smtp.example.org',
                     'port':465,'security':'tls','password_env':variable,'account':'stable-synthetic-key'}
            cfg={'sender':binding['sender'],'smtp':{k:binding[k] for k in ['username','host','port','security','password_env']}}
            with self.subTest(variable=variable), patch.multiple(adapter,**{n:getattr(adapter,n) for n in names}), \
                 patch.object(adapter,'read_config',side_effect=[binding,cfg]), \
                 patch.object(adapter,'keyring_operation',return_value='synthetic-credential') as lookup, \
                 patch.object(adapter.os,'execve') as execute:
                with self.assertRaises(ValueError):adapter.main(['run','doctor','--json'])
                lookup.assert_not_called();execute.assert_not_called()

    def test_application_specific_secret_names_remain_valid(self):
        for variable in ['AGENT_MAILER_SMTP_PASSWORD','SMTP_TOKEN','mail_password','MAIL-PASSWORD']:
            binding={'sender':'sender@example.org','username':'login@example.org','host':'smtp.example.org',
                     'port':465,'security':'tls','password_env':variable,'account':'stable-synthetic-key'}
            names=['ACCOUNT','LOGIN','HOST','PORT','SECURITY','PASSWORD_ENV','ATTRS']
            with patch.multiple(adapter,**{n:getattr(adapter,n) for n in names}), patch.object(adapter,'read_config',return_value=binding):
                adapter.configure_binding(Path('/synthetic'))
                self.assertEqual(adapter.PASSWORD_ENV,variable)


class ImapTests(unittest.TestCase):
    args = ['--package-id', '11111111-1111-4111-8111-111111111111',
            '--attempt-id', '22222222-2222-4222-8222-222222222222', '--json']

    def configs(self):
        return ({'imap': {'host': 'imap.example.org', 'port': 993, 'mailbox': 'INBOX'}},
                {'host': 'imap.example.org', 'username': adapter.LOGIN,
                 'password_env': adapter.PASSWORD_ENV})

    def test_fixed_executable_config_and_environment_only_secret(self):
        binding, cfg = self.configs()
        with patch.object(adapter, 'configure_binding'), \
             patch.object(adapter, 'read_config', side_effect=[binding, cfg]), \
             patch.object(adapter, 'keyring_operation', return_value='synthetic-secret'), \
             patch.object(adapter.os, 'execve') as execute:
            adapter.main(['imap-dsn', *self.args, '--uid', '42', '--dry-run'])
            binary, args, env = execute.call_args.args
            self.assertEqual(binary, str(Path.home() / '.local/bin/agent-mailer-imap-dsn'))
            self.assertEqual(args[:3], [binary, '--config', str(Path.home() / '.local/share/agent-mailer/imap.toml')])
            self.assertNotIn('synthetic-secret', repr(args))
            self.assertEqual(env[adapter.PASSWORD_ENV], 'synthetic-secret')

    def test_argument_redirection_and_invalid_ids_fail_before_read_or_lookup(self):
        bad = [['--config', 'other'], ['--config=other'], ['--home', 'other'],
               ['--worker'], ['--pack', self.args[1]], ['--uid', '0'],
               ['--uid', '4294967296'], ['--uid', '١'], ['--uid'],
               ['--package-id', 'invalid'], ['--attempt-id', self.args[3]],
               ['--report-message-id', '<bad@example.org>\r\nSTORE'],
               ['--report-message-id', '<test@example.org>', '--uid', '42'],
               ['--expected-uidvalidity', '77'], ['--uid','42','--expected-uidvalidity','0'], ['--uid','42','--expected-uidvalidity','bad'], ['--uid','42','--expected-uidvalidity','77','--expected-uidvalidity','78'],
               ['--report-search', 'text'], ['--correlate-by-headers'], ['--correlate-by-headers','--correlate-by-headers'],
               ['--report-message-id', '<test@example.org>', '--report-search', 'all'],
               ['--report-message-id', '<test@example.org>', '--report-search', 'sender-date'],
               ['--report-date', '2026-01-01'],
               ['--report-message-id', '<test@example.org>', '--report-search', 'sender-date', '--report-date', '2026-01-01', '--report-from', 'bad\r\nSTORE'],
               ['--json'], ['--dry-run', '--dry-run']]
        for extra in bad:
            with self.subTest(extra=extra), patch.object(adapter, 'configure_binding'), \
                 patch.object(adapter, 'read_config') as reader, \
                 patch.object(adapter, 'keyring_operation') as lookup:
                with self.assertRaises(ValueError):
                    adapter.main(['imap-dsn', *self.args, *extra])
                reader.assert_not_called()
                lookup.assert_not_called()

    def test_notification_message_id_is_passed_without_secret_in_argv(self):
        binding, cfg = self.configs()
        with patch.object(adapter, 'configure_binding'), \
             patch.object(adapter, 'read_config', side_effect=[binding, cfg]), \
             patch.object(adapter, 'keyring_operation', return_value='synthetic-secret'), \
             patch.object(adapter.os, 'execve') as execute:
            adapter.main(['imap-dsn', *self.args, '--report-message-id', '<report@example.org>', '--report-search', 'text'])
            self.assertEqual(execute.call_args.args[1][-4:], ['--report-message-id', '<report@example.org>', '--report-search', 'text'])
            self.assertNotIn('synthetic-secret', repr(execute.call_args.args[1]))

    def test_sender_date_selection_passes_only_reviewed_arguments(self):
        binding, cfg = self.configs()
        options = ['--correlate-by-headers', '--report-message-id', '<report@example.org>', '--report-search', 'sender-date',
                   '--report-date', '2026-01-01', '--report-from', 'mailer-daemon@example.org']
        with patch.object(adapter, 'configure_binding'), \
             patch.object(adapter, 'read_config', side_effect=[binding, cfg]), \
             patch.object(adapter, 'keyring_operation', return_value='synthetic-secret'), \
             patch.object(adapter.os, 'execve') as execute:
            adapter.main(['imap-dsn', *self.args, *options])
            self.assertEqual(execute.call_args.args[1][-len(options):], options)

    def test_known_uid_and_notification_pass_with_epoch_guard(self):
        binding,cfg=self.configs()
        options=['--uid','42','--expected-uidvalidity','77','--report-message-id','<report@example.org>','--correlate-by-headers']
        with patch.object(adapter,'configure_binding'), patch.object(adapter,'read_config',side_effect=[binding,cfg]), \
             patch.object(adapter,'keyring_operation',return_value='synthetic-secret'), patch.object(adapter.os,'execve') as execute:
            adapter.main(['imap-dsn',*self.args,*options])
            self.assertEqual(execute.call_args.args[1][-len(options):],options)
            self.assertNotIn('synthetic-secret',repr(execute.call_args.args[1]))

    def test_config_redirects_missing_binding_and_wrong_identity_fail_before_lookup(self):
        for field, value in [('host', 'attacker.example'), ('port', 143),
                             ('mailbox', 'Other'), ('username', 'other@example.org'),
                             ('password_env', 'OTHER_SECRET'), ('home', '/other'),
                             ('binary', '/other/binary')]:
            binding, cfg = self.configs()
            cfg[field] = value
            with self.subTest(field=field), patch.object(adapter, 'configure_binding'), \
                 patch.object(adapter, 'read_config', side_effect=[binding, cfg]), \
                 patch.object(adapter, 'keyring_operation') as lookup:
                with self.assertRaises(ValueError):
                    adapter.main(['imap-dsn', *self.args])
                lookup.assert_not_called()
        for binding in [{}, {'imap': {'host': 'imap.example.org', 'port': True, 'mailbox': 'INBOX'}}]:
            with patch.object(adapter, 'read_config', return_value=binding):
                with self.assertRaises(ValueError):
                    adapter.validate_imap(self.args, Path('/synthetic'))

    def test_hardlinked_config_is_rejected(self):
        with tempfile.TemporaryDirectory() as root:
            path = Path(root) / 'config'
            path.write_text("host='synthetic'")
            path.chmod(0o600)
            os.link(path, Path(root) / 'alias')
            with self.assertRaises(ValueError):
                adapter.read_config(path)


class PreflightTests(unittest.TestCase):
    def test_metadata_probe_never_loads_or_unlocks_secret(self):
        for locked, expected in [(False, 'available'), (True, 'locked')]:
            secret = Mock(); item = Mock(); item.get_locked.return_value = locked
            secret.Service.get_sync.return_value.search_sync.return_value = [item]
            self.assertEqual(adapter.probe_item(secret, 'schema'), expected)
            secret.Service.get_sync.assert_called_once_with(secret.ServiceFlags.NONE, None)
            secret.Service.get_sync.return_value.search_sync.assert_called_once_with('schema', adapter.ATTRS, secret.SearchFlags.ALL, None)
            item.load_secret_sync.assert_not_called(); item.get_secret.assert_not_called()
            secret.password_lookup_sync.assert_not_called()

    def test_metadata_probe_distinguishes_missing_ambiguous_and_unavailable(self):
        for items, expected in [([], 'missing'), ([Mock(), Mock()], 'ambiguous')]:
            secret = Mock(); secret.Service.get_sync.return_value.search_sync.return_value = items
            self.assertEqual(adapter.probe_item(secret, 'schema'), expected)
        secret = Mock(); secret.Service.get_sync.side_effect = RuntimeError('synthetic-private-detail')
        self.assertEqual(adapter.probe_item(secret, 'schema'), 'session_unavailable')
        with patch.object(adapter, 'secret_api', side_effect=ImportError('synthetic-private-detail')):
            self.assertEqual(adapter.probe_backend(), 'backend_unavailable')

    def test_missing_binding_reports_binary_without_keyring_or_exec(self):
        with patch.object(adapter, 'configure_binding', side_effect=FileNotFoundError()), \
             patch.object(adapter, 'executable_state', return_value='missing'), \
             patch.object(adapter, 'keyring_operation') as keyring, \
             patch.object(adapter.subprocess, 'run') as run, patch.object(adapter, 'emit', return_value=0) as emit:
            self.assertEqual(adapter.preflight(Path('/synthetic')), 0)
            result = emit.call_args.args[0]['result']
            self.assertEqual(result['checks']['binding'], 'missing')
            self.assertEqual(result['checks']['core_binary'], 'missing')
            self.assertFalse(result['ready']); self.assertFalse(result['secret_loaded'])
            keyring.assert_not_called(); run.assert_not_called()

    def test_preflight_uses_secretless_core_and_metadata_only_probe(self):
        result = {'schema_version': 1, 'ok': True, 'result': {'preflight_schema_version': 1, 'checks': {'smtp': 'ready'}}}
        with patch.object(adapter, 'configure_binding'), patch.object(adapter, 'validate_run', return_value=Path('/synthetic/config.toml')), \
             patch.object(adapter, 'executable_state', return_value='ready'), \
             patch.object(adapter, 'keyring_operation', return_value='available') as keyring, \
             patch.object(adapter.subprocess, 'run', return_value=Mock(returncode=0, stdout=__import__('json').dumps(result).encode())) as run, \
             patch.object(adapter, 'emit', return_value=0) as emit, patch.dict(os.environ, {adapter.PASSWORD_ENV: 'synthetic-do-not-disclose'}):
            self.assertEqual(adapter.preflight(Path('/synthetic')), 0)
            keyring.assert_called_once_with('probe', timeout=5)
            self.assertNotIn(adapter.PASSWORD_ENV, run.call_args.kwargs['env'])
            self.assertIn('preflight', run.call_args.args[0])
            emitted=emit.call_args.args[0]
            self.assertTrue(emitted['result']['ready'])
            self.assertNotIn('synthetic-do-not-disclose', str(emitted))
            self.assertFalse(emitted['result']['network_used']); self.assertFalse(emitted['result']['storage_opened'])

    def test_preflight_reports_deadlines_and_incompatible_core_without_details(self):
        import json
        cases = [
            (Mock(returncode=2, stdout=b'{"ok":false}'), None, 'core_incompatible', 'locked'),
            (None, subprocess.TimeoutExpired('synthetic-private-command', 10), 'core_timeout', 'timeout'),
        ]
        for output, failure, smtp_status, keyring_status in cases:
            with patch.object(adapter, 'configure_binding'), patch.object(adapter, 'validate_run', return_value=Path('/synthetic/config.toml')), \
                 patch.object(adapter, 'executable_state', return_value='ready'), \
                 patch.object(adapter.subprocess, 'run', return_value=output, side_effect=failure), \
                 patch.object(adapter, 'keyring_operation', return_value=keyring_status, side_effect=TimeoutError() if keyring_status == 'timeout' else None), \
                 patch.object(adapter, 'emit', return_value=0) as emit:
                self.assertEqual(adapter.preflight(Path('/synthetic')), 0)
                value = emit.call_args.args[0]
                self.assertEqual(value['result']['checks']['smtp_configuration'], smtp_status)
                self.assertEqual(value['result']['checks']['keyring_item'], keyring_status)
                self.assertFalse(value['result']['ready'])
                self.assertNotIn('synthetic-private-command', json.dumps(value))

    def test_executable_classification_refuses_symlink_and_writable_file(self):
        with tempfile.TemporaryDirectory() as directory:
            p=Path(directory)/'binary'
            self.assertEqual(adapter.executable_state(p), 'missing')
            p.write_bytes(b'synthetic');p.chmod(0o600)
            self.assertEqual(adapter.executable_state(p), 'not_executable')
            p.chmod(0o700);self.assertEqual(adapter.executable_state(p), 'ready')
            p.chmod(0o722);self.assertEqual(adapter.executable_state(p), 'unsafe')
            link=Path(directory)/'link';link.symlink_to(p)
            self.assertEqual(adapter.executable_state(link), 'unsafe')

if __name__ == "__main__":
    unittest.main()
