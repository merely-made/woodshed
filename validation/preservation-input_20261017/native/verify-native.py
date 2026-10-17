"""Read-only verification of recorded native artifacts and historical Git source."""
from pathlib import Path
import hashlib
import json
import re
import struct
import subprocess

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[2]


def load(path):
    return json.loads(path.read_text())


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify_hashes():
    manifest = load(ROOT / 'sha256.json')
    actual = {str(p.relative_to(ROOT)) for p in ROOT.rglob('*')
              if p.is_file() and p.name != 'sha256.json'}
    assert actual == set(manifest), 'artifact set changed'
    for name, expected in manifest.items():
        assert digest(ROOT / name) == expected, name
    return len(manifest)


def verify_source(source_name, hashes_name):
    source = load(ROOT / source_name)
    hashes = load(ROOT / hashes_name)
    assert not source['worktree']
    assert digest(ROOT / hashes_name) == source['source_hashes_sha256']
    for name, expected in hashes.items():
        data = subprocess.check_output(
            ['git', 'show', source['revision'] + ':' + name], cwd=REPO)
        assert hashlib.sha256(data).hexdigest() == expected, name
    return source, len(hashes)


def verify_phase(name, expected, source):
    folder = ROOT / name
    text = (folder / 'scenario.done').read_text()
    assert text.startswith('RESULT ok\n') and 'FAIL' not in text
    count = expected['captures']
    assert f'frames: {count} captured, 0 blank, {count} distinct digests' in text
    frames = tuple(map(int, re.search(
        r'presentation: (\d+) frames, (\d+) redraws, (\d+) unpresented redraws', text).groups()))
    assert frames == tuple(expected[k] for k in
                           ['presented_frames', 'redraws', 'unpresented_redraws'])
    images = list(folder.glob('*.png'))
    assert len(images) == count and expected['process_exited']
    for path in images:
        data = path.read_bytes()
        assert data[:8] == b'\x89PNG\r\n\x1a\n'
        assert struct.unpack('>II', data[16:24]) == (2360, 2000)
    launch = load(folder / 'launch.json')
    assert launch['source'] == source
    assert digest(folder / 'scenario.scn') == launch['scenario_sha256']
    assert launch['binary_sha256'] == source['binaries']['woodshed']
    for filename, expected_hash in load(folder / 'phase-hashes.json').items():
        assert digest(folder / filename) == expected_hash, filename


def main():
    artifacts = verify_hashes()
    persona, old_count = verify_source('source.json', 'source-hashes.json')
    close, current_count = verify_source('close-source.json', 'close-source-hashes.json')
    receipt = load(ROOT / 'receipt.json')
    assert receipt['status'] == 'qualified-bounded-woodshed-native'
    assert receipt['persona_source_revision'] == persona['revision']
    assert receipt['close_source_revision'] == close['revision']
    for name, expected in receipt['phases'].items():
        verify_phase(name, expected, persona if name.startswith('persona') else close)
    assert sum(p['captures'] for p in receipt['phases'].values()) == 12
    for name in ['close', 'quarantine-reopen']:
        actions = load(ROOT / name / 'native-close-actions.json')
        assert actions['process_exited'] and actions['original_quarantined_files_unchanged']
        assert actions['original_hashes'] == load(ROOT / 'quarantine-before.json')
    completion = load(ROOT / 'completion.json')
    assert completion['all_owned_app_processes_exited'] and completion['keeper_exited']
    assert completion['keeper_original_file_hashes_unchanged']
    assert completion['keeper_original_file_count'] == 4
    assert receipt['all_owned_processes_exited'] and receipt['original_quarantined_files_unchanged']
    print(f'Verified {artifacts} artifact hashes, {old_count}+{current_count} historical source bindings, twelve native captures and recorded final dispositions.')


if __name__ == '__main__':
    main()
