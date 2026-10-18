from pathlib import Path
import json,hashlib,subprocess
B=Path(__file__).parent;R=Path('/Users/markik/Code/repos/woodshed');T=B/'native-v2'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
checks={p.stem:json.loads(p.read_text()) for p in (B/'checks').glob('*.json')}
assert all(v.get('exit_code')==0 for v in checks.values())
completion=json.loads((T/'completion.json').read_text());assert completion['all_scenarios_ok'] and completion['owned_processes_exited']
captures={}
for phase,count in [('labels',7),('p2',9)]:
 ps=sorted((T/phase).glob('*.png'));assert len(ps)==count
 captures[phase]={p.name:sha(p) for p in ps}
receipt={'status':'locally qualified; publication pending','baseline_woodshed':'125be57d184f46b9f878463243d5a6ed8d8df4c3','supplier_source_revision':'d1a200b1877223f5b6ced7f5fbaca3671c10f5dd','native_woodshed_revision':completion['source_revision'],'checks':checks,'native':completion,'captures':captures,'lock_scope':json.loads((B/'lock-scope.json').read_text()),'metadata':json.loads((B/'metadata-closure.json').read_text()),'qualification_scope':'shared Cambium visible label layout and Woodshed graph receiving; no durable format, PCM, physical input or audio changes','native_fresh_reopen':'not required or claimed: presentation-only change with no persisted label geometry','source_build_binding':json.loads((T/'build.json').read_text()),'external_artifacts':str(B)}
(B/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
print('receipt',receipt['native_woodshed_revision'],sum(len(v) for v in captures.values()),'captures')
