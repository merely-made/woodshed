#!/usr/bin/env python3
"""Owned native acceptance with a private synthetic Keeper and receiving fixture."""
from pathlib import Path
import os,sys,subprocess,time,uuid,json,shutil,hashlib,threading
ROOT=Path('/Users/markik/Code/repos/woodshed')
SOURCE=ROOT/'ports/ringdown/validation/desktop-midi_20261020'
BASE=Path('/Users/markik/Code/testing/woodshed/desktop-midi_20261020')
cohort,scenario=sys.argv[1:3]
base=BASE/cohort;base.mkdir(exist_ok=False);(base/'frozen').mkdir()
exe=base/'frozen/woodshed-genet';shutil.copy2(ROOT/'target/debug/woodshed-genet',exe)
scn=base/'frozen'/scenario;shutil.copy2(SOURCE/scenario,scn)
keeper_exe=base/'frozen/keeper';shutil.copy2('/Users/markik/Code/testing/woodshed/audio-packs-ap3_20261020/woodshed-final/frozen/keeper',keeper_exe)
endpoint='/tmp/desktop-midi-'+uuid.uuid4().hex[:10]+'.sock'
keeper_log=(base/'keeper.log').open('w');keeper=subprocess.Popen([str(keeper_exe),endpoint,str(base/'keeper'),'no-wallet'],stdout=keeper_log,stderr=subprocess.STDOUT)
awake=subprocess.Popen(['/usr/bin/caffeinate','-d','-i','-t','1200'])
receiver=None
try:
 for _ in range(300):
  if (base/'keeper/ready').exists():break
  if keeper.poll() is not None:raise RuntimeError('private keeper exited')
  time.sleep(.1)
 else:raise RuntimeError('keeper not ready')
 if scenario!='guitar-reconnect.scn':
  capture=base/'frozen/midi_capture';shutil.copy2(ROOT/'target/debug/examples/midi_capture',capture)
  receiver=subprocess.Popen([str(capture),'Desktop MIDI qualification',str(base/'received.jsonl')],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=(base/'receiver.log').open('w'),text=True)
  assert receiver.stdout.readline().startswith('ready epoch')
 env={'WOODSHED_WIDTH':'1180','WOODSHED_HEIGHT':'980','WOODSHED_PRESENTATION_TIMEOUT_SECS':'60',
 'APPDATA':str(base/'profile'),'XDG_DATA_HOME':str(base/'data'),'XDG_CONFIG_HOME':str(base/'config'),
 'XDG_CACHE_HOME':str(base/'cache'),'MERE_ROOT':str(base/'mere'),'GRAPHSHELL_APP_ENDPOINT':endpoint,
 'WOODSHED_STATE':str(base/'profile/practice.bin'),'WOODSHED_SETTINGS':str(base/'profile/settings.json'),'TMPDIR':'/tmp'}
 command=[sys.executable,str(SOURCE/'run_macos_scenario.py'),'--binary',str(exe),'--prefix','WOODSHED','--scenario',str(scn),'--output',str(base/'run'),'--timeout','1050' if scenario=='guitar-reconnect.scn' else '360']
 for key,value in env.items():command+=['--env',key+'='+value]
 (base/'inputs.json').write_text(json.dumps({'command':command,'environment':env,'binary_sha256':hashlib.sha256(exe.read_bytes()).hexdigest(),'keeper_sha256':hashlib.sha256(keeper_exe.read_bytes()).hexdigest(),'scope':'Private synthetic no-wallet Keeper; no normal user profile'},indent=2)+'\n')
 if scenario=='desktop-hotplug.scn':
  def hotplug():
   before=base/'run/midi-before-hotplug.json'
   for _ in range(1800):
    if before.exists():break
    time.sleep(.1)
   else:return
   receiver.stdin.write('drop\n');receiver.stdin.flush();dropped=receiver.stdout.readline().strip()
   time.sleep(.5)
   receiver.stdin.write('open\n');receiver.stdin.flush();opened=receiver.stdout.readline().strip()
   (base/'hotplug.json').write_text(json.dumps({'drop':dropped,'open':opened},indent=2)+'\n')
  threading.Thread(target=hotplug,daemon=True).start()
 code=subprocess.call(command,cwd=ROOT)
 print('Native cohort exit',code,flush=True)
 sys.exit(code)
finally:
 if receiver is not None:
  if receiver.poll() is None:receiver.stdin.write('quit\n');receiver.stdin.flush()
  receiver.wait(timeout=5)
 (base/'keeper/stop').touch()
 try:keeper.wait(timeout=5)
 except subprocess.TimeoutExpired:keeper.terminate();keeper.wait(timeout=5)
 keeper_log.close();awake.terminate();awake.wait(timeout=5)
