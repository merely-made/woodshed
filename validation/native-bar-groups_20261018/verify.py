"""Read-only native grouping acceptance and receipt-integrity checks."""
from pathlib import Path
import copy, hashlib, json
root=Path(__file__).resolve().parent
read=lambda p:json.loads(p.read_text())
seed,reopen=root/'native/seed',root/'native/reopen'
b,p,s,u=[read(seed/(name+'.json')) for name in ('before','preview','saved','after-undo')]
r=read(reopen/'reopened.json')
assert b['live']['set']==b['stored']['set']==p['live']['set']==p['stored']['set']
expected=copy.deepcopy(b['live']['set'])
expected['progression_arrangement']['alignment']['bar_groups']=[{'first':0,'count':2},{'first':2,'count':2}]
assert expected==s['live']['set']==s['stored']['set']==u['live']['set']==u['stored']['set']==r['live']['set']==r['stored']['set']
assert s['document']==u['document']==r['document']
assert p['document']==s['document']!=b['document']
assert r['selection'] is None
for value in (b,p,s,u,r):
    assert not value['draft_dirty'] and value['error'] is None
for run in (seed,reopen):
    assert (run/'scenario.done').read_text().startswith('RESULT ok')
for name,digest in read(root/'sha256.json').items():
    assert hashlib.sha256((root/name).read_bytes()).hexdigest()==digest,name
print('PASS: preview preserves source; only bar_groups change; live/sealed Sets and complete scene reopen exactly; receipt hashes match.')
