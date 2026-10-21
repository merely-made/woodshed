#!/usr/bin/env python3
"""Assert Status/UI/independent receiver parity after the native runs."""
import argparse,json,hashlib
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('controls',type=Path);p.add_argument('--reopen',type=Path);p.add_argument('--close-cancel',type=Path);p.add_argument('--close-save',type=Path);a=p.parse_args();b=a.controls
load=lambda p:json.loads(p.read_text())
status=lambda label:load(b/'observations'/(label+'.json'))
ui=lambda label:load(b/'run'/('midi-'+label+'.json'))['diagnostics']
f=status('default');assert f['diagnostics'] is None;assert not ui('default')['history_available'];assert not ui('default')['recording']
x=status('stopped');y=status('off-send');z=status('retained');assert x['diagnostics']==y['diagnostics'];assert [x['output']['accepted'],y['output']['accepted'],z['output']['accepted']]==[2,4,6]
e=x['diagnostics']['events'];resumed=z['diagnostics']['events'];assert resumed[:len(e)]==e;assert [v['stage'] for v in resumed if v['stage'].startswith('recording-')]==['recording-start','recording-stop','recording-start','recording-stop']
assert not z['diagnostics']['recording'];assert ui('retained')['show_history'];assert len(ui('retained')['recent_events'])==min(len(resumed),12)
for label in ['cancelled','saved']:
 assert not ui(label)['busy'];assert status(label)['diagnostics']==z['diagnostics']
assert 'cancelled' in ui('cancelled')['notice'];assert 'exported to' in ui('saved')['notice']
export=load(b/'midi-diagnostics.json');assert export['format']=='woodshed-midi-diagnostics' and export['version']==1
expected=dict(status('retained'));expected.pop('id');assert export['status']==expected
packets=[loadline['bytes'] for line in (b/'received.jsonl').read_text().splitlines() if (loadline:=json.loads(line))['event']=='packet'];assert packets==[[176,67,127],[176,67,0]]*3
assert load(b/'run/midi-exit-recording.json')['diagnostics']['recording']
assert load(b/'run/process.json')['success']
result={'success':True,'native_received_messages':6,'status_export_equal':True,'stopped_history_unchanged_during_send':True,'restart_appends':True,'retained_events':len(resumed),'overwritten':z['diagnostics']['overwritten'],'truncated_events':ui('retained')['truncated_events'],'export_sha256':hashlib.sha256((b/'midi-diagnostics.json').read_bytes()).hexdigest()}
if a.reopen:
 r=a.reopen;assert load(r/'run/process.json')['success'];assert load(r/'observations/reopened.json')['diagnostics'] is None;assert not load(r/'run/midi-reopened.json')['diagnostics']['recording'];assert not load(r/'run/midi-reopened.json')['diagnostics']['history_available'];assert load(r/'observations/reopened.json')['bindings']==z['bindings'];assert 'diagnostics' not in json.dumps(load(r/'run/midi-reopened.json')['saved_settings']);result['same_profile_reopen_default_off']=True
for name,r in [('cancel',a.close_cancel),('save',a.close_save)]:
 if r:
  proc=load(r/'run/process.json');actions=load(r/'native-dialog-actions.json');assert not proc['timeout'] and proc['launcher_exit']==0;assert actions[-1]['closed_after_export'];assert not (r/'run/midi-should-not-survive-close.json').exists()
  held=load(r/'run/midi-close-held.json');assert held['diagnostics']['busy'];assert held['diagnostics']['notice']=='Finish or cancel diagnostic export before closing.'
  st=load(r/'observations/close-held.json');assert st['output']['attempted']==0
  assert not [v for line in (r/'received.jsonl').read_text().splitlines() if (v:=json.loads(line))['event']=='packet']
  if name=='save':
   st.pop('id');assert load(r/'midi-diagnostics.json')['status']==st
  else:assert not (r/'midi-diagnostics.json').exists()
  result['pending_close_'+name+'_completed']=True
(b/'verification.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
