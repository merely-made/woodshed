from pathlib import Path
import os,json,subprocess
import native_tools as n
B=Path('/Users/markik/Code/testing/woodshed/stage-p7b-20261017');T=B/'p7b-native-v2';C=B/'checks'
assert not n.process(int((T/'seed/native.pid').read_text()))
assert (T/'seed/scenario.done').read_text().startswith('RESULT ok\n')
facts=json.loads((T/'seed/p7b-final.json').read_text());assert facts['rate']==48000
before={str(p.relative_to(T/'profile')):n.sha(p) for p in (T/'profile').rglob('*') if p.is_file()}
env=os.environ.copy();env.update({k:v for k,v in json.loads((T/'seed/launch.json').read_text())['env'].items() if k not in ['WOODSHED_SCENARIO','WOODSHED_CAPTURE_DIR','WOODSHED_RECEIPT']});env['WOODSHED_EXPORT_REFERENCE_DIR']=str(T/'reference')
cmd=['cargo','test','--locked','--offline','-j2','-p','woodshed-genet','wav_export::tests::sealed_profile_export_reference','--','--exact','--ignored']
with (C/'sealed-reference.log').open('xb') as log:r=subprocess.run(cmd,cwd=n.REPO,env=env,stdout=log,stderr=subprocess.STDOUT)
n.write(C/'sealed-reference.json',{'command':cmd,'exit_code':r.returncode,'environment':{k:v for k,v in env.items() if k in ['WOODSHED_STATE','WOODSHED_SETTINGS','GRAPHSHELL_APP_ENDPOINT','WOODSHED_EXPORT_REFERENCE_DIR']}})
assert r.returncode==0
assert n.sha(T/'seed/loop-export.wav')==n.sha(T/'reference/loop-reference.wav')
after={str(p.relative_to(T/'profile')):n.sha(p) for p in (T/'profile').rglob('*') if p.is_file()};assert before==after
n.write(T/'reference/comparison.json',{'actual_native_worker_equals_separate_sealed_reference':True,'wav_sha256':n.sha(T/'seed/loop-export.wav'),'profile_hashes_before':before,'profile_hashes_after':after,'profile_unchanged':True,'source_revision':json.loads((T/'source.json').read_text())['revision']})
print('Actual native worker WAV equals independent sealed-profile offline reference; profile unchanged.')
