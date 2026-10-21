from pathlib import Path
import hashlib
import json
import re
import subprocess
import argparse
import sys
from datetime import datetime, timezone

if not __debug__:
    raise SystemExit('qualification is disabled under python -O; run with normal Python assertions enabled')

sys.path.insert(0, str(Path(__file__).resolve().parents[4] / 'validation'))
from evidence_process import validate_process, validate_run

parser = argparse.ArgumentParser(description='Assemble a CI receipt from retained artifacts and observed process receipts. Historical logs without process evidence are unverified and cannot qualify.')
parser.add_argument('--repo', type=Path, required=True, help='Repository whose lockfiles and current HEAD are inspected.')
parser.add_argument('--artifact', type=Path, required=True, help='Directory containing the logs referenced by process evidence.')
parser.add_argument('--output', type=Path, required=True, help='New receipt path; existing files are never overwritten.')
parser.add_argument('--process-evidence', type=Path, required=True, help='JSON mapping workspace names to captured process receipts. Each receipt must bind run_id, command, timestamps, exit, log path, and digest; cwd/source_commit are reported when captured.')
args = parser.parse_args()
process_evidence = json.loads(args.process_evidence.read_text())

repo = args.repo.resolve()
artifact = args.artifact.resolve()
logs = {
    'woodshed': 'woodshed-tests-rerun.log',
    'hocket': 'hocket-tests-rerun.log',
    'redshank': 'redshank-tests-rerun.log',
    'ringdown': 'ringdown-tests.log',
}
scopes = {
    'woodshed': ['woodshedding', 'audio-primitives', 'timed-text', 'woodshed-audio',
                 'woodshed-core', 'woodshed-graph', 'woodshed-views', 'woodshed-instrument'],
    'hocket': ['hocket-model', 'hocket-engine', 'hocket-genet'],
    'redshank': ['redshank-model', 'redshank-storage', 'redshank-cache', 'redshank-feed',
                 'redshank-playback', 'redshank-surfaces', 'redshank-desktop'],
    'ringdown': ['--workspace'],
}
expected = {'woodshed': (770, 0), 'hocket': (148, 1), 'redshank': (253, 7), 'ringdown': (157, 0)}

def observe(key, *, expected_log=None, test_log=False):
    evidence = process_evidence.get(key)
    if not evidence:
        raise SystemExit(f'{key}: missing process evidence; historical result is unverified')
    log_path = Path(evidence['log']).resolve()
    if not log_path.is_relative_to(artifact):
        raise SystemExit(f'{key}: evidence log escapes the artifact directory')
    if expected_log is not None and log_path != (artifact / expected_log).resolve():
        raise SystemExit(f'{key}: process evidence points to a different log')
    validator = validate_run if test_log else validate_process
    result = validator(args.process_evidence.parent / evidence['receipt'],
                       run_id=evidence.get('run_id'), command=evidence.get('command'),
                       log_path=log_path)
    if not test_log and result[0]['exit_code'] != 0:
        raise SystemExit(f'{key}: observed process exit was {result[0]["exit_code"]}')
    return result

tests = {}
for workspace, log in logs.items():
    observed, groups, _ = observe(workspace, expected_log=log, test_log=True)
    passed, failed, ignored = (sum(int(group[index + 1]) for group in groups) for index in range(1, 4))
    if failed != 0 or (passed, ignored) != expected[workspace]:
        raise SystemExit(f'{workspace}: suite totals disagree with expected {expected[workspace]}')
    tests[workspace] = {
        'exit_code': observed['exit_code'], 'run_id': observed['run_id'],
        'command': observed.get('command', 'unknown'),
        'cwd': observed.get('cwd', 'unknown'),
        'source_commit': observed.get('source_commit', 'unknown'),
        'passed': passed, 'failed': failed, 'ignored': ignored,
        'log': str((artifact / log).resolve()),
        'log_sha256': observed['log_sha256'],
    }

locks = {name: hashlib.sha256((repo / path).read_bytes()).hexdigest() for name, path in {
    'woodshed': 'Cargo.lock', 'hocket': 'ports/hocket/Cargo.lock',
    'redshank': 'ports/redshank/Cargo.lock', 'ringdown': 'ports/ringdown/Cargo.lock'
}.items()}
metadata = {}
for name in logs:
    record, _ = observe('metadata-' + name, expected_log=name + '-metadata.log')
    metadata[name] = {
        'exit_code': record['exit_code'], 'run_id': record['run_id'],
        'command': record.get('command', 'unknown'), 'cwd': record.get('cwd', 'unknown'),
        'source_commit': record.get('source_commit', 'unknown'),
        'log': str((artifact / (name + '-metadata.log')).resolve()),
        'log_sha256': record['log_sha256'],
    }
workflow, _ = observe('workflow-validation')
shell_syntax, _ = observe('shell-syntax')
whitespace, _ = observe('whitespace')
receipt = {
    'schema_version': 1,
    'historical_context': {
        'provenance_note': 'This block describes the original 2026-10-11 receipt context only; it is not observed context for process runs recorded below.',
        'date': '2026-10-11',
        'lane': 'F1 CI, release boundaries, and verified declaration cleanup',
        'branch': 'codex/audio-foundation-ci-20261011',
        'source_base': '95abb13f35029aa7e0c100043ba03532d4f6905a',
        'commits': ['84ad2010b6098ba1474204d3ac67fd235eb0a1e7', '73df29050b67cb9196c7cf04012acfedd044af98'],
        'host': 'macOS arm64', 'rust': '1.97.1',
        'expected_test_totals': {name: {'passed': counts[0], 'ignored': counts[1]}
                                 for name, counts in expected.items()},
        'expected_test_scopes': scopes,
        'historical_operational_note': 'Initial long test runs emitted a shell EOF diagnostic; only the retained rerun logs were considered for the historical result.',
        'qualification_limits': [
            'Committed Hocket script adds hocket-project for coordinator integration; this base has no hocket-project package. Its moved tests/dependency tree and final script scope require integrated-head verification.',
            'Mac CPU results do not qualify execution on hosted Linux/Windows runners.',
            'PowerShell package tests use fixture executable bytes on macOS; no actual Windows binary/build/native runtime qualified.',
            'GitHub dispatch/publish behavior has not run; workflows are statically validated.',
            'One custody receipt and seven Redshank device/fixture tests remain ignored.',
            'Native capture, audio/Bluetooth hardware, and fresh-process reopen were not part of this lane.',
        ],
        'linux_target_dependency_inspection': {
            'woodshed': ['alsa-sys', 'yeslogic-fontconfig-sys', 'libdbus-sys'],
            'hocket': ['alsa-sys', 'yeslogic-fontconfig-sys'],
            'redshank': ['alsa-sys', 'yeslogic-fontconfig-sys'],
            'ringdown': ['libdbus-sys'],
            'scope': 'locked cargo tree for x86_64-unknown-linux-gnu, normal/build edges',
            'not_a_linux_build': True,
        },
        'production_changes': ['workspace CI scopes', 'Woodshed Windows release tag/channel selection',
                               'validation script', 'derived README guidance', 'unused Hocket cpal/rtrb/basedrop declarations'],
        'unchanged': ['all lockfiles', 'canonical plans/index', 'updater transports', 'product Rust source'],
    },
    'assembly_context': {
        'repository_argument': str(repo),
        'head_at_assembly': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=repo, text=True).strip(),
        'lockfile_sha256_at_assembly': locks,
    },
    'assembled_at': datetime.now(timezone.utc).isoformat(),
    'locked_metadata': metadata,
    'cpu_tests': tests,
    'cpu_totals': {'passed': sum(test['passed'] for test in tests.values()),
                   'failed': 0, 'ignored': sum(test['ignored'] for test in tests.values())},
    'workflow_validation': {'exit_code': workflow['exit_code'], 'run_id': workflow['run_id'],
                            'command': workflow.get('command', 'unknown'), 'cwd': workflow.get('cwd', 'unknown'),
                            'source_commit': workflow.get('source_commit', 'unknown'),
                            'log': workflow['log'], 'log_sha256': workflow['log_sha256']},
    'shell_syntax': {'exit_code': shell_syntax['exit_code'], 'run_id': shell_syntax['run_id'],
                     'command': shell_syntax.get('command', 'unknown'), 'cwd': shell_syntax.get('cwd', 'unknown'),
                     'source_commit': shell_syntax.get('source_commit', 'unknown'),
                     'log': shell_syntax['log'], 'log_sha256': shell_syntax['log_sha256']},
    'whitespace': {'exit_code': whitespace['exit_code'], 'run_id': whitespace['run_id'],
                   'command': whitespace.get('command', 'unknown'), 'cwd': whitespace.get('cwd', 'unknown'),
                   'source_commit': whitespace.get('source_commit', 'unknown'),
                   'log': whitespace['log'], 'log_sha256': whitespace['log_sha256']},
}
output = args.output.resolve()
output.parent.mkdir(parents=True, exist_ok=True)
with output.open('x', encoding='utf-8') as destination:
    destination.write(json.dumps(receipt, indent=2) + '\n')
print(json.dumps(receipt['cpu_totals']))
