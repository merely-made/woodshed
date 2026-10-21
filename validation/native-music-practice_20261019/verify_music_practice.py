#!/usr/bin/env python3
"""Verify native Music Practice checkpoints and source/binary custody.

Usage:
  python3 verify_music_practice.py [native-run-dir] [--source-root WORKTREE]

This is a read-only verifier. Missing receipts are reported as incomplete and
cause a nonzero exit, so it can be run again after headed runs finish.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path

DEFAULT_RUN = Path(__file__).parent / "native-run"
DEFAULT_SOURCE = Path("/Users/markik/Code/worktrees/woodshed-arrangement-playback")
CHECKPOINTS = {
    "seed/music-practice-start.json": "start",
    "seed/music-practice-queued.json": "queued",
    "seed/music-practice-paused.json": "paused",
    "seed/music-practice-rest.json": "rest",
    "seed/music-practice-applied.json": "applied",
    "seed/music-practice-retimed.json": "retimed",
    "seed/music-practice-saved.json": "saved",
    "reopen/music-practice-reopened.json": "reopened",
    "reopen/music-practice-repeat.json": "repeat",
}
EXPECTED_ADVANCES = {
    "start": [12000],
    "queued": [12000],
    "paused": [12000, 24000],
    "rest": [12000, 24000, 80000],
    "applied": [12000, 24000, 80000, 29000],
    "retimed": [12000, 24000, 80000, 29000, 2000],
    "saved": [12000, 24000, 80000, 29000, 2000, 2000],
    "reopened": [],
    "repeat": [241000],
}
HEX64 = re.compile(r"^[0-9a-f]{64}$")


class Verify:
    def __init__(self) -> None:
        self.errors: list[str] = []
        self.notes: list[str] = []

    def check(self, condition: bool, message: str) -> None:
        if not condition:
            self.errors.append(message)

    def load(self, path: Path) -> dict | None:
        if not path.is_file():
            self.errors.append(f"missing receipt: {path}")
            return None
        try:
            value = json.loads(path.read_text())
        except Exception as exc:
            self.errors.append(f"invalid JSON in {path}: {exc}")
            return None
        if not isinstance(value, dict):
            self.errors.append(f"receipt root must be an object: {path}")
            return None
        return value


def digest(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def verify_pcm(v: Verify, label: str, receipt: dict) -> None:
    device = receipt.get("device", {})
    pcm = receipt.get("pcm", {})
    v.check(device.get("native_stream_count") == 0,
            f"{label}: expected zero native streams, got {device.get('native_stream_count')!r}")
    frames = pcm.get("frames")
    samples = pcm.get("samples")
    nz = pcm.get("nonzero_samples")
    expected_advances = EXPECTED_ADVANCES[label]
    expected_frames = sum(expected_advances)
    v.check(isinstance(frames, int) and frames == expected_frames,
            f"{label}: expected exactly {expected_frames} PCM frames, got {frames!r}")
    v.check(isinstance(samples, int) and samples == frames * 2,
            f"{label}: expected interleaved stereo sample count == frames*2, got {samples!r}/{frames!r}")
    if expected_frames:
        v.check(isinstance(nz, int) and 0 < nz <= (samples or 0),
                f"{label}: expected nonzero PCM samples within sample count, got {nz!r}/{samples!r}")
    else:
        v.check(nz == 0, f"{label}: expected empty PCM before playback, got {nz!r} nonzero samples")
    h = pcm.get("cumulative_blake3")
    v.check(isinstance(h, str) and bool(HEX64.fullmatch(h)), f"{label}: missing/invalid cumulative PCM BLAKE3")
    advances = pcm.get("advances")
    v.check(isinstance(advances, list) and
            [a.get("requested_frames") for a in advances] == expected_advances,
            f"{label}: expected exact advance frame list {expected_advances}")
    if isinstance(advances, list) and advances:
        requested = 0
        prior_end = None
        for i, advance in enumerate(advances):
            start, end = advance.get("device_frame_start"), advance.get("device_frame_end")
            count = advance.get("requested_frames")
            rendered = advance.get("rendered_samples")
            nonzero = advance.get("nonzero_samples")
            v.check(isinstance(start, int) and isinstance(end, int) and end - start == count,
                    f"{label}: advance {i} has inconsistent frame interval/count")
            if prior_end is not None:
                v.check(start == prior_end, f"{label}: advance {i} is not contiguous with prior interval")
            prior_end = end
            if isinstance(count, int):
                requested += count
            v.check(isinstance(rendered, int) and rendered == (count or 0) * 2,
                    f"{label}: advance {i} rendered sample count is not exact stereo")
            silent = label != "repeat" and i in (1, 5)  # paused, then explicitly stopped
            if silent:
                v.check(nonzero == 0, f"{label}: paused/stopped advance {i} was not silent")
            else:
                v.check(isinstance(nonzero, int) and 0 < nonzero <= (rendered or 0),
                        f"{label}: sounding advance {i} has no nonzero PCM or invalid sample count")
            ah = advance.get("pcm_f32le_blake3")
            v.check(isinstance(ah, str) and bool(HEX64.fullmatch(ah)),
                    f"{label}: advance {i} missing/invalid PCM BLAKE3")
        v.check(requested == frames, f"{label}: requested advance frames {requested} != PCM frames {frames}")
        v.check(sum(a.get("nonzero_samples", 0) for a in advances) == nz,
                f"{label}: per-advance nonzero sample counts do not sum to receipt total")
    v.check(device.get("output_frame") == expected_frames,
            f"{label}: expected device output frame {expected_frames}, got {device.get('output_frame')!r}")


def verify_source_ledger(v: Verify, run: Path, source_root: Path) -> None:
    ledger = v.load(run / "source-before-native.json")
    if ledger is None:
        return
    files = ledger.get("files")
    v.check(isinstance(files, dict) and bool(files), "source ledger has no file hash map")
    changed = []
    for rel, expected in (files or {}).items():
        path = source_root / rel
        if not path.is_file():
            changed.append(f"missing {rel}")
        elif digest(path) != expected:
            changed.append(rel)
    v.check(not changed, "source inputs changed since ledger: " + ", ".join(changed[:20]))
    binary = run / "frozen/woodshed-genet"
    expected_binary = ledger.get("binary_sha256")
    if binary.is_file():
        actual_binary = digest(binary)
        v.check(actual_binary == expected_binary,
                f"frozen native binary hash changed: expected {expected_binary}, got {actual_binary}")
    else:
        v.errors.append(f"missing frozen binary: {binary}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("run_dir", nargs="?", type=Path, default=DEFAULT_RUN)
    parser.add_argument("--source-root", type=Path, default=DEFAULT_SOURCE)
    args = parser.parse_args()
    run = args.run_dir.resolve()
    source_root = args.source_root.resolve()
    v = Verify()
    receipts: dict[str, dict] = {}
    for rel, label in CHECKPOINTS.items():
        receipt = v.load(run / rel)
        if receipt is not None:
            receipts[label] = receipt
            verify_pcm(v, label, receipt)

    start = receipts.get("start")
    queued = receipts.get("queued")
    paused = receipts.get("paused")
    rest = receipts.get("rest")
    applied = receipts.get("applied")
    retimed = receipts.get("retimed")
    saved = receipts.get("saved")
    reopened = receipts.get("reopened")
    repeat = receipts.get("repeat")

    if start and queued:
        sm, qm = start.get("music", {}), queued.get("music", {})
        sp, qp = start.get("playback", {}), queued.get("playback", {})
        v.check(qm.get("applied_canonical_xml_blake3") == sm.get("applied_canonical_xml_blake3"),
                "queued: source applied hash changed before boundary")
        v.check(qp.get("applied_revision") == 0 and qp.get("desired_revision") == 1 and qp.get("pending_update") is True,
                "queued: expected applied revision 0, desired revision 1, pending update")
        v.check(qm.get("draft_canonical_xml_blake3") != qm.get("saved_exact_xml_blake3"),
                "queued: expected edited draft to remain distinct from saved source")
        v.check(qm.get("saved_exact_xml_blake3") == qm.get("stored_exact_xml_blake3"),
                "queued: unapplied edit unexpectedly changed saved storage")
        v.check(qp.get("bar") == sp.get("bar") and qp.get("bar_beat_ticks") == sp.get("bar_beat_ticks"),
                "queued: checkpoint position changed without a clock advance")

    if paused and queued:
        p, q = paused.get("playback", {}), queued.get("playback", {})
        v.check(p.get("status") == "paused", "paused: practice status is not paused")
        v.check((p.get("bar"), p.get("bar_beat_ticks"), p.get("applied_revision")) ==
                (q.get("bar"), q.get("bar_beat_ticks"), q.get("applied_revision")),
                "paused: bar/tick/revision moved while paused")
    if rest:
        v.check(rest.get("playback", {}).get("rest") is True, "rest: authored rest was not observed")
    if applied:
        p, m = applied.get("playback", {}), applied.get("music", {})
        v.check(p.get("bar") == 1 and p.get("applied_revision") == 1 and
                p.get("desired_revision") == 1 and p.get("pending_update") is False,
                "applied: expected bar 1 and revision 1 fully applied")
        v.check(m.get("applied_canonical_xml_blake3") == m.get("draft_canonical_xml_blake3"),
                "applied: applied XML hash does not match edited draft")
    if retimed and applied:
        rm, am = retimed.get("music", {}), applied.get("music", {})
        rp, ap = retimed.get("playback", {}), applied.get("playback", {})
        v.check(rm.get("draft_canonical_xml_blake3") == am.get("draft_canonical_xml_blake3"),
                "retimed: tempo change altered the authored draft XML")
        v.check(rp.get("bpm") != ap.get("bpm"), "retimed: practice BPM did not change")
    if saved and retimed:
        sm, rm = saved.get("music", {}), retimed.get("music", {})
        v.check(sm.get("draft_dirty") is False, "saved: draft remains dirty")
        v.check(sm.get("stored_exact_xml_blake3") == sm.get("saved_exact_xml_blake3"),
                "saved: stored MusicXML differs from saved exact MusicXML")
        v.check(sm.get("saved_exact_xml_blake3") == rm.get("draft_canonical_xml_blake3"),
                "saved: saved XML hash differs from the authored draft")
    if reopened and saved:
        rm, sm = reopened.get("music", {}), saved.get("music", {})
        rp = reopened.get("playback", {})
        v.check(rm.get("stored_exact_xml_blake3") == sm.get("stored_exact_xml_blake3"),
                "reopened: exact stored MusicXML hash differs from save")
        v.check(rp.get("status") == "stopped" and rp.get("bar") is None,
                "reopened: practice should be stopped with no playhead")
    if repeat:
        p = repeat.get("playback", {})
        v.check(p.get("status") == "playing" and p.get("bar") == 0,
                "repeat: expected active playback wrapped back to bar 0")

    verify_source_ledger(v, run, source_root)
    print(f"run_dir: {run}")
    print(f"source_root: {source_root}")
    print(f"checkpoints: {', '.join(receipts) if receipts else '(none)'}")
    if v.errors:
        print(f"FAIL ({len(v.errors)} issue(s))")
        for issue in v.errors:
            print(f"- {issue}")
        return 1
    print("PASS: practice checkpoints, PCM receipts, frozen binary hash, and source input ledger match.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
