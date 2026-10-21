#!/usr/bin/env python3
"""Native retained controls, production key hook, real CoreMIDI input/output.

Fixtures own only explicitly named virtual endpoints. Synthetic keeper/profile
custody is isolated. Key injection is not an OS keyboard or native focus trial.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import select
import shutil
import subprocess
import sys
import threading
import time
import uuid

ROOT = Path(__file__).resolve().parents[4]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--keeper', type=Path, required=True)
args = parser.parse_args()
BASE = args.output.resolve()
BASE.mkdir(parents=True, exist_ok=False)
FROZEN = BASE / 'frozen'
FROZEN.mkdir()
for name, source in [('woodshed-genet', ROOT/'target/debug/woodshed-genet'),
                     ('midi_capture', ROOT/'target/debug/examples/midi_capture'),
                     ('midi_controller', ROOT/'target/debug/examples/midi_controller'),
                     ('keeper', args.keeper)]:
    shutil.copy2(source, FROZEN/name)
SUFFIX = uuid.uuid4().hex[:8]
SOURCE = 'Pedal input source ' + SUFFIX
RECEIVER = 'Pedal input receiver ' + SUFFIX
endpoint = '/tmp/pedal-input-' + SUFFIX + '.sock'
errors, transcript = [], []
expected = []
keeper = source = receiver = awake = None


def wait(path, seconds=120):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        if errors:
            raise RuntimeError(errors)
        if path.exists():
            return json.loads(path.read_text())
        time.sleep(.01)
    raise RuntimeError('checkpoint not observed: ' + str(path))


def packets():
    p = BASE/'received.jsonl'
    return [j for line in (p.read_text().splitlines() if p.exists() else [])
            if (j := json.loads(line)).get('event') == 'packet']


def command(proc, line):
    proc.stdin.write(line + '\n')
    proc.stdin.flush()
    if not select.select([proc.stdout], [], [], 5)[0]:
        raise RuntimeError('fixture did not answer: ' + line)
    result = proc.stdout.readline().strip()
    transcript.append({'command': line, 'response': result, 'at': time.monotonic()})
    return result


def send(*values):
    assert command(source, 'send ' + ' '.join(map(str, values))) == 'sent'


def stage(run, name, operation):
    wait(run/f'midi-{name}.json')
    operation()
    print('controller stage:', name, flush=True)


def cc_burst(release=True):
    if release:
        send(0xB1, 20, 0)
    send(0xB1, 20, 127)
    for _ in range(32):
        send(0xB1, 20, 100)
        time.sleep(.005)


def note_burst():
    send(0x82, 36, 64)
    send(0x92, 36, 110)
    for _ in range(32):
        send(0x92, 36, 100)
        time.sleep(.005)


def gesture(action):
    number = 67 if action == 'next' else 66
    expected.extend([[176, number, 127], [176, number, 0]])


def select_port(kind, name):
    return [f'click role:combobox @aria-label=MIDI {kind}', f'click role:option {name}']


def dwell(ms=400):
    return [f'act midi-dwell {ms}', 'wait 100000']


def point(name, capture=None):
    return [f'act midi-checkpoint {name}'] + ([f'capture {capture}'] if capture else [])


def accepted(n):
    return [f'assert snap midi-accepted == {n}']


def arm():
    return ['click .t-btn Arm pedal inputs', 'assert snap pedal-input-armed == true']


def key(value, extra=''):
    return [f'act pedal-input-key {value}' + (' '+extra if extra else '')]


def build_configure():
    lines = ['settle 8', 'click .pill Settings', 'click .side-item Audio and MIDI', 'click .t-btn Refresh ports']
    for action, number in [('Previous', 66), ('Next', 67)]:
        lines += [f'click .neck-step @aria-label=Effect {action} Number increase'] * number
        lines += [f'click .t-btn @aria-label=Effect {action} binding enabled', f'click .t-btn @aria-label=Effect {action} release']
    lines += ['click .t-btn @aria-label=Effect Next learn keyboard'] + key('n')
    lines += ['click .t-btn @aria-label=Effect Previous learn keyboard'] + key('n')
    lines += ['assert snap pedal-input-learn_key == effect-previous'] + key('p')
    lines += ['assert snap pedal-input-armed == false']
    lines += select_port('input', SOURCE) + dwell()
    lines += ['click .t-btn @aria-label=Effect Next learn MIDI'] + point('learn-cc') + dwell(700)
    lines += ['assert snap pedal-input-learn_controller == null']
    lines += ['click .t-btn @aria-label=Effect Previous learn MIDI'] + point('learn-note') + dwell(1000)
    lines += ['assert snap pedal-input-learn_controller == null', 'assert snap midi-attempted == 0']
    lines += point('configured', '01_learned_no_sends')
    lines += select_port('output', RECEIVER) + dwell() + ['assert snap midi-ready == true'] + arm()
    lines += key('n') + dwell() + accepted(2)
    lines += key('n', 'repeat') * 64 + key('n', 'shift') + key('n', 'ctrl') + key('n', 'meta')
    lines += dwell() + accepted(2)
    lines += key('p') + dwell() + accepted(4) + point('keys', '02_shortcuts_hold_once')
    for name, count in [('cc-hold', 6), ('note-hold', 8), ('note-zero', 10), ('note-off', 12), ('wrong-input', 12)]:
        lines += point(name) + dwell(900) + accepted(count)
    lines += point('controllers', '03_controller_holds_once')
    lines += ['click .t-btn Disarm pedal inputs'] + key('n') + point('unarmed-held') + dwell(800) + accepted(12)
    lines += arm() + point('rearmed-held') + dwell(800) + accepted(12)
    lines += point('released-fresh') + dwell(800) + accepted(14)
    lines += point('input-drop') + dwell(800) + ['assert snap pedal-input-armed == false', 'assert snap pedal-input-connected == null']
    lines += key('n') + accepted(14) + point('input-lost', '04_input_disconnect')
    lines += point('input-recreate') + dwell(500) + ['click .t-btn Refresh ports']
    lines += select_port('input', 'None') + dwell(200) + select_port('input', SOURCE) + dwell()
    lines += ['assert snap pedal-input-armed == false'] + arm() + point('input-held') + dwell(800) + accepted(14)
    lines += point('input-fresh') + dwell(800) + accepted(16)
    lines += point('partial-ready') + key('n') + dwell(1100) + accepted(17)
    lines += ['assert snap pedal-outcome == uncertain', 'assert snap pedal-input-armed == false']
    lines += point('partial', '05_partial_release_retained')
    lines += key('n') + key('n', 'repeat') * 8 + point('offline-input') + dwell(500) + accepted(17)
    lines += point('output-recreate') + dwell(500)
    lines += select_port('output', 'None') + dwell(200) + ['click .t-btn Refresh ports'] + select_port('output', RECEIVER) + dwell()
    lines += ['assert snap midi-ready == true', 'assert snap pedal-input-armed == false'] + accepted(17)
    lines += point('reconnected', '06_output_reconnect_no_replay') + arm()
    lines += key('n') + dwell() + key('p') + dwell() + accepted(21)
    lines += point('output-fresh-midi') + dwell(900) + accepted(25)
    lines += point('final-actions', '07_fresh_actions')
    lines += select_port('input', 'None') + select_port('output', 'None') + dwell()
    lines += point('closed', '08_closed_saved')
    return lines


def build_reopen():
    lines = ['settle 8', 'click .pill Settings', 'click .side-item Audio and MIDI',
             'assert snap midi-attempted == 0', 'assert snap pedal-input-armed == false',
             'assert snap pedal-input-connected == null', 'assert snap pedal-input-learn_key == null',
             'assert snap pedal-input-learn_controller == null'] + point('reopened', '01_reopened_preparation')
    lines += ['click .t-btn Refresh ports'] + select_port('input', SOURCE) + select_port('output', RECEIVER) + dwell()
    lines += ['assert snap midi-ready == true', 'assert snap pedal-input-armed == false'] + key('n') + dwell() + accepted(0)
    lines += arm() + key('n') + dwell() + key('p') + dwell() + accepted(4)
    lines += point('reopen-held') + dwell(700) + accepted(4)
    lines += point('reopen-fresh') + dwell(900) + accepted(8) + point('restored-actions', '02_restored_mappings_work')
    lines += point('busy-burst') + dwell(900) + accepted(10) + point('busy-refusal', '03_busy_refusal_visible')
    lines += ['click .t-btn @aria-label=Effect Previous clear keyboard', 'click .t-btn @aria-label=Effect Previous clear MIDI',
              'assert snap pedal-input-armed == false'] + key('p')
    lines += ['click .t-btn @aria-label=Effect Previous learn keyboard'] + key('q') + point('cleared-input') + dwell(700) + accepted(10)
    lines += point('cleared', '04_clear_reassign_saved')
    lines += select_port('input', 'None') + select_port('output', 'None') + dwell() + point('closed')
    return lines


def configure_controller(run):
    def learn_cc():
        send(0xB1, 20, 127); time.sleep(.08); send(0xB1, 20, 0)
    def learn_note():
        send(0xB1, 20, 127)  # duplicate mapping is rejected; learning remains active
        time.sleep(.15)
        send(0x92, 36, 110); time.sleep(.08); send(0x82, 36, 64)
    stage(run, 'learn-cc', learn_cc)
    stage(run, 'learn-note', learn_note)
    stage(run, 'cc-hold', cc_burst)
    stage(run, 'note-hold', note_burst)
    stage(run, 'note-zero', lambda: (send(0x92, 36, 0), send(0x92, 36, 110)))
    stage(run, 'note-off', lambda: (send(0x82, 36, 64), send(0x92, 36, 110)))
    stage(run, 'wrong-input', lambda: (send(0xB0, 20, 127), send(0xB1, 21, 127)))
    stage(run, 'unarmed-held', cc_burst)
    stage(run, 'rearmed-held', lambda: cc_burst(False))
    stage(run, 'released-fresh', cc_burst)
    stage(run, 'input-drop', lambda: command(source, 'drop'))
    stage(run, 'input-recreate', lambda: command(source, 'open'))
    stage(run, 'input-held', lambda: cc_burst(False))
    stage(run, 'input-fresh', cc_burst)
    wait(run/'midi-partial-ready.json')
    before = 16  # exact receiving count before the next keyboard press
    end = time.monotonic()+5
    while len(packets()) == before and time.monotonic() < end:
        time.sleep(.001)
    assert len(packets()) == before+1, packets()
    assert command(receiver, 'drop').startswith('dropped')
    stage(run, 'offline-input', cc_burst)
    stage(run, 'output-recreate', lambda: command(receiver, 'open'))
    def fresh():
        cc_burst(); time.sleep(.18); note_burst()
    stage(run, 'output-fresh-midi', fresh)


def reopen_controller(run):
    stage(run, 'reopen-held', lambda: cc_burst(False))
    def fresh():
        cc_burst(); time.sleep(.18); note_burst()
    stage(run, 'reopen-fresh', fresh)
    stage(run, 'busy-burst', lambda: command(source, 'sequence 177 20 0 177 20 127 177 20 0 177 20 127'))
    stage(run, 'cleared-input', lambda: (send(0x82, 36, 64), send(0x92, 36, 110)))


def phase(name, lines, controller=None):
    phase_dir = BASE/name
    phase_dir.mkdir()
    scenario = FROZEN/(name+'.scn')
    scenario.write_text('\n'.join(lines)+'\n')
    run = phase_dir/'run'
    env = {'WOODSHED_WIDTH': '1180', 'WOODSHED_HEIGHT': '1050', 'WOODSHED_PRESENTATION_TIMEOUT_SECS': '60',
           'APPDATA': str(BASE/'profile'), 'XDG_DATA_HOME': str(BASE/'data'),
           'XDG_CONFIG_HOME': str(BASE/'config'), 'XDG_CACHE_HOME': str(BASE/'cache'),
           'MERE_ROOT': str(BASE/'mere'), 'GRAPHSHELL_APP_ENDPOINT': endpoint,
           'WOODSHED_STATE': str(BASE/'profile/practice.bin'), 'WOODSHED_SETTINGS': str(BASE/'profile/settings.json'), 'TMPDIR': '/tmp'}
    cmd = [sys.executable, str(ROOT/'scripts/run-macos-scenario.py'), '--binary', str(FROZEN/'woodshed-genet'),
           '--prefix', 'WOODSHED', '--scenario', str(scenario), '--output', str(run), '--timeout', '360']
    for k,v in env.items():
        cmd += ['--env', k+'='+v]
    (phase_dir/'inputs.json').write_text(json.dumps({'command': cmd, 'environment': env,
        'binary_sha256': hashlib.sha256((FROZEN/'woodshed-genet').read_bytes()).hexdigest()}, indent=2)+'\n')
    def drive():
        try:
            controller(run)
        except BaseException as e:
            errors.append(repr(e)); (phase_dir/'controller-error.txt').write_text(repr(e)+'\n')
    thread = threading.Thread(target=drive, daemon=True) if controller else None
    if thread:
        thread.start()
    code = subprocess.call(cmd, cwd=ROOT)
    if thread:
        thread.join(timeout=3)
    assert code == 0, ('native phase failed', name, code)
    assert not errors, errors
    if thread:
        assert not thread.is_alive(), 'controller did not finish'
    return run


try:
    keeper_log = (BASE/'keeper.log').open('w')
    keeper = subprocess.Popen([str(FROZEN/'keeper'), endpoint, str(BASE/'keeper'), 'no-wallet'], stdout=keeper_log, stderr=subprocess.STDOUT)
    awake = subprocess.Popen(['/usr/bin/caffeinate', '-d', '-i', '-t', '900'])
    end = time.monotonic()+30
    while not (BASE/'keeper/ready').exists() and time.monotonic() < end:
        assert keeper.poll() is None, 'isolated keeper exited'
        time.sleep(.05)
    assert (BASE/'keeper/ready').exists(), 'isolated keeper did not become ready'
    receiver = subprocess.Popen([str(FROZEN/'midi_capture'), RECEIVER, str(BASE/'received.jsonl')], stdin=subprocess.PIPE,
                                stdout=subprocess.PIPE, stderr=(BASE/'receiver.log').open('w'), text=True)
    assert receiver.stdout.readline().startswith('ready epoch 1')
    source = subprocess.Popen([str(FROZEN/'midi_controller'), SOURCE, str(BASE/'sent.jsonl')], stdin=subprocess.PIPE,
                              stdout=subprocess.PIPE, stderr=(BASE/'source.log').open('w'), text=True)
    assert source.stdout.readline().startswith('ready epoch 1')
    one = phase('configure', build_configure(), configure_controller)
    for action in ['next', 'previous', 'next', 'previous', 'previous', 'previous', 'next', 'next']:
        gesture('next' if action == 'next' else 'prev')
    expected.append([176,67,127])
    for action in ['next', 'prev', 'next', 'prev']:
        gesture(action)
    assert [p['bytes'] for p in packets()] == expected, packets()
    configured = wait(one/'midi-configured.json')
    bindings = configured['bindings']
    assert configured['output']['accepted'] == 0
    assert configured['saved_settings']['midi_pedal'] == bindings
    assert bindings['inputs'] == {
        'effect_next': {'keyboard': {'key':'n','shift':False,'alt':False}, 'controller': {'channel':2,'kind':'control-change','number':20}},
        'effect_previous': {'keyboard': {'key':'p','shift':False,'alt':False}, 'controller': {'channel':3,'kind':'note','number':36}}}, bindings
    assert (closed := wait(one/'midi-closed.json'))['output']['accepted'] == 25
    assert closed['output']['pedal_failure']['accepted_messages'] == 1
    # Preserve the UI-authored preparation for a later guitar trial.
    shutil.copyfile(BASE/'profile/settings.json', BASE/'guitar-preparation.json')
    two = phase('reopen', build_reopen(), reopen_controller)
    reopened = wait(two/'midi-reopened.json')
    assert reopened['bindings'] == bindings and reopened['saved_settings']['midi_pedal'] == bindings
    assert reopened['input']['pending'] is None and reopened['input']['pending_target'] is None
    for action in ['next','prev','next','prev','next']:
        gesture(action)
    assert [p['bytes'] for p in packets()] == expected, packets()
    cleared = wait(two/'midi-cleared.json')
    assert cleared['bindings']['inputs']['effect_previous'] == {
        'keyboard': {'key':'q','shift':False,'alt':False}, 'controller': None}
    assert cleared['saved_settings']['midi_pedal'] == cleared['bindings']
    three = phase('reopen-cleared', ['settle 8','click .pill Settings','click .side-item Audio and MIDI',
        'assert snap midi-attempted == 0','assert snap pedal-input-armed == false']+point('reopened','01_reopened_cleared_mappings'))
    assert wait(three/'midi-reopened.json')['bindings'] == cleared['bindings']
    busy = wait(two/'midi-busy-refusal.json')
    assert 'still pending' in busy['input']['notice'], busy
    assert len(packets()) == 35
    (BASE/'verification.json').write_text(json.dumps({'success':True,'received_packets':len(packets()),
        'expected_bytes':expected,'bindings':bindings,'cleared_bindings':cleared['bindings'],
        'partial':wait(one/'midi-partial.json'),'transcript':transcript},indent=2)+'\n')
    print('Native mapping controls, persistence, holds, input/output replacement and partial release passed:', BASE, flush=True)
finally:
    (BASE/'fixture-transcript.json').write_text(json.dumps(transcript,indent=2)+'\n')
    for proc in [source, receiver]:
        if proc and proc.poll() is None:
            try:
                proc.stdin.write('quit\n'); proc.stdin.flush(); proc.wait(timeout=5)
            except (BrokenPipeError, subprocess.TimeoutExpired):
                proc.terminate(); proc.wait(timeout=5)
    if keeper:
        (BASE/'keeper/stop').touch()
        try:
            keeper.wait(timeout=5)
        except subprocess.TimeoutExpired:
            keeper.terminate(); keeper.wait(timeout=5)
        keeper_log.close()
    if awake:
        awake.terminate(); awake.wait(timeout=5)
