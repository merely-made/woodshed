#!/usr/bin/env python3
"""Read-only Status and retained UI checkpoint; optionally release scenario gate."""
import argparse,json,os,time
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('session',type=Path);p.add_argument('label');p.add_argument('--release',action='store_true');a=p.parse_args();b=a.session.resolve()
assert a.label and all(c.isalnum() or c=='-' for c in a.label)
deadline=time.monotonic()+90;checkpoint=b/'run'/('midi-'+a.label+'.json')
while not checkpoint.exists():
 assert time.monotonic()<deadline,'checkpoint missing '+str(checkpoint);time.sleep(.1)
fd=os.open(b/'commands.fifo',os.O_WRONLY|os.O_NONBLOCK)
try:os.write(fd,(json.dumps({'command':'status','id':a.label})+'\n').encode())
finally:os.close(fd)
deadline=time.monotonic()+5
while True:
 matches=[]
 for line in (b/'stdio.jsonl').read_text().splitlines():
  try:j=json.loads(line)
  except json.JSONDecodeError:continue
  if j.get('id')==a.label:matches.append(j)
 if matches:break
 assert time.monotonic()<deadline,'Status timeout';time.sleep(.05)
s=matches[-1];(b/'observations').mkdir(exist_ok=True);(b/'observations'/(a.label+'.json')).write_text(json.dumps(s,indent=2)+'\n')
c=json.loads(checkpoint.read_text());d=s['diagnostics']
assert c['diagnostics']['recording']==(d['recording'] if d else False)
assert c['diagnostics']['retained_events']==(len(d['events']) if d else 0)
print(json.dumps({'checkpoint':a.label,'ui':c['diagnostics'],'accepted':s['output']['accepted'],'events':len(d['events']) if d else 0},indent=2))
if a.release:(b/'run'/('gate-'+a.label)).touch()
