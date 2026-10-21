"""Select only an owned Mesquite app's native file panels, with a finite queue.
This helper is acceptance tooling on macOS; products remain cross-platform.
"""
import subprocess,sys,time,json
from pathlib import Path
pid=int(sys.argv[1]);queue=json.loads(Path(sys.argv[2]).read_text());log=Path(sys.argv[3])
def script(body):
 s='tell application "System Events"\nset p to first application process whose unix id is '+str(pid)+'\ntell p\n'+body+'\nend tell\nend tell'
 r=subprocess.run(['osascript','-e',s],text=True,capture_output=True,timeout=10)
 if r.returncode: raise RuntimeError(r.stderr)
 return r.stdout.strip()
def q(s):return json.dumps(str(s))
observed=[]
for choice in queue:
 deadline=time.monotonic()+120
 while time.monotonic()<deadline:
  try:
   # NSOpenPanel / NSSavePanel are AXDialog windows, owned by this exact PID.
   found=script('set found to ""\nrepeat with w in windows\ntry\nif subrole of w is "AXDialog" then set found to name of w\nend try\nend repeat\nreturn found')
   if found:break
  except Exception as e:
   log.write_text(json.dumps(observed+[{'error':str(e)}],indent=2));raise
  time.sleep(.2)
 else:raise RuntimeError('No owned file panel before deadline')
 path=Path(choice['path']);kind=choice['kind']
 detail=script('return {name of every window, name of every button of every window, value of every text field of every window}')
 observed.append({'panel':found,'before':detail,'choice':choice});log.write_text(json.dumps(observed,indent=2))
 target=path.parent if kind=='save' else path
 script('set frontmost to true\nkeystroke "g" using {command down, shift down}')
 time.sleep(.3)
 script('keystroke '+q(target)+'\nkey code 36')
 time.sleep(.5)
 if kind=='save':
  script('set value of text field 1 of window '+q(found)+' to '+q(path.name))
 # Press the named primary button in the observed, owned native panel.
 button='Save' if kind=='save' else ('Choose' if kind=='folder' else 'Open')
 try:script('click button '+q(button)+' of window '+q(found))
 except RuntimeError:
  # RFD folder pickers use Open on some macOS versions.
  if kind!='folder':raise
  script('click button "Open" of window '+q(found))
 time.sleep(.5)
 observed[-1]['selected']=True;log.write_text(json.dumps(observed,indent=2))
print(json.dumps({'completed':len(observed)}),flush=True)
