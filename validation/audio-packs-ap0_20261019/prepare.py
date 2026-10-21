"""Materialize isolated AP0 probes and original synthetic assets; no device access."""
from pathlib import Path
import argparse
import math
import shutil
import struct

parser = argparse.ArgumentParser()
parser.add_argument("destination", type=Path)
parser.add_argument("--woodshed", type=Path, default=Path(__file__).resolve().parents[2])
args = parser.parse_args()
source = Path(__file__).resolve().parent
dest = args.destination.resolve()
if dest.exists():
    raise SystemExit("Choose a new scratch directory; existing directories are preserved")
dest.mkdir(parents=True)
for name in ["Cargo.toml", "Cargo.lock"]:
    shutil.copyfile(source / name, dest / name)
for crate in ["portable", "runtime"]:
    for file in (source / crate).rglob("*"):
        if file.is_file():
            target = dest / file.relative_to(source)
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(file.read_text().replace("@WOODSHED_ROOT@", str(args.woodshed.resolve())))
(dest / "artifacts").mkdir()


def chunk(tag, data):
    return tag + struct.pack("<I", len(data)) + data + (b"\0" if len(data) % 2 else b"")


def name(value):
    return value.encode().ljust(20, b"\0")


# Original decaying sine; no vendor music or third-party SoundFont assets.
n = 4800
samples = [int(14000 * math.sin(2 * math.pi * 110 * i / 48000) * math.exp(-i / 1500)) for i in range(n)] + [0] * 46
info = chunk(b"LIST", b"INFO" + chunk(b"ifil", struct.pack("<HH", 2, 1)) + chunk(b"isng", b"EMU8000\0") + chunk(b"INAM", b"AP0 Synthetic\0"))
sdta = chunk(b"LIST", b"sdta" + chunk(b"smpl", struct.pack("<" + "h" * len(samples), *samples)))
phdr = name("AP0 Drum") + struct.pack("<HHHIII", 0, 128, 0, 0, 0, 0) + name("EOP") + struct.pack("<HHHIII", 0, 0, 1, 0, 0, 0)
inst = name("AP0 Tone") + struct.pack("<H", 0) + name("EOI") + struct.pack("<H", 1)
shdr = name("AP0 Tone") + struct.pack("<IIIIIBbHH", 0, n, 0, n, 48000, 36, 0, 0, 1) + name("EOS") + struct.pack("<IIIIIBbHH", n, n, n, n, 48000, 0, 0, 0, 1)
parts = [(b"phdr", phdr), (b"pbag", struct.pack("<HHHH", 0, 0, 1, 0)), (b"pmod", bytes(10)), (b"pgen", struct.pack("<HHHH", 41, 0, 0, 0)), (b"inst", inst), (b"ibag", struct.pack("<HHHH", 0, 0, 1, 0)), (b"imod", bytes(10)), (b"igen", struct.pack("<HHHH", 53, 0, 0, 0)), (b"shdr", shdr)]
pdta = chunk(b"LIST", b"pdta" + b"".join(chunk(tag, data) for tag, data in parts))
(dest / "artifacts/synthetic.sf2").write_bytes(chunk(b"RIFF", b"sfbk" + info + sdta + pdta))
print(dest)
