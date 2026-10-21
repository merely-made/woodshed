#!/usr/bin/env python3
"""Read the existing desktop Status command; never sends a MIDI trigger."""
import argparse, datetime, json, os, time
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--session',type=Path,required=True);p.add_argument('--label',required=True);a=p.parse_args()
assert a.label and len(a.label)<=64 and all(c.isalnum() or c in '-_' for c in a.label)
b=a.session.resolve();dest=b/'observations'/(a.label+'.json');dest.parent.mkdir(exist_ok=True);assert not dest.exists(),'use a new observation label'
command={'command':'status','id':a.label};fd=os.open(b/'commands.fifo',os.O_WRONLY|os.O_NONBLOCK)
try:os.write(fd,(json.dumps(command)+'\n').encode())
finally:os.close(fd)
end=time.monotonic()+5
while time.monotonic()<end:
 f=b/'stdio.jsonl'
 for line in f.read_text().splitlines() if f.exists() else []:
  try:j=json.loads(line)
  except json.JSONDecodeError:continue
  if j.get('id')==a.label:
   observation={'sampled_at_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'command':command,'response':j};dest.write_text(json.dumps(observation,indent=2)+'\n')
   print(json.dumps({'file':str(dest),'output':j.get('output'),'pedal':j.get('pedal'),'journal_events':len(j.get('diagnostics',{}).get('events',[])),'overwritten':j.get('diagnostics',{}).get('overwritten')},indent=2));raise SystemExit(0)
 time.sleep(.02)
raise SystemExit('status did not answer; no action was sent')
