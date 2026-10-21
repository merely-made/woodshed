"""Independent XSD and ElementTree checks of the native Rust emitter's exports."""
from pathlib import Path
import xml.etree.ElementTree as ET
import subprocess, hashlib, json, datetime
base=Path('/Users/markik/Code/output/woodshed-native-musicxml-20261018')
repo=Path('/Users/markik/Code/worktrees/woodshed-portable-fretboard')
schema=Path('/Users/markik/Code/output/woodshed-notation-study-20261018/musicxml.xsd')
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def pitch(e,prefix=''):
 return (int(e.findtext(prefix+'octave'))+1)*12+dict(C=0,D=2,E=4,F=5,G=7,A=9,B=11)[e.findtext(prefix+'step')]+int(e.findtext(prefix+'alter','0'))
results=[]
for path in sorted((base/'exported').glob('*.musicxml')):
 p=subprocess.run(['xmllint','--nonet','--noout','--schema',str(schema),str(path)],capture_output=True,text=True)
 assert p.returncode==0,(path,p.stderr)
 root=ET.parse(path).getroot();pairs=rests=events=0
 for m in root.findall('./part/measure'):
  attrs=m.find('attributes');div=int(attrs.findtext('divisions'));beats=int(attrs.findtext('time/beats'));beat_type=int(attrs.findtext('time/beat-type'))
  transpose=attrs.find('transpose');shift=0
  if transpose is not None:
   assert transpose.get('number')=='1'
   shift=int(transpose.findtext('chromatic','0'))+12*int(transpose.findtext('octave-change','0'))
  tunings={int(t.get('line')):pitch(t,'tuning-') for t in attrs.findall('./staff-details/staff-tuning')}
  staves=[[n for n in m.findall('note') if n.findtext('staff')==str(s)] for s in (1,2)]
  assert len(staves[0])==len(staves[1])
  for staff in staves:
   triplet_notes=0
   for note in staff:
    nominal=dict(whole=4,half=2,quarter=1,eighth=0.5)[note.findtext('type')]
    modification=note.find('time-modification')
    if modification is not None:
     assert modification.findtext('actual-notes')=='3' and modification.findtext('normal-notes')=='2'
     assert int(note.findtext('duration'))*3==int(nominal*div)*2
     if note.find('chord') is None:triplet_notes+=1
    else:assert int(note.findtext('duration'))==nominal*div
   assert len([t for n in staff for t in n.findall('./notations/tuplet') if t.get('type')=='start'])*3==triplet_notes
   assert len([t for n in staff for t in n.findall('./notations/tuplet') if t.get('type')=='stop'])*3==triplet_notes
   assert sum(int(n.findtext('duration')) for n in staff if n.find('chord') is None)*beat_type==beats*4*div
  for standard,tab in zip(*staves):
   assert standard.findtext('duration')==tab.findtext('duration')
   assert (standard.find('chord') is None)==(tab.find('chord') is None)
   if standard.find('rest') is not None:
    assert tab.find('rest') is not None;rests+=1;continue
   a=pitch(standard.find('pitch'))+shift;b=pitch(tab.find('pitch'));assert a==b,(path,a,b)
   tech=tab.find('./notations/technical');string=int(tech.findtext('string'));fret=int(tech.findtext('fret'))
   assert b==tunings[len(tunings)+1-string]+fret
   assert standard.find('./notations/technical') is None
   pairs+=1
  events+=len([n for n in staves[0] if n.find('chord') is None])
 results.append(dict(file=str(path),sha256=sha(path),xsd_pass=True,pitched_staff_pairs=pairs,rest_staff_pairs=rests,events=events,measures=len(root.findall('./part/measure')),independent_pitch_duration_and_fingering_pass=True,xmllint=p.stderr.strip()))
receipt=dict(date_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),scope='Six native-emitter MusicXML fixtures; structural XSD/independent reader admission plus independent semantic pitch/duration/fingering checks, not third-party engraving or playback qualification.',sources={str(p):sha(p) for p in [repo/'crates/woodshedding/src/notation.rs',base/'model-check/Cargo.toml',base/'model-check/Cargo.lock',base/'model-check/src/bin/export.rs',Path(__file__)]},schemas={str(p):sha(p) for p in [schema,schema.with_name('xml.xsd'),schema.with_name('xlink.xsd')]},independent_reader={'crate':'musicxml','version':'1.1.2','accepted':len(results),'log':str(base/'independent-reader.log')},exports=results,pass_counts={'xsd':len(results),'independent_reader':len(results),'semantic_fixture_checks':len(results),'pitched_staff_pairs':sum(r['pitched_staff_pairs'] for r in results),'rest_staff_pairs':sum(r['rest_staff_pairs'] for r in results),'measures':sum(r['measures'] for r in results)})
assert len(results)==6
(base/'export-qualification.json').write_text(json.dumps(receipt,indent=2)+'\n')
print(json.dumps(receipt['pass_counts']))
