#!/usr/bin/env python3
"""Verify portable acceptance metadata; --local also checks retained full captures/binaries."""
import argparse
import hashlib
import json
import re
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--local', action='store_true')
args = parser.parse_args()
root = Path(__file__).resolve().parent
repo = root.parent.parent
codec = root.parent / 'compressed-captures_font-family_20261012'

def read(path):
    return json.loads(path.read_text())

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

freeze = read(root / 'source-freeze.json')
changed = subprocess.check_output(['git', 'diff', '--name-only', freeze['compiled_source'], freeze['source']], cwd=repo, text=True).splitlines()
assert set(changed) == {'scenarios/font_roles.scn', 'scenarios/font_roles_compact_reopen.scn'}, changed
for name, expected in freeze['source_files'].items():
    assert sha(repo / name) == expected, name
    committed = subprocess.check_output(['git', 'show', freeze['source'] + ':' + name], cwd=repo)
    assert hashlib.sha256(committed).hexdigest() == expected, name

checks = read(root / 'checks.json')
assert sum(checks['tests'][name]['tests'][0] for name in ['core', 'views', 'host']) == 556
for name, expected in [('core', 236), ('views', 161), ('host', 159)]:
    log = (root / 'cpu' / ('combined-' + name + '.log')).read_text()
    results = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored', log)
    assert results and int(results[-1][0]) == expected and results[-1][1] == '0'
assert checks['tests']['build']['exit'] == 0

for packet, runs, captures, frames in [(root, 3, 19, 230), (codec, 2, 3, 54)]:
    for path in packet.rglob('*.json'):
        read(path)
    native = read(packet / 'native.json')
    assert native['source'] == freeze['source']
    assert len(native['runs']) == runs
    assert native['captures'] == captures and native['presented_frames'] == frames
    assert sum(len(run['captures']) for run in native['runs']) == captures
    assert sum(run['presented_frames'] for run in native['runs']) == frames
    for run in native['runs']:
        assert run['result'] == 'ok'
        done = packet / 'native' / run['name'] / 'scenario.done'
        assert done.read_text().startswith('RESULT ok\n')
        if args.local:
            assert sha(Path(run['full_log_local_path'])) == run['full_log_sha256']
            for capture in run['captures']:
                assert sha(Path(capture['local_path'])) == capture['sha256']
    cleanup = read(packet / 'cleanup.json')
    assert cleanup['native_processes_remaining'] == []

before = read(root / 'before-fonts.json')
after = read(root / 'after-fonts.json')
assert after['fonts'] == {'heading': ['Source Serif 4'], 'reading': ['Source Serif 4'], 'label': ['IBM Plex Mono']}
assert after['appearance']['theme_choice']['theme_id'] == 'theme:woodshed-font-acceptance'
assert after['appearance']['theme_choice']['theme_mode'] == 'hc_light'
for name in ['metronome', 'fretboard']:
    assert before[name] == after[name]
different = {name for name in before['domain'] if before['domain'][name] != after['domain'][name]}
assert different == {'workspace_json', 'overview_presentation_json'}, different
old_graph = json.loads(before['domain']['overview_presentation_json'])
new_graph = json.loads(after['domain']['overview_presentation_json'])
assert old_graph.pop('positions') == []
assert len(new_graph.pop('positions')) == 10
assert old_graph == new_graph
old_tabs = json.loads(before['domain']['workspace_json'])
new_tabs = json.loads(after['domain']['workspace_json'])
assert old_tabs['tree']['stack']['tabs'] == new_tabs['tree']['stack']['tabs']
assert old_tabs['active_panel'] == 'practice' and new_tabs['active_panel'] == 'settings'

old = read(codec / 'codec-before' / 'inspect.json')
new = read(codec / 'codec-after-reopen' / 'inspect.json')
assert old['manifest_before']['version'] == 1 and new['manifest_before']['version'] == 2
assert new['manifest_before']['takes'][0]['codec'] == 'wavpack-f32-v1'
assert not new['migration_needed_before'] and not new['unavailable_before']
assert old['captures_before'] == new['captures_before']
assert old['manifest_before']['namespace'] == new['manifest_before']['namespace']
parity = read(codec / 'codec-parity.json')
assert parity['old_blobs'][0]['old_bytes'] == 384064
assert parity['new_blobs'][0]['bytes'] == 262824
for packet, directory in [(root, 'wav-parity'), (codec, 'codec-after-wav')]:
    for name, expected in [('loop-reference.verified.json', '51966f89d016ade8cd01b32dfa8583cc2fbe452e5a3bc620f6dc44daeb3401e0'), ('backing-reference.verified.json', 'e4911029b436d2bcee6635d7780fc01b893fdca00f8fed34061822a53b3d9e58')]:
        wav = read(packet / directory / name)
        assert wav['status'] == 'passed' and wav['pcm_sha256'] == expected
        assert wav['frames'] == 192000 and wav['sample_rate_hz'] == 48000

if args.local:
    for binary in freeze['binaries'].values():
        assert sha(Path(binary['path'])) == binary['sha256']
    local = Path(read(codec / 'native' / 'CaptureMigrateCombined' / 'launch.json')['env']['WOODSHED_STATE']).parent
    for blob in parity['old_blobs']:
        assert sha(local / blob['relative']) == blob['sha256']
    for blob in parity['new_blobs']:
        assert sha(local / blob['path']) == blob['sha256']
print(json.dumps({'status': 'passed', 'source': freeze['source'], 'compiled_source': freeze['compiled_source'], 'source_files': len(freeze['source_files']), 'CPU_cases': 556, 'native_processes': 5, 'captures': 22, 'presented_frames': 284, 'local_captures_binaries_blobs_checked': args.local}, indent=2))
