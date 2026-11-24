"""Render Window Seat, an original six-bar piano take at 120 BPM."""

from array import array
from math import exp, pi, sin
from pathlib import Path
import sys
import wave

RATE = 48_000
BEAT = 0.5
BARS = (
    ((38, 45, 54, 61), (66, None, 69, None, 76, 74, 73, None)),
    ((38, 45, 54, 64), (66, 69, 73, None, 76, 74, 69, None)),
    ((35, 42, 57, 61), (66, None, 69, 73, 71, None, 69, 66)),
    ((43, 50, 57, 66), (67, 71, 74, None, 76, 74, 71, None)),
    ((45, 52, 55, 62), (69, 74, 76, None, 73, 71, 69, None)),
    ((38, 45, 54, 61), (66, None, 64, None, 62, None, None, None)),
)


def strike(mix, midi, onset, duration, gain):
    frequency = 440 * 2 ** ((midi - 69) / 12)
    angle = 2 * pi * frequency / RATE
    first = round(onset * RATE)
    frames = min(len(mix) // 2 - first, round((duration + 0.48) * RATE))
    pan = max(0.18, min(0.82, 0.5 + (midi - 60) / 100))
    left, right = (1 - pan) ** 0.5, pan ** 0.5
    partials = ((1.0, 0.72, 2.1), (2.001, 0.20, 4.0),
                (3.006, 0.07, 6.8), (4.013, 0.025, 10.0), (6.04, 0.010, 22.0))
    for frame in range(frames):
        seconds = frame / RATE
        attack = 1 - exp(-seconds / 0.0028)
        release = exp(-max(0, seconds - duration) / 0.065)
        sound = sum(level * exp(-seconds * decay) * sin(angle * frame * ratio)
                    for ratio, level, decay in partials)
        sample = gain * attack * release * sound
        index = (first + frame) * 2
        mix[index] += sample * left
        mix[index + 1] += sample * right


def write_take(path: Path) -> None:
    """Write 12 seconds of 48 kHz stereo signed PCM WAV."""
    mix = array("f", [0]) * (2 * RATE * 12)
    for bar, (harmony, tune) in enumerate(BARS):
        downbeat = bar * 4 * BEAT
        bass, fifth, inner, upper = harmony
        for beat, midi, duration, gain in (
            (0, bass, 0.82, 0.21), (0.5, fifth, 0.55, 0.13),
            (1, inner, 0.8, 0.10), (1.5, upper, 0.7, 0.095),
            (2, bass + 12, 0.65, 0.14), (2.5, fifth, 0.6, 0.12),
            (3, inner, 0.52, 0.085),
        ):
            strike(mix, midi, downbeat + beat * BEAT, duration, gain)
        for eighth, midi in enumerate(tune):
            if midi is not None:
                duration = 0.64 if eighth == 7 or tune[eighth + 1] is None else 0.24
                strike(mix, midi, downbeat + eighth * BEAT / 2 + 0.006, duration,
                       0.22 if eighth in (0, 4) else 0.17)

    # A short stereo room reflection, then a quiet release at the file boundary.
    delays = (round(RATE * 0.037) * 2, round(RATE * 0.053) * 2)
    for index in range(len(mix) - 1, max(delays), -1):
        mix[index] += 0.09 * mix[(index - delays[index % 2]) ^ 1]
    scale = 28_800 / max(abs(sample) for sample in mix)
    fade = round(RATE * 0.035) * 2
    pcm = array("h", (round(sample * scale * min(1, (len(mix) - index - 1) / fade))
                      for index, sample in enumerate(mix)))
    if sys.byteorder != "little":
        pcm.byteswap()
    with wave.open(str(path), "wb") as output:
        output.setparams((2, 2, RATE, 0, "NONE", "not compressed"))
        output.writeframes(pcm.tobytes())
