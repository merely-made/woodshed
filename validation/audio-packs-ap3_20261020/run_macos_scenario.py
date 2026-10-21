#!/usr/bin/env python3
# Copyright 2026 Mark Alan Boykin
# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.
# SPDX-License-Identifier: MPL-2.0

"""Run a Mesquite native scenario through macOS LaunchServices.

The temporary application bundle launches the real binary without modifying
its host or rendering path. Product profile isolation is supplied with --env.
The Mesquite receipt, rather than open's exit status, determines success.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
import tempfile
import time
import uuid


def owns_executable(process: str, executable: Path) -> bool:
    """Match this temporary product, including macOS's canonical /private path.

    The generated executable path has no spaces. Compare only its complete
    path or the same path followed by arguments, never another bundle's name.
    """
    paths = {str(executable), str(executable.resolve())}
    return any(process == path or process.startswith(path + " ") for path in paths)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--prefix", required=True, help="Mesquite product environment prefix")
    parser.add_argument("--scenario", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True, help="New receipt directory")
    parser.add_argument("--env", action="append", default=[], metavar="KEY=VALUE")
    parser.add_argument("--timeout", type=float, default=60)
    parser.add_argument("args", nargs=argparse.REMAINDER, help="Product arguments after --")
    options = parser.parse_args()
    if sys.platform != "darwin":
        parser.error("this launcher requires macOS LaunchServices")
    binary = options.binary.resolve(strict=True)
    scenario = options.scenario.resolve(strict=True)
    if not os.access(binary, os.X_OK) or options.timeout <= 0:
        parser.error("binary must be executable and timeout must be positive")
    environment = {}
    for assignment in options.env:
        key, separator, value = assignment.partition("=")
        if not separator or not key:
            parser.error("--env requires KEY=VALUE")
        environment[key] = value
    output = options.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    receipt = output / "scenario.done"
    environment.update({
        options.prefix + "_SCENARIO": str(scenario),
        options.prefix + "_CAPTURE_DIR": str(output),
        options.prefix + "_RECEIPT": str(receipt),
    })
    arguments = options.args[1:] if options.args[:1] == ["--"] else options.args
    started = time.monotonic()
    timed_out = False
    with tempfile.TemporaryDirectory(prefix="mesquite-native-") as scratch:
        app = Path(scratch) / "MesquiteAcceptance.app"
        executable_name = "product-" + uuid.uuid4().hex[:12]
        executable = app / "Contents/MacOS" / executable_name
        executable.parent.mkdir(parents=True)
        # A copy keeps a concurrent rebuild from replacing the launched image.
        shutil.copy2(binary, executable)
        binary_hash = hashlib.sha256(executable.read_bytes()).hexdigest()
        with (app / "Contents/Info.plist").open("wb") as file:
            plistlib.dump({
                "CFBundleIdentifier": "made.merely.mesquite.acceptance." + uuid.uuid4().hex,
                "CFBundleName": "Mesquite Native Acceptance",
                "CFBundleExecutable": executable_name,
                "CFBundlePackageType": "APPL",
                "CFBundleVersion": "1",
                "NSHighResolutionCapable": True,
            }, file)
        command = ["/usr/bin/open", "-n", "-W", "--stdout", str(output / "native.log"),
                   "--stderr", str(output / "native.log")]
        for key, value in environment.items():
            command += ["--env", key + "=" + value]
        command += [str(app), "--args", *arguments]
        launch = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        try:
            stdout, stderr = launch.communicate(timeout=options.timeout)
        except subprocess.TimeoutExpired:
            timed_out = True
            # Terminate only this runner's unique executable, not other apps.
            listing = subprocess.check_output(["ps", "-axo", "pid=,command="], text=True)
            owned = []
            for line in listing.splitlines():
                pid, _, process = line.strip().partition(" ")
                process = process.lstrip()
                if owns_executable(process, executable):
                    owned.append(int(pid))
                    try:
                        os.kill(int(pid), 15)
                    except ProcessLookupError:
                        pass
            try:
                stdout, stderr = launch.communicate(timeout=3)
            except subprocess.TimeoutExpired:
                for pid in owned:
                    try:
                        os.kill(pid, 9)
                    except ProcessLookupError:
                        pass
                launch.terminate()
                stdout, stderr = launch.communicate(timeout=3)
    (output / "launcher.log").write_text(stdout + stderr)
    result = receipt.read_text() if receipt.exists() else ""
    success = not timed_out and launch.returncode == 0 and result.splitlines()[:1] == ["RESULT ok"]
    observation = {
        "launch_method": "LaunchServices temporary application bundle",
        "launcher_exit": launch.returncode,
        "timeout": timed_out,
        "success": success,
        "elapsed_seconds": round(time.monotonic() - started, 3),
        "binary_sha256": binary_hash,
        "scenario_sha256": hashlib.sha256(scenario.read_bytes()).hexdigest(),
        "receipt": receipt.name if receipt.exists() else None,
        "captures": len(list(output.glob("*.png"))),
    }
    (output / "process.json").write_text(json.dumps(observation, indent=2) + "\n")
    print(json.dumps(observation), flush=True)
    print(result, end="" if result.endswith("\n") else "\n", flush=True)
    return 0 if success else 1


if __name__ == "__main__":
    raise SystemExit(main())
