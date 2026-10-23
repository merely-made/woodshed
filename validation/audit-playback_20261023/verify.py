#!/usr/bin/env python3
"""Verify captured B8 process outcomes and source/evidence identity."""
import hashlib
import json
from pathlib import Path
import re
import subprocess


HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent


def load(relative):
    return json.loads((HERE / relative).read_text())


def sha(data):
    return hashlib.sha256(data).hexdigest()


def git_blob(revision, relative):
    return subprocess.check_output(["git", "show", f"{revision}:{relative}"], cwd=REPO)


def verify():
    receipt = load("receipt.json")
    qualified = receipt["qualified_source_revision"]
    for path, expected in load("evidence-sha256.json").items():
        target = HERE / path
        assert target.resolve().is_relative_to(HERE), path
        assert sha(target.read_bytes()) == expected, ("evidence", path)
    bindings = load("source-sha256.json")
    assert len(bindings) == receipt["source_binding_count"]
    for path, expected in bindings.items():
        target = REPO / path
        assert target.resolve().is_relative_to(REPO), path
        assert sha(target.read_bytes()) == expected, ("current source", path)
        assert sha(git_blob(qualified, path)) == expected, ("qualified source", path)

    counts = {}
    for gate in receipt["positive_gates"] + list(receipt["negative_controls"]):
        record = load(f"checks/{gate}/process.json")
        raw = (HERE / "checks" / gate / "command.log").read_bytes()
        assert sha(raw) == record["log_sha256"], gate
        assert record["source_stable"] is True, gate
        assert record["source_before"] == record["source_after"], gate
        assert record["started_at_utc"] <= record["ended_at_utc"], gate
        text = raw.decode(errors="replace")
        summaries = re.findall(
            r"^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;",
            text, re.MULTILINE,
        )
        if gate in receipt["positive_gates"]:
            assert record["exit_code"] == 0, gate
            assert all(status == "ok" and failed == "0" for status, _, failed, _ in summaries), gate
            assert not re.search(r"^error:", text, re.MULTILINE), gate
            for path, expected in record["source_after"].items():
                assert bindings[path] == expected, (gate, path)
            counts[gate] = (sum(int(row[1]) for row in summaries), sum(int(row[3]) for row in summaries))
        else:
            control = receipt["negative_controls"][gate]
            assert record["exit_code"] == control["expected_exit"], gate
            assert sum(int(row[2]) for row in summaries) == control["failed_cases"], gate
    assert counts["playback-final"] == (42, 7)
    assert counts["receiving-final-r2"] == (125, 1)
    assert counts["desktop-build"] == (0, 0)
    assert "Finished `dev` profile" in (HERE / "checks/desktop-build/command.log").read_text()
    assert counts["format-final"] == (0, 0)
    assert sum(value[0] for value in counts.values()) == receipt["test_accounting"]["distinct_passed"] == 167
    assert sum(value[1] for value in counts.values()) == receipt["test_accounting"]["ignored_existing"] == 8

    cache = (HERE / "checks/baseline-cache/command.log").read_text()
    assert "RefCell already borrowed" in cache
    assert "Playback recovered from an internal audio failure" in cache
    eof = (HERE / "checks/baseline-eof/command.log").read_text()
    assert "left: Idle" in eof and "left: Loading" in eof
    selector = (HERE / "checks/receiving-final/command.log").read_text()
    assert "redshank-genet" in selector and "did not match any packages" in selector

    recheck = load("recheck.json")
    assert recheck["all_original_ledger_rows_retained"] == 97
    for path, expected in recheck["frozen_current_source_sha256"].items():
        assert sha(git_blob(recheck["revision"], path)) == expected, ("recheck", path)
    ledger = json.loads((REPO / "validation/full-source-audit_20261016/current-finding-ledger.json").read_text())
    assert len(ledger["findings"]) == 97
    rows = {row["id"]: row for row in ledger["findings"]}
    assert len(rows) == 97
    for finding in receipt["findings"]:
        assert rows[finding]["implementation_revision"] == qualified, finding
    for path, expected in receipt["review"]["frozen_source_sha256"].items():
        assert bindings[path] == expected, ("review", path)
    artifact = load("desktop-artifact.json")
    assert artifact["qualified_source_revision"] == qualified
    print(f"PASS: {len(bindings)} committed source bindings, process/log/evidence hashes;")
    print("      167 distinct CPU passes, 8 existing ignores; locked desktop build;")
    print("      3 reproduced baseline failures and 1 selector error correctly excluded;")
    print("      frozen-current frontier hashes and all 97 ledger rows retained.")
    print("Scope: CPU/build only; no new native, physical audio or device acceptance.")


if __name__ == "__main__":
    verify()
