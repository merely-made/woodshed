from pathlib import Path
import hashlib
import json
import re
import subprocess

repo = Path('/Users/markik/Code/worktrees/woodshed-ci-release-20261011')
artifact = Path('/Users/markik/Code/testing/woodshed-ci-release-20261011')
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
tests = {}
for workspace, log in logs.items():
    contents = (artifact / log).read_text()
    assert 'unexpected EOF' not in contents
    groups = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored', contents)
    assert groups, workspace
    passed, failed, ignored = (sum(int(group[index]) for group in groups) for index in range(3))
    assert failed == 0
    assert (passed, ignored) == expected[workspace], (workspace, passed, ignored)
    tests[workspace] = {'exit_code': 0, 'passed': passed, 'failed': failed, 'ignored': ignored,
                        'scope': scopes[workspace], 'log': log,
                        'entry': 'validate-workspace.sh test ' + workspace if workspace != 'hocket'
                        else 'cargo +1.97.1 test --manifest-path ports/hocket/Cargo.toml --locked -p hocket-model -p hocket-engine -p hocket-genet',
                        'outside_repository_cargo_config': True}

locks = {name: hashlib.sha256((repo / path).read_bytes()).hexdigest() for name, path in {
    'woodshed': 'Cargo.lock', 'hocket': 'ports/hocket/Cargo.lock',
    'redshank': 'ports/redshank/Cargo.lock', 'ringdown': 'ports/ringdown/Cargo.lock'
}.items()}
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
    'locked_metadata': {name: {'exit_code': 0, 'log': name + '-metadata.log'} for name in logs},
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
    'workflow_validation': {'tool': 'actionlint 1.7.12', 'exit_code': 0,
                            'scope': 'all four .github/workflows/*.yml; optional shellcheck disabled'},
    'shell_syntax': {'command': 'bash -n scripts/validate-workspace.sh', 'exit_code': 0},
    'whitespace': {'command': 'git diff --check', 'exit_code': 0},
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
(artifact / 'receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
print(json.dumps(receipt['cpu_totals']))
