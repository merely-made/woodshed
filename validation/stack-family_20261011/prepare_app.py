#!/usr/bin/env python3
"""Prepare a private scenario app bundle; launch it only through native control.

No application is launched by this helper. The wrapper redirects logs and
supplies explicit fixture environment before executing a copy of the product.
"""
import argparse
import hashlib
import json
from pathlib import Path
import plistlib
import shlex
import shutil

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("config", type=Path)
options = parser.parse_args()
config = json.loads(options.config.read_text())
output = Path(config["output"])
output.mkdir(parents=True, exist_ok=False)
app = output / (config["name"] + ".app")
macos = app / "Contents/MacOS"
macos.mkdir(parents=True)
binary = macos / "product"
shutil.copy2(config["binary"], binary)
prefix = config["prefix"]
environment = dict(config["env"])
environment.update({prefix + "_SCENARIO": config["scenario"],
                    prefix + "_CAPTURE_DIR": str(output),
                    prefix + "_RECEIPT": str(output / "scenario.done")})
wrapper = macos / "launch"
wrapper.write_text("#!/bin/sh\n" + "cd " + shlex.quote(str(output)) + "\n" +
                   "exec /usr/bin/env " +
                   " ".join(shlex.quote(k + "=" + str(v)) for k, v in environment.items()) +
                   " " + shlex.quote(str(binary)) +
                   " > " + shlex.quote(str(output / "native.log")) + " 2>&1\n")
wrapper.chmod(0o755)
with (app / "Contents/Info.plist").open("wb") as f:
    plistlib.dump({"CFBundleIdentifier": "dev.woodshed.stack.20261011." + config["name"].lower(),
                  "CFBundleName": config["name"], "CFBundleExecutable": "launch",
                  "CFBundlePackageType": "APPL", "CFBundleVersion": "1",
                  "NSHighResolutionCapable": True}, f)
record = dict(config)
record.update({"app": str(app),
               "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
               "scenario_sha256": hashlib.sha256(Path(config["scenario"]).read_bytes()).hexdigest(),
               "launch_method": "native control of prepared app bundle"})
(output / "launch.json").write_text(json.dumps(record, indent=2) + "\n")
print(app)
