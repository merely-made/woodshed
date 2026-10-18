from pathlib import Path
import hashlib
import json
import re
import subprocess
import argparse
import sys

if not __debug__:
    raise SystemExit('qualification is disabled under python -O; run with normal Python assertions enabled')

sys.path.insert(0, str(Path(__file__).resolve().parents[4] / 'validation'))
from evidence_process import validate_process, validate_run

parser = argparse.ArgumentParser(description='Assemble a CI receipt from retained artifacts and observed process receipts. Historical logs without process evidence are unverified and cannot qualify.')
parser.add_argument('--repo', type=Path, required=True, help='Repository whose lockfiles and current HEAD are inspected.')
parser.add_argument('--artifact', type=Path, required=True, help='Directory containing the logs referenced by process evidence.')
parser.add_argument('--output', type=Path, required=True, help='New receipt path; existing files are never overwritten.')
parser.add_argument('--process-evidence', type=Path, required=True, help='JSON mapping workspace names to run_id, command, started_at, ended_at, exit_code, log, and log_sha256 captured by the invoking wrapper.')
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
    tests[workspace] = {'exit_code': observed['exit_code'], 'run_id': observed['run_id'], 'passed': passed, 'failed': failed, 'ignored': ignored,
                        'scope': scopes[workspace], 'log': log,
                        'entry': 'validate-workspace.sh test ' + workspace if workspace != 'hocket'
                        else 'cargo +1.97.1 test --manifest-path ports/hocket/Cargo.toml --locked -p hocket-model -p hocket-engine -p hocket-genet',
                        'outside_repository_cargo_config': True}

locks = {name: hashlib.sha256((repo / path).read_bytes()).hexdigest() for name, path in {
    'woodshed': 'Cargo.lock', 'hocket': 'ports/hocket/Cargo.lock',
    'redshank': 'ports/redshank/Cargo.lock', 'ringdown': 'ports/ringdown/Cargo.lock'
}.items()}
metadata = {}
for name in logs:
    record, _ = observe('metadata-' + name, expected_log=name + '-metadata.log')
    metadata[name] = {'exit_code': record['exit_code'], 'run_id': record['run_id'], 'log': name + '-metadata.log'}
workflow, _ = observe('workflow-validation')
shell_syntax, _ = observe('shell-syntax')
whitespace, _ = observe('whitespace')
receipt = {
    'schema_version': 1,
    'date': '2026-10-11',
    'lane': 'F1 CI, release boundaries, and verified declaration cleanup',
    'worktree': str(repo),
    'branch': 'codex/audio-foundation-ci-20261011',
    'source_base': '95abb13f35029aa7e0c100043ba03532d4f6905a',
    'commits': ['84ad2010b6098ba1474204d3ac67fd235eb0a1e7', '73df29050b67cb9196c7cf04012acfedd044af98'],
    'head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=repo, text=True).strip(),
    'host': 'macOS arm64',
    'rust': '1.97.1',
    'lockfile_sha256': locks,
    'locked_metadata': metadata,
    'cpu_tests': tests,
    'cpu_commands': {
        'working_directory': '/Users/markik/Code/testing',
        'CARGO_BUILD_JOBS': '2',
        'woodshed': {
            'command': str(repo / 'scripts/validate-workspace.sh') + ' test woodshed',
            'CARGO_TARGET_DIR': '/Users/markik/Code/worktrees/woodshed-audit-20261011/target',
        },
        'redshank': {
            'command': str(repo / 'scripts/validate-workspace.sh') + ' test redshank',
            'CARGO_TARGET_DIR': '/Users/markik/Code/worktrees/woodshed-audit-20261011/ports/redshank/target',
        },
        'hocket': {
            'command': 'cargo +1.97.1 test --manifest-path ' + str(repo / 'ports/hocket/Cargo.toml')
                       + ' --locked -p hocket-model -p hocket-engine -p hocket-genet',
            'CARGO_TARGET_DIR': '/Users/markik/Code/worktrees/woodshed-audit-20261011/ports/hocket/target',
        },
        'ringdown': {
            'command': str(repo / 'scripts/validate-workspace.sh') + ' test ringdown',
            'CARGO_TARGET_DIR': str(artifact / 'ringdown-target'),
        },
    },
    'cpu_totals': {'passed': sum(test['passed'] for test in tests.values()),
                   'failed': 0, 'ignored': sum(test['ignored'] for test in tests.values())},
    'workflow_validation': {'tool': 'actionlint 1.7.12', 'exit_code': workflow['exit_code'], 'run_id': workflow['run_id'],
                            'scope': 'all four .github/workflows/*.yml; optional shellcheck disabled'},
    'shell_syntax': {'command': 'bash -n scripts/validate-workspace.sh', 'exit_code': shell_syntax['exit_code'], 'run_id': shell_syntax['run_id']},
    'whitespace': {'command': 'git diff --check', 'exit_code': whitespace['exit_code'], 'run_id': whitespace['run_id']},
    'release_fixture_results': 'release-fixture-results.json',
    'linux_target_dependency_inspection': {
        'woodshed': ['alsa-sys', 'yeslogic-fontconfig-sys', 'libdbus-sys'],
        'hocket': ['alsa-sys', 'yeslogic-fontconfig-sys'],
        'redshank': ['alsa-sys', 'yeslogic-fontconfig-sys'],
        'ringdown': ['libdbus-sys'],
        'scope': 'locked cargo tree for x86_64-unknown-linux-gnu, normal/build edges',
        'not_a_linux_build': True,
    },
    'qualification_limits': [
        'Committed Hocket script adds hocket-project for coordinator integration; this base has no hocket-project package. Its moved tests/dependency tree and final script scope require integrated-head verification.',
        'Mac CPU results do not qualify execution on hosted Linux/Windows runners.',
        'PowerShell package tests use fixture executable bytes on macOS; no actual Windows binary/build/native runtime qualified.',
        'GitHub dispatch/publish behavior has not run; workflows are statically validated.',
        'One custody receipt and seven Redshank device/fixture tests remain ignored.',
        'Native capture, audio/Bluetooth hardware, and fresh-process reopen were not part of this lane.',
    ],
    'operational_note': 'Initial long test runs emitted a shell EOF diagnostic because the validation script was edited after launch to add hocket-project. Frozen-script reruns/equivalent pre-extraction Hocket command all exit 0; only these rerun logs qualify the wrapper result.',
    'production_changes': ['workspace CI scopes', 'Woodshed Windows release tag/channel selection',
                            'validation script', 'derived README guidance', 'unused Hocket cpal/rtrb/basedrop declarations'],
    'unchanged': ['all lockfiles', 'canonical plans/index', 'updater transports', 'product Rust source'],
}
output = args.output.resolve()
output.parent.mkdir(parents=True, exist_ok=True)
with output.open('x', encoding='utf-8') as destination:
    destination.write(json.dumps(receipt, indent=2) + '\n')
print(json.dumps(receipt['cpu_totals']))
