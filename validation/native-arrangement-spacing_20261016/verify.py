from pathlib import Path
import hashlib, json
root = Path(__file__).resolve().parent
read = lambda p: json.loads(p.read_text())
assert (root / "scenario.done").read_text().startswith("RESULT ok")
current = read(root / "spacing-saved.json")
previous = read(root.parent / "native-arrangement_20261016/native/seed/saved.json")
assert current["document"] == previous["document"]
assert current["live"]["set"] == previous["live"]["set"] == current["stored"]["set"]
assert not current["draft_dirty"] and current["error"] is None
for name, digest in read(root / "sha256.json").items():
    assert hashlib.sha256((root / name).read_bytes()).hexdigest() == digest, name
print("PASS: native spacing run, sealed Set and unchanged scene document, receipt hashes.")
