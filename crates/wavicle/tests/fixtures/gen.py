# Copyright 2026 Mark Alan Boykin
# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.
# SPDX-License-Identifier: MPL-2.0

# Fixture generator for wavicle's conformance tests.
#
# Generates deterministic raw PCM, encodes it with the reference `wavpack`
# CLI (the oracle), and captures `wvunpack -ss` output as goldens. Run from
# this directory. Requires the pinned reference CLI; see TOOLS below.
#
# Deterministic on purpose: pure tones, ramps, and silence, no randomness,
# so regeneration is reproducible and the fixtures are ours to license.
# A cargo xtask may replace this script later; the .wv and .ss.txt outputs
# are what is checked in and what the tests read.

import math
import struct
import subprocess
import sys
from pathlib import Path

TOOLS = Path("../../../../testing/wavicle/tools")  # relative to this dir
WAVPACK = TOOLS / "wavpack.exe"
WVUNPACK = TOOLS / "wvunpack.exe"

HERE = Path(".")


def tone(n, rate, freq, amp):
    return [amp * math.sin(2 * math.pi * freq * i / rate) for i in range(n)]


def interleave(*chans):
    return [s for frame in zip(*chans) for s in frame]


def pack_i16(samples):
    return b"".join(struct.pack("<h", max(-32768, min(32767, round(s * 32767)))) for s in samples)


def pack_i24(samples):
    out = bytearray()
    for s in samples:
        v = max(-(1 << 23), min((1 << 23) - 1, round(s * ((1 << 23) - 1))))
        out += struct.pack("<i", v)[:3]
    return bytes(out)


def pack_i32(samples):
    return b"".join(
        struct.pack("<i", max(-(1 << 31), min((1 << 31) - 1, round(s * ((1 << 31) - 1)))))
        for s in samples
    )


def pack_f32(samples):
    return b"".join(struct.pack("<f", s) for s in samples)


# name -> (raw bytes, --raw-pcm spec, extra wavpack args)
def fixtures():
    r48, r44 = 48000, 44100
    n = 4800  # 0.1 s at 48k
    fx = {}

    fx["int16_mono"] = (pack_i16(tone(n, r44, 220.0, 0.6)), f"{r44},16s,1,le", [])
    fx["int16_stereo"] = (
        pack_i16(interleave(tone(n, r48, 220.0, 0.6), tone(n, r48, 330.0, 0.5))),
        f"{r48},16s,2,le",
        [],
    )
    fx["int24_stereo"] = (
        pack_i24(interleave(tone(n, r48, 261.6, 0.7), tone(n, r48, 392.0, 0.4))),
        f"{r48},24s,2,le",
        [],
    )
    fx["int32_stereo"] = (
        pack_i32(interleave(tone(n, r48, 174.6, 0.5), tone(n, r48, 523.3, 0.3))),
        f"{r48},32s,2,le",
        [],
    )
    # 32-bit ints whose low 12 bits are zero: exercises the zeros/shift path
    # of ID_INT32_INFO (no wvx stream needed for losslessness).
    shifted = [
        (round(s * ((1 << 19) - 1)) << 12) / float(1 << 31)
        for s in tone(n, r48, 220.0, 0.6)
    ]
    fx["int32_shifted_mono"] = (pack_i32(shifted), f"{r48},32s,1,le", [])
    fx["f32_mono"] = (pack_f32(tone(n, r48, 220.0, 0.6)), f"{r48},32f,1,le", [])
    fx["f32_stereo"] = (
        pack_f32(interleave(tone(n, r48, 220.0, 0.6), tone(n, r48, 330.0, 0.5))),
        f"{r48},32f,2,le",
        [],
    )
    # Adversarial float values: the reconstruction must reproduce these exact
    # 32-bit patterns, including signed zero, denormals, NaN payloads, and
    # infinities. Given as raw bit patterns, tiled with a quiet tone so the
    # encoder has ordinary material around the exceptions.
    specials = [
        0x00000000,  # +0.0
        0x80000000,  # -0.0
        0x3F800000,  # 1.0
        0xBF800000,  # -1.0
        0x00000001,  # smallest positive denormal
        0x80000001,  # smallest negative denormal
        0x007FFFFF,  # largest denormal
        0x7F800000,  # +inf
        0xFF800000,  # -inf
        0x7FC00000,  # quiet NaN
        0x7F800001,  # signaling NaN, payload 1
        0x7FFFFFFF,  # NaN, full payload
        0xFFC00000,  # negative quiet NaN
        0x7F7FFFFF,  # max finite normal
        0x00800000,  # smallest positive normal
        0x3F000000,  # 0.5
    ]
    special_bytes = b"".join(struct.pack("<I", b) for b in specials)
    quiet = pack_f32(tone(240, r48, 55.0, 0.02))
    fx["f32_specials_mono"] = (special_bytes * 16 + quiet, f"{r48},32f,1,le", [])
    # Identical channels: the 0x410 reference stores this mono with FALSE_STEREO.
    same = tone(n, r48, 110.0, 0.5)
    fx["false_stereo"] = (pack_i16(interleave(same, same)), f"{r48},16s,2,le", [])
    fx["silence_stereo"] = (pack_i16([0.0] * (n * 2)), f"{r48},16s,2,le", [])
    # Small block size forces a multi-block stream (20000 frames / 4096).
    fx["multiblock_int16_mono"] = (
        pack_i16(tone(20000, r44, 220.0, 0.6)),
        f"{r44},16s,1,le",
        ["--blocksize=4096"],
    )
    # Out of scope for the decoder: hybrid lossy. Parsed headers must reject it.
    fx["hybrid_lossy_int16_stereo"] = (
        pack_i16(interleave(tone(n, r48, 220.0, 0.6), tone(n, r48, 330.0, 0.5))),
        f"{r48},16s,2,le",
        ["-b256"],
    )
    return fx


def main():
    for name, (raw, spec, extra) in fixtures().items():
        raw_path = HERE / f"{name}.raw"
        wv_path = HERE / f"{name}.wv"
        ss_path = HERE / f"{name}.ss.txt"
        raw_path.write_bytes(raw)
        subprocess.run(
            [str(WAVPACK), f"--raw-pcm={spec}", *extra, "-m", "-y", "-q",
             str(raw_path), "-o", str(wv_path)],
            check=True,
        )
        ss = subprocess.run(
            [str(WVUNPACK), "-ss", str(wv_path)],
            capture_output=True, text=True,
        )
        ss_path.write_text(ss.stdout + ss.stderr)
        raw_path.unlink()
        print(f"{name}: {wv_path.stat().st_size} bytes")
    print("done")


if __name__ == "__main__":
    sys.exit(main())
