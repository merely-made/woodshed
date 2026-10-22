#!/usr/bin/env python3
"""Check bundled D1 evidence and source bindings without custody or native I/O."""
import hashlib
import json
from pathlib import Path
import re
import struct


HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent


def load(path):
    return json.loads((HERE / path).read_text())


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify():
    receipt = load("receipt.json")
    for relative, expected in load("evidence-sha256.json").items():
        path = HERE / relative
        assert path.resolve().is_relative_to(HERE), relative
        assert sha(path) == expected, ("evidence", relative)
    for relative, expected in load("source-sha256.json").items():
        path = REPO / relative
        assert path.resolve().is_relative_to(REPO), relative
        assert sha(path) == expected, ("source", relative)

    phases = ["aba", "reopen-a", "reopen-b", "incoming-failure", "locked"]
    assert load("completed-phases.json") == phases
    images = []
    for phase in phases:
        directory = HERE / "native" / phase
        metadata = load(f"native/{phase}/process.json")
        assert metadata["success"] is True and metadata["timeout"] is False, phase
        assert metadata["launcher_exit"] == 0, phase
        assert metadata["binary_sha256"] == receipt["binary_sha256"], phase
        assert metadata["scenario_sha256"] == sha(HERE / "scenarios" / (phase + ".scn")), phase
        report = (directory / "scenario.done").read_text()
        assert report.startswith("RESULT ok\n"), phase
        captures = sorted(directory.glob("*.png"))
        assert len(captures) == metadata["captures"], phase
        assert f"{len(captures)} captured, 0 blank" in report, phase
        for path in captures:
            header = path.read_bytes()[:24]
            assert header[:8] == b"\x89PNG\r\n\x1a\n", path
            assert struct.unpack(">II", header[16:24]) == (2200, 1400), path
            images.append(str(path.relative_to(HERE)))
    assert len(images) == 12
    assert sorted(images) == sorted(receipt["reviewed_captures"])

    seed = load("readback/seed.json")
    assert len(seed) == 2
    assert load("readback/verified.json") == seed
    assert load("readback/verified-after-failure.json") == seed
    assert seed[0]["owner"] != seed[1]["owner"]
    assert seed[0]["personal_relations"]["source_id"] != seed[1]["personal_relations"]["source_id"]
    assert seed[0]["pcm_bits_blake3"] != seed[1]["pcm_bits_blake3"]
    for entry, profile, count, theme, bpm in zip(
        seed, ["default", "d1-b"], [1, 2], ["Ember", "Dusk"], [87.0, 143.0]
    ):
        assert entry["profile"] == profile and len(entry["cards"]) == count
        assert entry["settings"]["theme"] == theme and entry["settings"]["bpm"] == bpm
        assert entry["pcm_frames"] == 2000
        takes = entry["captures"]["takes"]
        assert len(takes) == 1
        take = takes[0]
        assert take["frames"] == 2000 and take["sample_rate"] == 1000
        assert take["frames"] == take["bars"] * take["beats"] * 60 / take["bpm"] * take["sample_rate"]

    checkpoints = {
        "aba/a-before": 0,
        "aba/b-before": 1,
        "aba/a-returned": 0,
        "aba/a-same-persona": 0,
        "reopen-a/reopen-a": 0,
        "reopen-b/reopen-b": 1,
        "incoming-failure/a-before-failure": 0,
        "incoming-failure/a-after-cancel": 0,
    }
    for name, index in checkpoints.items():
        checkpoint = load(f"native/{name}.json")
        assert checkpoint["dirty"] is False and checkpoint["live"] == checkpoint["stored"], name
        state = checkpoint["live"]
        assert state["set"]["cards"] == seed[index]["cards"], name
        assert state["personal_relations"] == seed[index]["personal_relations"], name
        assert state["loop_captures"] == seed[index]["captures"], name
    assert load("native/aba/a-returned.json") == load("native/aba/a-same-persona.json")
    assert load("native/incoming-failure/a-before-failure.json") == load("native/incoming-failure/a-after-cancel.json")

    failure = load("failure-hashes.json")
    assert failure["before"] != failure["damaged"] == failure["after_failure"]
    assert failure["family_still_default"] is True
    status = load("custody/control-status-2.json")["answer"]["Ok"]
    assert status["profile"] == "default" and status["lock"] == "Unlocked"
    status = load("custody/control-status-4.json")["answer"]["Ok"]
    assert status["profile"] == "default" and status["lock"] == "Locked"
    locked = load("locked-hashes.json")
    assert locked["unchanged"] is True and locked["before"] == locked["after"]
    assert len(locked["before"]) == 8
    close = load("native-close-observation.json")
    assert close["receipt_completed_before_action"] is True
    assert close["observed_button"] == "Discard unsaved practice and close"

    core = (HERE / "checks/test-r1.log").read_text()
    host = (HERE / "checks/test-r5.log").read_text()
    assert "328 passed; 0 failed; 0 ignored" in core
    assert "5 passed; 0 failed; 0 ignored" in core
    assert core.count("1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out") == 2
    assert "343 passed; 0 failed; 8 ignored" in host
    assert len(re.findall(r"1 passed; 0 failed; 0 ignored; 0 measured; 350 filtered out", host)) == 5
    assert "Finished `dev` profile" in (HERE / "checks/build-final.log").read_text()
    print("PASS: source/evidence hashes; 5 native processes, 12 captures, 8 exact checkpoints;")
    print("      distinct persona readback, refused incoming store, nonwriting locked custody;")
    print("      678 distinct CPU parent/integration passes, 5 separate nested invocations.")
    print("Visual review is recorded human/agent observation, not inferred by this verifier.")


if __name__ == "__main__":
    verify()
