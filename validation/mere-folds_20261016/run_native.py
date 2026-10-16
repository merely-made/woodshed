from pathlib import Path
import datetime, hashlib, json, os, shutil, subprocess, sys, time, uuid
root=Path('/Users/markik/Code/worktrees/woodshed-graph-folds')
out=Path(__file__).parent
mere=Path('/Users/markik/Code/worktrees/mere-knot-catalog-folds')
cohort=sys.argv[1] if len(sys.argv)>1 else 'r1'
base=out/('native-'+cohort)
assert not base.exists(), 'use a fresh cohort'
base.mkdir()
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
frozen=base/'frozen';frozen.mkdir()
binary=frozen/'woodshed-genet';shutil.copy2(root/'target/debug/woodshed-genet',binary)
keeper_binary=frozen/'tucket-custody-receipt-host';shutil.copy2(mere/'target/debug/tucket-custody-receipt-host',keeper_binary)
for name in ['mere_folds.scn','mere_folds_reopen.scn']:shutil.copy2(root/'scenarios'/name,frozen/name)
paths=subprocess.check_output(['git','ls-files','-c','-o','--exclude-standard','crates','scenarios','Cargo.toml','Cargo.lock'],cwd=root,text=True).splitlines()
inputs={'captured_before_launch_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),
 'initial_upstream_base':'e6f0950477a907033545c6d51f065131f6a56822',
 'integrated_upstream':subprocess.check_output(['git','rev-parse','origin/main'],cwd=root,text=True).strip(),
 'woodshed_source':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip(),
 'mere_lock':'6575075057e8909032475ae6c88118a19969eadc',
 'build_environment':{'CARGO_PROFILE_DEV_DEBUG':'0','CARGO_INCREMENTAL':'0','native_command':'cargo build --locked --offline -p woodshed-genet -j4'},
 'rustc':subprocess.check_output(['rustc','-vV'],text=True),
 'cargo_config_sha256':sha(root/'.cargo/config.toml'),
 'keeper_source':subprocess.check_output(['git','rev-parse','HEAD'],cwd=mere,text=True).strip(),
 'binary_sha256':sha(binary),'keeper_binary_sha256':sha(keeper_binary),
 'source_sha256':{p:sha(root/p) for p in sorted(set(paths)) if (root/p).is_file()},
 'launcher_sha256':sha(mere/'scripts/run_macos_scenario.py'),'driver_sha256':sha(Path(__file__)),
 'os':subprocess.check_output(['sw_vers'],text=True),'architecture':subprocess.check_output(['uname','-m'],text=True).strip()}
(base/'inputs.json').write_text(json.dumps(inputs,indent=2)+'\n')
(base/'source.patch').write_bytes(subprocess.check_output(['git','diff','--binary'],cwd=root))
endpoint='/tmp/ws-fold-'+uuid.uuid4().hex[:12]+'.sock'
keeper_root=base/'keeper';keeper_log=(base/'keeper.log').open('w')
awake=subprocess.Popen(['/usr/bin/caffeinate','-d','-i','-t','360'])
keeper=subprocess.Popen([str(keeper_binary),endpoint,str(keeper_root),'no-wallet'],stdout=keeper_log,stderr=subprocess.STDOUT)
results=[];before={}
try:
 for _ in range(300):
  if (keeper_root/'ready').exists():break
  if keeper.poll() is not None:raise RuntimeError('keeper failed: '+(base/'keeper.log').read_text()[-3000:])
  time.sleep(.1)
 else:raise RuntimeError('keeper not ready in 30s')
 before={str(p.relative_to(keeper_root)):sha(p) for p in keeper_root.rglob('*') if p.is_file()}
 for mode in ['seed','reopen']:
  assert all(sha(root/p)==h for p,h in inputs['source_sha256'].items()),'runtime source changed'
  output=base/mode
  env={'WOODSHED_WIDTH':'1180','WOODSHED_HEIGHT':'900','WOODSHED_PRESENTATION_TIMEOUT_SECS':'60',
   'WOODSHED_STATE':str(base/'profile/practice.bin'),'WOODSHED_SETTINGS':str(base/'profile/settings.json'),
   'APPDATA':str(base/'profile'),'XDG_DATA_HOME':str(base/'data'),'XDG_CONFIG_HOME':str(base/'config'),
   'XDG_CACHE_HOME':str(base/'cache'),'MERE_ROOT':str(base/'mere'),'GRAPHSHELL_APP_ENDPOINT':endpoint,'TMPDIR':'/tmp'}
  scenario=frozen/('mere_folds.scn' if mode=='seed' else 'mere_folds_reopen.scn')
  command=[sys.executable,str(mere/'scripts/run_macos_scenario.py'),'--binary',str(binary),'--prefix','WOODSHED',
   '--scenario',str(scenario),'--output',str(output),'--timeout','150']
  for k,v in env.items():command+=['--env',k+'='+v]
  (base/(mode+'-config.json')).write_text(json.dumps({'command':command,'environment':env,'binary_sha256':sha(binary),'scenario_sha256':sha(scenario),'captured_before_launch_utc':datetime.datetime.now(datetime.timezone.utc).isoformat()},indent=2)+'\n')
  with (base/('launch-'+mode+'.log')).open('w') as log:
   result=subprocess.run(command,cwd=root,stdout=log,stderr=subprocess.STDOUT)
  results.append({'phase':mode,'exit_code':result.returncode})
  print(mode, (output/'process.json').read_text() if (output/'process.json').exists() else 'no process receipt',flush=True)
  if result.returncode:
   print((output/'native.log').read_text()[-4000:] if (output/'native.log').exists() else 'no native log',flush=True)
   sys.exit(result.returncode)
  if mode=='seed':
   (base/'profile-after-seed.json').write_text(json.dumps({str(p.relative_to(base)):sha(p) for p in (base/'profile').rglob('*') if p.is_file()},indent=2)+'\n')
finally:
 if keeper_root.exists():(keeper_root/'stop').touch()
 try:keeper.wait(timeout=10)
 except subprocess.TimeoutExpired:keeper.terminate();keeper.wait(timeout=5)
 keeper_log.close()
 awake.terminate();awake.wait(timeout=5)
 after={str(p.relative_to(keeper_root)):sha(p) for p in keeper_root.rglob('*') if p.is_file() and p.name!='stop'}
 (base/'keeper-process.json').write_text(json.dumps({'pid':keeper.pid,'exited':keeper.poll() is not None,'exit_code':keeper.returncode,'public_synthetic_fixture':True,'bounded_display_sleep_assertion_exited':awake.poll() is not None,'before_sha256':before,'after_sha256':after,'original_files_unchanged':all(after.get(p)==h for p,h in before.items()),'runs':results},indent=2)+'\n')
