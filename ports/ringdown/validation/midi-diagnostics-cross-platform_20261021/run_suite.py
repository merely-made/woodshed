#!/usr/bin/env python3
"""Native acceptance using the existing product scenarios, Status and chooser.

Linux uses an independent ALSA receiver. Windows captures explicit no-output
refusals through the production dispatcher; it does not qualify MIDI delivery.
"""
import argparse, hashlib, json, os, queue, shutil, subprocess, sys, threading, time, uuid
from pathlib import Path
HERE = Path(__file__).resolve().parent
p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--binary', type=Path, required=True)
p.add_argument('--keeper', type=Path, required=True)
p.add_argument('--receiver', type=Path)
p.add_argument('--settings-template', type=Path, required=True)
p.add_argument('--bindings', type=Path, required=True)
p.add_argument('--output', type=Path, required=True)
a = p.parse_args()
b = a.output.resolve(); b.mkdir(exist_ok=False)
windows = sys.platform == 'win32'
assert windows or sys.platform.startswith('linux')
profile = b / 'profile'; profile.mkdir()
shutil.copy2(a.settings_template, profile / 'settings.json')
expected_bindings = json.loads(a.bindings.read_text(encoding='utf-8'))
endpoint = (r'\\.\pipe\woodshed-diagnostics-' if windows else '/tmp/woodshed-diagnostics-') + uuid.uuid4().hex[:12]
keeper = receiver = app = None
receiver_name = 'Diagnostic receiver ' + uuid.uuid4().hex[:8]
results = {}


def wait_for(predicate, label, timeout=120):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        value = predicate()
        if value:
            return value
        if app and app.proc.poll() is not None:
            raise RuntimeError('App exited before ' + label)
        time.sleep(.05)
    raise TimeoutError(label)


def checkpoint_block(label):
    return f'act midi-checkpoint {label}\nact diagnostic-await {label}\nwait 100000\nassert snap diagnostic-gate-released == true\n'


def scene(phase):
    start = 'settle 8\nclick .pill Settings\nclick .side-item Audio and MIDI\n'
    if phase == 'reopen':
        return (HERE / 'reopen.scn').read_text(encoding='utf-8')
    if phase.startswith('close-'):
        return start + ('click .t-btn Refresh ports\nclick role:combobox @aria-label=MIDI output\nclick role:option None\n'
            'click .t-btn Record MIDI diagnostics\nclick .t-btn Stop MIDI diagnostics\n'
            'click .t-btn Export MIDI diagnostics…\n') + checkpoint_block('close-dialog') + checkpoint_block('close-held') + 'act midi-checkpoint should-not-survive-close\n'
    if not windows:
        return (HERE / 'controls-linux.scn').read_text(encoding='utf-8').replace('OUTPUT_RECEIVER', receiver_name)
    recipe = start + 'act midi-checkpoint default\ncapture 01_default_off\nact diagnostic-await default\nwait 100000\nassert snap diagnostic-gate-released == true\n'
    recipe += 'click .t-btn Record MIDI diagnostics\n' + checkpoint_block('recording-first')
    recipe += 'click .t-btn Stop MIDI diagnostics\n' + checkpoint_block('stopped')
    recipe += checkpoint_block('off-send')
    recipe += 'click .t-btn Record MIDI diagnostics\n' + checkpoint_block('recording-second')
    recipe += 'click .t-btn Stop MIDI diagnostics\nclick .t-btn Show latest events\n'
    # The remaining controls/cancel/save/exit recipe is shared with the Mac lane.
    common = (HERE / 'controls-linux.scn').read_text(encoding='utf-8')
    return recipe + common[common.index('act midi-checkpoint retained\n'):]


class App:
    def __init__(self, phase):
        self.root = b / phase; self.root.mkdir()
        self.capture = self.root / 'captures'; self.capture.mkdir()
        self.observations = self.root / 'status'; self.observations.mkdir()
        self.responses = {}; self.condition = threading.Condition(); self.serial = 0
        script = self.root / 'executed.scn'; script.write_text(scene(phase), encoding='utf-8')
        env = os.environ.copy(); env.pop('INSTRUMENT_MIDI_DIAGNOSTICS', None)
        env.update({'WOODSHED_WIDTH':'1400', 'WOODSHED_HEIGHT':'950', 'WOODSHED_PRESENTATION_TIMEOUT_SECS':'60',
            'WOODSHED_SCENARIO':str(script), 'WOODSHED_CAPTURE_DIR':str(self.capture),
            'WOODSHED_RECEIPT':str(self.capture / 'scenario.done'), 'WOODSHED_SETTINGS':str(profile / 'settings.json'),
            'WOODSHED_STATE':str(profile / 'practice.bin'), 'APPDATA':str(profile),
            'XDG_DATA_HOME':str(b / 'data'), 'XDG_CONFIG_HOME':str(b / 'config'), 'XDG_CACHE_HOME':str(b / 'cache'),
            'MERE_ROOT':str(b / 'mere'), 'GRAPHSHELL_APP_ENDPOINT':endpoint})
        if windows:
            env['LOCALAPPDATA'] = str(b / 'local')
        if not windows:
            env['GDK_BACKEND'] = 'x11'; env.pop('WAYLAND_DISPLAY', None)
        (self.root / 'inputs.json').write_text(json.dumps({'command':[str(a.binary), '--midi-pedal-stdio'],
            'environment':{k:v for k,v in env.items() if k.startswith(('WOODSHED_', 'XDG_', 'GRAPHSHELL_')) or k in ['APPDATA', 'LOCALAPPDATA', 'MERE_ROOT', 'DISPLAY', 'XAUTHORITY', 'GDK_BACKEND', 'DBUS_SESSION_BUS_ADDRESS']},
            'startup_recording_opt_in':False}, indent=2))
        self.native = (self.root / 'native.log').open('w', encoding='utf-8')
        self.stdout = (self.root / 'stdio.jsonl').open('w', encoding='utf-8')
        self.proc = subprocess.Popen([str(a.binary), '--midi-pedal-stdio'], stdin=subprocess.PIPE,
            stdout=subprocess.PIPE, stderr=self.native, env=env, cwd=self.root, text=True, encoding='utf-8')
        self.thread = threading.Thread(target=self.read, daemon=True); self.thread.start()

    def read(self):
        for line in self.proc.stdout:
            self.stdout.write(line); self.stdout.flush()
            try:
                value = json.loads(line)
            except json.JSONDecodeError:
                continue
            if 'id' in value:
                with self.condition:
                    self.responses[value['id']] = value; self.condition.notify_all()

    def command(self, value):
        self.serial += 1; label = str(value.get('id', 'request-' + str(self.serial))); value['id'] = label
        self.proc.stdin.write(json.dumps(value) + '\n'); self.proc.stdin.flush()
        end = time.monotonic() + 8
        with self.condition:
            while label not in self.responses:
                assert time.monotonic() < end, 'Status/command timeout ' + label
                self.condition.wait(.05)
            response = self.responses[label]
        assert 'error' not in response, response
        return response

    def status(self, label):
        value = self.command({'command':'status', 'id':'status-' + label})
        (self.observations / (label + '.json')).write_text(json.dumps(value, indent=2) + '\n')
        return value

    def checkpoint(self, label, release=False):
        path = self.capture / ('midi-' + label + '.json')
        wait_for(path.exists, 'checkpoint ' + label)
        value = json.loads(path.read_text(encoding='utf-8'))
        state = self.status(label)
        diagnostic = state['diagnostics']
        assert value['diagnostics']['recording'] == (diagnostic['recording'] if diagnostic else False)
        assert value['diagnostics']['retained_events'] == (len(diagnostic['events']) if diagnostic else 0)
        if release: self.gate(label)
        return value, state

    def gate(self, label):
        (self.capture / ('gate-' + label)).touch()

    def refuse(self, label):
        state = self.command({'command':'status'})
        value = self.command({'command':'trigger', 'id':'refuse-' + label, 'action':'effect-next',
            'output_revision':state['output']['revision'], 'generation':state['output']['generation']})
        assert value['pedal']['outcome'] == 'refused', value
        (self.observations / ('trigger-' + label + '.json')).write_text(json.dumps(value, indent=2))

    def ui(self, action, capture=None, destination=None):
        if windows:
            ps = str(Path(os.environ['WINDIR']) / 'System32/WindowsPowerShell/v1.0/powershell.exe')
            cmd = [ps, '-STA', '-NoProfile', '-NonInteractive', '-File', str(HERE / 'native_windows.ps1'), '-AppPid', str(self.proc.pid), '-Action', action]
            if capture: cmd += ['-Capture', str(capture)]
            if destination: cmd += ['-Destination', str(destination)]
        else:
            cmd = [sys.executable, str(HERE / 'native_linux.py'), '--pid', str(self.proc.pid), '--action', action]
            if capture: cmd += ['--capture', str(capture)]
            if destination: cmd += ['--destination', str(destination)]
        result = subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8", timeout=25)
        with (self.root / 'native-actions.jsonl').open('a', encoding='utf-8') as file:
            file.write(json.dumps({'command':cmd, 'exit':result.returncode, 'stdout':result.stdout, 'stderr':result.stderr}) + '\n')
        assert result.returncode == 0, result.stderr + result.stdout
        return result

    def finish(self, deliberate_close=False):
        code = self.proc.wait(timeout=30); self.thread.join(timeout=5)
        self.native.close(); self.stdout.close()
        receipt = self.capture / 'scenario.done'
        text = receipt.read_text(encoding='utf-8') if receipt.exists() else None
        assert code == 0, code
        if not deliberate_close: assert text and text.startswith('RESULT ok\n'), text
        else: assert not (self.capture / 'midi-should-not-survive-close.json').exists()
        (self.root / 'process.json').write_text(json.dumps({'exit':code, 'deliberate_close':deliberate_close,
            'scenario_result':text.splitlines()[0] if text else None, 'binary_sha256':hashlib.sha256(a.binary.read_bytes()).hexdigest()}, indent=2))


def assert_export(path, status):
    obj = json.loads(path.read_text(encoding='utf-8')); expected = dict(status); expected.pop('id')
    assert obj['format'] == 'woodshed-midi-diagnostics' and obj['version'] == 1
    assert obj['status'] == expected


try:
    keeper_log = (b / 'keeper.log').open('w', encoding='utf-8')
    keeper = subprocess.Popen([str(a.keeper), endpoint, str(b / 'keeper'), 'no-wallet'], stdout=keeper_log, stderr=subprocess.STDOUT)
    wait_for((b / 'keeper/ready').exists, 'synthetic keeper', 45)
    if not windows:
        assert a.receiver
        receiver = subprocess.Popen([str(a.receiver), receiver_name, str(b / 'received.jsonl')], stdin=subprocess.PIPE,
            stdout=(b / 'receiver.log').open('w'), stderr=(b / 'receiver-errors.log').open('w'), text=True)
        wait_for(lambda: (b / 'received.jsonl').exists() and '"event":"open"' in (b / 'received.jsonl').read_text(encoding='utf-8'), 'ALSA receiver')
    app = App('controls')
    _, initial = app.checkpoint('default'); assert initial['diagnostics'] is None
    assert initial['bindings'] == expected_bindings, 'Synthetic encrypted settings did not load'
    app.gate('default')
    if windows:
        app.checkpoint('recording-first'); app.refuse('first'); app.gate('recording-first')
    _, stopped = app.checkpoint('stopped')
    if windows: app.refuse('off')
    app.gate('stopped')
    _, off = app.checkpoint('off-send'); assert off['diagnostics'] == stopped['diagnostics']; app.gate('off-send')
    if windows:
        app.checkpoint('recording-second'); app.refuse('second'); app.gate('recording-second')
    shown, retained = app.checkpoint('retained'); assert not retained['diagnostics']['recording']; assert shown['diagnostics']['show_history']
    first = stopped['diagnostics']['events']; entries = retained['diagnostics']['events']; assert entries[:len(first)] == first
    assert [v['stage'] for v in entries if v['stage'].startswith('recording-')] == ['recording-start','recording-stop','recording-start','recording-stop']
    assert len(shown['diagnostics']['recent_events']) == min(12, len(entries)); assert retained['diagnostics']['overwritten'] == 0; assert shown['diagnostics']['truncated_events'] == 0
    app.gate('retained')
    pending, _ = app.checkpoint('cancel-dialog'); assert pending['diagnostics']['busy']
    app.ui('cancel', app.root / 'cancel-panel.png'); time.sleep(.4); app.gate('cancel-dialog')
    cancelled, status = app.checkpoint('cancelled'); assert not cancelled['diagnostics']['busy']; assert status['diagnostics'] == retained['diagnostics']; assert 'cancelled' in cancelled['diagnostics']['notice']; app.gate('cancelled')
    pending, _ = app.checkpoint('save-dialog'); assert pending['diagnostics']['busy']
    destination = app.root / 'midi-diagnostics.json'; app.ui('save', app.root / 'save-panel.png', destination)
    wait_for(destination.exists, 'native JSON save', 15); time.sleep(.4); app.gate('save-dialog')
    saved, status = app.checkpoint('saved'); assert not saved['diagnostics']['busy']; assert status['diagnostics'] == retained['diagnostics']; assert 'exported to' in saved['diagnostics']['notice']; assert_export(destination, retained); app.gate('saved')
    wait_for((app.capture / 'midi-exit-recording.json').exists, 'recording on before exit')
    assert json.loads((app.capture / 'midi-exit-recording.json').read_text(encoding='utf-8'))['diagnostics']['recording']
    app.finish(); app = None
    app = App('reopen'); reopened, status = app.checkpoint('reopened'); assert status['diagnostics'] is None and not reopened['diagnostics']['recording']; assert status['bindings'] == expected_bindings
    assert reopened['saved_settings'] is not None and 'diagnostics' not in json.dumps(reopened['saved_settings']); app.gate('reopened'); app.finish(); app = None
    results['reopen_default_off'] = True
    for phase in ['close-cancel', 'close-save']:
        app = App(phase); pending, _ = app.checkpoint('close-dialog'); assert pending['diagnostics']['busy']
        app.ui('inspect', app.root / 'pending-panel.png'); app.ui('close'); time.sleep(.5); assert app.proc.poll() is None; app.gate('close-dialog')
        held, state = app.checkpoint('close-held'); assert held['diagnostics']['busy']; assert held['diagnostics']['notice'] == 'Finish or cancel diagnostic export before closing.'
        assert state['output']['attempted'] == 0
        if phase == 'close-save':
            destination = app.root / 'midi-diagnostics.json'; app.ui('save', app.root / 'save-panel.png', destination); wait_for(destination.exists, 'save before close', 15); assert_export(destination, state)
        else: app.ui('cancel', app.root / 'cancel-panel.png')
        app.finish(deliberate_close=True); app = None; results[phase] = True
    if not windows:
        packets = [v['bytes'] for line in (b / 'received.jsonl').read_text(encoding='utf-8').splitlines() if (v := json.loads(line))['event'] == 'packet']
        assert packets == [[176,67,127],[176,67,0]] * 3, packets
        results['independently_received_messages'] = len(packets)
    else:
        assert all(v['output']['attempted'] == 0 for v in [initial, stopped, off, retained]); results['midi_delivery_qualified'] = False
    results.update({'success':True, 'platform':sys.platform, 'retained_events':len(entries), 'status_export_equal':True, 'stopped_history_unchanged':True,
        'midi_data_lane':'dispatcher-refusals-no-output' if windows else 'ALSA-independent-receiver', 'binary_sha256':hashlib.sha256(a.binary.read_bytes()).hexdigest()})
    (b / 'verification.json').write_text(json.dumps(results, indent=2) + '\n'); print(json.dumps(results), flush=True)
except BaseException as error:
    (b / 'failure.json').write_text(json.dumps({'error':repr(error), 'partial_results':results}, indent=2)); raise
finally:
    if app and app.proc.poll() is None:
        app.proc.terminate()
        try: app.proc.wait(timeout=10)
        except subprocess.TimeoutExpired: app.proc.kill(); app.proc.wait(timeout=5)
    if receiver and receiver.poll() is None:
        receiver.stdin.write('quit\n'); receiver.stdin.flush(); receiver.wait(timeout=10)
    if keeper and keeper.poll() is None:
        (b / 'keeper/stop').touch()
        try: keeper.wait(timeout=10)
        except subprocess.TimeoutExpired: keeper.terminate(); keeper.wait(timeout=10)

    # GPU shader/font caches belong only to this fresh acceptance profile.
    # Keep observations, saved profile and keeper evidence; discard rebuildable cache.
    cache = b / 'cache'
    if cache.is_dir():
        size = sum(v.stat().st_size for v in cache.rglob('*') if v.is_file())
        shutil.rmtree(cache)
        (b / 'cache-cleanup.json').write_text(json.dumps({'removed_runtime_cache_bytes':size, 'path':str(cache)}))
