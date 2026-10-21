"""Verify real app checkpoints and outputs; no installed-pack fallback allowed."""
from pathlib import Path
import array,hashlib,json,math,struct,sys,zipfile
from inspect_midi import inspect
BASE=Path('/Users/markik/Code/testing/woodshed/audio-packs-ap3_20261020')
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
load=lambda p:json.loads(p.read_text())
result={}
for app in sys.argv[1:]:
 cohort=BASE/(app+'-final')
 for n in (1,2):assert load(cohort/f'run-{n}/process.json')['success']
 assert (cohort/'uninstalled-kit').is_dir()
 if app=='woodshed':
  initial=load(cohort/'run-1/edited-installed.json')
  refused=load(cohort/'run-1/failed-replacement-retained.json')
  reopened=load(cohort/'run-2/reopened-without-installation.json')
  switched=load(cohort/'run-2/switched-without-installation.json')
  for value in [initial,refused,reopened,switched]:
   assert value['selection']==value['saved_selection']
   assert value['selection']==initial['selection']
   assert value['bpm']==125
 else:
  rendered=load(cohort/'run-1/rendered.json')
  assert rendered['audio']['drum_output_observed'] and not rendered['audio']['click_enabled']
  assert rendered['engine']['assigned']==1 and rendered['engine']['queued']==0
  assert any(row['engine'].get('meter_active') for row in rendered['output_trace'])
  initial=load(cohort/'run-1/saved.json')
  refused=load(cohort/'run-1/failed-replacement-retained.json')
  reopened=load(cohort/'run-2/reopened-without-installation.json')
  switched=load(cohort/'run-2/editable-without-installation.json')
  for value in [initial,refused,reopened,switched]:
   assert value['bundle']==initial['bundle']
   assert value['media']==initial['media']
   assert not value['missing_media'] and not value['dirty'] and not value['busy']
  archive=BASE/'hocket-final.hock'
  with zipfile.ZipFile(archive) as z:
   names=z.namelist();assert 'manifest.cbor' in names
   assert len([x for x in names if x.startswith('media/') and x.endswith('.wv')])==1
  a,b=[BASE/f'hocket-final-{phase}.wav' for phase in ['before','after']]
  assert a.read_bytes()==b.read_bytes()
  data=a.read_bytes();assert data[:4]==b'RIFF' and data[8:12]==b'WAVE'
  pos=12;fmt=None;subformat=None;samples=None
  while pos+8<=len(data):
   tag,size=struct.unpack_from('<4sI',data,pos);chunk=data[pos+8:pos+8+size]
   if tag==b'fmt ':
    fmt=struct.unpack_from('<HHIIHH',chunk)
    if fmt[0]==65534:subformat=chunk[24:40]
   if tag==b'data':samples=array.array('f',chunk)
   pos+=8+size+(size%2)
  assert fmt and (fmt[0]==3 or (fmt[0]==65534 and subformat==bytes.fromhex('0300000000001000800000aa00389b71')))
  assert fmt[1]==2 and fmt[2]==48000 and fmt[5]==32
  if sys.byteorder!='little':samples.byteswap()
  assert samples and all(math.isfinite(v) for v in samples)
  peak=max(abs(v) for v in samples);assert peak>0
  result['hocket']={'bundle_and_media_exact':True,'archive_entries':names,'wav_sha256':sha(a),'media':initial['media'],
   'muted_click_graph_output_observed':True,'wav':{'channels':fmt[1],'sample_rate':fmt[2],'frames':len(samples)//fmt[1],'peak':peak}}
 a,b=[BASE/f'{app}-final-{phase}.mid' for phase in ['before','after']]
 assert a.read_bytes()==b.read_bytes()
 info=inspect(a);assert info['format']==0 and info['ppq']==480 and info['end_ticks']==[1920]
 assert info['notes_on']==12 and info['unterminated_notes']==0 and info['unmatched_offs']==0
 result.setdefault(app,{}).update({'installed_folder_removed_before_reopen':True,
  'saved_selection_exact_after_failed_replacement_and_reopen':True,'midi_bytes_equal':True,'midi':info,
  'processes':[load(cohort/f'run-{n}/process.json') for n in (1,2)]})
print(json.dumps(result,indent=2))
