#!/usr/bin/env python3
"""Isolated native controls; external driver observes/answers actual OS dialogs."""
import argparse,hashlib,json,os,select,shutil,subprocess,sys,time,uuid
from pathlib import Path
ROOT=Path(__file__).resolve().parents[4]
p=argparse.ArgumentParser();p.add_argument('--output',type=Path,required=True);p.add_argument('--phase',choices=['controls','reopen','close'],required=True);p.add_argument('--profile',type=Path);a=p.parse_args()
b=a.output.resolve();b.mkdir(exist_ok=False);(b/'frozen').mkdir()
profile=a.profile.resolve() if a.profile else b/'profile';profile.mkdir(exist_ok=True)
if not a.profile:shutil.copy2('/Users/markik/Code/testing/woodshed/pedal-input-acceptance_20261020/native-r4/guitar-preparation.json',profile/'settings.json')
for n,src in [('woodshed-genet',ROOT/'target/debug/woodshed-genet'),('midi_capture',ROOT/'target/debug/examples/midi_capture'),('keeper',Path('/Users/markik/Code/testing/woodshed/pedal-input-acceptance_20261020/native-r4/frozen/keeper')),('run.py',Path(__file__))]:shutil.copy2(src,b/'frozen'/n)
suffix=uuid.uuid4().hex[:8];receiver_name='Diagnostics receiver '+suffix;endpoint='/tmp/diagnostics-'+suffix+'.sock'
(b/'frozen/executed.scn').write_text(Path(__file__).with_name(a.phase+'.scn').read_text().replace('OUTPUT_RECEIVER',receiver_name))
os.mkfifo(b/'commands.fifo');fd=os.open(b/'commands.fifo',os.O_RDWR|os.O_NONBLOCK)
keeper=receiver=None
try:
 os.environ.pop('INSTRUMENT_MIDI_DIAGNOSTICS',None)
 assert subprocess.run(['/bin/launchctl','getenv','INSTRUMENT_MIDI_DIAGNOSTICS'],capture_output=True,text=True).stdout.strip()!='1','launchd has an explicit recording opt-in'
 keeper=subprocess.Popen([str(b/'frozen/keeper'),endpoint,str(b/'keeper'),'no-wallet'],stdout=(b/'keeper.log').open('w'),stderr=subprocess.STDOUT)
 deadline=time.monotonic()+30
 while not (b/'keeper/ready').exists():
  assert keeper.poll() is None and time.monotonic()<deadline;time.sleep(.05)
 receiver=subprocess.Popen([str(b/'frozen/midi_capture'),receiver_name,str(b/'received.jsonl')],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=(b/'receiver.log').open('w'),text=True)
 assert select.select([receiver.stdout],[],[],5)[0];assert receiver.stdout.readline().startswith('ready')
 env={'WOODSHED_WIDTH':'1400','WOODSHED_HEIGHT':'1150','WOODSHED_PRESENTATION_TIMEOUT_SECS':'60','APPDATA':str(profile),'XDG_DATA_HOME':str(b/'data'),'XDG_CONFIG_HOME':str(b/'config'),'XDG_CACHE_HOME':str(b/'cache'),'MERE_ROOT':str(b/'mere'),'GRAPHSHELL_APP_ENDPOINT':endpoint,'WOODSHED_STATE':str(profile/'practice.bin'),'WOODSHED_SETTINGS':str(profile/'settings.json'),'TMPDIR':'/tmp'}
 cmd=[sys.executable,str(ROOT/'scripts/run-macos-scenario.py'),'--binary',str(b/'frozen/woodshed-genet'),'--prefix','WOODSHED','--scenario',str(b/'frozen/executed.scn'),'--output',str(b/'run'),'--timeout','900','--stdin',str(b/'commands.fifo'),'--stdout',str(b/'stdio.jsonl'),'--bundle-identifier','made.merely.woodshed.diagnostics.acceptance']
 for k,v in env.items():cmd+=['--env',k+'='+v]
 cmd+=['--','--midi-pedal-stdio'];(b/'inputs.json').write_text(json.dumps({'command':cmd,'environment':env},indent=2))
 sys.exit(subprocess.call(cmd,cwd=ROOT))
finally:
 if receiver and receiver.poll() is None:receiver.stdin.write('quit\n');receiver.stdin.flush();receiver.wait(timeout=5)
 os.close(fd)
 if keeper:(b/'keeper/stop').touch();keeper.wait(timeout=5)
