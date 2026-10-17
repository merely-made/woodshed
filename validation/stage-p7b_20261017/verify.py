"""Read-only P7b evidence verification. Historical source comes from Git."""
from pathlib import Path
import hashlib,json,re,subprocess
ROOT=Path(__file__).resolve().parents[2]
HERE=Path(__file__).resolve().parent
receipt=json.loads((HERE/'receipt.json').read_text())
revision=receipt.get('implementation_revision','bc6c460d158bf16174ecd42b7c9f663b7b8709ec')
for item in receipt['source_hashes']:
    data=subprocess.check_output(['git','show',revision+':'+item['path']],cwd=ROOT)
    assert hashlib.sha256(data).hexdigest()==item['sha256'],item['path']
gate=json.loads((HERE/'checks/candidate-gate.json').read_text())
assert gate['exit_code']==0 and gate['suites']
text=(HERE/'checks/candidate-gate.log').read_text()
assert 'test result: FAILED.' not in text and not re.search(r'^error(?:\[|:)',text,re.M)
assert re.search(r'test result: ok\.',text)
assert all(s['failed']==0 for s in gate['suites'])
assert sum(s['passed'] for s in gate['suites'] if not s['filtered_out'])==receipt['counts']['affected_gate_cases']
assert sum(s['passed'] for s in gate['suites'] if s['filtered_out'])==receipt['counts']['capture_child_invocations_separate']
for name in ['native-fixture-tests','native-fixture-build','hocket-shared-leaf','redshank-shared-leaf']:
    assert json.loads((HERE/'checks'/f'{name}.json').read_text())['exit_code']==0,name
assert json.loads((HERE/'checks/recipe-check.json').read_text())['exit_code']==101
if 'artifact_hashes' in receipt:
    for name,digest in receipt['artifact_hashes'].items():
        assert hashlib.sha256((HERE/name).read_bytes()).hexdigest()==digest,name
print(f"Verified {len(receipt['source_hashes'])} historical source bindings and affected gate {receipt['counts']['affected_gate_cases']} cases; native status: {receipt['native']}")
