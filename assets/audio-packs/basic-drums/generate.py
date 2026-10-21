# SPDX-License-Identifier: MPL-2.0
"""Generate the original bundled kit; no imported samples or third-party assets."""
from pathlib import Path
import json
import math
import struct
import wave

root = Path(__file__).resolve().parent
(root / "samples").mkdir(exist_ok=True)
rate = 48_000
noise = 1729
for voice, duration in [("kick", 0.22), ("snare", 0.14), ("hat", 0.055)]:
    samples = []
    phase = 0.0
    previous = 0.0
    for index in range(round(duration * rate)):
        t = index / rate
        noise = (1664525 * noise + 1013904223) & 0xFFFFFFFF
        white = noise / 2147483648.0 - 1.0
        high = (white - previous) / 2.0
        previous = white
        if voice == "kick":
            phase += 2 * math.pi * (48 + 110 * math.exp(-t * 38)) / rate
            value = 0.72 * math.sin(phase) * math.exp(-t * 22)
        elif voice == "snare":
            value = (0.48 * white + 0.15 * math.sin(2 * math.pi * 185 * t)) * math.exp(-t * 36)
        else:
            value = 0.36 * high * math.exp(-t * 85)
        samples.append(round(max(-1.0, min(1.0, value)) * 32767))
    samples[-1] = 0
    with wave.open(str(root / "samples" / f"{voice}.wav"), "wb") as output:
        output.setparams((1, 2, rate, 0, "NONE", "not compressed"))
        output.writeframes(struct.pack("<" + "h" * len(samples), *samples))


def track(voice, hits, accented, gain):
    return {
        "name": voice.title(),
        "steps": [{"Active": {"accent": i in accented}} if i in hits else "Empty" for i in range(16)],
        "sound": {"Sample": {"id": voice, "gain": gain, "accent_gain": 1.15, "pitch_semitones": 0.0}},
        "muted": False,
    }


definition = {
    "format": 1,
    "id": "woodshed.basic-drums",
    "version": "1.0.0",
    "name": "Basic drums",
    "license": "MPL-2.0",
    "requires": ["sample.mono.v1", "step.one_bar.v1", "midi.smf0.v1"],
    "parameters": {
        "gain": {"label": "Level", "unit": "linear", "min": 0.0, "max": 4.0, "default": 1.0},
        "accent_gain": {"label": "Accent level", "unit": "linear", "min": 0.0, "max": 4.0, "default": 1.15},
        "pitch_semitones": {"label": "Tuning", "unit": "semitones", "min": -24.0, "max": 24.0, "default": 0.0},
    },
    "voices": {
        voice: {"sample": f"samples/{voice}.wav", "attribution": "Woodshed contributors; original synthesized sample (generate.py)", "midi": {"channel": 10, "note": note, "velocity": velocity, "accent_velocity": accent, "gate_percent": 50}}
        for voice, note, velocity, accent in [("kick", 36, 88, 112), ("snare", 38, 90, 116), ("hat", 42, 62, 85)]
    },
    "patterns": {
        "backbeat": {"bpm": 100.0, "time_signature": {"numerator": 4, "denominator": 4}, "subdivision": {"divisions_per_beat": 4}, "tracks": [
            track("kick", [0, 8], [0], 0.8),
            track("snare", [4, 12], [4, 12], 0.7),
            track("hat", list(range(0, 16, 2)), [0, 8], 0.55),
        ]},
    },
}
(root / "audio-pack.json").write_text(json.dumps(definition, indent=2) + "\n")
