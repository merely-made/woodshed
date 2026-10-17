"""Read-only verification of published P6 evidence, including exact sealed reopen."""
from pathlib import Path
import hashlib
import json
import re
import struct
import subprocess
import sys

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parent.parent


def load(path):
    return json.loads(path.read_text())


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


manifest = load(ROOT / 'sha256.json')
actual = {str(p.relative_to(ROOT)) for p in ROOT.rglob('*')
          if p.is_file() and p.name != 'sha256.json'}
assert actual == set(manifest), 'artifact set changed'
for name, expected in manifest.items():
    assert digest(ROOT / name) == expected, name
source = load(ROOT / 'source.json')
assert not source['worktree']
assert digest(ROOT / 'source-hashes.json') == source['source_hashes_sha256']
hashes = load(ROOT / 'source-hashes.json')
for name, expected in hashes.items():
    data = subprocess.check_output(['git', 'show', source['revision'] + ':' + name], cwd=REPO)
    assert hashlib.sha256(data).hexdigest() == expected, name
receipt = load(ROOT / 'receipt.json')
assert receipt['status'] == 'qualified' and receipt['native_source_revision'] == source['revision']
assert receipt['checks']['suite_total'] == 910
assert receipt['checks']['suite_total'] == sum(receipt['checks'][key] for key in
    ['audio', 'core_unit', 'core_card_loop_integration', 'core_cli', 'core_portability', 'desktop', 'views'])
for check in ['integrated-final', 'desktop-guard-final', 'candidate-build', 'final-fixture-check']:
    assert load(ROOT / 'checks' / (check + '.json'))['exit_code'] == 0
for name, expected in receipt['native']['phases'].items():
    folder = ROOT / 'native' / name
    text = (folder / 'scenario.done').read_text()
    assert text.startswith('RESULT ok\n') and 'FAIL' not in text
    count = expected['captures']
    assert f'frames: {count} captured, 0 blank, {count} distinct digests' in text
    assert tuple(map(int, re.search(
        r'presentation: (\d+) frames, (\d+) redraws, (\d+) unpresented redraws', text).groups())) == tuple(
            expected[key] for key in ['presented_frames', 'redraws', 'unpresented_redraws'])
    images = list(folder.glob('*.png'))
    assert len(images) == count and expected['process_exited']
    for path in images:
        data = path.read_bytes()
        assert data[:8] == b'\x89PNG\r\n\x1a\n'
        assert struct.unpack('>II', data[16:24]) in [(2360, 2000), (840, 2200)]
    launch = load(folder / 'launch.json')
    assert launch['source'] == source
    assert launch['binary_sha256'] == source['binaries']['woodshed']
    assert digest(folder / 'scenario.scn') == launch['scenario_sha256']
    for filename, expected_hash in load(folder / 'phase-hashes.json').items():
        assert digest(folder / filename) == expected_hash, filename
seed = load(ROOT / 'native/seed/p6-final.json')
reopen = load(ROOT / 'native/reopen/p6-reopened.json')
assert all(seed[key] == reopen[key] for key in ['set', 'owner', 'history'])
assert len(seed['set']['cards']) == 3 and seed['set']['cursor'] == 2
assert seed['set']['loop_mode'] == 'Card' and len(seed['history']) == 22
assert seed['sealed_exact'] and reopen['sealed_exact'] and reopen['transient_cleared']
assert seed['sample_rate'] == 48000 and seed['program']['status'] == 'Complete'
copy = load(ROOT / 'native/reopen/expected-copy.json')
assert copy['copied_after_seed_exit']
assert copy['sha256'] == digest(ROOT / 'native/seed/p6-final.json')
assert copy['sha256'] == digest(ROOT / 'native/reopen/p6-final.json')
assert load(ROOT / 'completion.json')['all_owned_app_processes_exited']
assert load(ROOT / 'completion.json')['keeper_original_file_hashes_unchanged']
subprocess.run([sys.executable, str(REPO / 'validation/preservation-input_20261017/native/verify-native.py')], check=True)
print(f'Verified {len(manifest)} P6 artifact hashes, {len(hashes)} historical source bindings, eight captures and exact sealed Set/owner/22-row history reopening.')
