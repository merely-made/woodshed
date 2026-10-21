#!/usr/bin/env python3
"""Existing native host/fixtures with read-only stdio diagnostic snapshots.

Native mode drives the production keyboard hook and parsed controller input.
Guitar mode performs preparation only; every hardware gesture is owner-gated.
"""
import argparse, hashlib, json, os, select, shutil, subprocess, sys, threading, time, uuid
from pathlib import Path
ROOT = Path(__file__).resolve().parents[4]
p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--output',type=Path,required=True)
p.add_argument('--mode',choices=['native','guitar'],required=True)
p.add_argument('--preparation',type=Path,required=True)
p.add_argument('--keeper',type=Path,required=True)
p.add_argument('--bundle-identifier')
a=p.parse_args(); base=a.output.resolve();base.mkdir(exist_ok=False);(base/'frozen').mkdir();(base/'profile').mkdir()
shutil.copy2(a.preparation,base/'profile/settings.json')
for n,src in [('woodshed-genet',ROOT/'target/debug/woodshed-genet'),('midi_controller',ROOT/'target/debug/examples/midi_controller'),('midi_capture',ROOT/'target/debug/examples/midi_capture'),('keeper',a.keeper),('compare.py',Path(__file__))]:shutil.copy2(src,base/'frozen'/n)
suffix=uuid.uuid4().hex[:8];source_name='Previous input '+suffix;receiver_name='Previous receiver '+suffix;endpoint='/tmp/previous-'+suffix+'.sock'
scenario=(Path(__file__).with_name(a.mode+'.scn')).read_text().replace('INPUT_SOURCE',source_name).replace('OUTPUT_RECEIVER',receiver_name)
(base/'frozen/executed.scn').write_text(scenario)
for n in ['commands','controller']:os.mkfifo(base/(n+'.fifo'))
command_fd=os.open(base/'commands.fifo',os.O_RDWR|os.O_NONBLOCK);controller_fd=os.open(base/'controller.fifo',os.O_RDWR|os.O_NONBLOCK)
keeper=source=receiver=awake=None;errors=[];driver=None

def wait(path,seconds=150):
    end=time.monotonic()+seconds
    while time.monotonic()<end:
        if path.exists():return json.loads(path.read_text())
        time.sleep(.02)
    raise RuntimeError('checkpoint timeout '+str(path))

def controller(line):os.write(controller_fd,(line+'\n').encode())
def status(label):
    os.write(command_fd,(json.dumps({'command':'status','id':label})+'\n').encode())
    end=time.monotonic()+5
    while time.monotonic()<end:
        f=base/'stdio.jsonl'
        for line in (f.read_text().splitlines() if f.exists() else []):
            try:j=json.loads(line)
            except json.JSONDecodeError:continue
            if j.get('id')==label:return j
        time.sleep(.02)
    raise RuntimeError('status query timeout '+label)

def drive():
    try:
        for n in [1,2]:
            wait(base/'run'/f'midi-note-ready-{n}.json')
            controller('send 130 36 64');controller('send 146 36 110')
            for _ in range(32):controller('send 146 36 100');time.sleep(.005)
        wait(base/'run/midi-compared.json')
        j=status('native-final');(base/'status-final.json').write_text(json.dumps(j,indent=2)+'\n')
    except BaseException as e:errors.append(repr(e));(base/'driver-error.txt').write_text(repr(e)+'\n')
try:
    keeper=subprocess.Popen([str(base/'frozen/keeper'),endpoint,str(base/'keeper'),'no-wallet'],stdout=(base/'keeper.log').open('w'),stderr=subprocess.STDOUT)
    end=time.monotonic()+30
    while not (base/'keeper/ready').exists():
        assert keeper.poll() is None,'synthetic keeper exited'
        assert time.monotonic()<end,'synthetic keeper timeout'
        time.sleep(.05)
    awake=subprocess.Popen(['/usr/bin/caffeinate','-d','-i','-t','1800'])
    source=subprocess.Popen([str(base/'frozen/midi_controller'),source_name,str(base/'sent.jsonl')],stdin=open(base/'controller.fifo'),stdout=(base/'controller.log').open('w'),stderr=(base/'controller-error.log').open('w'))
    if a.mode=='native':
        receiver=subprocess.Popen([str(base/'frozen/midi_capture'),receiver_name,str(base/'received.jsonl')],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=(base/'receiver-error.log').open('w'),text=True)
        assert select.select([receiver.stdout],[],[],5)[0];assert receiver.stdout.readline().startswith('ready')
        driver=threading.Thread(target=drive,daemon=True);driver.start()
    env={'WOODSHED_WIDTH':'1180','WOODSHED_HEIGHT':'1050','WOODSHED_PRESENTATION_TIMEOUT_SECS':'60','APPDATA':str(base/'profile'),'XDG_DATA_HOME':str(base/'data'),'XDG_CONFIG_HOME':str(base/'config'),'XDG_CACHE_HOME':str(base/'cache'),'MERE_ROOT':str(base/'mere'),'GRAPHSHELL_APP_ENDPOINT':endpoint,'WOODSHED_STATE':str(base/'profile/practice.bin'),'WOODSHED_SETTINGS':str(base/'profile/settings.json'),'TMPDIR':'/tmp','CAMBIUM_HOST_KEY_TRACE':'1','INSTRUMENT_MIDI_DIAGNOSTICS':'1'}
    cmd=[sys.executable,str(ROOT/'scripts/run-macos-scenario.py'),'--binary',str(base/'frozen/woodshed-genet'),'--prefix','WOODSHED','--scenario',str(base/'frozen/executed.scn'),'--output',str(base/'run'),'--timeout',str(300 if a.mode=='native' else 1700),'--stdin',str(base/'commands.fifo'),'--stdout',str(base/'stdio.jsonl')]
    if a.bundle_identifier:cmd+=['--bundle-identifier',a.bundle_identifier]
    for k,v in env.items():cmd+=['--env',k+'='+v]
    cmd+=['--','--midi-pedal-stdio']
    (base/'inputs.json').write_text(json.dumps({'command':cmd,'environment':env,'mode':a.mode,'binary_sha256':hashlib.sha256((base/'frozen/woodshed-genet').read_bytes()).hexdigest()},indent=2)+'\n')
    files=['crates/woodshed-genet/src/midi.rs','crates/woodshed-genet/src/midi/controller.rs','crates/woodshed-genet/src/midi/pedal.rs','crates/woodshed-genet/src/midi/worker.rs','crates/woodshed-genet/src/midi/stdio.rs','crates/woodshed-genet/src/pedal_inputs.rs','crates/woodshed-instrument/src/midi/diagnostics.rs','crates/woodshed-instrument/src/midi/ble.rs','Cargo.toml','Cargo.lock']
    (base/'source.json').write_text(json.dumps({f:hashlib.sha256((ROOT/f).read_bytes()).hexdigest() for f in files},indent=2)+'\n')
    result=subprocess.call(cmd,cwd=ROOT)
    if driver:driver.join(timeout=8)
    assert not errors,errors
    if a.mode=='native' and result==0:
        j=json.loads((base/'status-final.json').read_text());events=j['diagnostics']['events'];assert j['diagnostics']['overwritten']==0
        packets=[json.loads(l)['bytes'] for l in (base/'received.jsonl').read_text().splitlines() if json.loads(l)['event']=='packet']
        assert packets==[[176,66,127],[176,66,0]]*4,packets
        sends=[e for e in events if e['stage']=='writer-send-start'];assert [e['bytes'] for e in sends]==packets
        assert [e['request_id'] for e in sends]==[1,1,2,2,3,3,4,4]
        assert len([e for e in events if e['stage']=='keyboard-input'])==2
        assert len([e for e in events if e['stage']=='controller-decision'])==2
        assert all(e['stage']!='ble-notify-start' for e in events)
        summary={'success':True,'independent_received_messages':8,'writer_matches_receiver':True,'keyboard_requests':[1,3],'note_requests':[2,4],'release_gaps_micros':[sends[i+1]['elapsed_micros']-sends[i]['elapsed_micros'] for i in range(0,8,2)],'journal_events':len(events),'overwritten':0,'guitar_used':False}
        (base/'verification.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps(summary),flush=True)
    sys.exit(result)
finally:
    if source and source.poll() is None:
        controller('quit')
        try:source.wait(timeout=5)
        except subprocess.TimeoutExpired:source.terminate();source.wait(timeout=5)
    if receiver and receiver.poll() is None:
        receiver.stdin.write('quit\n');receiver.stdin.flush()
        try:receiver.wait(timeout=5)
        except subprocess.TimeoutExpired:receiver.terminate();receiver.wait(timeout=5)
    os.close(command_fd);os.close(controller_fd)
    if keeper:
        (base/'keeper/stop').touch()
        try:keeper.wait(timeout=5)
        except subprocess.TimeoutExpired:keeper.terminate();keeper.wait(timeout=5)
    if awake:awake.terminate();awake.wait(timeout=5)
