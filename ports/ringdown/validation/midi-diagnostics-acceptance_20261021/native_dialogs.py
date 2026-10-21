#!/usr/bin/env python3
"""Native dialog interactions, never replacements for the production chooser."""
import argparse,json,pathlib,subprocess,time,os
p=argparse.ArgumentParser();p.add_argument('session',type=pathlib.Path);p.add_argument('--phase',choices=['controls','close-cancel','close-save'],default='controls');a=p.parse_args();b=a.session.resolve();here=pathlib.Path(__file__).parent;log=[]
def wait(path,secs=100):
 end=time.monotonic()+secs
 while not path.exists():
  assert time.monotonic()<end,'timeout '+str(path);time.sleep(.05)
 return path

def checkpoint(label,release=False):
 wait(b/'run'/('midi-'+label+'.json'))
 if (b/'observations'/(label+'.json')).exists():
  if release:gate(label)
  return json.loads((b/'run'/('midi-'+label+'.json')).read_text())
 cmd=['python3',str(here/'observe.py'),str(b),label]
 if release:cmd+=['--release']
 r=subprocess.run(cmd,capture_output=True,text=True);log.append({'observation':label,'exit':r.returncode});assert r.returncode==0,r.stderr
 return json.loads((b/'run'/('midi-'+label+'.json')).read_text())
def gate(label):(b/'run'/('gate-'+label)).touch()
def apple(script,label):
 r=subprocess.run(['osascript'],input='with timeout of 10 seconds\n'+script+'\nend timeout\n',capture_output=True,text=True,timeout=15);log.append({'action':label,'script':script,'exit':r.returncode,'stdout':r.stdout,'stderr':r.stderr});(b/'native-dialog-actions.json').write_text(json.dumps(log,indent=2)+'\n');assert r.returncode==0 or (label=='native-quit-request' and '(-128)' in r.stderr),r.stderr

def process():
 pids=subprocess.check_output(['/usr/sbin/lsof','-t',str(b/'run/native.log')],text=True).split();assert len(pids)==1,pids
 pid=int(pids[0]);name=pathlib.Path(subprocess.check_output(['ps','-p',str(pid),'-o','comm='],text=True).strip()).name;assert name.startswith('product-');return pid,name

def panel(pid,label):
 helper='/Users/markik/Code/output/turnstone-gui-native-20261020/window_info'
 data=json.loads(subprocess.check_output([helper,'--activate',str(pid)],text=True))
 (b/(label+'-windows.json')).write_text(json.dumps(data,indent=2)+'\n')
 # The native save service can own the panel rather than the product PID.
 # This fixed region was read from its observed native window bounds.
 time.sleep(.3)
 subprocess.run(['/usr/sbin/screencapture','-x','-R1065,304,430,219',str(b/(label+'.png'))],check=True)

def keys(name,body,label):apple('tell application "System Events" to tell process "'+name+'"\nset frontmost to true\n'+body+'\nend tell',label)

def save(name):
 # Observed native Go to Folder and default Save controls. Keep activation,
 # directory entry and confirmation in one short native interaction.
 keys(name,'keystroke "g" using {command down, shift down}\ndelay 0.4\nkeystroke "'+str(b)+'"\ndelay 0.2\nkey code 36','native-save-navigate')
 time.sleep(2)
 keys(name,'key code 36','native-save-confirm')
 wait(b/'midi-diagnostics.json',15);time.sleep(.4)

if a.phase=='controls':
 for label in ['default','stopped','off-send']:checkpoint(label,True)
 checkpoint('retained');gate('retained');checkpoint('cancel-dialog');pid,name=process();panel(pid,'cancel-panel')
 keys(name,'key code 53','native-cancel');time.sleep(.4);gate('cancel-dialog');assert not checkpoint('cancelled')['diagnostics']['busy'];gate('cancelled')
 checkpoint('save-dialog');panel(pid,'save-panel');save(name);gate('save-dialog');assert not checkpoint('saved')['diagnostics']['busy'];gate('saved')
else:
 checkpoint('close-dialog');held=checkpoint('close-held');assert held['diagnostics']['busy'];assert held['diagnostics']['notice']=='Finish or cancel diagnostic export before closing.'
 pid,name=process();panel(pid,'close-deferred');os.kill(pid,0)
 log.append({'host_window_command_close_deferred':True,'ui_notice':held['diagnostics']['notice']})
 if a.phase=='close-save':save(name)
 else:keys(name,'key code 53','native-cancel-after-close')
 end=time.monotonic()+15
 while True:
  try:os.kill(pid,0)
  except ProcessLookupError:break
  assert time.monotonic()<end,'did not close after export completion';time.sleep(.1)
 log.append({'closed_after_export':True,'process':pid,'phase':a.phase});(b/'native-dialog-actions.json').write_text(json.dumps(log,indent=2)+'\n')
print(json.dumps({'success':True,'phase':a.phase,'actions':len(log)}))
