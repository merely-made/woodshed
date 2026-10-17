import subprocess,json
from pathlib import Path
b=Path(__file__).parent;r=Path('/Users/markik/Code/repos/woodshed');rows={}
for root in ['.','ports/hocket','ports/redshank','validation/relationship-recipe']:
 p=subprocess.run(['cargo','metadata','--offline','--locked','--format-version','1'],cwd=r/root,capture_output=True,text=True)
 if p.returncode:print(root,p.stderr[-3000:]);raise SystemExit(p.returncode)
 m=json.loads(p.stdout);by={}
 for v in m['packages']:
  s=v.get('source')
  if s and s.startswith('git+'):
   by.setdefault(s,[]).append(v['name'])
 assert not any('path+' in (x.get('source') or '') for x in m['packages'])
 for family in ['mere.git','genet.git','netrender.git']:
  found=[s for s in by if '/'+family+'?' in s];assert len(found)==1,(root,family,found)
 rows[root]={'git_sources':{k:sorted(v) for k,v in by.items()},'package_count':len(m['packages']),'duplicate_names_from_multiple_git_sources':{name:sorted({p['source'] for p in m['packages'] if p['name']==name and p.get('source','') and p['source'].startswith('git+')}) for name in {p['name'] for p in m['packages']} if len({p['source'] for p in m['packages'] if p['name']==name and p.get('source','') and p['source'].startswith('git+')})>1}}
 assert not rows[root]['duplicate_names_from_multiple_git_sources'],(root,rows[root]['duplicate_names_from_multiple_git_sources'])
 print(root,'single Mere/Genet/Netrender source',flush=True)
(b/'metadata-closure.json').write_text(json.dumps(rows,indent=2)+'\n')
