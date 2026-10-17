"""Read-only verification of the committed native span-editing receipt."""
from pathlib import Path
import copy, hashlib, json
root = Path(__file__).resolve().parent
read = lambda p: json.loads(p.read_text())
seed, reopen = root / "native/seed", root / "native/reopen"
b, c, s = [read(seed / (name + ".json")) for name in ["before", "cancelled", "saved"]]
r = read(reopen / "reopened.json")
assert b["live"]["set"] == b["stored"]["set"] == c["live"]["set"] == c["stored"]["set"]
assert b["document"] == c["document"]
expected = copy.deepcopy(b["live"]["set"])
a = expected["progression_arrangement"]["alignment"]
a["cards"][1]["placement"]["span"] = {"start": 20, "length": 24}
a["rests"][0]["placement"]["span"] = {"start": 12, "length": 4}
assert expected == s["live"]["set"] == s["stored"]["set"] == r["live"]["set"] == r["stored"]["set"]
assert s["document"] == r["document"]
assert r["selection"] is None
for value in [b, c, s, r]:
    assert not value["draft_dirty"] and value["error"] is None
for run in [seed, reopen]:
    assert (run / "scenario.done").read_text().startswith("RESULT ok")
for name, digest in read(root / "sha256.json").items():
    assert hashlib.sha256((root / name).read_bytes()).hexdigest() == digest, name
print("PASS: exact cancellation; only intended Card/rest spans change; sealed Sets and entire document reopen exactly; receipt hashes match.")
