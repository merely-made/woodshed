from pathlib import Path
import json,sys,shutil
import native_tools as n
BASE=Path('/Users/markik/Code/testing/woodshed/looper-followups-20261017');T=BASE/'native-v2'
def finish(name):
 p=T/name
 assert (p/'scenario.done').read_text().startswith('RESULT ok\n')
 assert not n.process(int((p/'native.pid').read_text()))
 n.write(p/'phase-hashes.json',{f.name:n.sha(f) for f in p.iterdir() if f.is_file()})
step=sys.argv[1]
if step=='seed':
 n.freeze(T,{'woodshed':n.REPO/'target/debug/woodshed-genet'})
 endpoint=n.keeper(T,'/tmp/ws-followups-1017-v2.sock')
 env={'WOODSHED_STATE':T/'profile/practice.bin','WOODSHED_SETTINGS':T/'profile/settings.json','GRAPHSHELL_APP_ENDPOINT':endpoint}
 app=n.phase(T,'seed','woodshed',(n.REPO/'scenarios/looper_followups_seed.scn').read_text(),env)
 print(app)
elif step=='reopen':
 finish('seed')
 env=json.loads((T/'seed/launch.json').read_text())['env'];env={k:v for k,v in env.items() if k in ['WOODSHED_STATE','WOODSHED_SETTINGS','GRAPHSHELL_APP_ENDPOINT']}
 app=n.phase(T,'reopen','woodshed',(n.REPO/'scenarios/looper_followups_reopen.scn').read_text(),env)
 shutil.copy2(T/'seed/p7b-final.json',T/'reopen/p7b-final.json')
 n.write(T/'reopen/expected-copy.json',{'copied_after_seed_exit':True,'seed_pid':int((T/'seed/native.pid').read_text()),'sha256':n.sha(T/'seed/p7b-final.json')})
 print(app)
elif step=='finish':
 finish('reopen')
 a=json.loads((T/'seed/p7b-final.json').read_text());b=json.loads((T/'reopen/p7b-reopened.json').read_text())
 keys=['set','owner','looper','saved_backing','manifest','captures','admitted','backend_admitted','requested','wav_frames','wav_digest']
 assert all(a[k]==b[k] for k in keys)
 assert n.sha(T/'seed/loop-export.wav')==n.sha(T/'reopen/loop-export.wav')
 assert a['worker_wav_exact'] is True
 assert json.loads((T/'reopen/p7b-reopenexport.json').read_text())['worker_wav_exact'] is True
 assert a['sealed_exact'] and b['sealed_exact'] and not b['playing'] and b['input_unopened']
 assert all(n.sha(T/'seed'/p)==h for p,h in json.loads((T/'seed/phase-hashes.json').read_text()).items())
 n.stop_keeper(T)
 n.write(T/'completion.json',{'source_revision':json.loads((T/'source.json').read_text())['revision'],'all_owned_processes_exited':True,'keeper_exited':True,'keeper_original_files_unchanged':True,'keeper_original_file_count':len(json.loads((T/'keeper-before.json').read_text())),'exact_reopen':{k:a[k]==b[k] for k in keys},'actual_worker_wav_sha256':n.sha(T/'seed/loop-export.wav'),'actual_worker_wavs_byte_identical':True,'worker_wav_frames':a['wav_frames'],'input_unopened':True,'sample_rate':a['rate']})
 print('Exact Set/capture PCM/WAV reopen verified; owned apps and Keeper exited.')
else:raise SystemExit('unknown step')
