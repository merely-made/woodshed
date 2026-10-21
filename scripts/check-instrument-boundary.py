#!/usr/bin/env python3
"""Report the selected normal/build dependency closure; reject private adapters.

This is an artifact-composition check, not a license or hardware verdict.
Cargo.lock and metadata's full workspace package list also contain unselected
optional dependencies, so neither is a valid substitute for walking the graph.
"""

import argparse
import json
import os
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest-path", type=Path, required=True)
    parser.add_argument("--package", required=True)
    parser.add_argument("--target", required=True)
    parser.add_argument("--no-default-features", action="store_true")
    parser.add_argument("--features")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    command = [
        os.environ.get("CARGO", "cargo"), "metadata", "--locked", "--offline",
        "--format-version", "1", "--manifest-path", str(args.manifest_path.resolve()),
        "--filter-platform", args.target,
    ]
    if args.no_default_features:
        command.append("--no-default-features")
    if args.features:
        command.extend(["--features", args.features])
    result = subprocess.run(command, check=True, capture_output=True, text=True)
    metadata = json.loads(result.stdout)
    packages = {p["id"]: p for p in metadata["packages"]}
    nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
    roots = [p["id"] for p in packages.values() if p["name"] == args.package]
    if len(roots) != 1:
        raise SystemExit(f"Expected one {args.package} package, found {len(roots)}")
    pending = roots[:]
    selected = set()
    while pending:
        package_id = pending.pop()
        if package_id in selected:
            continue
        selected.add(package_id)
        for dep in nodes[package_id]["deps"]:
            if any(kind["kind"] in (None, "build") for kind in dep["dep_kinds"]):
                pending.append(dep["pkg"])
    private_names = {"ringdown", "ringdown-client", "ringdown-ble", "btleplug"}
    private = sorted({packages[p]["name"] for p in selected} & private_names)
    report = {
        "package": args.package,
        "target": args.target,
        "default_features": not args.no_default_features,
        "requested_features": args.features,
        "scope": "Selected normal/build dependencies, excluding dev dependencies",
        "packages": sorted([
            {"name": packages[p]["name"], "version": packages[p]["version"],
             "features": nodes[p]["features"], "source": packages[p]["source"]}
            for p in selected
        ], key=lambda p: (p["name"], p["version"])),
        "private_dependencies": private,
        "passed": not private,
        "limits": "Dependency composition only; no license clearance, device or platform execution",
    }
    encoded = json.dumps(report, indent=2) + "\n"
    if args.output:
        args.output.write_text(encoded)
    else:
        print(encoded, end="")
    if private:
        raise SystemExit("Private dependencies selected: " + ", ".join(private))


if __name__ == "__main__":
    main()
