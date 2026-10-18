"""Focused negative and positive controls for scoped receipt gates."""
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
import subprocess
import sys
import time

from evidence_process import validate_run


class ProcessEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.log = self.root / 'tests.log'
        self.receipt = self.root / 'process.json'
        self.run_id = 'run-2026-10-17T12:00:00Z-a1b2'
        self.command = 'cargo test --locked -p sample'
        self.write('running 1 test\ntest result: ok. 1 passed; 0 failed; 0 ignored\n')

    def tearDown(self):
        self.temp.cleanup()

    def write(self, contents, *, exit_code=0):
        self.log.write_text(contents)
        self.receipt.write_text(json.dumps({
            'run_id': self.run_id, 'command': self.command,
            'started_at': '2026-10-17T12:00:00Z', 'ended_at': '2026-10-17T12:00:02Z',
            'exit_code': exit_code, 'log': str(self.log),
            'log_sha256': hashlib.sha256(self.log.read_bytes()).hexdigest(),
        }))

    def verify(self, receipt=None):
        return validate_run(receipt or self.receipt, run_id=self.run_id,
                            command=self.command, log_path=self.log)

    def test_valid_run(self):
        _, outcomes, _ = self.verify()
        self.assertEqual(len(outcomes), 1)

    def test_late_failed_suite_is_rejected(self):
        self.write('running 1 test\ntest result: ok. 1 passed; 0 failed; 0 ignored\n'
                   'running 1 test\ntest result: FAILED. 0 passed; 1 failed; 0 ignored\n')
        with self.assertRaisesRegex(ValueError, 'failed suite'):
            self.verify()

    def test_nonzero_wrapper_after_passing_summaries_is_rejected(self):
        self.write('test result: ok. 1 passed; 0 failed; 0 ignored\n', exit_code=1)
        with self.assertRaisesRegex(ValueError, 'exit was not zero'):
            self.verify()

    def test_missing_or_mismatched_receipt_is_rejected(self):
        with self.assertRaisesRegex(ValueError, 'missing process receipt'):
            self.verify(self.root / 'absent.json')
        data = json.loads(self.receipt.read_text())
        data['run_id'] = 'another-run'
        self.receipt.write_text(json.dumps(data))
        with self.assertRaisesRegex(ValueError, 'run_id mismatch'):
            self.verify()

    def test_truncated_log_is_rejected(self):
        self.write('running 1 test\ntest result: ok. 1 passed; 0 failed; 0 ignored')
        with self.assertRaisesRegex(ValueError, 'truncated or errored'):
            self.verify()

    def test_running_suite_without_outcome_is_rejected(self):
        self.write('running 1 test\ntest result: ok. 1 passed; 0 failed; 0 ignored\n'
                   'running 1 test\n')
        with self.assertRaisesRegex(ValueError, 'started suite without'):
            self.verify()

    def test_unordered_timestamps_are_rejected(self):
        data = json.loads(self.receipt.read_text())
        data['ended_at'] = '2026-10-17T11:59:59Z'
        self.receipt.write_text(json.dumps(data))
        with self.assertRaisesRegex(ValueError, 'unordered'):
            self.verify()

    def test_nested_child_summaries_close_without_double_counting_parent(self):
        self.write('running 165 tests\n'
                   'running 1 test\ntest result: ok. 1 passed; 0 failed; 0 ignored; 164 filtered out\n'
                   'test result: ok. 159 passed; 0 failed; 6 ignored\n')
        _, outcomes, _ = self.verify()
        self.assertEqual((int(outcomes[-1].group(2)), int(outcomes[-1].group(4))), (159, 6))

    def test_capture_cli_records_real_exit_and_validates_its_log(self):
        log = self.root / 'captured.log'
        receipt = self.root / 'captured.json'
        command = [sys.executable, str(Path(__file__).with_name('capture_process.py')),
                   '--receipt', str(receipt), '--log', str(log), '--run-id', 'captured-run', '--',
                   sys.executable, '-c',
                   "print('running 1 test\\ntest result: ok. 1 passed; 0 failed; 0 ignored')"]
        result = subprocess.run(command, check=False, capture_output=True, text=True, timeout=10)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(validate_run(receipt, run_id='captured-run',
                                      command=[sys.executable, '-c',
                                               "print('running 1 test\\ntest result: ok. 1 passed; 0 failed; 0 ignored')"],
                                      log_path=log)[0]['exit_code'], 0)
        self.assertNotIn('source_commit', json.loads(receipt.read_text()))

    def test_capture_cli_preserves_nonzero_exit(self):
        log = self.root / 'failed.log'
        receipt = self.root / 'failed.json'
        command = [sys.executable, str(Path(__file__).with_name('capture_process.py')),
                   '--receipt', str(receipt), '--log', str(log), '--run-id', 'failed-run', '--',
                   sys.executable, '-c', 'raise SystemExit(7)']
        result = subprocess.run(command, check=False, capture_output=True, text=True, timeout=10)
        self.assertEqual(result.returncode, 7)
        self.assertEqual(json.loads(receipt.read_text())['exit_code'], 7)

    def test_capture_cli_times_out_after_child_closes_output(self):
        log = self.root / 'closed-output.log'
        receipt = self.root / 'closed-output.json'
        command = [sys.executable, str(Path(__file__).with_name('capture_process.py')),
                   '--receipt', str(receipt), '--log', str(log), '--run-id', 'closed-output-run',
                   '--timeout-seconds', '1', '--', sys.executable, '-c',
                   'import os,time; os.close(1); os.close(2); time.sleep(5)']
        start = time.monotonic()
        result = subprocess.run(command, check=False, capture_output=True, text=True, timeout=4)
        self.assertEqual(result.returncode, 124, result.stderr)
        self.assertLess(time.monotonic() - start, 3)
        self.assertEqual(json.loads(receipt.read_text())['exit_code'], 124)

    def test_capture_cli_refuses_existing_log_and_receipt(self):
        capture_cli = Path(__file__).with_name('capture_process.py')
        for existing in ('log', 'receipt'):
            log = self.root / f'existing-{existing}.log'
            receipt = self.root / f'existing-{existing}.json'
            target = log if existing == 'log' else receipt
            target.write_text('preserve me')
            result = subprocess.run([sys.executable, str(capture_cli), '--receipt', str(receipt),
                                     '--log', str(log), '--run-id', f'existing-{existing}', '--',
                                     sys.executable, '-c', 'print("should not run")'],
                                    check=False, capture_output=True, text=True, timeout=10)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(target.read_text(), 'preserve me')

    def test_ci_writer_requires_evidence_and_never_replaces_existing_output(self):
        repo = self.root / 'repo'
        artifact = self.root / 'artifact'
        repo.mkdir()
        artifact.mkdir()
        evidence = self.root / 'evidence.json'
        evidence.write_text('{}')
        output = self.root / 'receipt.json'
        output.write_text('historical\n')
        script = (Path(__file__).resolve().parents[1] /
                  'design_docs/receipts/audio_foundation_20261011/ci/write-receipt.py')
        result = subprocess.run([sys.executable, str(script), '--repo', str(repo),
                                 '--artifact', str(artifact), '--output', str(output),
                                 '--process-evidence', str(evidence)],
                                check=False, capture_output=True, text=True, timeout=10)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('missing process evidence', result.stderr)
        self.assertEqual(output.read_text(), 'historical\n')

    def test_ci_writer_validates_temporary_receipts_end_to_end(self):
        repo = self.root / 'repo'
        artifact = self.root / 'artifact'
        evidence_dir = self.root / 'evidence'
        repo.mkdir()
        artifact.mkdir()
        evidence_dir.mkdir()
        for name in ('Cargo.lock', 'ports/hocket/Cargo.lock',
                     'ports/redshank/Cargo.lock', 'ports/ringdown/Cargo.lock'):
            path = repo / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text('fixture lock\n')
        subprocess.run(['git', 'init', '-q', str(repo)], check=True, timeout=10)
        subprocess.run(['git', '-C', str(repo), 'config', 'user.email', 'fixture@example.invalid'],
                       check=True, timeout=10)
        subprocess.run(['git', '-C', str(repo), 'config', 'user.name', 'Fixture'],
                       check=True, timeout=10)
        subprocess.run(['git', '-C', str(repo), 'add', '.'], check=True, timeout=10)
        subprocess.run(['git', '-C', str(repo), 'commit', '-qm', 'fixture'], check=True, timeout=10)
        capture_cli = Path(__file__).with_name('capture_process.py')
        evidence = {}

        def capture(key, log_relative, output):
            log = artifact / log_relative
            receipt = evidence_dir / f'{key}.json'
            command = [sys.executable, '-c', f'print({output!r})']
            call = [sys.executable, str(capture_cli), '--receipt', str(receipt),
                    '--log', str(log), '--run-id', f'fixture-{key}',
                    '--source-root', str(repo), '--', *command]
            result = subprocess.run(call, check=False, capture_output=True, text=True, timeout=10)
            if result.returncode != 0:
                raise AssertionError(result.stderr)
            evidence[key] = {'receipt': receipt.name, 'run_id': f'fixture-{key}',
                             'command': command, 'log': str(log.resolve())}

        totals = {'woodshed': (770, 0), 'hocket': (148, 1),
                  'redshank': (253, 7), 'ringdown': (157, 0)}
        log_names = {'woodshed': 'woodshed-tests-rerun.log',
                     'hocket': 'hocket-tests-rerun.log',
                     'redshank': 'redshank-tests-rerun.log',
                     'ringdown': 'ringdown-tests.log'}
        for name, (passed, ignored) in totals.items():
            capture(name, log_names[name],
                    f'running {passed + ignored} tests\ntest result: ok. {passed} passed; 0 failed; {ignored} ignored\n')
        for name in totals:
            capture('metadata-' + name, name + '-metadata.log', 'metadata ok')
        for key in ('workflow-validation', 'shell-syntax', 'whitespace'):
            capture(key, key + '.log', 'check ok')
        manifest = evidence_dir / 'process-evidence.json'
        manifest.write_text(json.dumps(evidence))
        output = self.root / 'new-receipt.json'
        script = (Path(__file__).resolve().parents[1] /
                  'design_docs/receipts/audio_foundation_20261011/ci/write-receipt.py')
        result = subprocess.run([sys.executable, str(script), '--repo', str(repo),
                                 '--artifact', str(artifact), '--output', str(output),
                                 '--process-evidence', str(manifest)],
                                check=False, capture_output=True, text=True, timeout=10)
        self.assertEqual(result.returncode, 0, result.stderr)
        receipt = json.loads(output.read_text())
        self.assertEqual(receipt['cpu_totals'], {'passed': 1328, 'failed': 0, 'ignored': 8})
        self.assertEqual(receipt['historical_context']['date'], '2026-10-11')
        self.assertEqual(receipt['historical_context']['branch'], 'codex/audio-foundation-ci-20261011')
        self.assertIn('original 2026-10-11 receipt context only',
                      receipt['historical_context']['provenance_note'])
        self.assertEqual(receipt['historical_context']['expected_test_totals']['woodshed'],
                         {'passed': 770, 'ignored': 0})
        self.assertNotIn('date', receipt)
        self.assertNotIn('branch', receipt)
        self.assertNotIn('source_base', receipt)
        self.assertNotIn('cpu_commands', receipt)
        observed_test = receipt['cpu_tests']['woodshed']
        self.assertEqual(observed_test['source_commit'], receipt['assembly_context']['head_at_assembly'])
        self.assertEqual(observed_test['cwd'], str(Path.cwd()))
        self.assertEqual(observed_test['command'], [sys.executable, '-c',
                                                    "print('running 770 tests\\ntest result: ok. 770 passed; 0 failed; 0 ignored\\n')"])
        self.assertEqual(receipt['workflow_validation']['command'][0], sys.executable)
        self.assertTrue(all(check['exit_code'] == 0 for check in receipt['locked_metadata'].values()))
        self.assertEqual(receipt['workflow_validation']['exit_code'], 0)


if __name__ == '__main__':
    unittest.main()
