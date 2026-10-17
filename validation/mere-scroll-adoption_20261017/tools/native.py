from pathlib import Path
import json,sys,shutil,plistlib
import native_tools as n
import hashlib,subprocess
B=Path(__file__).parent;T=B/'native-v2';ID='made.merely.woodshed.scrolladoption20261017'
def phase(name,scenario,profile="profile"):
 scenario_name={'seed':'looper_scroll_receipt.scn','reopen':'looper_scroll_reopen.scn','canvas':'canvas_controls_p2.scn'}[name]
 assert n.sha(T/'scenarios'/scenario_name)==json.loads((T/'scenario-hashes.json').read_text())[scenario_name]
 assert n.sha(T/'scenarios'/scenario_name)==json.loads((T/'build.json').read_text())['sources']['scenarios/'+scenario_name]
 o=T/name;o.mkdir();(o/'scenario.scn').write_text(scenario)
 app=o/'Woodshed Scroll Adoption.app';m=app/'Contents/MacOS';m.mkdir(parents=True)
 for f in ['launcher','woodshed']:shutil.copy2(T/'frozen'/f,m/f)
 env={'WOODSHED_CAPTURE_DIR':str(o),'WOODSHED_RECEIPT':str(o/'scenario.done'),'WOODSHED_SCENARIO':str(o/'scenario.scn'),'WOODSHED_PRESENTATION_TIMEOUT_SECS':'60','WOODSHED_WIDTH':'1180','WOODSHED_HEIGHT':'1000','TMPDIR':'/tmp','CAMBIUM_HOST_FRAME_TRACE':'1','WOODSHED_STATE':str(T/profile/'practice.bin'),'WOODSHED_SETTINGS':str(T/profile/'settings.json'),'GRAPHSHELL_APP_ENDPOINT':'/tmp/ws-scroll-prod1017-v2.sock','XDG_DATA_HOME':str(T/'data'),'XDG_CONFIG_HOME':str(T/'config'),'XDG_CACHE_HOME':str(T/'cache'),'MERE_ROOT':str(T/'mere')}
 info={'CFBundleExecutable':'launcher','CFBundleIdentifier':ID,'CFBundleName':'Woodshed Scroll Adoption','CFBundlePackageType':'APPL','CFBundleVersion':'1','NSHighResolutionCapable':True,'WoodshedAcceptanceBinarySHA256':n.sha(m/'woodshed'),'WoodshedAcceptanceScenarioSHA256':n.sha(o/'scenario.scn'),'WoodshedAcceptanceSourceRevision':json.loads((T/'source.json').read_text())['revision'],'NSMicrophoneUsageDescription':'Woodshed uses microphone input only when requested. This scroll acceptance scenario does not open input.','WoodshedAcceptanceEnvironment':env,'WoodshedAcceptanceWorkingDirectory':str(n.REPO)}
 with (app/'Contents/Info.plist').open('wb') as f:plistlib.dump(info,f)
 n.write(o/'launch.json',{'app':str(app),'env':env,'source':json.loads((T/'source.json').read_text()),'bundle_id':ID,'launcher_sha256':n.sha(m/'launcher'),'binary_sha256':n.sha(m/'woodshed'),'scenario_sha256':n.sha(o/'scenario.scn')});print(app)
def exited(name):
 p=T/name;assert (p/'scenario.done').read_text().startswith('RESULT ok\n')
 e=json.loads((p/'exit.json').read_text());launch=json.loads((p/'launch.json').read_text())
 assert e['normal_exit'] and e['exit_code']==0 and e['signal']==0 and e['executable_identity_proven'] and e['binary_unchanged'] and e['scenario_unchanged']
 assert e['observed_executable']==e['expected_executable']==str(Path(launch['app'])/'Contents/MacOS/woodshed')
 assert e['binary_sha256']==launch['binary_sha256'] and e['scenario_sha256']==launch['scenario_sha256'] and e['source_revision']==launch['source']['revision']
 assert not n.process(e['child_pid']) and not n.process(e['supervisor_pid'])
 assert e['child_pid']==int((p/'native.pid').read_text()) and e['supervisor_pid']==int((p/'supervisor.pid').read_text())
step=sys.argv[1]
if step=='prepare':
 build=json.loads((B/'checks/bound-desktop-build.json').read_text());assert build['exit_code']==0 and build['sources_unchanged']
 assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=n.REPO,text=True).strip()==build['source_revision']
 names=subprocess.check_output(['git','ls-files'],cwd=n.REPO,text=True).splitlines()
 assert {name:n.sha(n.REPO/name) for name in names}==build['sources'] and n.sha(Path(build['executable']))==build['executable_sha256']
 n.freeze(T,{'woodshed':Path(build['executable']),'launcher':B/'stable_launcher'})
 n.write(T/'launcher-source.json',{'source_sha256':n.sha(B/'stable_launcher.m'),'binary_sha256':n.sha(B/'stable_launcher'),'supervised_exit':True})
 shutil.copy2(B/'checks/bound-desktop-build.json',T/'build.json');(T/'scenarios').mkdir()
 for name in ['looper_scroll_receipt.scn','looper_scroll_reopen.scn','canvas_controls_p2.scn']:shutil.copy2(n.REPO/'scenarios'/name,T/'scenarios'/name)
 n.write(T/'scenario-hashes.json',{p.name:n.sha(p) for p in (T/'scenarios').glob('*.scn')})
 n.keeper(T,'/tmp/ws-scroll-prod1017-v2.sock')
 phase('seed',(T/'scenarios/looper_scroll_receipt.scn').read_text())
elif step=='reopen':
 exited('seed');n.write(T/'capture-companions-before-reopen.json',{str(p.relative_to(T/'profile')):n.sha(p) for p in (T/'profile/practice.loop-pcm').rglob('*') if p.is_file()})
 phase('reopen',(T/'scenarios/looper_scroll_reopen.scn').read_text());shutil.copy2(T/'seed/p7a-final.json',T/'reopen/p7a-final.json')
 n.write(T/'reopen/expected-copy.json',{'copied_after_seed_exit':True,'sha256':n.sha(T/'seed/p7a-final.json')})
elif step=='canvas':
 exited('reopen');phase('canvas',(T/'scenarios/canvas_controls_p2.scn').read_text(),'canvas-profile')
elif step=='finish':
 exited('seed');exited('reopen');exited('canvas')
 a=json.loads((T/'seed/p7a-final.json').read_text());b=json.loads((T/'reopen/p7a-reopened.json').read_text());z=json.loads((T/'reopen/p7a-reopened-export.json').read_text());keys=['set','owner','looper','saved_backing','manifest','captures','admitted','backend_admitted','requested','wav_frames','wav_digest']
 for actual in [a,b,z]:
  assert all(a[k]==actual[k] for k in keys) and actual['sealed_exact'] and actual['input_unopened'] and not actual['playing'] and len(actual['captures'])==1
 assert n.sha(T/'seed/loop-export.wav')==n.sha(T/'reopen/loop-export.wav')
 before=json.loads((T/'capture-companions-before-reopen.json').read_text());after={str(p.relative_to(T/'profile')):n.sha(p) for p in (T/'profile/practice.loop-pcm').rglob('*') if p.is_file()};assert before and before==after
 for name in ['seed','reopen','canvas']:
  p=T/name;launch=json.loads((p/'launch.json').read_text());app=Path(launch['app']);assert n.sha(app/'Contents/MacOS/woodshed')==n.sha(T/'frozen/woodshed');assert n.sha(app/'Contents/MacOS/launcher')==n.sha(T/'frozen/launcher');assert launch['bundle_id']==ID
 assert json.loads((T/'reference/comparison.json').read_text())['native_seed_reopen_and_separate_reference_byte_identical']
 n.stop_keeper(T);n.write(T/'completion.json',{'source_revision':json.loads((T/'source.json').read_text())['revision'],'seed_and_reopen_scenarios_passed':True,'owned_apps_and_keeper_exited':True,'keeper_original_fixture_files_unchanged':True,'capture_companions_unchanged':True,'exact_reopen_keys':keys,'actual_worker_wav_sha256':n.sha(T/'seed/loop-export.wav'),'input_unopened':True,'raw_audio_local_only':True})
else:raise SystemExit('unknown phase')
