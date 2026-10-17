from pathlib import Path
import json,subprocess,hashlib,datetime
B=Path(__file__).parent;R=Path('/Users/markik/Code/repos/woodshed')
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def sources():
 names=subprocess.check_output(['git','ls-files'],cwd=R,text=True).splitlines()
 return {p:sha(R/p) for p in names}
def digest(v):return hashlib.sha256(json.dumps(v,sort_keys=True).encode()).hexdigest()
assert not subprocess.check_output(['git','status','--porcelain'],cwd=R,text=True)
rev=subprocess.check_output(['git','rev-parse','HEAD'],cwd=R,text=True).strip();before=sources();cmd=['cargo','build','--offline','--locked','-j2','-p','woodshed-genet','--message-format=json']
start=datetime.datetime.now(datetime.timezone.utc).isoformat()
p=subprocess.run(cmd,cwd=R,capture_output=True,text=True);(B/'checks/bound-desktop-build.log').write_text(p.stdout+'\n'+p.stderr)
assert p.returncode==0,p.stderr[-2000:];after=sources();assert before==after;assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=R,text=True).strip()==rev;assert not subprocess.check_output(['git','status','--porcelain'],cwd=R,text=True)
artifacts=[json.loads(l) for l in p.stdout.splitlines() if l.startswith('{')];executables=[x for x in artifacts if x.get('reason')=='compiler-artifact' and x.get('target',{}).get('name')=='woodshed-genet' and x.get('executable')];assert len(executables)==1
exe=Path(executables[0]['executable']);receipt={'command':cmd,'exit_code':p.returncode,'source_revision':rev,'sources':before,'source_digest':digest(before),'sources_unchanged':True,'executable':str(exe),'executable_sha256':sha(exe),'started_utc':start,'ended_utc':datetime.datetime.now(datetime.timezone.utc).isoformat()}
(B/'checks/bound-desktop-build.json').write_text(json.dumps(receipt,indent=2)+'\n');print('Build bound to',rev,exe,receipt['executable_sha256'])
