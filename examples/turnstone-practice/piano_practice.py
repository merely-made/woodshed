"""Mark an original piano take in Turnstone, detach its playing transport and reopen it in Woodshed."""
import argparse
from contextlib import ExitStack
import hashlib
import json
from pathlib import Path
import secrets
import shutil
import tempfile
import threading
import time
import wave

from piano_take import write_take
from practice import HERE, custody_fixture, lesson_server, woodshed
from receive import Handoff, launch, serve, until, write_result
from wav_preview import WavPreview


def serve_until_closed(stack, server):
    threading.Thread(target=server.serve_forever, daemon=True).start()
    stack.callback(server.server_close)
    stack.callback(server.shutdown)


def read_receipt(producer):
    return json.loads((producer.mailbox / "practice-clip-receipt.json").read_bytes())


def verify_marks(observations, clip):
    marks = {row["marker"]: row for row in observations if "marker" in row}
    for label, field in (("in", "start_ms"), ("out", "end_ms")):
        value = marks.get(label)
        if not value or not value["playing"] or value["position_ms"] // 2000 * 2000 != clip[field]:
            raise RuntimeError(f"The {label} mark was not sampled from the playing WAV: {value}")


def sample_wrap(handoff, label):
    initial = until(handoff.producer.read, "WAV status")["wav_loop"]["completed_wraps"]
    def wrapped():
        status = handoff.producer.read()
        if not status:
            return False
        if status["device_errors"] or status.get("audio_error"):
            raise RuntimeError(f"Woodshed audio output failed: {status}")
        return status["wav_loop"]["completed_wraps"] > initial
    until(wrapped, "WAV callback to cross the out mark", 25)
    return handoff.sample(label, "window", True)


def verify_continuity(points, clip):
    first = points[0]
    reference = first["wav_loop"]
    def elapsed(wav):
        return wav["completed_wraps"] * wav["region_frames"] + wav["region_cursor"]
    for point in points:
        wav = point["wav_loop"]
        duration = (clip["end_ms"] - clip["start_ms"]) * wav["sample_rate"]
        if (not wav["playing"] or not 0 <= wav["region_cursor"] < wav["region_frames"]
                or abs(1000 * wav["region_frames"] - duration) >= 1000
                or (point["pid"], point["stream_id"], wav["stream_epoch"], wav["region_frames"])
                != (first["pid"], first["stream_id"], reference["stream_epoch"], reference["region_frames"])
                or elapsed(wav) - elapsed(reference) != wav["device_frames"] - reference["device_frames"]):
            raise RuntimeError(f"WAV loop lost continuity at {point['stage']}: {wav}")
    if points[-1]["wav_loop"]["completed_wraps"] <= reference["completed_wraps"]:
        raise RuntimeError("No complete wrap observed")


def export_region(source, clip, path):
    with wave.open(str(source), "rb") as take, wave.open(str(path), "wb") as excerpt:
        excerpt.setparams(take.getparams())
        rate = take.getframerate()
        first, last = (clip[field] * rate // 1000 for field in ("start_ms", "end_ms"))
        take.setpos(first)
        excerpt.writeframes(take.readframes(last - first))


def run(args, report):
    report["builds"] = {}
    for binary in (args.woodshed, args.turnstone):
        if binary:
            with binary.open("rb") as executable:
                report["builds"][binary.name] = hashlib.file_digest(executable, "sha256").hexdigest()
    with tempfile.TemporaryDirectory(prefix="woodshed-piano-") as temp, ExitStack() as services:
        root = Path(temp)
        custody = custody_fixture(services, args.keeper, root)
        source = root / "Window Seat.wav"
        write_take(source)
        with ExitStack() as first_session:
            producer = woodshed(first_session, args, root, "first", custody)
            handoff = Handoff(producer, report)
            token = secrets.token_urlsafe(32)
            pane, pane_url = serve(handoff, token)
            serve_until_closed(first_session, pane)
            preview = WavPreview(source)
            first_session.callback(preview.close)
            imported = {}
            lock = threading.Lock()

            def import_clip(clip):
                with lock:
                    if imported:
                        if clip == imported["source"]:
                            return {"transport_url": pane_url}
                        raise ValueError("This practice already has a selection")
                    (producer.mailbox / "practice-clip-request.json").write_text(
                        json.dumps(clip, ensure_ascii=False), encoding="utf-8")
                    shutil.copyfile(source, producer.mailbox / "practice-clip.wav")
                    producer.command("import-wav")
                    saved = read_receipt(producer)
                    if saved["source"] != clip or saved["wav_bytes"] != source.stat().st_size:
                        raise RuntimeError("Imported media or marks differ from the selection")
                    preview.close()
                    producer.command("play-wav")
                    until(lambda: (producer.read().get("wav_loop") or {}).get("playing"), "playing WAV")
                    handoff.sample("standalone", "window", True)
                    imported.update(saved)
                    return {"transport_url": pane_url}

            lesson = lesson_server(import_clip, "piano.html", wav_path=source, preview=preview)
            serve_until_closed(first_session, lesson)
            url = "http://" + lesson.authority + "/piano.html"
            if args.manual:
                print(f"Open {url} in Turnstone's Genet (Bole) viewer. Mark a passage, send it, "
                      "then attach and detach the transport.", flush=True)
                while not handoff.returned:
                    producer.read()
                    if report.get("error"):
                        raise RuntimeError(report["error"])
                    time.sleep(0.2)
            else:
                capture = args.receipt / "turnstone"
                capture.mkdir()
                scenario = root / "piano.scn"
                instructions = (HERE / "piano.scn").read_text(encoding="utf-8")
                instructions += "\n" + (HERE / "piano-detach.scn").read_text(encoding="utf-8")
                scenario.write_text(instructions.replace("${WOODSHED_PIANO_URL}", url), encoding="utf-8")
                env = dict(TURNSTONE_ROOT=str(root / "turnstone"), MERE_ROOT=str(root / "mere"),
                           TURNSTONE_GENET_ENGINE="bole", TURNSTONE_SCENARIO=str(scenario),
                           TURNSTONE_CAPTURE_DIR=str(capture))
                turn = launch(first_session, args.turnstone, env, args.turnstone_root,
                              args.receipt / "turnstone.log", token)
                def finished():
                    error = report.get("error") or lesson.failure
                    if error:
                        raise RuntimeError(error)
                    return (capture / "scenario.done").exists() or turn.poll() is not None
                until(finished, "Turnstone marks and transport detachment", 240)
                done = capture / "scenario.done"
                if not done.exists() or done.read_text().splitlines()[:1] != ["RESULT ok"] or not handoff.returned:
                    raise RuntimeError("Turnstone did not complete the marked loop handoff")
            if not args.manual or until(producer.read, "returned transport")["wav_loop"]["playing"]:
                sample_wrap(handoff, "returned-and-wrapped")
            clip = imported["source"]
            if not args.manual and (clip["title"], clip["start_ms"], clip["end_ms"], clip["note"]) != (
                    "Window Seat", 2000, 10000, "Keep the left hand light on the repeat."):
                raise RuntimeError(f"The four-bar selection changed: {clip}")
            stages = [p["stage"] for p in report["checkpoints"]]
            if stages == ["standalone", "mounted", "detach", "returned-and-wrapped"]:
                verify_continuity(report["checkpoints"], clip)
            elif not args.manual:
                raise RuntimeError("Expected an uninterrupted attach and detach")
            report["preview"] = preview.observations
            if not args.manual:
                verify_marks(preview.observations, clip)
            report["source"] = clip
            report["wav_blake3"] = imported["wav_blake3"]
            take = args.receipt / "practice-clip.wav"
            shutil.copyfile(source, take)
            report["wav_sha256"] = hashlib.sha256(take.read_bytes()).hexdigest()
            (args.receipt / "phrase.json").write_text(json.dumps(imported, indent=2, ensure_ascii=False), encoding="utf-8")
            export_region(take, clip, args.receipt / "marked-phrase.wav")
            # Neither input path survives, and the local web source closes before restart.
            source.unlink()
            (producer.mailbox / "practice-clip.wav").unlink()

        with ExitStack() as restored:
            reopened = woodshed(restored, args, root, "reopened", custody)
            reopened.command("inspect-wav")
            if read_receipt(reopened) != imported or reopened.process.pid == producer.process.pid:
                raise RuntimeError("The restarted application did not recover the same saved WAV phrase")
            reopened.command("play-wav")
            until(lambda: (reopened.read().get("wav_loop") or {}).get("playing"), "reopened WAV playback")
            fresh = {"checkpoints": []}
            restored_handoff = Handoff(reopened, fresh)
            restored_handoff.sample("reopened", "window", True)
            sample_wrap(restored_handoff, "reopened-and-wrapped")
            verify_continuity(fresh["checkpoints"], clip)
            report["reopened"] = fresh["checkpoints"]
        with ExitStack() as portable:
            opened = woodshed(portable, args, root, "portable", custody, data_dir=root / "second-store",
                              arguments=("--open-wav-phrase", str(args.receipt / "phrase.json")))
            opened.command("inspect-wav")
            from_file = read_receipt(opened)
            for field in ("source", "wav_blake3", "wav_bytes"):
                if from_file[field] != imported[field]:
                    raise RuntimeError("Opening the portable phrase changed " + field)
            report["portable_open_pid"] = opened.process.pid
        if not args.manual:
            with ExitStack() as view:
                captures = args.receipt / "woodshed"
                captures.mkdir()
                env = dict(custody, WOODSHED_DATA_DIR=str(root / "practice"),
                           WOODSHED_SCENARIO=str(HERE / "reopen-piano.scn"), WOODSHED_CAPTURE_DIR=str(captures))
                desktop = launch(view, args.woodshed, env, HERE.parents[1], args.receipt / "view.log", "__unused__")
                done = captures / "scenario.done"
                until(lambda: done.exists() or desktop.poll() is not None, "the reopened practice window", 120)
                if not done.exists() or done.read_text().splitlines()[:1] != ["RESULT ok"]:
                    raise RuntimeError("The reopened practice window did not show the saved phrase")
        report["passed"] = True


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("woodshed", "turnstone", "turnstone-root", "receipt", "keeper"):
        parser.add_argument("--" + name, type=lambda value: Path(value).resolve(), required=name in ("woodshed", "receipt"))
    parser.add_argument("--manual", action="store_true", help="mark the passage in your open Turnstone window")
    args = parser.parse_args()
    if not args.manual and not (args.turnstone and args.turnstone_root):
        parser.error("scripted runs require --turnstone and --turnstone-root")
    args.receipt.mkdir(parents=True, exist_ok=False)
    report = {"passed": False, "checkpoints": []}
    try:
        run(args, report)
    except Exception as error:
        report["error"] = str(error)
        raise
    finally:
        write_result(args.receipt, report)
