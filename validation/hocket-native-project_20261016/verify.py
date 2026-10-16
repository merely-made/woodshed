"""Verify retained, synthetic native Hocket evidence; no UI or custody mutation."""
import hashlib
import json
from pathlib import Path
import struct
import sys
import zipfile

root = Path(sys.argv[1])
load = lambda name: json.loads((root / name).read_text())
sha = lambda data: hashlib.sha256(data).hexdigest()
saved = load('qualified-seed/saved.json')
staged = load('qualified-seed/staged.json')
accepted = load('qualified-seed/accepted.json')
adopted = load('qualified-reopen/adopted.json')
reopened = load('qualified-reopen/reopened.json')
assert accepted['bundle'] == reopened['bundle']
assert saved['bundle'] == adopted['bundle']
assert accepted['media'] == reopened['media'] == saved['media'] == adopted['media']
for report in [saved, staged, accepted, adopted, reopened]:
    assert not report['busy'] and not report['missing_media']
    assert len(report['media']) == 1
    media = report['media'][0]
    assert media['fixture_bits_exact'] and media['frames'] == 48000
    assert media['sample_rate'] == 48000
    assert report['contact_token'] == saved['contact_token']
assert staged['has_incoming'] and 'continues this session' in staged['incoming_summary']
assert len(staged['bundle']['session']['tracks']) == 6
assert len(accepted['bundle']['session']['tracks']) == 5
assert not accepted['has_incoming'] and not accepted['dirty'] and not reopened['dirty']
a = accepted['bundle']['history']['nodes']
s = saved['bundle']['history']['nodes']
assert len(a) == 4 and len(s) == 3 and all(a[key] == value for key, value in s.items())
assert accepted['bundle']['history']['head'] == saved['bundle']['history']['head']
assert accepted['bundle']['session']['phrases'] == load('fixtures/fixture.json')['session']['phrases']
reference = (root / 'fixtures/reference.wav').read_bytes()
assert reference == (root / 'seed.wav').read_bytes() == (root / 'reopen.wav').read_bytes()
media_members = {}
for name in ['fixtures/fixture.hock', 'project.hock', 'adopted.hock']:
    with zipfile.ZipFile(root / name) as archive:
        members = {key: sha(archive.read(key)) for key in archive.namelist() if key.startswith('media/')}
        assert len(members) == 1
        media_members[name] = members
assert media_members['fixtures/fixture.hock'] == media_members['project.hock'] == media_members['adopted.hock']
for name, digest in load('baseline-hashes.json').items():
    assert sha((root / name).read_bytes()) == digest
captures = []
for phase, expected in [('qualified-seed', 3), ('qualified-reopen', 4)]:
    done = (root / phase / 'scenario.done').read_text()
    assert done.startswith('RESULT ok\n') and 'FAIL:' not in done
    images = sorted((root / phase).glob('*.png'))
    assert len(images) == expected
    for path in images:
        data = path.read_bytes()
        width, height = struct.unpack('>II', data[16:24])
        captures.append({'path': str(path), 'sha256': sha(data), 'pixels': [width, height]})
print(json.dumps({'exact_bundle_reopen': True, 'exact_adopted_bundle': True,
    'history_branch_retained': True, 'pcm_bits_exact': True, 'frames': 48000,
    'sample_rate': 48000, 'compressed_media_identical': media_members,
    'fixtures_unchanged': True, 'wav_sha256': sha(reference), 'wav_bytes': len(reference),
    'public_identity': saved['contact_token'], 'captures': captures}, indent=2))
