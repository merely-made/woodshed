#!/usr/bin/env python3
"""Actual app controls and JSONL writer, isolated custody and receiving-only MIDI."""
from pathlib import Path
import sys,os,time,json,uuid,shutil,subprocess,threading,hashlib
ROOT=Path('/Users/markik/Code/repos/woodshed')
SOURCE=ROOT/'ports/ringdown/validation/pedal-acceptance_20261020'
BASE=Path('/Users/markik/Code/testing/woodshed/pedal-acceptance_20261020')/sys.argv[1]
BASE.mkdir(exist_ok=False);(BASE/'frozen').mkdir()
for name,source in [('woodshed-genet',ROOT/'target/debug/woodshed-genet'),('midi_capture',ROOT/'target/debug/examples/midi_capture'),('keeper',Path('/Users/markik/Code/testing/woodshed/audio-packs-ap3_20261020/woodshed-final/frozen/keeper'))]:shutil.copy2(source,BASE/'frozen'/name)
endpoint='/tmp/pedal-'+uuid.uuid4().hex[:12]+'.sock'
keeper_log=(BASE/'keeper.log').open('w');keeper=subprocess.Popen([str(BASE/'frozen/keeper'),endpoint,str(BASE/'keeper'),'no-wallet'],stdout=keeper_log,stderr=subprocess.STDOUT)
awake=subprocess.Popen(['/usr/bin/caffeinate','-d','-i','-t','900'])
receiver=None; errors=[]; transcript=[]
def wait_file(path,seconds=180):
 end=time.monotonic()+seconds
 while time.monotonic()<end:
  if path.exists():return json.loads(path.read_text())
  time.sleep(.02)
 raise RuntimeError('not observed: '+str(path))
def packets():
 if not (BASE/'received.jsonl').exists():return []
 return [json.loads(l) for l in (BASE/'received.jsonl').read_text().splitlines() if json.loads(l).get('event')=='packet']
def phase(name,scripted=False):
 output=BASE/name;output.mkdir();fifo=output/'commands.fifo';os.mkfifo(fifo)
 fifo_fd=os.open(fifo,os.O_RDWR|os.O_NONBLOCK); responses=output/'stdio.jsonl';responses.touch()
 scn=BASE/'frozen'/(name+'.scn');shutil.copy2(SOURCE/('reopen-script.scn' if scripted else 'configure.scn'),scn)
 env={'WOODSHED_WIDTH':'1180','WOODSHED_HEIGHT':'1050','WOODSHED_PRESENTATION_TIMEOUT_SECS':'60',
 'APPDATA':str(BASE/'profile'),'XDG_DATA_HOME':str(BASE/'data'),'XDG_CONFIG_HOME':str(BASE/'config'),
 'XDG_CACHE_HOME':str(BASE/'cache'),'MERE_ROOT':str(BASE/'mere'),'GRAPHSHELL_APP_ENDPOINT':endpoint,
 'WOODSHED_STATE':str(BASE/'profile/practice.bin'),'WOODSHED_SETTINGS':str(BASE/'profile/settings.json'),'TMPDIR':'/tmp'}
 command=[sys.executable,str(ROOT/'scripts/run-macos-scenario.py'),'--binary',str(BASE/'frozen/woodshed-genet'),'--prefix','WOODSHED','--scenario',str(scn),'--output',str(output/'run'),'--timeout','420','--stdin',str(fifo),'--stdout',str(responses)]
 for k,v in env.items():command+=['--env',k+'='+v]
 command+=['--','--midi-pedal-stdio']
 (output/'inputs.json').write_text(json.dumps({'command':command,'environment':env,'binary_sha256':hashlib.sha256((BASE/'frozen/woodshed-genet').read_bytes()).hexdigest()},indent=2)+'\n')
 def controller():
  try:
   ready=wait_file(output/'run/midi-script-ready.json');before=len(packets())
   def request(value):
    start=time.monotonic();os.write(fifo_fd,(json.dumps(value)+'\n').encode())
    end=time.monotonic()+5
    while time.monotonic()<end:
     for line in responses.read_text().splitlines():
      try:result=json.loads(line)
      except json.JSONDecodeError:continue
      if result.get('id')==value.get('id'):
       transcript.append({'request':value,'response':result,'seconds':time.monotonic()-start});return result
     time.sleep(.005)
    raise RuntimeError('no stdio response: '+str(value))
   def trigger(id,action,status):return request({'id':id,'command':'trigger','action':action,'output_revision':status['output']['revision'],'generation':status['output']['generation']})
   status=request({'id':'s1','command':'status'});assert status['output']['ready']
   for id,action in [('s-next','effect-next'),('s-prev','effect-previous')]:
    result=trigger(id,action,status);assert result['pedal']['outcome']=='accepted' and result['pedal']['accepted_messages']==2,result
   # Same id is refused, with no new action. The earlier response retains that id too.
   duplicate={'id':'s-next','command':'trigger','action':'effect-next','output_revision':status['output']['revision'],'generation':status['output']['generation']}
   old_lines=len(responses.read_text().splitlines());os.write(fifo_fd,(json.dumps(duplicate)+'\n').encode())
   end=time.monotonic()+5
   while len(responses.read_text().splitlines())<=old_lines and time.monotonic()<end:time.sleep(.005)
   result=json.loads(responses.read_text().splitlines()[-1]);assert 'Duplicate' in result.get('error',''),result
   transcript.append({'request':duplicate,'response':result})
   # Drop the actual receiving endpoint as soon as its next press is recorded.
   press_index=len(packets());drop_result=[]
   def drop_on_press():
    end=time.monotonic()+5
    while time.monotonic()<end:
     if len(packets())>press_index:
      receiver.stdin.write('drop\n');receiver.stdin.flush();drop_result.append(receiver.stdout.readline().strip());return
     time.sleep(.001)
   thread=threading.Thread(target=drop_on_press);thread.start()
   partial=trigger('s-partial','effect-next',status);thread.join(timeout=6)
   assert drop_result and partial['pedal']['outcome']=='uncertain' and partial['pedal']['accepted_messages']==1,partial
   observed=wait_file(output/'run/midi-after-loss.json')
   assert observed['pedal']['outcome']=='uncertain' and observed['pedal']['accepted_messages']==1,observed
   end=time.monotonic()+10
   while not (output/'run/02_partial_release_visible.png').exists() and time.monotonic()<end:time.sleep(.01)
   assert (output/'run/02_partial_release_visible.png').exists()
   offline=trigger('s-offline','effect-next',status);assert offline['pedal']['outcome']=='refused' and offline['pedal']['accepted_messages']==0,offline
   # Recreate only the receiving fixture. The app must explicitly reopen its output.
   receiver.stdin.write('open\n');receiver.stdin.flush();assert receiver.stdout.readline().startswith('ready epoch 2')
   fresh=wait_file(output/'run/midi-fresh-ready.json')
   assert len(packets())==before+5,(before,packets())
   assert fresh['output']['pedal_failure']['accepted_messages']==1,fresh
   assert '1 message(s) accepted' in fresh['ui_notice'],fresh
   stale=trigger('s-stale','effect-next',status);assert stale['pedal']['outcome']=='refused',stale
   fresh_status=request({'id':'s2','command':'status'});assert fresh_status['output']['ready']
   for id,action in [('s-fresh-next','effect-next'),('s-fresh-prev','effect-previous')]:
    result=trigger(id,action,fresh_status);assert result['pedal']['outcome']=='accepted' and result['pedal']['accepted_messages']==2,result
   (output/'script-verification.json').write_text(json.dumps({'success':True,'packets_before':before,'packets_after':len(packets()),'partial':partial,'drop':drop_result,'transcript':transcript},indent=2)+'\n')
  except BaseException as e:
   errors.append(str(e));(output/'controller-error.txt').write_text(repr(e)+'\n')
 thread=threading.Thread(target=controller,daemon=True) if scripted else None
 if thread:thread.start()
 try:
  code=subprocess.call(command,cwd=ROOT)
  if thread:thread.join(timeout=10)
  assert code==0,('native scenario failed',name,code)
  assert not errors,errors
 finally:os.close(fifo_fd)
 return output
try:
 for _ in range(300):
  if (BASE/'keeper/ready').exists():break
  if keeper.poll() is not None:raise RuntimeError('private keeper exited')
  time.sleep(.1)
 else:raise RuntimeError('keeper not ready')
 receiver=subprocess.Popen([str(BASE/'frozen/midi_capture'),'Pedal acceptance receiver',str(BASE/'received.jsonl')],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=(BASE/'receiver.log').open('w'),text=True)
 assert receiver.stdout.readline().startswith('ready epoch 1')
 one=phase('configure')
 configured=wait_file(one/'run/midi-configured.json');closed=wait_file(one/'run/midi-closed.json')
 expected={'effect_next':{'enabled':True,'channel':1,'kind':'control-change','number':67,'value':127,'release':0},'effect_previous':{'enabled':True,'channel':1,'kind':'control-change','number':66,'value':127,'release':0}}
 assert configured['bindings']==expected,configured
 assert configured['saved_settings']['midi_pedal']==expected,configured
 assert len(packets())==4,packets()
 two=phase('reopen',True)
 reopened=wait_file(two/'run/midi-reopened.json');final=wait_file(two/'run/midi-final.json')
 assert reopened['bindings']==expected and reopened['saved_settings']['midi_pedal']==expected,reopened
 assert reopened['output']['attempted']==0 and reopened['pedal'] is None,reopened
 expected_bytes=[[176,67,127],[176,67,0],[176,66,127],[176,66,0]]*2+[[176,67,127]]+[[176,67,127],[176,67,0],[176,66,127],[176,66,0]]
 assert [p['bytes'] for p in packets()]==expected_bytes,packets()
 assert not final['output']['ready'] and final['output']['accepted']==9,final
 assert final['output']['pedal_failure']['accepted_messages']==1,final
 assert '1 message(s) accepted' in final['ui_notice'],final
 (BASE/'verification.json').write_text(json.dumps({'success':True,'received_packets':len(packets()),'expected_bytes':expected_bytes,'saved_bindings':expected,'final':final},indent=2)+'\n')
 print('Actual controls, save/reopen, stdio, disconnect and partial release verified:',BASE,flush=True)
finally:
 if receiver and receiver.poll() is None:
  receiver.stdin.write('quit\n');receiver.stdin.flush();receiver.wait(timeout=5)
 (BASE/'keeper/stop').touch()
 try:keeper.wait(timeout=5)
 except subprocess.TimeoutExpired:keeper.terminate();keeper.wait(timeout=5)
 keeper_log.close();awake.terminate();awake.wait(timeout=5)
