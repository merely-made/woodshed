from pathlib import Path
import json,sys,shutil,plistlib,subprocess
import native_tools as n
B=Path(__file__).parent;T=B/'native-v2';ID='made.merely.woodshed.relatedlabels20261017';ENDPOINT='/tmp/ws-labels1017-v2.sock'
def phase(name,scenario_name):
 build=json.loads((T/'build.json').read_text());source=n.REPO/'scenarios'/scenario_name
 assert n.sha(source)==build['sources']['scenarios/'+scenario_name]
 o=T/name;o.mkdir();shutil.copy2(source,o/'scenario.scn')
 app=o/'Woodshed Related Labels.app';m=app/'Contents/MacOS';m.mkdir(parents=True)
 for f in ['launcher','woodshed']:shutil.copy2(T/'frozen'/f,m/f)
 env={'WOODSHED_CAPTURE_DIR':str(o),'WOODSHED_RECEIPT':str(o/'scenario.done'),'WOODSHED_SCENARIO':str(o/'scenario.scn'),'WOODSHED_PRESENTATION_TIMEOUT_SECS':'60','WOODSHED_WIDTH':'1180','WOODSHED_HEIGHT':'1000','TMPDIR':'/tmp','CAMBIUM_HOST_FRAME_TRACE':'1','WOODSHED_STATE':str(T/(name+'-profile')/'practice.bin'),'WOODSHED_SETTINGS':str(T/(name+'-profile')/'settings.json'),'GRAPHSHELL_APP_ENDPOINT':ENDPOINT,'XDG_DATA_HOME':str(T/'data'),'XDG_CONFIG_HOME':str(T/'config'),'XDG_CACHE_HOME':str(T/'cache'),'MERE_ROOT':str(T/'mere')}
 info={'CFBundleExecutable':'launcher','CFBundleIdentifier':ID,'CFBundleName':'Woodshed Related Labels','CFBundlePackageType':'APPL','CFBundleVersion':'1','NSHighResolutionCapable':True,'WoodshedAcceptanceBinarySHA256':n.sha(m/'woodshed'),'WoodshedAcceptanceScenarioSHA256':n.sha(o/'scenario.scn'),'WoodshedAcceptanceSourceRevision':build['source_revision'],'NSMicrophoneUsageDescription':'Woodshed uses microphone input only when requested. This label acceptance scenario does not record.','WoodshedAcceptanceEnvironment':env,'WoodshedAcceptanceWorkingDirectory':str(n.REPO)}
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
 return e
step=sys.argv[1]
if step=='prepare':
 build=json.loads((B/'checks/bound-desktop-build.json').read_text());assert build['exit_code']==0 and build['sources_unchanged']
 assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=n.REPO,text=True).strip()==build['source_revision']
 names=subprocess.check_output(['git','ls-files'],cwd=n.REPO,text=True).splitlines()
 assert {name:n.sha(n.REPO/name) for name in names}==build['sources'] and n.sha(Path(build['executable']))==build['executable_sha256']
 n.freeze(T,{'woodshed':Path(build['executable']),'launcher':B/'stable_launcher'})
 shutil.copy2(B/'checks/bound-desktop-build.json',T/'build.json')
 n.keeper(T,ENDPOINT);phase('labels','related_labels_receipt.scn')
elif step=='p2':
 exited('labels');phase('p2','canvas_controls_p2.scn')
elif step=='finish':
 exits={name:exited(name) for name in ['labels','p2']};n.stop_keeper(T)
 n.write(T/'completion.json',{'source_revision':json.loads((T/'build.json').read_text())['source_revision'],'all_scenarios_ok':True,'supervised_process_exits':exits,'keeper_original_fixture_files_unchanged':True,'owned_processes_exited':True,'input_recording_not_exercised':True})
else:raise SystemExit('unknown step')
