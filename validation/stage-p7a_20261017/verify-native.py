"""Read-only P7a artifact, historical source, sealed reopen and exact WAV checks."""
from pathlib import Path
import hashlib
import json
import re
import struct
import subprocess

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parent.parent


def load(path):
    return json.loads(path.read_text())


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


manifest = load(ROOT / 'sha256.json')
actual = {str(p.relative_to(ROOT)) for p in ROOT.rglob('*')
          if p.is_file() and p.name != 'sha256.json'}
assert actual == set(manifest), 'published artifact set changed'
for name, expected in manifest.items():
    assert sha(ROOT / name) == expected, name
source = load(ROOT / 'source.json')
assert not source['worktree']
assert sha(ROOT / 'source-hashes.json') == source['source_hashes_sha256']
sources = load(ROOT / 'source-hashes.json')
for name, expected in sources.items():
    data = subprocess.check_output(['git', 'show', source['revision'] + ':' + name], cwd=REPO)
    assert hashlib.sha256(data).hexdigest() == expected, name
receipt = load(ROOT / 'receipt.json')
assert receipt['status'] == 'qualified-bounded-p7a'
assert receipt['native_source_revision'] == source['revision']
checks = receipt['checks']
assert checks['suite_total'] == 925 == sum(checks[key] for key in
    ['audio', 'core_unit', 'core_card_loop_integration', 'core_cli', 'core_portability', 'desktop', 'views'])
for name in ['candidate-gate', 'candidate-build', 'sealed-reference']:
    assert load(ROOT / 'checks' / (name + '.json'))['exit_code'] == 0
for name, phase in receipt['native']['phases'].items():
    folder = ROOT / 'native' / name
    text = (folder / 'scenario.done').read_text()
    assert text.startswith('RESULT ok\n') and 'FAIL' not in text
    count = phase['captures']
    assert f'frames: {count} captured, 0 blank, {count} distinct digests' in text
    observed = tuple(map(int, re.search(
        r'presentation: (\d+) frames, (\d+) redraws, (\d+) unpresented redraws', text).groups()))
    assert observed == tuple(phase[key] for key in ['presented_frames', 'redraws', 'unpresented_redraws'])
    images = list(folder.glob('*.png'))
    assert len(images) == count and phase['process_exited']
    for path in images:
        data = path.read_bytes()
        assert data[:8] == b'\x89PNG\r\n\x1a\n'
        assert struct.unpack('>II', data[16:24]) in [(2360, 2000), (840, 2200)]
    launch = load(folder / 'launch.json')
    assert launch['source'] == source
    assert launch['binary_sha256'] == source['binaries']['woodshed']
    assert sha(folder / 'scenario.scn') == launch['scenario_sha256']
    for filename, expected in load(folder / 'phase-hashes.json').items():
        assert sha(folder / filename) == expected, filename
seed = load(ROOT / 'native/seed/p7a-final.json')
reopen = load(ROOT / 'native/reopen/p7a-reopened.json')
for key in ['set', 'owner', 'looper', 'saved_backing', 'manifest', 'captures',
            'admitted', 'backend_admitted', 'requested', 'wav_frames', 'wav_digest']:
    assert seed[key] == reopen[key], key
assert seed['admitted'] == seed['backend_admitted'] == seed['requested']
assert seed['sealed_exact'] and reopen['sealed_exact']
assert seed['streams'] == reopen['streams'] == 1 and seed['rate'] == reopen['rate'] == 48000
assert not seed['playing'] and not reopen['playing']
assert seed['input_unopened'] and reopen['input_unopened']
assert len(seed['set']['cards']) == 3 and len(seed['saved_backing']['bars']) == 4
assert len(seed['captures']) == 1 and seed['captures'][0]['id'] == {'card': 2, 'set': 1}
assert seed['captures'][0]['frames'] == 96000 and seed['captures'][0]['sealed_pcm_exact']
assert seed['selected_index'] == 0 and seed['selected_has_capture']
reordered = load(ROOT / 'native/seed/p7a-reordered.json')
assert reordered['selected_index'] == 0 and reordered['selected_has_capture']
refused = load(ROOT / 'native/seed/p7a-refused.json')
assert not refused['requested_ready'] and refused['admitted'] == refused['backend_admitted']
assert refused['admitted'] == seed['admitted']
copy = load(ROOT / 'native/reopen/expected-copy.json')
assert copy['copied_after_seed_exit']
assert copy['sha256'] == sha(ROOT / 'native/seed/p7a-final.json') == sha(ROOT / 'native/reopen/p7a-final.json')
paths = [ROOT / 'native/seed/loop-export.wav', ROOT / 'native/reopen/loop-export.wav',
         ROOT / 'reference/loop-reference.wav']
assert all(sha(path) == receipt['native']['worker_wav_sha256'] for path in paths)
data = paths[0].read_bytes()
assert data[:4] == b'RIFF' and data[8:12] == b'WAVE'
chunks = {}
position = 12
while position + 8 <= len(data):
    tag, length = struct.unpack_from('<4sI', data, position)
    chunks[tag] = data[position + 8:position + 8 + length]
    position += 8 + length + length % 2
fmt = chunks[b'fmt ']
format_tag, channels, rate, byte_rate, align, bits = struct.unpack_from('<HHIIHH', fmt)
assert (channels, rate, byte_rate, align, bits) == (1, 48000, 192000, 4, 32)
if format_tag == 0xfffe:
    assert len(fmt) == 40
    extension_size, valid_bits, channel_mask = struct.unpack_from('<HHI', fmt, 16)
    assert (extension_size, valid_bits, channel_mask) == (22, 32, 1)
    assert fmt[24:40] == bytes.fromhex('0300000000001000800000aa00389b71')
else:
    assert format_tag == 3
assert len(chunks[b'data']) // 4 == seed['wav_frames'] == 160800
comparison = load(ROOT / 'reference/comparison.json')
assert comparison['actual_native_worker_equals_separate_sealed_reference']
assert comparison['profile_hashes_before'] == comparison['profile_hashes_after']
completion = load(ROOT / 'completion.json')
assert completion['all_owned_processes_exited'] and completion['keeper_exited']
assert completion['keeper_original_files_unchanged'] and completion['keeper_original_file_count'] == 4
assert all(completion['exact_reopen'].values())
print(f'Verified {len(manifest)} artifact hashes, {len(sources)} historical source bindings, eight native captures, exact sealed take reopening and three byte-identical 160800-frame WAVs.')
