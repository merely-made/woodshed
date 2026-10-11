from pathlib import Path
import subprocess,json
base=Path('/Users/markik/Code/testing/hocket-firewheel-f3-20261011')
repo=Path('/Users/markik/Code/worktrees/hocket-firewheel-f3-20261011/ports/hocket')
launcher=Path('/Users/markik/.cargo/git/checkouts/mere-9d8e871c24d9ba76/e95326d/scripts/run_macos_scenario.py')
python=Path('/Users/markik/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/bin/python3')
env={
 'HOCKET_APPEARANCE_RECEIPT':'1',
 'HOCKET_SETTINGS':str(base/'profile/settings.json'),
 'HOCKET_APPEARANCE':str(base/'profile/appearance.json'),
 'HOCKET_THEME_LIBRARY':str(base/'profile/themes.json'),
 'XDG_DATA_HOME':str(base/'profile/data'),
 'XDG_CONFIG_HOME':str(base/'profile/config'),
 'XDG_RUNTIME_DIR':'/tmp/hocket-firewheel-f3-r-20261011',
}
seq=[('native-seed','tabard_appearance.scn'),('native-reopen','tabard_appearance_reopen.scn'),('native-same-id','tabard_appearance_same_id.scn'),('native-reopen-brown','tabard_appearance_reopen.scn'),('native-recorder','recorder_controls.scn')]
results=[]
for name,scenario in seq:
 cmd=[str(python),str(launcher),'--binary',str(base/'target/debug/hocket-genet'),'--prefix','HOCKET','--scenario',str(repo/'scenarios'/scenario),'--output',str(base/name),'--timeout','45']
 for key,value in env.items(): cmd+=['--env',key+'='+value]
 result=subprocess.run(cmd,text=True,capture_output=True)
 (base/name/'launcher.log').write_text(result.stdout+result.stderr)
 outcome=json.loads((base/name/'process.json').read_text())
 outcome['name']=name;outcome['scenario']=scenario;results.append(outcome)
 print(name,'exit',result.returncode,'captures',outcome['captures'],'timeout',outcome['timeout'],flush=True)
 (base/'native-results.json').write_text(json.dumps(results,indent=2)+'\n')
 if result.returncode or not outcome['success']: raise SystemExit(1)
