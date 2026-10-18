from pathlib import Path
import json,shutil,hashlib
B=Path(__file__).parent;R=Path('/Users/markik/Code/repos/woodshed');T=B/'native-v2';O=R/'validation/related-labels_20261017';O.mkdir(exist_ok=False)
for name in ['receipt.json','lock-scope.json','metadata-closure.json','registry-edge-restorations.json']:
 shutil.copy2(B/name,O/name)
shutil.copy2(T/'build.json',O/'build.json');shutil.copy2(T/'completion.json',O/'completion.json');shutil.copy2(B/'native-v1/excluded.json',O/'excluded-first-trial.json')
for phase in ['labels','p2']:
 out=O/phase;out.mkdir()
 for p in (T/phase).iterdir():
  if p.suffix=='.png' or p.name in ['scenario.scn','scenario.done','exit.json','launch.json','process-identity.json']:
   shutil.copy2(p,out/p.name)
shutil.copytree(B/'checks',O/'checks');tools=O/'tools';tools.mkdir()
for name in ['check.py','bound_build.py','native.py','native_tools.py','stable_launcher.m','metadata.py','lock_audit.py','summarize.py','archive_receipt.py']:
 shutil.copy2(B/name,tools/name)
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
(tools/'hashes.json').write_text(json.dumps({p.name:sha(p) for p in tools.iterdir() if p.is_file()},indent=2)+'\n')
p=O/'receipt.json';v=json.loads(p.read_text());v['known_limit']='420px camera toolbar overflows horizontally; 125 percent UI zoom clips the right graph/control region. No complete compact-layout or screen-visibility claim.';v['native_accepted_trial']='native-v2';v['excluded_trial']='native-v1: incorrect expected material title; corrected runtime scenario, identical product binary';v['review']='Parent reviewed all seven label captures; independent full16 capture review recorded separately.';p.write_text(json.dumps(v,indent=2)+'\n')
print(O)
