#!/usr/bin/env python3
"""Owner-gated real keyboard and selected virtual-controller guitar trial.

No action commands are sent by this launcher. The owner operates the keyboard;
controller input is supplied deliberately through the fixture's stdin FIFO.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time
import uuid

ROOT = Path(__file__).resolve().parents[4]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--native', type=Path, required=True, help='Passed isolated native cohort, keeper stopped')
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--bundle-identifier', help='Reuse the explicitly granted acceptance bundle identity')
options = parser.parse_args()
NATIVE = options.native.resolve()
assert json.loads((NATIVE/'verification.json').read_text())['success']
BASE = options.output.resolve()
BASE.mkdir(parents=True, exist_ok=False)
(BASE/'frozen').mkdir()
(BASE/'profile').mkdir()
# The fixture uses a deterministic synthetic identity and requires a new root.
# Never open or modify the user's keeper or the passed native cohort's vault.
KEEPER_ROOT = BASE/'keeper'
shutil.copyfile(NATIVE/'guitar-preparation.json', BASE/'profile/settings.json')
for name, source in [('woodshed-genet', ROOT/'target/debug/woodshed-genet'),
                     ('midi_controller', ROOT/'target/debug/examples/midi_controller'),
                     ('keeper', NATIVE/'frozen/keeper')]:
    shutil.copy2(source, BASE/'frozen'/name)
source_name = 'Pedal guitar input ' + uuid.uuid4().hex[:8]
scenario = BASE/'frozen/guitar.scn'
scenario.write_text((Path(__file__).with_name('guitar.scn')).read_text().replace('PEDAL_INPUT_SOURCE', source_name))
endpoint = '/tmp/pedal-input-guitar-' + uuid.uuid4().hex[:10] + '.sock'
fifo = BASE/'controller.fifo'
os.mkfifo(fifo)
fd = os.open(fifo, os.O_RDWR|os.O_NONBLOCK)
keeperlog = (BASE/'keeper.log').open('w')
keeper = subprocess.Popen([str(BASE/'frozen/keeper'), endpoint, str(KEEPER_ROOT), 'no-wallet'], stdout=keeperlog, stderr=subprocess.STDOUT)
awake = subprocess.Popen(['/usr/bin/caffeinate','-d','-i','-t','1800'])
source = None
try:
    end = time.monotonic()+30
    while not (KEEPER_ROOT/'ready').exists() and time.monotonic()<end:
        assert keeper.poll() is None, 'isolated keeper exited'
        time.sleep(.05)
    assert (KEEPER_ROOT/'ready').exists()
    source = subprocess.Popen([str(BASE/'frozen/midi_controller'), source_name, str(BASE/'sent.jsonl')],
                              stdin=open(fifo,'r'), stdout=(BASE/'controller.log').open('w'),
                              stderr=(BASE/'controller-error.log').open('w'), text=True)
    env = {'CAMBIUM_HOST_KEY_TRACE':'1','WOODSHED_WIDTH':'1180','WOODSHED_HEIGHT':'1050','WOODSHED_PRESENTATION_TIMEOUT_SECS':'60',
           'APPDATA':str(BASE/'profile'),'XDG_DATA_HOME':str(BASE/'data'),'XDG_CONFIG_HOME':str(BASE/'config'),
           'XDG_CACHE_HOME':str(BASE/'cache'),'MERE_ROOT':str(BASE/'mere'),'GRAPHSHELL_APP_ENDPOINT':endpoint,
           'WOODSHED_STATE':str(BASE/'profile/practice.bin'),'WOODSHED_SETTINGS':str(BASE/'profile/settings.json'),'TMPDIR':'/tmp'}
    command = [sys.executable,str(ROOT/'scripts/run-macos-scenario.py'),'--binary',str(BASE/'frozen/woodshed-genet'),
               '--prefix','WOODSHED','--scenario',str(scenario),'--output',str(BASE/'run'),'--timeout','1700']
    if options.bundle_identifier:
        command += ['--bundle-identifier', options.bundle_identifier]
    for key,value in env.items():
        command += ['--env',key+'='+value]
    (BASE/'inputs.json').write_text(json.dumps({'command':command,'environment':env,'source':source_name,
        'binary_sha256':hashlib.sha256((BASE/'frozen/woodshed-genet').read_bytes()).hexdigest()},indent=2)+'\n')
    sys.exit(subprocess.call(command,cwd=ROOT))
finally:
    if source and source.poll() is None:
        os.write(fd,b'quit\n')
        try: source.wait(timeout=5)
        except subprocess.TimeoutExpired: source.terminate(); source.wait(timeout=5)
    os.close(fd)
    (KEEPER_ROOT/'stop').touch()
    try: keeper.wait(timeout=5)
    except subprocess.TimeoutExpired: keeper.terminate();keeper.wait(timeout=5)
    awake.terminate();awake.wait(timeout=5);keeperlog.close()
