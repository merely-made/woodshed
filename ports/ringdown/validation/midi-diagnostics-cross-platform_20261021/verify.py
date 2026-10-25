#!/usr/bin/env python3
"""Recheck native receipts without running or modifying either application."""
import hashlib, json, sys
from pathlib import Path
r = Path(sys.argv[1])
def read(p): return json.loads((r / p).read_text(encoding='utf-8'))
def state(phase, name): return read(f'{phase}/status/{name}.json')
def snap(phase, name): return read(f'{phase}/captures/midi-{name}.json')
def parity(phase, name, expected):
    export = read(f'{phase}/midi-diagnostics.json')
    status = dict(expected); status.pop('id')
    assert export['format'] == 'woodshed-midi-diagnostics' and export['version'] == 1
    assert export['status'] == status
    return hashlib.sha256((r / phase / 'midi-diagnostics.json').read_bytes()).hexdigest()
initial, stopped, off, retained = (state('controls', p) for p in ['default', 'stopped', 'off-send', 'retained'])
assert initial['diagnostics'] is None
first, history = stopped['diagnostics'], retained['diagnostics']
assert not first['recording'] and not history['recording']
assert off['diagnostics'] == first
assert history['events'][:len(first['events'])] == first['events']
assert [v['stage'] for v in history['events'] if v['stage'].startswith('recording-')] == ['recording-start','recording-stop']*2
assert not history['overwritten'] and all(not v['truncated'] for v in history['events'])
assert snap('controls','retained')['diagnostics']['retained_events'] == len(history['events'])
assert len(snap('controls','retained')['diagnostics']['recent_events']) == min(12,len(history['events']))
for name in ['cancelled','saved']:
    assert state('controls',name)['diagnostics'] == history
    assert not snap('controls',name)['diagnostics']['busy']
assert 'cancelled' in snap('controls','cancelled')['diagnostics']['notice']
assert 'exported to' in snap('controls','saved')['diagnostics']['notice']
assert snap('controls','exit-recording')['diagnostics']['recording']
reopen = state('reopen','reopened')
assert reopen['diagnostics'] is None and reopen['bindings'] == initial['bindings']
assert not snap('reopen','reopened')['diagnostics']['recording']
assert 'diagnostics' not in json.dumps(snap('reopen','reopened')['saved_settings'])
exports = {'controls':parity('controls','retained',retained)}
for phase in ['close-cancel','close-save']:
    held = snap(phase,'close-held'); st = state(phase,'close-held')
    assert held['diagnostics']['busy']
    assert held['diagnostics']['notice'] == 'Finish or cancel diagnostic export before closing.'
    assert st['output']['attempted'] == 0
    assert not (r/phase/'captures/midi-should-not-survive-close.json').exists()
    actions = [json.loads(v) for v in (r/phase/'native-actions.jsonl').read_text(encoding='utf-8').splitlines()]
    assert any('native-WM_' in v['stdout'] and v['exit']==0 for v in actions)
    if phase == 'close-save': exports[phase] = parity(phase,'close-held',st)
processes = [read(f'{p}/process.json') for p in ['controls','reopen','close-cancel','close-save']]
assert all(p['exit']==0 for p in processes)
assert len({p['binary_sha256'] for p in processes}) == 1
assert all(read(f'{p}/process.json')['scenario_result']=='RESULT ok' for p in ['controls','reopen'])
if (r/'received.jsonl').exists():
    packets = [v['bytes'] for line in (r/'received.jsonl').read_text(encoding='utf-8').splitlines() if (v:=json.loads(line))['event']=='packet']
    assert packets == [[176,67,127],[176,67,0]]*3
    assert [v['output']['accepted'] for v in [stopped,off,retained]] == [2,4,6]
else:
    packets = None
    assert all(v['output']['attempted']==0 for v in [initial,stopped,off,retained])
print(json.dumps({'success':True,'retained_events':len(history['events']),'reopen_default_off':True,'native_cancel_save_and_close':True,'full_status_parity':True,'exports_sha256':exports,'independent_midi_packets':packets,'binary_sha256':processes[0]['binary_sha256']},indent=2))
