from pathlib import Path
import hashlib
import json
import os
import re
import shutil
import subprocess
import tempfile
import textwrap

source = Path('/Users/markik/Code/worktrees/woodshed-ci-release-20261011')
artifact = Path('/Users/markik/Code/testing/woodshed-ci-release-20261011')
pwsh = artifact / 'tools/powershell/pwsh'
workflow = (source / '.github/workflows/release-windows.yml').read_text()
block = re.search(
    r'- name: Package portable ZIP\n(?:        [^|\n]+\n)*        run: \|\n(?P<run>(?:          [^\n]*\n|\n)+)',
    workflow,
)
assert block is not None
assert "prerelease: ${{ contains(steps.package.outputs.version, '-') }}" in workflow
run = textwrap.dedent(block['run'])
(artifact / 'release-version-fixture.ps1').write_text(run)
cases = [
    ('branch', 'main', '0.1.0-alpha.0.manual', True),
    ('tag', 'woodshed-v0.2.3', '0.2.3', False),
    ('tag', 'woodshed-v0.2.3-alpha.1', '0.2.3-alpha.1', True),
    ('tag', 'hocket-v0.2.3', None, None),
    ('tag', 'redshank-v0.2.3', None, None),
    ('tag', 'ringdown-v0.2.3', None, None),
    ('tag', 'v0.2.3', None, None),
    ('tag', 'woodshed-vjunk', None, None),
]
results = []
for index, (kind, ref, expected, expected_prerelease) in enumerate(cases):
    fixture = Path(tempfile.mkdtemp(prefix=f'{index}-', dir=artifact / 'release-fixtures'))
    (fixture / 'scripts').mkdir(parents=True, exist_ok=True)
    (fixture / 'target/release').mkdir(parents=True, exist_ok=True)
    shutil.copy2(source / 'scripts/package-windows.ps1', fixture / 'scripts')
    for name in ['README.md', 'LICENSE', 'Cargo.lock']:
        shutil.copy2(source / name, fixture / name)
    # Qualifies package mechanics only, never an actual Windows executable.
    (fixture / 'target/release/woodshed-genet.exe').write_bytes(b'packaging fixture\n')
    (fixture / 'package.ps1').write_text(run)
    env = os.environ.copy()
    env.pop('CARGO_TARGET_DIR', None)
    env['GITHUB_REF_TYPE'] = kind
    env['GITHUB_REF_NAME'] = ref
    output = fixture / 'github-output.txt'
    env['GITHUB_OUTPUT'] = str(output)
    process = subprocess.run(
        [str(pwsh), '-NoLogo', '-NoProfile', '-File', str(fixture / 'package.ps1')],
        cwd=fixture, env=env, capture_output=True, text=True,
    )
    success = process.returncode == 0
    assert success == (expected is not None), (ref, process.stdout, process.stderr)
    if expected is not None:
        archive = fixture / f'dist/Woodshed-{expected}-windows-x86_64.zip'
        assert archive.is_file(), (ref, process.stdout)
        checksum = archive.with_name(archive.name + '.sha256').read_text().split()[0]
        assert checksum == hashlib.sha256(archive.read_bytes()).hexdigest()
        output_version = output.read_text().strip().removeprefix('version=')
        assert output_version == expected
        assert ('-' in output_version) == expected_prerelease
    else:
        assert not list(fixture.glob('dist/*.zip'))
    results.append({'ref_type': kind, 'ref_name': ref, 'version': expected,
                    'prerelease': expected_prerelease,
                    'exit_code': process.returncode, 'passed': True})

result = {
    'tool': 'PowerShell 7.6.6 on macOS arm64',
    'scope': 'Exact workflow package block and real package-windows.ps1 with fixture bytes',
    'not_qualified': ['GitHub dispatch behavior', 'Windows binary/build/native runtime'],
    'cases': results,
}
(artifact / 'release-fixture-results.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(result, indent=2))
