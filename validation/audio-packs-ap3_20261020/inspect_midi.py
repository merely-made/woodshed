"""Bounded read-only SMF inspection for factory/custom beat comparisons.
Reports timing and aggregate note/channel maps; does not redistribute MIDI.
"""
import collections,hashlib,json,struct,sys
from pathlib import Path

def inspect(path):
 data=Path(path).read_bytes();assert len(data)<=4*1024*1024
 assert data[:4]==b'MThd'
 header_size=struct.unpack_from('>I',data,4)[0]
 fmt,tracks,division=struct.unpack_from('>HHH',data,8)
 assert not division&0x8000,'SMPTE division is not supported by this beat inspector'
 pos=8+header_size;counts=collections.Counter();off=collections.Counter();tempos=[];meters=[];ends=[];active={};durations=[];unmatched=0;on_ticks=[];off_ticks=[];note_ticks=collections.defaultdict(list);messages=collections.Counter();metas=collections.Counter();controllers=[]
 for track in range(tracks):
  assert data[pos:pos+4]==b'MTrk';size=struct.unpack_from('>I',data,pos+4)[0];pos+=8;end=pos+size;tick=0;running=None
  def varlen():
   nonlocal pos
   value=0
   for _ in range(4):
    assert pos<end;v=data[pos];pos+=1;value=(value<<7)|(v&127)
    if not v&128:return value
   raise ValueError('Invalid SMF variable length')
  while pos<end:
   tick+=varlen();status=data[pos]
   if status&128:pos+=1
   else:assert running is not None;status=running
   if status==255:
    meta=data[pos];metas[hex(meta)]+=1;pos+=1;n=varlen();value=data[pos:pos+n];pos+=n
    if meta==81:tempos.append({'tick':tick,'microseconds_per_quarter':int.from_bytes(value,'big')})
    if meta==88:meters.append({'tick':tick,'numerator':value[0],'denominator':2**value[1]})
    if meta==47:ends.append(tick)
    continue
   if status in (240,247):
    n=varlen();pos+=n;running=None;continue
   assert 128<=status<240,'Unsupported system message'
   running=status;kind=status>>4;messages[hex(kind)]+=1;channel=(status&15)+1;n=1 if kind in (12,13) else 2
   values=data[pos:pos+n];pos+=n;assert len(values)==n
   key=(track,channel,values[0])
   if kind==11:controllers.append({"tick":tick,"channel":channel,"number":values[0],"value":values[1]})
   if kind==9 and values[1]>0:
    counts[(channel,values[0])]+=1;active.setdefault(key,[]).append(tick);on_ticks.append(tick);note_ticks[(channel,values[0])].append(tick)
   elif kind==8 or (kind==9 and values[1]==0):
    off[(channel,values[0])]+=1;off_ticks.append(tick)
    if active.get(key):durations.append(tick-active[key].pop(0))
    else:unmatched+=1
  assert pos==end
 return {'file':Path(path).name,'sha256':hashlib.sha256(data).hexdigest(),'bytes':len(data),
  'format':fmt,'tracks':tracks,'ppq':division,'end_ticks':ends,'end_quarters':[x/division for x in ends],
  'tempos':tempos,'meters':meters,'notes':[{'channel':c,'note':n,'on':v,'off':off[(c,n)]} for (c,n),v in sorted(counts.items())],
  'first_note_tick':min(on_ticks) if on_ticks else None,'last_note_on_tick':max(on_ticks) if on_ticks else None,'last_note_off_tick':max(off_ticks) if off_ticks else None,
  'note_bar_counts':{str(n):dict(sorted(collections.Counter(t//(division*meters[0]['numerator']*4//meters[0]['denominator']) for t in ts).items())) for (c,n),ts in sorted(note_ticks.items())} if len(meters)==1 else {},
  'controllers':controllers,'message_kinds':dict(messages),'meta_types':dict(metas),
  'notes_on':sum(counts.values()),'unmatched_offs':unmatched,'unterminated_notes':sum(map(len,active.values())),
  'gate_ticks_range':[min(durations),max(durations)] if durations else None}
if __name__=='__main__':print(json.dumps([inspect(p) for p in sys.argv[1:]],indent=2))
