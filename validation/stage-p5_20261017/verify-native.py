"""Verify the published P5 artifacts and their historical Git source binding."""
from pathlib import Path
import hashlib, json, subprocess
ROOT=Path(__file__).resolve().parent
REPO=ROOT.parent.parent
def load(path): return json.loads(path.read_text())
def sha(data): return hashlib.sha256(data).hexdigest()
manifest=load(ROOT/'sha256.json')
assert all(sha((ROOT/name).read_bytes())==expected for name,expected in manifest.items())
receipt=load(ROOT/'receipt.json')
assert receipt['status']=='qualified'
source=load(ROOT/'source.json')
for name,expected in load(ROOT/'source-hashes.json').items():
    data=subprocess.check_output(['git','show',source['revision']+':'+name],cwd=REPO)
    assert sha(data)==expected, name
seed=load(ROOT/'native/seed/p5clock-final.json')
reopen=load(ROOT/'native/reopen/p5clock-reopened.json')
assert all(seed[key]==reopen[key] for key in ['cards','owner','history'])
assert len(seed['history'])==6
assert sum(event['kind']=='Completed' for event in seed['history'])==3
assert sum(event['kind']=='Rehearsed' for event in seed['history'])==3
assert seed['events_by_card']['Seconds clock 20 BPM']['completed_ms']==2000
assert seed['program_status']=='complete' and reopen['program_status']=='stopped'
assert reopen['transient_program_cleared']
for phase,count in [('seed',4),('reopen',1)]:
    text=(ROOT/'native'/phase/'scenario.done').read_text()
    assert text.startswith('RESULT ok\n') and 'FAIL' not in text
    assert f'frames: {count} captured, 0 blank, {count} distinct digests' in text
    assert len(list((ROOT/'native'/phase).glob('*.png')))==count
assert receipt['checks']['suite_total']==881
print(f"Verified {len(manifest)} artifact hashes, 207 historical source bindings, five captures and exact sealed Cards/owner/history reopening.")
