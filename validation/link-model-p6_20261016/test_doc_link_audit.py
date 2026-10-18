"""Regression coverage for historical and current Markdown link resolution."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest


AUDITOR = Path(__file__).resolve().with_name("doc_link_audit.py")
STAGE = "design_docs/2026-07-11_stage_set_tools_plan.md"
README = "design_docs/DOC_README.md"


class HistoricalTargetTests(unittest.TestCase):
    def run_audit(self, changes=None, stage=None, readme=None, target=None, prepare=None):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            docs = repo / "design_docs"
            docs.mkdir()
            (docs / Path(STAGE).name).write_text(stage or "# Stage\n")
            (docs / Path(README).name).write_text(readme or "# Docs\n")
            if target is not None:
                (docs / "target.md").write_text(target)
            if prepare:
                prepare(docs)
            subprocess.run(["git", "init", "-q"], cwd=repo, check=True)
            subprocess.run(["git", "config", "user.email", "audit@example.invalid"], cwd=repo, check=True)
            subprocess.run(["git", "config", "user.name", "Audit Fixture"], cwd=repo, check=True)
            subprocess.run(["git", "add", "."], cwd=repo, check=True)
            subprocess.run(["git", "commit", "-qm", "baseline"], cwd=repo, check=True)
            baseline = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip()

            if changes:
                changes(repo / "design_docs")
                subprocess.run(["git", "add", "-A"], cwd=repo, check=True)
                subprocess.run(["git", "commit", "-qm", "current"], cwd=repo, check=True)
            result = subprocess.run(
                ["python3", str(AUDITOR), "--repo", str(repo), "--base", baseline],
                cwd=repo, text=True, capture_output=True)
            self.assertIn(result.returncode, (0, 1), result.stderr)
            return result.returncode, json.loads(result.stdout)

    def test_deleted_heading_is_new_finding_and_same_document_uses_baseline_text(self):
        def remove_heading(docs):
            path = docs / Path(STAGE).name
            path.write_text("# Stage\n\n[local](#kept-heading)\n")

        code, output = self.run_audit(
            stage="# Stage\n\n[local](#kept-heading)\n\n## Kept Heading\n",
            readme="# Docs\n",
            changes=remove_heading)
        self.assertEqual(code, 1)
        self.assertEqual(output["baseline"]["findings"], [])
        self.assertEqual(output["new_findings"][0]["reason"], "missing heading")

    def test_renamed_heading_is_new_finding(self):
        def rename_heading(docs):
            (docs / Path(STAGE).name).write_text("# Stage\n\n## New Name\n")

        code, output = self.run_audit(
            stage="# Stage\n\n## Old Name\n",
            readme="# Docs\n\n[stage](2026-07-11_stage_set_tools_plan.md#old-name)\n",
            changes=rename_heading)
        self.assertEqual(code, 1)
        self.assertEqual(output["baseline"]["findings"], [])
        self.assertEqual(output["new_findings"][0]["reason"], "missing heading")

    def test_deleted_target_file_is_new_finding(self):
        def delete_target(docs):
            (docs / "target.md").unlink()

        code, output = self.run_audit(
            readme="# Docs\n\n[target](target.md)\n", target="# Target\n", changes=delete_target)
        self.assertEqual(code, 1)
        self.assertEqual(output["baseline"]["findings"], [])
        self.assertEqual(output["new_findings"][0]["reason"], "missing path")

    def test_baseline_existing_broken_link_is_not_new(self):
        code, output = self.run_audit(readme="# Docs\n\n[missing](absent.md)\n")
        self.assertEqual(code, 0)
        self.assertEqual(len(output["baseline"]["findings"]), 1)
        self.assertEqual(output["new_findings"], [])

    def test_unchanged_valid_control_has_no_findings(self):
        code, output = self.run_audit(
            stage="# Stage\n\n## Kept Heading\n",
            readme="# Docs\n\n[stage](2026-07-11_stage_set_tools_plan.md#kept-heading)\n")
        self.assertEqual(code, 0)
        self.assertEqual(output["baseline"]["findings"], [])
        self.assertEqual(output["current"]["findings"], [])
        self.assertEqual(output["new_findings"], [])

    def test_valid_binary_links_do_not_require_text_decoding(self):
        def prepare(docs):
            for name in ("image.png", "document.pdf", "audio.wav"):
                (docs / name).write_bytes(b"\x89\xff\x00\xfe")

        code, output = self.run_audit(
            readme="# Docs\n[png](image.png)\n[pdf](document.pdf)\n[wav](audio.wav)\n",
            prepare=prepare)
        self.assertEqual(code, 0)
        for tree in ("baseline", "current"):
            self.assertEqual(output[tree]["checked_local_links"], 3)
            self.assertEqual(output[tree]["findings"], [])

    def test_baseline_path_does_not_follow_current_symlink(self):
        def replace_with_symlink(docs):
            (docs / "target.md").unlink()
            (docs / "replacement.md").write_text("# Renamed\n")
            (docs / "target.md").symlink_to("replacement.md")

        code, output = self.run_audit(
            readme="# Docs\n[target](target.md#original)\n", target="# Original\n",
            changes=replace_with_symlink)
        self.assertEqual(code, 1)
        self.assertEqual(output["baseline"]["findings"], [])
        self.assertEqual(output["new_findings"][0]["reason"], "missing heading")


if __name__ == "__main__":
    unittest.main()
