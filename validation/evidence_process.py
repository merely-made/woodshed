"""Small fail-closed process/log receipt checks used by scoped validators."""
import hashlib
import json
import re
from pathlib import Path
from datetime import datetime

SUMMARY = re.compile(r"^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored\b", re.M)
RUNNING = re.compile(r"^running (\d+) tests?$", re.M)


def validate_process(receipt_path, *, run_id, command, log_path):
    """Require observed process evidence bound to this command and exact log."""
    receipt_path = Path(receipt_path)
    if not receipt_path.is_file():
        raise ValueError(f"missing process receipt: {receipt_path}")
    receipt = json.loads(receipt_path.read_text())
    log_path = Path(log_path)
    if receipt.get("run_id") != run_id or not run_id:
        raise ValueError("process receipt run_id mismatch")
    if receipt.get("command") != command or not command:
        raise ValueError("process receipt command mismatch")
    if not receipt.get("log") or Path(receipt["log"]).resolve() != log_path.resolve():
        raise ValueError("process receipt log identity mismatch")
    if not receipt.get("started_at") or not receipt.get("ended_at"):
        raise ValueError("process receipt lacks start/end timestamps")
    try:
        started = datetime.fromisoformat(receipt["started_at"].replace("Z", "+00:00"))
        ended = datetime.fromisoformat(receipt["ended_at"].replace("Z", "+00:00"))
    except (TypeError, ValueError) as error:
        raise ValueError("process receipt timestamps are invalid") from error
    if started.tzinfo is None or ended.tzinfo is None or ended < started:
        raise ValueError("process receipt timestamps are unordered or lack timezone")
    if type(receipt.get("exit_code")) is not int or receipt["exit_code"] != 0:
        raise ValueError(f"process exit was not zero: {receipt.get('exit_code')!r}")
    if not log_path.is_file():
        raise ValueError(f"missing test log: {log_path}")
    data = log_path.read_bytes()
    if receipt.get("log_sha256") != hashlib.sha256(data).hexdigest():
        raise ValueError("process receipt log digest mismatch")
    return receipt, data


def validate_run(receipt_path, *, run_id, command, log_path):
    receipt, data = validate_process(receipt_path, run_id=run_id, command=command, log_path=log_path)
    text = data.decode("utf-8", errors="strict")
    if not text.endswith("\n") or "unexpected EOF" in text or "error:" in text:
        raise ValueError("truncated or errored test log")
    matches = list(SUMMARY.finditer(text))
    # Every Cargo test-result line must parse, and every parsed outcome must pass.
    result_lines = [line for line in text.splitlines() if line.startswith("test result:")]
    if len(matches) != len(result_lines) or not matches:
        raise ValueError("missing, malformed, or truncated test outcomes")
    if any(m.group(1) != "ok" or int(m.group(3)) != 0 for m in matches):
        raise ValueError("test log contains a failed suite")
    open_suites = []
    match_by_line = {text.count("\n", 0, match.start()) + 1: match for match in matches}
    for line_number, line in enumerate(text.splitlines(), start=1):
        running_match = RUNNING.match(line)
        if running_match:
            open_suites.append(int(running_match.group(1)))
        outcome = match_by_line.get(line_number)
        if outcome:
            if not open_suites:
                raise ValueError("test outcome has no matching running suite")
            count = open_suites.pop()
            passed, failed, ignored = (int(outcome.group(index)) for index in (2, 3, 4))
            if passed + failed + ignored != count:
                raise ValueError("test outcome count does not close its running suite")
    if open_suites:
        raise ValueError("test log has a started suite without a matching outcome")
    return receipt, matches, text
