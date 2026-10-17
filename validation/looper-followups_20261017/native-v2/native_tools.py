"""Owned, isolated macOS acceptance launchers. No user profile defaults."""
import hashlib
import json
import pathlib
import plistlib
import shutil
import subprocess
import time

REPO = pathlib.Path('/Users/markik/Code/repos/woodshed')
KEEPER = pathlib.Path('/Users/markik/Code/testing/woodshed/link-model-p3-20261016/frozen-keeper-ee324e517/dr-c-receipt-host')
KEEPER_SHA = '848685e001d63724ca869fa58a97021b3fa15821f0d1fa9954c4853080c518f6'

def sha(path):
    return hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()

def write(path, value):
    with pathlib.Path(path).open('x') as stream:
        json.dump(value, stream, indent=2, default=str)
        stream.write('\n')

def freeze(trial, binaries):
    trial = pathlib.Path(trial)
    trial.mkdir(exist_ok=False)
    (trial / 'frozen').mkdir()
    status = subprocess.check_output(['git', 'status', '--porcelain'], cwd=REPO, text=True)
    if status:
        raise RuntimeError('Native candidate must be committed and clean')
    revision = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=REPO, text=True).strip()
    names = subprocess.check_output(['git', 'ls-files', 'crates', 'ports', 'Cargo.toml', 'Cargo.lock'], cwd=REPO, text=True).splitlines()
    sources = {name: sha(REPO / name) for name in names if name.endswith(('.rs', 'Cargo.toml', 'Cargo.lock'))}
    frozen = {}
    for name, source in binaries.items():
        shutil.copy2(source, trial / 'frozen' / name)
        frozen[name] = sha(trial / 'frozen' / name)
    write(trial / 'source-hashes.json', sources)
    write(trial / 'source.json', {'revision': revision, 'worktree': status, 'binaries': frozen, 'source_hashes_sha256': sha(trial / 'source-hashes.json')})
    return trial

def keeper(trial, endpoint):
    trial = pathlib.Path(trial)
    if pathlib.Path(endpoint).exists() or sha(KEEPER) != KEEPER_SHA:
        raise RuntimeError('Keeper binary or endpoint identity refused')
    with (trial / 'keeper.log').open('xb') as log:
        process = subprocess.Popen([str(KEEPER), endpoint, str(trial / 'keeper'), 'unlocked'], stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
    write(trial / 'keeper-process.json', {'pid': process.pid, 'binary': str(KEEPER), 'binary_sha256': KEEPER_SHA, 'endpoint': endpoint})
    began = time.monotonic()
    while not (trial / 'keeper/ready').exists():
        if process.poll() is not None or time.monotonic() - began > 20:
            raise RuntimeError('Keeper did not become ready')
        time.sleep(.05)
    write(trial / 'keeper-before.json', {str(p.relative_to(trial / 'keeper')): sha(p) for p in (trial / 'keeper').rglob('*') if p.is_file()})
    return endpoint

def phase(trial, name, product, scenario, env, binary=None, width=1180, height=1000):
    trial = pathlib.Path(trial)
    output = trial / name
    output.mkdir(exist_ok=False)
    script = output / 'scenario.scn'
    script.write_text(scenario)
    prefix = product.upper()
    app_name = 'Acceptance' + product.title() + ''.join(part.title() for part in name.split('-')) + trial.name.replace('-', '').title()
    app = output / (app_name + '.app')
    mac = app / 'Contents/MacOS'
    mac.mkdir(parents=True)
    executable = trial / 'frozen' / (binary or product)
    environment = {prefix + '_SCENARIO': str(script), prefix + '_CAPTURE_DIR': str(output), prefix + '_RECEIPT': str(output / 'scenario.done'), prefix + '_PRESENTATION_TIMEOUT_SECS': '60', prefix + '_WIDTH': str(width), prefix + '_HEIGHT': str(height), 'TMPDIR': '/tmp', 'CAMBIUM_HOST_FRAME_TRACE': '1', **env}
    for key, folder in [('XDG_DATA_HOME', 'data'), ('XDG_CONFIG_HOME', 'config'), ('XDG_CACHE_HOME', 'cache'), ('MERE_ROOT', 'mere')]:
        environment.setdefault(key, str(trial / folder))
    lines = ['#include <unistd.h>', '#include <stdlib.h>', '#include <stdio.h>', '#include <fcntl.h>', 'int main(void) {', f'if(chdir({json.dumps(str(REPO))}))return 120;']
    lines += [f'if(setenv({json.dumps(k)},{json.dumps(str(v))},1))return 121;' for k, v in environment.items()]
    lines += [f'int once=open({json.dumps(str(output / "launch.once"))},O_WRONLY|O_CREAT|O_EXCL,0600);if(once<0)return 0;close(once);']
    lines += [f'FILE *pid=fopen({json.dumps(str(output / "native.pid"))},"w");if(pid){{fprintf(pid,"%d\\n",getpid());fclose(pid);}}', f'int log=open({json.dumps(str(output / "native.log"))},O_WRONLY|O_CREAT|O_TRUNC,0600);if(log<0||dup2(log,1)<0||dup2(log,2)<0)return 122;', f'char *args[]={{{json.dumps(str(executable))},NULL}};execv(args[0],args);return 123;', '}']
    launcher = output / 'launcher.c'
    launcher.write_text('\n'.join(lines) + '\n')
    subprocess.run(['cc', str(launcher), '-o', str(mac / 'launcher')], check=True)
    with (app / 'Contents/Info.plist').open('wb') as stream:
        plistlib.dump({'CFBundleExecutable': 'launcher', 'CFBundleIdentifier': 'org.merely.acceptance.' + app_name.lower(), 'CFBundleName': app_name, 'CFBundlePackageType': 'APPL', 'CFBundleVersion': '1', 'NSHighResolutionCapable': True}, stream)
    write(output / 'launch.json', {'app': str(app), 'env': environment, 'source': json.loads((trial / 'source.json').read_text()), 'scenario_sha256': sha(script), 'binary_sha256': sha(executable)})
    return app

def launch(app):
    subprocess.run(['open', '-n', str(app)], check=True)

def process(pid):
    return subprocess.run(['ps', '-p', str(pid), '-o', 'comm='], capture_output=True, text=True).stdout.strip()

def stop_keeper(trial):
    trial = pathlib.Path(trial)
    keeper_info = json.loads((trial / 'keeper-process.json').read_text())
    pid = keeper_info['pid']
    if process(pid) != str(KEEPER):
        raise RuntimeError('Owned Keeper process identity changed')
    subprocess.run(['kill', '-TERM', str(pid)], check=True)
    for _ in range(50):
        if not process(pid):
            break
        time.sleep(.1)
    if process(pid):
        raise RuntimeError('Keeper still running')
    before = json.loads((trial / 'keeper-before.json').read_text())
    if not all(sha(trial / 'keeper' / name) == digest for name, digest in before.items()):
        raise RuntimeError('Generated Keeper fixture was changed')
