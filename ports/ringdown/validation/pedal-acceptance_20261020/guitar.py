#!/usr/bin/env python3
"""Live owner-gated trial with the actual desktop owner and saved preparations."""
from pathlib import Path
import sys,os,shutil,uuid,time,subprocess,json,hashlib
ROOT=Path('/Users/markik/Code/repos/woodshed');SOURCE=ROOT/'ports/ringdown/validation/pedal-acceptance_20261020'
SCRATCH=Path('/Users/markik/Code/testing/woodshed/pedal-acceptance_20261020')
BASE=SCRATCH/sys.argv[1];BASE.mkdir(exist_ok=False);(BASE/'frozen').mkdir()
# Copy only this qualification's synthetic custody; never the normal user profile.
(BASE/'profile').mkdir()
shutil.copy2(SCRATCH/'native-r2/profile/settings.json',BASE/'profile/settings.json')
for name,source in [('woodshed-genet',ROOT/'target/debug/woodshed-genet'),('keeper',SCRATCH/'native-r2/frozen/keeper'),('guitar.scn',SOURCE/'guitar.scn')]:shutil.copy2(source,BASE/'frozen'/name)
endpoint='/tmp/pedal-guitar-'+uuid.uuid4().hex[:10]+'.sock'
keeperlog=(BASE/'keeper.log').open('w');keeper=subprocess.Popen([str(BASE/'frozen/keeper'),endpoint,str(BASE/'keeper'),'no-wallet'],stdout=keeperlog,stderr=subprocess.STDOUT)
awake=subprocess.Popen(['/usr/bin/caffeinate','-d','-i','-t','1200']);fifo=BASE/'commands.fifo';os.mkfifo(fifo);fd=os.open(fifo,os.O_RDWR|os.O_NONBLOCK)
try:
 for _ in range(300):
  if (BASE/'keeper/ready').exists():break
  if keeper.poll() is not None:raise RuntimeError('private keeper exited')
  time.sleep(.1)
 else:raise RuntimeError('private keeper not ready')
 env={'WOODSHED_WIDTH':'1180','WOODSHED_HEIGHT':'1050','WOODSHED_PRESENTATION_TIMEOUT_SECS':'60',
 'APPDATA':str(BASE/'profile'),'XDG_DATA_HOME':str(BASE/'data'),'XDG_CONFIG_HOME':str(BASE/'config'),'XDG_CACHE_HOME':str(BASE/'cache'),
 'MERE_ROOT':str(BASE/'mere'),'GRAPHSHELL_APP_ENDPOINT':endpoint,'WOODSHED_STATE':str(BASE/'profile/practice.bin'),'WOODSHED_SETTINGS':str(BASE/'profile/settings.json'),'TMPDIR':'/tmp'}
 command=[sys.executable,str(ROOT/'scripts/run-macos-scenario.py'),'--binary',str(BASE/'frozen/woodshed-genet'),'--prefix','WOODSHED','--scenario',str(BASE/'frozen/guitar.scn'),'--output',str(BASE/'run'),'--timeout','1100','--stdin',str(fifo),'--stdout',str(BASE/'stdio.jsonl')]
 for k,v in env.items():command+=['--env',k+'='+v]
 command+=['--','--midi-pedal-stdio']
 (BASE/'inputs.json').write_text(json.dumps({'command':command,'environment':env,'binary_sha256':hashlib.sha256((BASE/'frozen/woodshed-genet').read_bytes()).hexdigest()},indent=2)+'\n')
 sys.exit(subprocess.call(command,cwd=ROOT))
finally:
 os.close(fd);(BASE/'keeper/stop').touch()
 try:keeper.wait(timeout=5)
 except subprocess.TimeoutExpired:keeper.terminate();keeper.wait(timeout=5)
 awake.terminate();awake.wait(timeout=5);keeperlog.close()
