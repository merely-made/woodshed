from pathlib import Path
import hashlib,json,shlex,sys
base=Path(__file__).resolve().parent
repo=Path('/Users/markik/Code/worktrees/woodshed-portable-fretboard')
d=base.parent/'native-build/target/debug/woodshed-genet.d'
paths=set(shlex.split(d.read_text().splitlines()[0].split(': ',1)[1]))
for folder in ['crates/woodshed-genet','crates/woodshed-views','crates/woodshed-core','crates/woodshedding','assets/fonts/bravura']:
 paths.update(str(p) for p in (repo/folder).rglob('*') if p.is_file() and p.suffix in ['.rs','.toml','.otf','.txt'])
paths.update(str(p) for p in [repo/'Cargo.lock',base.parent/'native-build/Cargo.toml',base.parent/'native-build/Cargo.lock',base.parent/'native-build/.cargo/config.toml'])
files={p:hashlib.sha256(Path(p).read_bytes()).hexdigest() for p in sorted(paths) if Path(p).is_file()}
(base/sys.argv[1]).write_text(json.dumps(files,indent=2)+'\n')
print(len(files))
