#!/usr/bin/env python3
# Copyright 2026 Mark Alan Boykin
# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.
# SPDX-License-Identifier: MPL-2.0
"""Compare local Markdown link findings in the two changed receiving docs."""
import argparse
from collections import Counter
import json
from pathlib import Path
import re
import subprocess
from urllib.parse import unquote, urlsplit


def scan(repo, names, revision=None):
    findings = []
    checked = 0
    for name in names:
        text = (subprocess.check_output(["git", "show", f"{revision}:{name}"], cwd=repo, text=True)
                if revision else (repo / name).read_text())
        text = re.sub(r"```[\s\S]*?```", lambda match: "\n" * match[0].count("\n"), text)
        for match in re.finditer(r"(?<!!)\[[^\]\n]*\]\((<[^>\n]+>|[^)\n]+)\)", text):
            destination = match[1].strip().strip("<>")
            destination = re.sub(r'\s+"[^"]*"$', "", destination)
            parsed = urlsplit(destination)
            if parsed.scheme or parsed.netloc:
                continue
            target = ((repo / name).parent / unquote(parsed.path)).resolve() if parsed.path else repo / name
            checked += 1
            if not target.exists():
                findings.append({"document": name, "target": destination, "reason": "missing path"})
            elif parsed.fragment and target.suffix == ".md":
                anchors = set(re.findall(r'<a\s+(?:id|name)=["\']([^"\']+)', target.read_text()))
                for heading in re.findall(r"^#{1,6}\s+(.+?)\s*#*\s*$", target.read_text(), re.M):
                    heading = re.sub(r"[`*_]", "", heading).lower()
                    anchors.add(re.sub(r"\s+", "-", re.sub(r"[^\w\s-]", "", heading)))
                if unquote(parsed.fragment) not in anchors:
                    findings.append({"document": name, "target": destination, "reason": "missing heading"})
    return {"checked_local_links": checked, "findings": findings}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path.cwd())
    parser.add_argument("--base", required=True)
    args = parser.parse_args()
    names = ["design_docs/2026-07-11_stage_set_tools_plan.md", "design_docs/DOC_README.md"]
    baseline = scan(args.repo.resolve(), names, args.base)
    current = scan(args.repo.resolve(), names)
    counts = lambda result: Counter(json.dumps(row, sort_keys=True) for row in result["findings"])
    added = list((counts(current) - counts(baseline)).elements())
    print(json.dumps({"scope": names, "baseline_revision": args.base, "baseline": baseline,
                      "current": current, "new_findings": [json.loads(row) for row in added],
                      "no_new_findings": not added}, indent=2))
    raise SystemExit(bool(added))


if __name__ == "__main__":
    main()
