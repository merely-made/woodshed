"""Read-only checks of the committed native seed/reopen observations."""
from pathlib import Path
import json

root = Path(__file__).resolve().parent
read = lambda name: json.loads((root / name).read_text())
for run in ("seed", "reopen"):
    assert (root / "native" / run / "scenario.done").read_text().startswith("RESULT ok")
before = read("native/seed/before.json")
saved = read("native/seed/saved.json")
reopened = read("native/reopen/reopened.json")
assert before["live"]["set"].get("progression_arrangement") is None
cards = before["live"]["set"]["cards"]
for observation in (saved, reopened):
    assert not observation["draft_dirty"]
    assert observation["error"] is None
    assert observation["live"]["set"] == observation["stored"]["set"]
    assert observation["live"]["set"]["cards"] == cards
assert saved["live"]["set"] == reopened["live"]["set"]
assert saved["document"] == reopened["document"]
arrangement = saved["live"]["set"]["progression_arrangement"]
a = arrangement["alignment"]
assert a["name"] == "Native ii-V-I study"
assert a["extent"] == 64
assert a["role_spans"][2] == {"start": 32, "length": 32}
assert a["cards"][1]["placement"]["span"] == {"start": 16, "length": 8}
assert a["cards"][3]["role"] == 2
assert a["rests"][0]["placement"]["span"] == {"start": 24, "length": 8}
linked = {entry["occurrence"]: entry["scene"] for entry in saved["document"]["linked"]}
assert linked["card-3"]["facts"]["data"]["positions"] != linked["card-4"]["facts"]["data"]["positions"]
assert saved["selection"] == "rest-rest-1"
assert reopened["selection"] is None
print("PASS: sealed Set, Card instructions, authored alignment and shared scene document survive fresh reopening; the two tonic occurrences retain distinct saved voicings.")
