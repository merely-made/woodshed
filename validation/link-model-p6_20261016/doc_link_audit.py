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
import os
from pathlib import Path
import re
import subprocess
from urllib.parse import unquote, urlsplit


def scan(repo, names, revision=None):
    findings = []
    checked = 0
    repo = repo.resolve()

    def historical_path(target):
        try:
            return f"{revision}:{target.relative_to(repo).as_posix()}" if revision else None
        except ValueError:
            return None

    for name in names:
        text = (subprocess.check_output(["git", "show", f"{revision}:{name}"], cwd=repo, text=True)
                if revision else (repo / name).read_text())
        document_text = text
        text = re.sub(r"```[\s\S]*?```", lambda match: "\n" * match[0].count("\n"), text)
        for match in re.finditer(r"(?<!!)\[[^\]\n]*\]\((<[^>\n]+>|[^)\n]+)\)", text):
            destination = match[1].strip().strip("<>")
            destination = re.sub(r'\s+"[^"]*"$', "", destination)
            parsed = urlsplit(destination)
            if parsed.scheme or parsed.netloc:
                continue
            target = (repo / name).parent / unquote(parsed.path) if parsed.path else repo / name
            # Historical paths must not follow symlinks in today's worktree.
            target = Path(os.path.abspath(target)) if revision else target.resolve()
            historical = historical_path(target)
            checked += 1
            exists = (subprocess.run(["git", "cat-file", "-e", historical], cwd=repo,
                                     stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0
                      if historical else target.exists())
            if not exists:
                findings.append({"document": name, "target": destination, "reason": "missing path"})
            elif parsed.fragment and target.suffix == ".md":
                if not parsed.path:
                    target_text = document_text
                elif historical:
                    target_text = subprocess.check_output(
                        ["git", "show", historical], cwd=repo, text=True)
                else:
                    target_text = target.read_text()
                anchors = set(re.findall(r'<a\s+(?:id|name)=["\']([^"\']+)', target_text))
                for heading in re.findall(r"^#{1,6}\s+(.+?)\s*#*\s*$", target_text, re.M):
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
