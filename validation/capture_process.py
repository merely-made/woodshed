#!/usr/bin/env python3
"""Run one bounded command and write process evidence bound to its captured log.

Example: python3 validation/capture_process.py --receipt run.json --log run.log \
  --run-id unique-run-name -- cargo test --locked -p package
The child's real status is returned. Timeout/log-limit failures are recorded as
nonzero outcomes; the helper never fabricates a successful exit.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import selectors
import subprocess
import sys
import time

MAX_LOG_BYTES = 32 * 1024 * 1024
MAX_RUNTIME_SECONDS = 30 * 60


def now():
    return datetime.now(timezone.utc).isoformat().replace('+00:00', 'Z')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--receipt', required=True, type=Path)
    parser.add_argument('--log', required=True, type=Path)
    parser.add_argument('--run-id', required=True, help='Unique human-readable identity for this invocation')
    parser.add_argument('--timeout-seconds', type=int, default=MAX_RUNTIME_SECONDS)
    parser.add_argument('command', nargs=argparse.REMAINDER, help='Command after --')
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ['--'] else args.command
    if not args.run_id.strip() or not command or args.timeout_seconds < 1 or args.timeout_seconds > MAX_RUNTIME_SECONDS:
        parser.error('provide --run-id, a command after --, and timeout from 1 to 1800 seconds')
    started = now()
    args.log.parent.mkdir(parents=True, exist_ok=True)
    args.receipt.parent.mkdir(parents=True, exist_ok=True)
    if args.log.resolve() == args.receipt.resolve():
        parser.error('--log and --receipt must be different paths')
    if args.log.exists() or args.receipt.exists():
        parser.error('refusing to overwrite an existing log or process receipt')
    status = None
    reason = None
    size = 0
    deadline = time.monotonic() + args.timeout_seconds
    created_log = False
    try:
        log_output = args.log.open('xb')
        created_log = True
        receipt_reservation = args.receipt.open('x', encoding='utf-8')
    except FileExistsError:
        if created_log:
            log_output.close()
            args.log.unlink()
        parser.error('refusing to overwrite an existing log or process receipt')
    receipt_reservation.close()
    with log_output as output:
        process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        selector = selectors.DefaultSelector()
        selector.register(process.stdout, selectors.EVENT_READ)
        try:
            while selector.get_map():
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    reason = 'command timeout'
                    process.kill()
                    break
                ready = selector.select(min(remaining, 0.25))
                if not ready and process.poll() is not None:
                    break
                for key, _ in ready:
                    chunk = os.read(key.fd, min(65536, MAX_LOG_BYTES - size + 1))
                    if not chunk:
                        selector.unregister(key.fileobj)
                        continue
                    if size + len(chunk) > MAX_LOG_BYTES:
                        output.write(chunk[:MAX_LOG_BYTES - size])
                        size = MAX_LOG_BYTES
                        reason = 'log limit exceeded'
                        process.kill()
                        break
                    output.write(chunk)
                    size += len(chunk)
                if reason:
                    break
        finally:
            selector.close()
            process.stdout.close()
            remaining = max(0, deadline - time.monotonic())
            try:
                status = process.wait(timeout=remaining)
            except subprocess.TimeoutExpired:
                reason = 'command timeout'
                process.kill()
                status = process.wait()
    ended = now()
    if reason:
        status = 124 if reason == 'command timeout' else 125
    record = {
        'schema_version': 1, 'run_id': args.run_id, 'command': command,
        'started_at': started, 'ended_at': ended, 'exit_code': status,
        'log': str(args.log.resolve()),
        'log_sha256': hashlib.sha256(args.log.read_bytes()).hexdigest(),
    }
    if reason:
        record['capture_error'] = reason
    with args.receipt.open('w', encoding='utf-8') as receipt_file:
        receipt_file.write(json.dumps(record, indent=2) + '\n')
    return status


if __name__ == '__main__':
    raise SystemExit(main())
