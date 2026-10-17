from pathlib import Path
import os,json,subprocess,re
import native_tools as n
B=Path(__file__).parent;T=B/'native-v2'
assert (T/'reopen/scenario.done').read_text().startswith('RESULT ok\n')
e=json.loads((T/'reopen/exit.json').read_text());assert e['normal_exit'] and e['exit_code']==0 and e['executable_identity_proven'];assert not n.process(e['child_pid']) and not n.process(e['supervisor_pid'])
build=json.loads((T/'build.json').read_text());source=build['sources']
def check_source():
 names=subprocess.check_output(['git','ls-files'],cwd=n.REPO,text=True).splitlines()
 assert {name:n.sha(n.REPO/name) for name in names}==source
 assert not subprocess.check_output(['git','status','--porcelain'],cwd=n.REPO,text=True)
 assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=n.REPO,text=True).strip()==build['source_revision']
check_source();before={str(p.relative_to(T/'profile')):n.sha(p) for p in (T/'profile').rglob('*') if p.is_file()}
env=os.environ.copy();env.update({k:v for k,v in json.loads((T/'seed/launch.json').read_text())['env'].items() if k not in ['WOODSHED_SCENARIO','WOODSHED_CAPTURE_DIR','WOODSHED_RECEIPT']});env['WOODSHED_EXPORT_REFERENCE_DIR']=str(T/'reference')
assert not (T/'reference').exists(),'Fresh reference output directory required'
cmd=['cargo','test','--offline','--locked','-j2','-p','woodshed-genet','--no-run','--message-format=json']
p=subprocess.run(cmd,cwd=n.REPO,env=env,capture_output=True,text=True);(B/'checks/separate-reference-build.log').write_text(p.stdout+'\n'+p.stderr);assert p.returncode==0,p.stderr[-2000:];check_source()
artifacts=[json.loads(l) for l in p.stdout.splitlines() if l.startswith('{')];executables=[x for x in artifacts if x.get('reason')=='compiler-artifact' and x.get('target',{}).get('name')=='woodshed-genet' and x.get('profile',{}).get('test') and x.get('executable')];assert len(executables)==1
exe=Path(executables[0]['executable']);exe_sha=n.sha(exe);run=[str(exe),'wav_export::tests::sealed_profile_export_reference','--exact','--ignored']
with (B/'checks/separate-sealed-reference.log').open('x') as f:q=subprocess.run(run,cwd=n.REPO,env=env,stdout=f,stderr=subprocess.STDOUT)
check_source();assert n.sha(exe)==exe_sha
n.write(B/'checks/separate-sealed-reference.json',{'build_command':cmd,'build_exit_code':p.returncode,'command':run,'exit_code':q.returncode,'executable':str(exe),'executable_sha256':exe_sha,'source_revision':build['source_revision'],'source_digest':build['source_digest'],'sources_unchanged_before_after':True})
assert q.returncode==0
text=(B/'checks/separate-sealed-reference.log').read_text();suites=re.findall(r'test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;',text);assert suites==[('ok','1','0','0')]
assert n.sha(T/'seed/loop-export.wav')==n.sha(T/'reopen/loop-export.wav')==n.sha(T/'reference/loop-reference.wav')
assert (T/'reference/loop-reference.wav').read_bytes()!=(T/'reference/backing-reference.wav').read_bytes(),'stepped-triangle-v1 synthetic envelope must contribute signal'
after={str(p.relative_to(T/'profile')):n.sha(p) for p in (T/'profile').rglob('*') if p.is_file()};assert before==after
facts=json.loads((T/'reference/reference.json').read_text());assert facts['captured_takes']==1 and facts['rate']==48000 and facts['frames']==132000
n.write(T/'reference/comparison.json',{'native_seed_reopen_and_separate_reference_byte_identical':True,'wav_sha256':n.sha(T/'seed/loop-export.wav'),'synthetic_envelope_contributes_signal':True,'fixture':'stepped-triangle-v1','microphone_evidence':False,'profile_hashes_before':before,'profile_hashes_after':after,'profile_unchanged':True,'executable_sha256':exe_sha,'source_revision':build['source_revision'],'source_digest':build['source_digest'],'sources_unchanged_before_after':True,'raw_audio_local_only':True})
print('48 kHz synthetic-envelope native seed/reopen and separate sealed reference WAVs match; stepped-triangle-v1 contributes signal; profile unchanged.')
