#!/usr/bin/env python3
"""Verify retained evidence without launching products, devices or Cargo."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import struct
import subprocess


def digest(data):
    return hashlib.sha256(data).hexdigest()


def load(path):
    return json.loads(path.read_text())


def require(condition, message):
    if not condition:
        raise SystemExit("FAIL: " + message)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--working-tree", action="store_true",
                        help="also require current repository inputs to match the qualified source")
    args = parser.parse_args()
    bundle = Path(__file__).resolve().parent
    root = bundle.parents[1]
    receipt = load(bundle / "receipt.json")
    bindings = load(bundle / "source-sha256.json")
    manifest = load(bundle / "evidence-sha256.json")
    excluded = {"evidence-sha256.json", "publication.json"}
    actual = {str(f.relative_to(bundle)) for f in bundle.rglob("*")
              if f.is_file() and f.name not in excluded and "__pycache__" not in f.parts}
    require(actual == set(manifest), "evidence manifest membership")
    for name, expected in manifest.items():
        path = (bundle / name).resolve()
        require(path.is_relative_to(bundle), "evidence path escapes bundle: " + name)
        require(digest(path.read_bytes()) == expected, "evidence hash: " + name)
    require(len(bindings) == receipt["source_bindings"]["count"], "source binding count")
    for name, expected in bindings.items():
        require((root / name).resolve().is_relative_to(root), "source path escapes repository")
        blob = subprocess.check_output(["git", "show", receipt["source_revision"] + ":" + name], cwd=root)
        require(digest(blob) == expected, "qualified Git source: " + name)
        if args.working_tree:
            require(digest((root / name).read_bytes()) == expected, "current source: " + name)

    logs = {}
    for folder in sorted((bundle / "checks").iterdir()):
        process = load(folder / "process.json")
        log = (folder / "command.log").read_bytes()
        logs[folder.name] = log.decode()
        require(digest(log) == process["log_sha256"], "process log hash: " + folder.name)
        require(process["source_stable"] and process["source_before"] == process["source_after"],
                "source changed during process: " + folder.name)
        if folder.name in receipt["positive_gates"]:
            require(process["exit_code"] == 0, "positive process exit: " + folder.name)
            for name, expected in process["source_before"].items():
                require(bindings.get(name) == expected, "positive source bytes: " + folder.name + ":" + name)
        elif folder.name in {"baseline-dock", "baseline-export", "baseline-availability",
                            "redshank-first", "woodshed-first", "upstream-viewport-control", "viewport-fixed"}:
            require(process["exit_code"] == 101, "retained failed/control process: " + folder.name)
            require(re.search(r"test result: FAILED\..*; 1 failed;", logs[folder.name]),
                    "retained failed assertion: " + folder.name)
        else:
            require(process["exit_code"] == 0, "superseded/control process exit: " + folder.name)

    pattern = r"test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; \d+ measured; (\d+) filtered out"
    red = [tuple(map(int, row)) for row in re.findall(pattern, logs["redshank-final"])]
    wood = [tuple(map(int, row)) for row in re.findall(pattern, logs["woodshed-formatted-final"])]
    children = [row for row in wood if row[3] > 0]
    require(len(children) == 5 and all(row == (1, 0, 0, 362) for row in children), "nested child accounting")
    require(sum(row[0] for row in red) == 287, "Redshank pass count")
    require(sum(row[0] for row in wood) - sum(row[0] for row in children) == 866, "Woodshed distinct pass count")
    require(sum(row[2] for row in red + wood) == 16, "existing ignored count")
    require(all(row[1] == 0 for row in red + wood), "final failed test count")
    added = load(bundle / "added-tests.json")
    require(len(added) == 24 and len({(x["path"], x["name"]) for x in added}) == 24, "added test inventory")
    for item in added:
        log = logs["redshank-final" if item["path"].startswith("ports/redshank/") else "woodshed-formatted-final"]
        require(re.search(r"test [^\n]*\b" + re.escape(item["name"]) + r" \.\.\. ok", log),
                "new test execution: " + item["name"])
    require("352 filtered out" in logs["viewport-fixed"], "excluded stale artifact inventory")
    require("362 filtered out" in logs["viewport-fixed-r2"], "corrected current artifact inventory")

    artifact = load(bundle / "native-artifact.json")
    require(artifact["source_revision"] == receipt["source_revision"], "native source identity")
    observations = load(bundle / "native-close-observations.json")
    captures = []
    for name, count in [("absent", 5), ("offline", 3)]:
        folder = bundle / "native" / name
        process = load(folder / "process.json")
        require(process["success"] and not process["timeout"] and process["launcher_exit"] == 0,
                "native process result: " + name)
        require(process["binary_sha256"] == artifact["sha256"] == receipt["native"]["binary_sha256"],
                "native binary binding")
        require(process["scenario_sha256"] == digest((bundle / "scenarios" / (name + ".scn")).read_bytes()),
                "native scenario binding")
        scenario = (folder / "scenario.done").read_text()
        require("RESULT ok" in scenario and "frames: %d captured, 0 blank" % count in scenario,
                "native scenario completion: " + name)
        images = sorted(folder.glob("*.png"))
        require(len(images) == process["captures"] == count, "native capture count")
        for path in images:
            data = path.read_bytes()
            require(data[:8] == b"\x89PNG\r\n\x1a\n" and struct.unpack(">II", data[16:24]) == (2200, 1400),
                    "native PNG dimensions: " + path.name)
            captures.append(str(path.relative_to(bundle)))
        close = observations[name]
        require(close["receipt_completed_before_click"] and close["product_exit"] == 0
                and not close["timeout"] and close["persona_owned_files"] == 0, "saved native close observation")
    require(sorted(captures) == sorted(receipt["native"]["captures"]), "reviewed image inventory")
    # The review itself is a human/agent observation, not something hashes prove.
    ledger = load(root / "validation/full-source-audit_20261016/current-finding-ledger.json")
    require(len(ledger["findings"]) == 97 and len({x["id"] for x in ledger["findings"]}) == 97, "ledger row preservation")
    require(set(receipt["scope"]) == set(ledger["capture_availability_followup_20261025"]["scope"]), "ledger scope")
    print("PASS: %d source bindings; %d evidence files; 1,153 distinct passes, 16 existing ignores, "
          "5 child invocations separately; 24 added tests; 2 native processes / 8 reviewed captures."
          % (len(bindings), len(manifest)))
    print("No product, Cargo, microphone, physical output or remote host was launched by this verifier.")


if __name__ == "__main__":
    main()
