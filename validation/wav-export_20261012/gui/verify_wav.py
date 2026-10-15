#!/usr/bin/env python3
"""Independently inspect mono float32 RIFF WAV without a hound decoder."""
import argparse
import hashlib
import json
import math
import struct
from pathlib import Path


FLOAT_GUID = bytes.fromhex("0300000000001000800000aa00389b71")


def verify(path, expected_rate=None, expected_frames=None, expected_pcm_sha256=None):
    path = Path(path)
    size = path.stat().st_size
    chunks = []
    fmt = None
    data = None
    fact = None
    with path.open("rb") as stream:
        header = stream.read(12)
        if len(header) != 12:
            raise ValueError("truncated RIFF header")
        riff, declared, wave = struct.unpack("<4sI4s", header)
        if riff != b"RIFF" or wave != b"WAVE" or declared + 8 != size:
            raise ValueError("RIFF/WAVE identity or declared size differs")
        while stream.tell() < size:
            chunk_header = stream.read(8)
            if len(chunk_header) != 8:
                raise ValueError("truncated chunk header")
            name, length = struct.unpack("<4sI", chunk_header)
            offset = stream.tell()
            next_offset = offset + length + (length & 1)
            if next_offset > size:
                raise ValueError("chunk exceeds RIFF bounds")
            chunks.append({"id": name.decode("ascii", "replace"), "offset": offset, "bytes": length})
            if name == b"fmt ":
                if fmt is not None or not 16 <= length <= 65536:
                    raise ValueError("duplicate or invalid fmt chunk")
                payload = stream.read(length)
                tag, channels, rate, byte_rate, alignment, bits = struct.unpack("<HHIIHH", payload[:16])
                if tag == 0xFFFE:
                    if length < 40 or struct.unpack_from("<H", payload, 16)[0] < 22:
                        raise ValueError("truncated extensible fmt")
                    valid_bits, channel_mask = struct.unpack_from("<HI", payload, 18)
                    # A single-channel file may use any one speaker bit (hound
                    # uses front-left), or zero for unspecified positioning.
                    if payload[24:40] != FLOAT_GUID or valid_bits != 32 or channel_mask & (channel_mask - 1):
                        raise ValueError("extensible fmt is not mono float32")
                elif tag != 3:
                    raise ValueError("fmt is not IEEE float")
                if channels != 1 or bits != 32 or alignment != 4 or byte_rate != rate * 4 or not 1000 <= rate <= 384000:
                    raise ValueError("unexpected mono float32 format/rate")
                fmt = {"format_tag": tag, "channels": channels, "sample_rate_hz": rate,
                       "bits_per_sample": bits, "block_alignment": alignment, "byte_rate": byte_rate}
            elif name == b"data":
                if data is not None or length == 0 or length % 4:
                    raise ValueError("duplicate, empty or misaligned data chunk")
                data = (offset, length)
            elif name == b"fact":
                if fact is not None or length < 4:
                    raise ValueError("duplicate or invalid fact chunk")
                fact = struct.unpack("<I", stream.read(4))[0]
            stream.seek(next_offset)
        if fmt is None or data is None:
            raise ValueError("missing fmt/data")
        frames = data[1] // 4
        if fact is not None and fact != frames:
            raise ValueError("fact frame count differs")
        if expected_rate is not None and fmt["sample_rate_hz"] != expected_rate:
            raise ValueError("sample rate differs from expected")
        if expected_frames is not None and frames != expected_frames:
            raise ValueError("frame count differs from accepted cycle")
        stream.seek(data[0])
        left = data[1]
        digest = hashlib.sha256()
        peak = 0.0
        sum_squared = 0.0
        nonzero = 0
        while left:
            block = stream.read(min(left, 65536))
            if not block or len(block) % 4:
                raise ValueError("truncated or misaligned PCM")
            digest.update(block)
            for (value,) in struct.iter_unpack("<f", block):
                if not math.isfinite(value) or abs(value) > 1.0:
                    raise ValueError("PCM is nonfinite or exceeds live clamp")
                peak = max(peak, abs(value))
                sum_squared += value * value
                nonzero += value != 0
            left -= len(block)
        pcm_hash = digest.hexdigest()
        if expected_pcm_sha256 is not None and pcm_hash != expected_pcm_sha256:
            raise ValueError("PCM bytes differ from supplied processor reference")
        stream.seek(0)
        whole_file = hashlib.sha256()
        while True:
            block = stream.read(65536)
            if not block:
                break
            whole_file.update(block)
        file_digest = whole_file.hexdigest()
    return {"path": str(path.resolve()), "status": "passed", "file_bytes": size,
            "file_sha256": file_digest, "chunks": chunks, **fmt, "frames": frames,
            "duration_seconds": frames / fmt["sample_rate_hz"], "pcm_sha256": pcm_hash,
            "peak": peak, "rms": math.sqrt(sum_squared / frames), "nonzero_frames": nonzero,
            "expected_rate_hz": expected_rate, "expected_frames": expected_frames,
            "expected_pcm_sha256": expected_pcm_sha256,
            "limits": "Container and PCM checks; acoustic/device behavior is separately qualified."}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("path", type=Path)
    parser.add_argument("--rate", type=int)
    parser.add_argument("--frames", type=int)
    parser.add_argument("--pcm-sha256")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    result = verify(args.path, args.rate, args.frames, args.pcm_sha256)
    encoded = json.dumps(result, indent=2) + "\n"
    if args.output:
        args.output.write_text(encoded)
    print(encoded, end="")
