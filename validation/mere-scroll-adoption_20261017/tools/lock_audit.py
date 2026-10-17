import re,json,hashlib
from pathlib import Path
b=Path(__file__).parent;r=Path('/Users/markik/Code/repos/woodshed')
def packages(p):
 s=p.read_text();out={}
 for part in re.split(r'(?m)^\[\[',s)[1:]:
  if not part.startswith('package]]\n'):continue
  name=re.search(r'^name = "([^"]+)"',part,re.M).group(1);version=re.search(r'^version = "([^"]+)"',part,re.M).group(1);source=re.search(r'^source = "([^"]+)"',part,re.M)
  out[(name,version)]=(source.group(1) if source else None,part)
 return out
rows={}
for root in ['.','ports/hocket','ports/redshank','validation/relationship-recipe']:
 p=Path('Cargo.lock') if root=='.' else Path(root)/'Cargo.lock';a=packages(b/'baseline'/p);z=packages(r/p)
 changed=[];registry_unchanged=0
 for key,(source,block) in a.items():
  if source and source.startswith('registry+'):
   assert key in z and z[key]==(source,block),(root,key,'registry changed');registry_unchanged+=1
  elif source and source.startswith('git+') and not any('/'+f+'.git?' in source for f in ['mere','genet','netrender','dramatis']):
   assert key in z and z[key]==(source,block),(root,key,'unrelated Git changed')
  if key in z and z[key]!=(source,block):changed.append(key)
 added=[key for key in z if key not in a];removed=[key for key in a if key not in z]
 assert not any(z[k][0] and z[k][0].startswith('registry+') for k in added),(root,'new registry package')
 rows[root]={'registry_package_records_unchanged':registry_unchanged,'unrelated_git_package_records_unchanged':True,'added':added,'removed':removed,'changed_package_records':changed,'baseline_sha256':hashlib.sha256((b/'baseline'/p).read_bytes()).hexdigest(),'candidate_sha256':hashlib.sha256((r/p).read_bytes()).hexdigest()}
 print(root,'preserved',registry_unchanged,'registry records; added',added,'removed',removed,flush=True)
(b/'lock-scope.json').write_text(json.dumps(rows,indent=2)+'\n')
