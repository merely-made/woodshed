#!/usr/bin/env python3
"""Verify the retained native evidence without launching apps or using live data."""
import hashlib
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parent


def read(path):
    return json.loads((ROOT / path).read_text())


def main():
    receipt = read("receipt.json")
    for check in receipt["checks"]:
        process = read(check["process"])
        # Paths are deliberately relocated; raw receipts remain unmodified.
        log = (ROOT / check["log"]).read_bytes()
        assert process["command"] == check["command"]
        assert process["exit_code"] == check["exit_code"] == 0
        assert process["started_at"] <= process["ended_at"]
        assert hashlib.sha256(log).hexdigest() == process["log_sha256"]
    final_test = (ROOT / "checks/desktop-final.log").read_text()
    outcomes = re.findall(r"^test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored;", final_test, re.M)
    assert outcomes == [("ok", "92", "0", "1")]
    assert "error:" not in final_test and final_test.endswith("\n")
    accepted = []
    for capture in receipt["captures"]:
        data = (ROOT / capture["path"]).read_bytes()
        assert data.startswith(b"\xff\xd8\xff") and data.endswith(b"\xff\xd9")
        assert hashlib.sha256(data).hexdigest() == capture["sha256"]
        assert capture["reviewed"]
        if capture["phase"] == "committed acceptance":
            accepted.append(capture)
    assert len(accepted) == receipt["accepted_native_capture_count"] == 10
    source = read("checks/final-source-process.json")
    reopen = read("checks/reopen-process.json")
    assert source["ended_at"] < reopen["started_at"]
    assert source["source_commit"] == reopen["source_commit"] == receipt["qualified_source"]
    context = read("observations/final-context.json")
    assert context["binary_sha256"] == receipt["binary_sha256"]
    before = read("observations/copy-before-reopen.json")
    after = read("observations/final-files.json")
    for path, digest in context["before"].items():
        if path.startswith("failed-listener/"):
            assert after["inventory"][path] == digest
    assert "Native Recovery Source.wav" not in after["inventory"]
    assert after["inventory"]["Native Recovery Source.wav.unavailable"] == context["before"]["Native Recovery Source.wav"]
    for key in ["annotations", "library", "selected_item", "queue"]:
        assert before["model"][key] == after["newest_model"][key]
    assert next(iter(before["model"]["annotations"].values()))["body"]["plain_text"] == (
        "Committed native recovery draft: preserve this exact text and the local audio."
    )
    copied = "recovered-session/media/" + Path(before["media"]).name
    assert after["inventory"][copied] == before["media_sha256"] == context["before"]["Native Recovery Source.wav"]
    assert not any(path.startswith("final-missing-media-copy/state-") for path in after["inventory"])
    assert not any("refused-inside-original" in path for path in after["inventory"])
    print("Verified 10 accepted native captures, process exits/order, 92 desktop passes, exact recovered state/media and unchanged original generations.")


if __name__ == "__main__":
    main()
