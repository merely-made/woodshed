"""Owned native Mesquite launch; private synthetic Keeper and app profile."""
from pathlib import Path
import sys,subprocess,time,uuid,hashlib,json,shutil,os
REPO=Path('/Users/markik/Code/repos/woodshed')
BASE=Path('/Users/markik/Code/testing/woodshed/audio-packs-ap3_20261020')
MERE=Path('/Users/markik/Code/repos/mere')
app,cohort,scenario=sys.argv[1:4]
assert app in ('woodshed','hocket')
base=BASE/cohort;base.mkdir(exist_ok=False)
(base/'frozen').mkdir()
binary=base/'frozen'/f'{app}-genet';shutil.copy2(REPO/f'target/debug/{app}-genet',binary)
keeper_binary=base/'frozen/keeper';shutil.copy2('/Users/markik/Code/worktrees/mere-knot-catalog-folds/target/debug/tucket-custody-receipt-host',keeper_binary)
endpoint='/tmp/ap3-'+uuid.uuid4().hex[:12]+'.sock'
log=(base/'keeper.log').open('w')
keeper=subprocess.Popen([str(keeper_binary),endpoint,str(base/'keeper'),'no-wallet'],stdout=log,stderr=subprocess.STDOUT)
awake=subprocess.Popen(['/usr/bin/caffeinate','-d','-i','-t','600'])
try:
 for _ in range(300):
  if (base/'keeper/ready').exists():break
  if keeper.poll() is not None:raise RuntimeError('isolated keeper exited')
  time.sleep(.1)
 else:raise RuntimeError('isolated keeper timeout')
 prefix=app.upper()
 env={prefix+'_WIDTH':'1180',prefix+'_HEIGHT':'980',prefix+'_PRESENTATION_TIMEOUT_SECS':'60',
 'APPDATA':str(base/'profile'),'XDG_DATA_HOME':str(base/'data'),'XDG_CONFIG_HOME':str(base/'config'),
 'XDG_CACHE_HOME':str(base/'cache'),'MERE_ROOT':str(base/'mere'),'GRAPHSHELL_APP_ENDPOINT':endpoint,'TMPDIR':'/tmp'}
 if app=='woodshed':env.update(WOODSHED_STATE=str(base/'profile/practice.bin'),WOODSHED_SETTINGS=str(base/'profile/settings.json'))
 else:env.update(HOCKET_APPEARANCE=str(base/'profile/appearance.json'),HOCKET_THEME_LIBRARY=str(base/'profile/themes.json'))
 phases=[scenario]
 if len(sys.argv)>4: phases.append(sys.argv[4])
 results=[]
 for i,phase in enumerate(phases):
  if i:
   kit=BASE/'installed-kit'; assert kit.is_dir();kit.rename(base/'uninstalled-kit')
  scn=REPO/'validation/audio-packs-ap3_20261020'/phase
  frozen=base/'frozen'/phase;shutil.copy2(scn,frozen)
  command=[sys.executable,str(REPO/'validation/audio-packs-ap3_20261020/run_macos_scenario.py'),'--binary',str(binary),'--prefix',prefix,
   '--scenario',str(frozen),'--output',str(base/f'run-{i+1}'),'--timeout','240']
  for k,v in env.items():command+=['--env',k+'='+v]
  (base/f'command-{i+1}.json').write_text(json.dumps(command,indent=2)+'\n')
  launch_tmp=base/'launch-tmp';launch_tmp.mkdir(exist_ok=True)
  launch_env=dict(os.environ,TMPDIR=str(launch_tmp))
  launch=subprocess.Popen(command,cwd=REPO,env=launch_env)
  dialog_queue=scn.with_suffix('.dialogs.json')
  helper=None
  if dialog_queue.exists():
   for _ in range(300):
    listing=subprocess.check_output(['ps','-axo','pid=,command='],text=True)
    owned=[]
    for line in listing.splitlines():
     pid,_,process=line.strip().partition(' ')
     if process.startswith(str(launch_tmp)+'/mesquite-native-') and '/Contents/MacOS/product-' in process:owned.append(int(pid))
    if owned:
     assert len(owned)==1
     helper=subprocess.Popen([sys.executable,str(REPO/'validation/audio-packs-ap3_20261020/dialogs.py'),str(owned[0]),str(dialog_queue),str(base/f'dialogs-{i+1}.json')])
     break
    if launch.poll() is not None:break
    time.sleep(.1)
  result=launch.wait()
  if helper is not None:
   if helper.poll() is None:helper.terminate()
   helper.wait(timeout=5)
  results.append(result)
  if result:break
 (base/'inputs.json').write_text(json.dumps({'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),
  'keeper_sha256':hashlib.sha256(keeper_binary.read_bytes()).hexdigest(),'keeper_source':'prior Mere fold acceptance fixture, isolated synthetic no-wallet',
  'environment':env,'results':results},indent=2)+'\n')
 sys.exit(max(results))
finally:
 (base/'keeper/stop').touch()
 try:keeper.wait(timeout=10)
 except subprocess.TimeoutExpired:keeper.terminate();keeper.wait(timeout=5)
 log.close();awake.terminate();awake.wait(timeout=5)
