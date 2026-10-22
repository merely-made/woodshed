from pathlib import Path
import hashlib,json,os,subprocess,sys
base=Path('/Users/markik/Code/testing/woodshed-persona-store-20261022')
root=base/'final'
repo=Path('/Users/markik/Code/worktrees/woodshed-persona-store-20261022')
endpoint='/tmp/ws-d1-final-20261022.sock'
env=os.environ.copy()
for name in ('WOODSHED_STATE','WOODSHED_SETTINGS'): env.pop(name,None)
env.update(GRAPHSHELL_APP_ENDPOINT=endpoint,WOODSHED_DATA_DIR=str(root/'product'),TMPDIR='/tmp',CARGO_TARGET_DIR='/Users/markik/Code/repos/woodshed/target',CARGO_BUILD_BUILD_DIR='/Users/markik/Code/repos/woodshed/target/build')
completed=[]
def control(action,*args):
    data=subprocess.check_output([sys.executable,str(base/'keeper_control.py'),endpoint,action,*args],env=env,text=True)
    result=json.loads(data)
    (root/('control-'+action+'-'+str(len(list(root.glob('control-*.json'))))+'.json')).write_text(data)
    return result

def phase(name,scenario=None,manual_close=False):
    scenario=scenario or name
    cmd=[sys.executable,str(repo/'scripts/run-macos-scenario.py'),'--binary',str(root/'bin/woodshed-genet'),'--prefix','WOODSHED','--scenario',str(base/'scenarios'/(scenario+'.scn')),'--output',str(root/'native'/name),'--env','GRAPHSHELL_APP_ENDPOINT='+endpoint,'--env','WOODSHED_DATA_DIR='+str(root/'product'),'--env','WOODSHED_APPEARANCE_RECEIPT=1','--env','TMPDIR=/tmp','--timeout','180' if manual_close else '60']
    if name=='aba': cmd+=['--env','WOODSHED_WIDTH=1100','--env','WOODSHED_HEIGHT=700']
    if manual_close: cmd+=['--bundle-identifier','made.merely.woodshed.d1.final.locked']
    print('START',name,flush=True)
    with (root/(name+'-launch.log')).open('w') as log:
        result=subprocess.run(cmd,cwd=repo,env=env,stdout=log,stderr=subprocess.STDOUT)
    meta=json.loads((root/'native'/name/'process.json').read_text())
    assert result.returncode==0 and meta['success'] and not meta['timeout'] and meta['launcher_exit']==0,(name,meta)
    completed.append(name)
    (root/'completed-phases.json').write_text(json.dumps(completed,indent=2))
    print('PASS',name,meta['captures'],'captures',flush=True)

def verify(name):
    verify_env=env.copy();verify_env.update(WOODSHED_PERSONA_RECEIPT_MODE='verify',WOODSHED_PERSONA_RECEIPT_OUTPUT=str(root/(name+'.json')))
    cmd=['cargo','test','--locked','-p','woodshed-genet','persona_store_receipt::seed_or_verify_native_persona_stores','--','--ignored','--exact','--nocapture']
    with (root/(name+'.log')).open('w') as log: subprocess.run(cmd,cwd=repo,env=verify_env,stdout=log,stderr=subprocess.STDOUT,check=True)
    seed=json.loads((root/'seed.json').read_text());observed=json.loads((root/(name+'.json')).read_text())
    for a,b in zip(seed,observed):
        for key in ('profile','owner','cards','captures','personal_relations','pcm_frames','pcm_bits_blake3'): assert a[key]==b[key],(a['profile'],key)
        for key in ('theme','bpm'): assert a['settings'][key]==b['settings'][key]
    print('PASS exact keeper-backed readback',name,flush=True)

def store_hashes():
    return {str(p.relative_to(root/'product')):hashlib.sha256(p.read_bytes()).hexdigest() for p in (root/'product/personas').rglob('*') if p.is_file()}

phase('aba')
control('choose','default'); phase('reopen-a')
control('choose','d1-b'); phase('reopen-b')
verify('verified')
seed=json.loads((root/'seed.json').read_text())
settings=root/'product/personas/v1'/seed[1]['owner']/'settings'
original=settings.read_bytes();(root/'b-settings.before').write_bytes(original)
settings.write_bytes(b'D1 deliberately unreadable sealed settings')
corrupt_hash=hashlib.sha256(settings.read_bytes()).hexdigest()
phase('incoming-failure')
assert control('status')['answer']['Ok']['profile']=='default'
assert hashlib.sha256(settings.read_bytes()).hexdigest()==corrupt_hash
(root/'failure-hashes.json').write_text(json.dumps({'before':hashlib.sha256(original).hexdigest(),'damaged':corrupt_hash,'after_failure':hashlib.sha256(settings.read_bytes()).hexdigest(),'family_still_default':True},indent=2))
settings.write_bytes(original)
verify('verified-after-failure')
before=store_hashes()
control('lock_vault')
assert control('status')['answer']['Ok']['lock']=='Locked'
# This scenario completes its receipt at the visible close guard. The root
# agent then clicks its observed Discard button through the native UI.
phase('locked', 'locked-final', manual_close=True)
after=store_hashes();assert before==after
(root/'locked-hashes.json').write_text(json.dumps({'before':before,'after':after,'unchanged':True},indent=2))
print('ALL FINAL NATIVE PHASES PASS',flush=True)
