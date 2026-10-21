"""Freeze bounded AP3 receipts, excluding executables, profiles and Keeper vaults."""
from pathlib import Path
import datetime, gzip, hashlib, json, re, shutil, subprocess
ROOT=Path(__file__).resolve().parents[2]
OUT=Path(__file__).resolve().parent
BASE=Path('/Users/markik/Code/testing/woodshed/audio-packs-ap3_20261020')
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
def dump(path,value):path.write_text(json.dumps(value,indent=2)+'\n')
# Checkpoints contain exact sample snapshots. Compress them without timestamps;
# copy smaller observations and presented captures directly.
for app in ('woodshed','hocket'):
 src=BASE/(app+'-final');dst=OUT/'native'/app;dst.mkdir(parents=True,exist_ok=True)
 for path in src.glob('*.json'):shutil.copy2(path,dst/path.name)
 for n in (1,2):
  run=src/f'run-{n}'; target=dst/f'run-{n}';target.mkdir(exist_ok=True)
  assert json.loads((run/'process.json').read_text())['success']
  for path in run.iterdir():
   if not path.is_file():continue
   if path.suffix=='.json' and path.stat().st_size>100_000:
    (target/(path.name+'.gz')).write_bytes(gzip.compress(path.read_bytes(),mtime=0))
   elif path.suffix in ('.png','.json','.log','.done'):shutil.copy2(path,target/path.name)
 for phase in ('before','after'):
  path=BASE/f'{app}-final-{phase}.mid';shutil.copy2(path,OUT/'artifacts'/path.name)
logs=['shared-audio-final-tests.log','project-tests.log','hocket-deadline-final-tests.log',
 'build-woodshed-r3.log','hocket-deadline-build.log','guitar-trial-prep.log']
(OUT/'logs').mkdir(exist_ok=True)
tests=[]
for name in logs:
 path=BASE/'logs'/name;shutil.copy2(path,OUT/'logs'/name)
 if 'tests' in name:
  for line in path.read_text().splitlines():
   m=re.search(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;',line)
   if m:tests.append({'log':name,'passed':int(m[1]),'failed':int(m[2]),'ignored':int(m[3])})
old=json.loads((ROOT/'validation/audio-packs-ap2_20261020/source-hashes.json').read_text())
paths=set(old)|{'crates/audio-packs/src/tests.rs','ports/hocket/crates/hocket-engine/src/lib.rs','ports/hocket/crates/hocket-engine/src/offline_tests.rs','crates/woodshed-audio/src/offline/song_export/tests.rs','crates/woodshed-audio/src/output_runtime/tests.rs','crates/woodshed-genet/src/scenario.rs','ports/hocket/crates/hocket-genet/src/scenario.rs','ports/hocket/crates/hocket-genet/src/recorder_audio.rs','ports/hocket/crates/hocket-genet/src/theme.rs'}
current={p:sha(ROOT/p) for p in sorted(paths) if (ROOT/p).is_file()}
dump(OUT/'source-hashes.json',current)
dump(OUT/'source-delta-ap2.json',{p:{'ap2_baseline_recorded':p in old,'ap2':old.get(p),'ap3':v} for p,v in current.items() if old.get(p)!=v})
assert all(current[p]==old[p] for p in ('Cargo.lock','ports/hocket/Cargo.lock','ports/ringdown/Cargo.lock'))
attempts=[]
for d in sorted(BASE.iterdir()):
 if d.is_dir() and (d.name.startswith(('hocket-','woodshed-'))):
  values=[]
  for p in sorted(d.glob('run-*/process.json')):values.append({'run':p.parent.name,**json.loads(p.read_text())})
  if values:attempts.append({'cohort':d.name,'runs':values,'scratch':str(d)})
dump(OUT/'attempts.json',attempts)
artifacts={}
for p in sorted(BASE.glob('*-final*')):
 if p.is_file():artifacts[p.name]={'bytes':p.stat().st_size,'sha256':sha(p),'scratch':str(p)}
report={'recorded_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),
 'baseline_head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
 'host':{'machine':'Q-PC','arch':'x86_64','os':'macOS 15.8.1 (24H32)','rust':'1.99.1'},
 'status':'Bounded macOS native/project and owner-observed guitar playback acceptance; target-desktop matrix remains open',
 'tests':tests,'totals':{key:sum(v[key] for v in tests) for key in ('passed','failed','ignored')},
 'lockfiles_unchanged_from_ap2':True,'artifacts':artifacts,
 'native_scope':'Real retained control clicks, native dialogs, presented captures, two processes per app, installed-kit directory moved before reopen; private synthetic Keeper/profile. No handoff transport trial.',
 'output_scope':'Woodshed real stream transport plus offline production-processor nonzero/allocation checks; Hocket actual graph nonzero meter with click muted and queued-to-assigned transition; physical guitar playback reported by owner. Desktop speakers not independently heard.',
 'limits':['Windows/Linux GUI execution and browser audio not run.','Slow debug UI frames mean performance latency and sample-accurate starts are not qualified.','Guitar five identical bars do not identify precise loop/count-in semantics.','No complete MIDI note/velocity map or remote Beats control qualification.'],
 'scratch_root':str(BASE),'no_commit_or_push':True}
dump(OUT/'receipt.json',report)
dump(OUT/'evidence-hashes.json',{str(p.relative_to(OUT)):sha(p) for p in sorted(OUT.rglob('*')) if p.is_file() and '__pycache__' not in p.parts and p.name!='evidence-hashes.json'})
print(json.dumps({'tests':report['totals'],'source_delta':len(json.loads((OUT/'source-delta-ap2.json').read_text())),'evidence_files':len(json.loads((OUT/'evidence-hashes.json').read_text()))}))
