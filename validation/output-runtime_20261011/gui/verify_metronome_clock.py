#!/usr/bin/env python3
"""Check measured metronome processor transitions from the frozen native log."""
import argparse
import json
from pathlib import Path
import re

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("native_log", type=Path)
options = parser.parse_args()
samples = [dict(streams=int(s), frames=int(f), blocks=int(b), click=c == "true",
                playing=p == "true", capture_invalid=i == "true")
           for s, f, b, c, p, i in re.findall(
               r"output-clock streams=(\d+) frames=(\d+) blocks=(\d+) click=(true|false) playing=(true|false) capture-invalid=(true|false)",
               options.native_log.read_text())]
assert len(samples) == 5, f"expected five clock samples, got {len(samples)}"
assert [s["click"] for s in samples] == [False, True, False, True, False], samples
assert all(s["streams"] == 1 and not s["capture_invalid"] and not s["playing"]
           and s["frames"] > 0 and s["blocks"] > 0 for s in samples), samples
assert all(b["frames"] > a["frames"] and b["blocks"] > a["blocks"]
           for a, b in zip(samples, samples[1:])), samples
print(json.dumps({"status": "ok", "samples": samples}, indent=2))
