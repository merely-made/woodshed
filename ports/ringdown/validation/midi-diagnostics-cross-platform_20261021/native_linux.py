#!/usr/bin/env python3
"""Owned X11 window inventory, native GTK chooser and WM close observations."""
import argparse,ctypes as C,ctypes.util,json,time,os
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--pid',type=int,required=True);p.add_argument('--action',choices=['inspect','save','cancel','close'],required=True);p.add_argument('--destination');p.add_argument('--capture');a=p.parse_args()
x=C.CDLL(ctypes.util.find_library('X11'))
x.XOpenDisplay.argtypes=[C.c_char_p];x.XOpenDisplay.restype=C.c_void_p;d=x.XOpenDisplay(None);assert d,'X11 display unavailable'
x.XDefaultRootWindow.argtypes=[C.c_void_p];x.XDefaultRootWindow.restype=C.c_ulong
x.XInternAtom.argtypes=[C.c_void_p,C.c_char_p,C.c_int];x.XInternAtom.restype=C.c_ulong
x.XGetWindowProperty.argtypes=[C.c_void_p,C.c_ulong,C.c_ulong,C.c_long,C.c_long,C.c_int,C.c_ulong,C.POINTER(C.c_ulong),C.POINTER(C.c_int),C.POINTER(C.c_ulong),C.POINTER(C.c_ulong),C.POINTER(C.POINTER(C.c_ubyte))]
x.XFree.argtypes=[C.c_void_p];x.XFlush.argtypes=[C.c_void_p]
def atom(s):return x.XInternAtom(d,s.encode(),False)
def prop(w,name):
 actual=C.c_ulong();fmt=C.c_int();n=C.c_ulong();left=C.c_ulong();data=C.POINTER(C.c_ubyte)()
 code=x.XGetWindowProperty(d,w,atom(name),0,4096,False,0,C.byref(actual),C.byref(fmt),C.byref(n),C.byref(left),C.byref(data));assert code==0
 try:
  if not data:return []
  return list(C.cast(data,C.POINTER(C.c_ulong))[:n.value]) if fmt.value==32 else C.string_at(data,n.value)
 finally:
  if data:x.XFree(data)
def inventory():
 owned=[]
 for w in prop(x.XDefaultRootWindow(d),'_NET_CLIENT_LIST'):
  pid=prop(w,'_NET_WM_PID')
  if pid and pid[0]==a.pid:
   title=prop(w,'_NET_WM_NAME');owned.append({'window':w,'title':title.decode(errors='replace') if isinstance(title,bytes) else ''})
 return owned
end=time.monotonic()+10
while True:
 owned=inventory();main=[w for w in owned if w['title']=='Woodshed'];panels=[w for w in owned if w['title']=='Export MIDI diagnostics to a new file']
 if (main if a.action=='close' else panels):break
 assert time.monotonic()<end,owned
 time.sleep(.1)
if a.action=='close':
 assert len(main)==1,owned
 class Data(C.Union):_fields_=[('l',C.c_long*5),('b',C.c_char*20)]
 class Client(C.Structure):_fields_=[('type',C.c_int),('serial',C.c_ulong),('send_event',C.c_int),('display',C.c_void_p),('window',C.c_ulong),('message_type',C.c_ulong),('format',C.c_int),('data',Data)]
 class Event(C.Union):_fields_=[('client',Client),('pad',C.c_long*24)]
 e=Event();e.client.type=33;e.client.send_event=1;e.client.display=d;e.client.window=main[0]['window'];e.client.message_type=atom('WM_PROTOCOLS');e.client.format=32;e.client.data.l[0]=atom('WM_DELETE_WINDOW');e.client.data.l[1]=0
 x.XSendEvent.argtypes=[C.c_void_p,C.c_ulong,C.c_int,C.c_long,C.POINTER(Event)]
 assert x.XSendEvent(d,main[0]['window'],False,0,C.byref(e));x.XFlush(d)
 print(json.dumps({'action':'native-WM_DELETE_WINDOW','owned':owned}));raise SystemExit(0)
assert len(panels)==1,owned;w=panels[0]['window']
if a.capture:
 import gi
 gi.require_version('Gdk','3.0');gi.require_version('GdkX11','3.0')
 from gi.repository import Gdk,GdkX11
 Gdk.init([])
 win=GdkX11.X11Window.foreign_new_for_display(Gdk.Display.get_default(),w)
 pix=Gdk.pixbuf_get_from_window(win,0,0,win.get_width(),win.get_height());assert pix and pix.get_width()>200 and pix.get_height()>100;pix.savev(a.capture,'png',[],[])
# Invoke the owned GTK dialog's actual accessible controls. This avoids
# compositor keyboard focus and Escape binding ambiguity on Xwayland.
import gi
gi.require_version('Atspi','2.0')
from gi.repository import Atspi
accessible=[]
def descend(node,depth=0):
 if depth>20 or len(accessible)>800:return
 accessible.append(node)
 for i in range(node.get_child_count()):
  child=node.get_child_at_index(i)
  if child:descend(child,depth+1)
desktop=Atspi.get_desktop(0)
apps=[desktop.get_child_at_index(i) for i in range(desktop.get_child_count())]
for application in apps:
 if application.get_process_id()==a.pid:descend(application)
dialogs=[node for node in accessible if node.get_name()=='Export MIDI diagnostics to a new file']
assert len(dialogs)==1,[(node.get_name(),node.get_role_name()) for node in accessible if node.get_role_name() in ['frame','dialog','file chooser']]
root=dialogs[0];accessible=[];descend(root)
controls=[];buttons={};entry=None
for node in accessible:
 name=node.get_name();role=node.get_role_name()
 if name in ['Cancel','Save'] and node.is_action():
  buttons[name]=node
  controls.append({'name':name,'role':role,'actions':[node.get_action_name(i) for i in range(node.get_n_actions())]})
 if node.is_editable_text():
  value=Atspi.Text.get_text(node,0,-1)
  controls.append({'name':name,'role':role,'text':value})
  if value=='midi-diagnostics.json':entry=node
print(json.dumps({'action':a.action,'owned':owned,'controls':controls,'destination':a.destination}),flush=True)
if a.action in ['save','cancel']:
 label='Save' if a.action=='save' else 'Cancel'
 assert label in buttons,{'controls':controls,'nodes':[(n.get_name(),n.get_role_name()) for n in accessible]}
 if a.action=='save':
  assert a.destination and not Path(a.destination).exists()
  assert entry,'Native filename entry missing'
  assert entry.set_text_contents(a.destination),'Native filename edit refused'
 button=buttons[label]
 assert button.get_n_actions()>0
 assert button.do_action(0),'Native dialog action refused'
