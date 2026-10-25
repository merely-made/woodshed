#!/usr/bin/env python3
import datetime, hashlib, json, os, subprocess, sys, time
from pathlib import Path
ROOT = Path(os.environ.get('WOODSHED_GATE_ROOT', '/Users/markik/Code/worktrees/woodshed-audit-next-20261025'))
HERE = Path(__file__).resolve().parent
name, workspace, *command = sys.argv[1:]
assert workspace in ("root", "redshank")
WORKSPACE = ROOT if workspace == "root" else ROOT/"ports/redshank"
TARGET = Path("/Users/markik/Code/repos/woodshed/target") if workspace == "root" else Path("/Users/markik/Code/repos/woodshed/ports/redshank/target")
if command and command[0] == '--': command.pop(0)
assert command
output=HERE/name
output.mkdir(exist_ok=False)
def stamp(): return datetime.datetime.now(datetime.timezone.utc).isoformat()
def git(*args): return subprocess.check_output(['git',*args],cwd=ROOT,text=True).strip()
def sources():
    paths=git('ls-files','--cached','--others','--exclude-standard').splitlines()
    def relevant(path):
        if path in ('Cargo.toml','Cargo.lock','rust-toolchain.toml','rustfmt.toml') or path.startswith(('.cargo/','assets/')): return True
        if workspace == 'root': return path.startswith('crates/')
        return (path.startswith('ports/redshank/') and not path.startswith(('ports/redshank/validation/','ports/redshank/design_docs/','ports/redshank/docs/'))) or path.startswith(('crates/audio-primitives/','crates/timed-text/'))
    return {path:hashlib.sha256((ROOT/path).read_bytes()).hexdigest() for path in sorted(set(paths)) if relevant(path) and (ROOT/path).is_file()}

env=os.environ.copy()
env.update(CARGO_TARGET_DIR=str(TARGET), CARGO_BUILD_BUILD_DIR=str(TARGET/'build'), TMPDIR='/tmp')
record={'command':command,'cwd':str(WORKSPACE),'source_revision':git('rev-parse','HEAD'),'source_state':git('status','--porcelain=v1'),'started_at_utc':stamp(),'environment':{k:env[k] for k in ('CARGO_TARGET_DIR','CARGO_BUILD_BUILD_DIR','TMPDIR')},'source_before':sources()}
(output/'process.json').write_text(json.dumps(record,indent=2)+'\n')
started=time.monotonic()
with (output/'command.log').open('wb') as log:
    result=subprocess.run(command,cwd=WORKSPACE,env=env,stdout=log,stderr=subprocess.STDOUT)
record.update(exit_code=result.returncode,ended_at_utc=stamp(),elapsed_seconds=round(time.monotonic()-started,3),source_after=sources())
record['source_stable']=record['source_before']==record['source_after']
record['log_sha256']=hashlib.sha256((output/'command.log').read_bytes()).hexdigest()
(output/'process.json').write_text(json.dumps(record,indent=2)+'\n')
print(name, 'exit', result.returncode,'source stable',record['source_stable'], 'elapsed',record['elapsed_seconds'])
text=(output/'command.log').read_text(errors='replace')
for line in text.splitlines():
    if line.startswith(('test result:','error:','failures:')):print(line)
sys.exit(result.returncode)
