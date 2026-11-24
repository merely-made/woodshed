"""Receive a Turnstone selection in Woodshed, close the source, and reopen the practice."""
import argparse
from contextlib import ExitStack
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import threading
import time
import uuid
import xml.etree.ElementTree as ET

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "turnstone-transport"))
from receive import Producer, launch, until, write_result


def lesson_server(import_clip, page_name="lesson.html", *, wav_path=None, preview=None):
    class Lesson(BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass

        def reply(self, content, content_type):
            self.send_response(200)
            self.send_header("Content-Type", content_type)
            self.send_header("Cache-Control", "no-store")
            self.send_header("Content-Length", str(len(content)))
            self.end_headers()
            self.wfile.write(content)

        def do_GET(self):
            if self.headers.get("Host") != self.server.authority:
                return self.send_error(404)
            if self.path == "/" + page_name:
                return self.reply((HERE / page_name).read_bytes(), "text/html; charset=utf-8")
            if wav_path is not None and self.path == "/piano.wav":
                return self.send_wav()
            self.send_error(404)

        def send_wav(self):
            size = wav_path.stat().st_size
            start, end = 0, size - 1
            header = self.headers.get("Range")
            if header:
                match = re.fullmatch(r"bytes=(\d*)-(\d*)", header)
                if match and any(match.groups()):
                    first, last = match.groups()
                    if first:
                        start = int(first)
                        end = min(int(last), end) if last else end
                    else:
                        start = max(0, size - int(last))
                else:
                    start = size
                if start > end:
                    self.send_response(416)
                    self.send_header("Content-Range", f"bytes */{size}")
                    self.end_headers()
                    return
            self.send_response(206 if header else 200)
            self.send_header("Content-Type", "audio/wav")
            self.send_header("Accept-Ranges", "bytes")
            self.send_header("Content-Length", str(end - start + 1))
            if header:
                self.send_header("Content-Range", f"bytes {start}-{end}/{size}")
            self.end_headers()
            with wav_path.open("rb") as wav:
                wav.seek(start)
                try:
                    self.wfile.write(wav.read(end - start + 1))
                except (BrokenPipeError, ConnectionResetError):
                    pass

        def do_POST(self):
            origin = "http://" + self.server.authority
            is_preview = preview is not None and self.path == "/preview"
            if ((self.path != "/clip" and not is_preview) or self.headers.get("Host") != self.server.authority
                    or self.headers.get("Origin") not in (None, origin)
                    or self.headers.get_content_type() != "application/json"):
                return self.send_error(403)
            try:
                length = int(self.headers.get("Content-Length", "0"))
                if not 0 < length <= 16_384:
                    return self.send_error(413)
                clip = json.loads(self.rfile.read(length))
                if is_preview and isinstance(clip, dict):
                    return self.reply(json.dumps(preview(clip)).encode(), "application/json")
                if not isinstance(clip, dict) or clip.get("source_page_url") != origin + "/" + page_name:
                    return self.send_error(400)
                result = import_clip(clip)
            except (ValueError, RuntimeError, OSError) as error:
                if not is_preview:
                    self.server.failure = str(error)
                return self.send_error(422)
            self.reply(json.dumps({"ok": True, **(result or {})}).encode(), "application/json")

    server = ThreadingHTTPServer(("127.0.0.1", 0), Lesson)
    server.authority = f"127.0.0.1:{server.server_port}"
    server.failure = None
    return server


def custody_fixture(stack, binary, root):
    if binary is None:
        return {}
    endpoint = r"\\.\pipe\woodshed-practice-" + uuid.uuid4().hex
    directory = root / "keeper"
    child = subprocess.Popen([str(binary), endpoint, str(directory), "woodshed-practice"],
                             stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    def close():
        if child.poll() is None:
            (directory / "stop").touch()
            try:
                child.wait(timeout=10)
            except subprocess.TimeoutExpired:
                child.terminate()
                child.wait()
    stack.callback(close)
    def ready():
        if child.poll() is not None:
            raise RuntimeError(f"custody fixture exited with {child.returncode}")
        return (directory / "ready").exists()
    until(ready, "isolated custody fixture", 45)
    return {"GRAPHSHELL_APP_ENDPOINT": endpoint}


def woodshed(stack, args, root, instance, custody, *, data_dir=None, arguments=()):
    mailbox = root / instance
    mailbox.mkdir()
    env = dict(custody, WOODSHED_DATA_DIR=str(data_dir or root / "practice"), WOODSHED_TRANSPORT_DIR=str(mailbox))
    child = launch(stack, args.woodshed, env, HERE.parents[1], args.receipt / (instance + ".log"), "__unused__", arguments)
    producer = Producer(mailbox, child)
    until(producer.read, instance + " desktop startup", 120)
    return producer


def check_score(clip, receipt):
    if receipt["source"] != clip or receipt["title"] != clip["title"]:
        raise RuntimeError("saved source or title differs from the selection")
    score = ET.fromstring(receipt["musicxml"])
    measures = score.findall("./part/measure")
    if len(measures) != 2:
        raise RuntimeError("expected two measures")
    # Verify pitches' string/fret annotations and explicit eighths against the browser selection.
    rows = [line.strip().split("|")[1:3] for line in clip["selected_text"].strip().splitlines()]
    if len(rows) != 6:
        raise RuntimeError("expected six selected guitar strings")
    for i, measure in enumerate(measures):
        columns = list(zip(*(row[i].split() for row in rows), strict=True))
        notes = [n for n in measure.findall("note") if n.findtext("staff") == "2"]
        if len(columns) != 8 or len(notes) != 8:
            raise RuntimeError("expected eight written eighth notes or rests per bar")
        for column, note in zip(columns, notes):
            expected = [(s, int(fret)) for s, fret in enumerate(column, 1) if fret != "-"]
            if note.findtext("type") != "eighth":
                raise RuntimeError("written duration differs from the selected grid")
            if note.find("rest") is not None:
                actual = []
            else:
                technical = note.find("./notations/technical")
                actual = [(int(technical.findtext("string")), int(technical.findtext("fret")))]
            if expected != actual:
                raise RuntimeError(f"string/fret mismatch: {expected} vs {actual}")
    if float(score.find("./part/measure/direction/sound").attrib["tempo"]) != clip["tempo_bpm"]:
        raise RuntimeError("tempo changed during import")


def check_listening(selected, receipt, *, scripted):
    if receipt.get("source") != selected:
        raise RuntimeError("Woodshed did not preserve the selected listening reference")
    if not scripted:
        return
    expected = {
        "title": "Ben Folds – Fired",
        "listening_url": "https://www.youtube.com/watch?v=7_xQ4o796H0",
        "start_ms": 72000,
        "end_ms": 76500,
        "note": "Match the pickup and keep the left hand even.",
    }
    if any(selected.get(key) != value for key, value in expected.items()):
        raise RuntimeError(f"the marked range or note did not reach Woodshed: {selected}")


def observe_score_loop(producer):
    producer.command("play-practice")
    bars = []
    def observed_loop():
        state = producer.read()
        music = state.get("music_practice", {})
        bar = music.get("bar")
        if music.get("program_status", "").lower() == "playing" and bar is not None:
            if state["streams"] != 1 or state["device_errors"] or state.get("audio_error"):
                raise RuntimeError("Woodshed audio output reported a failure")
            if not bars or bars[-1]["bar"] != bar:
                bars.append(dict(bar=bar, device_frames=state["device_frames"], stream=state["stream_id"],
                                 run=music["run_epoch"]))
        return len(bars) >= 3
    until(observed_loop, "the reopened score to play both bars and loop", 30)
    if (bars[0]["bar"] != bars[2]["bar"] or len({b["run"] for b in bars}) != 1
            or len({b["stream"] for b in bars}) != 1
            or not bars[0]["device_frames"] < bars[1]["device_frames"] < bars[2]["device_frames"]):
        raise RuntimeError(f"unexpected loop observations {bars}")
    return bars


def run(args, report):
    kind = "listening" if args.listening else "practice"
    page_name = "listening.html" if args.listening else "lesson.html"
    scenario_name = "listening.scn" if args.listening else "turnstone.scn"
    placeholder = "${WOODSHED_LISTENING_URL}" if args.listening else "${WOODSHED_LESSON_URL}"
    with tempfile.TemporaryDirectory(prefix="woodshed-practice-") as temp, ExitStack() as services:
        root = Path(temp)
        custody = custody_fixture(services, args.keeper, root)
        with ExitStack() as source:
            first = woodshed(source, args, root, "first", custody)
            lock = threading.Lock()
            selected = {}

            def import_clip(clip):
                with lock:
                    if selected:
                        raise ValueError("one selection per run")
                    path = first.mailbox / "practice-clip-request.json"
                    path.write_text(json.dumps(clip, ensure_ascii=False, separators=(",", ":")), encoding="utf-8")
                    first.command("import-" + kind)
                    selected.update(clip)

            server = lesson_server(import_clip, page_name)
            source.callback(server.server_close)
            page = "http://" + server.authority + "/" + page_name
            threading.Thread(target=server.serve_forever, daemon=True).start()
            source.callback(server.shutdown)
            if args.mark_listening:
                print(f"Open {page} in Turnstone's Genet (Bole) viewer. "
                      "Set the in and out times, enter your note, and choose Practice this phrase.", flush=True)
                while not selected:
                    first.read()
                    if server.failure:
                        print(server.failure, file=sys.stderr, flush=True)
                        server.failure = None
                    time.sleep(0.25)
            else:
                captures = args.receipt / "turnstone"
                captures.mkdir()
                scenario = root / "lesson.scn"
                scenario.write_text((HERE / scenario_name).read_text().replace(placeholder, page))
                env = dict(TURNSTONE_ROOT=str(root / "turnstone"), MERE_ROOT=str(root / "mere"),
                           TURNSTONE_CAPTURE_DIR=str(captures), TURNSTONE_SCENARIO=str(scenario))
                turn = launch(source, args.turnstone, env, args.turnstone_root, args.receipt / "turnstone.log", "__unused__")
                def finished():
                    if server.failure:
                        raise RuntimeError(server.failure)
                    return turn.poll() is not None or (captures / "scenario.done").is_file()
                until(finished, "Turnstone selection", 240)
                done = captures / "scenario.done"
                if not done.exists() or done.read_text().splitlines()[:1] != ["RESULT ok"] or not selected:
                    raise RuntimeError("Turnstone did not deliver the selection")
            initial = json.loads((first.mailbox / "practice-clip-receipt.json").read_bytes())
            if args.listening:
                check_listening(selected, initial, scripted=not args.mark_listening)
            else:
                check_score(selected, initial)
                (args.receipt / "study.musicxml").write_bytes(initial["musicxml"].encode("utf-8"))
            portable = args.receipt / ("fired.listening.json" if args.listening else "selection.json")
            portable.write_text(json.dumps(selected, ensure_ascii=False, separators=(",", ":")), encoding="utf-8")
            report["imported_pid"] = first.process.pid

        # Separate process, fresh mailbox, same Woodshed practice store. The source server is closed.
        with ExitStack() as reopened:
            second = woodshed(reopened, args, root, "second", custody)
            second.command("inspect-" + kind)
            saved = json.loads((second.mailbox / "practice-clip-receipt.json").read_bytes())
            if saved != initial:
                raise RuntimeError("practice changed after closing both applications and reopening")
            report.update(reopened_pid=second.process.pid, title=selected["title"])
            if args.listening:
                report.update(start_ms=selected["start_ms"], end_ms=selected["end_ms"], note=selected["note"])
            else:
                report.update(bars=observe_score_loop(second), score_hash=saved["musicxml_blake3"])

        if args.listening:
            # Import the portable file into another empty store using the real desktop entry point.
            with ExitStack() as opened:
                native = woodshed(opened, args, root, "file-open", custody, data_dir=root / "portable-practice",
                                  arguments=("--open-listening-phrase", str(portable)))
                native.command("inspect-listening")
                imported = json.loads((native.mailbox / "practice-clip-receipt.json").read_bytes())
                check_listening(selected, imported, scripted=not args.mark_listening)
                report["file_open_pid"] = native.process.pid

        if args.mark_listening:
            print(f"Saved and reopened your listening phrase in Woodshed. Keep {portable}", flush=True)
            report["passed"] = True
            return

        with ExitStack() as view:
            output = args.receipt / "woodshed"
            output.mkdir()
            env = dict(custody, WOODSHED_DATA_DIR=str(root / "practice"),
                       WOODSHED_SCENARIO=str(HERE / ("reopen-listening.scn" if args.listening else "reopen.scn")),
                       WOODSHED_CAPTURE_DIR=str(output))
            desktop = launch(view, args.woodshed, env, HERE.parents[1], args.receipt / "view.log", "__unused__")
            done = output / "scenario.done"
            until(lambda: done.exists() or desktop.poll() is not None, "reopened score view", 120)
            if not done.exists() or done.read_text().splitlines()[:1] != ["RESULT ok"]:
                raise RuntimeError("Woodshed did not show the saved practice; see woodshed/scenario.done")
            report["passed"] = True


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--listening", action="store_true", help="verify the scripted recording selection")
    mode.add_argument("--mark-listening", action="store_true", help="choose your own range and note in an open Turnstone window")
    for name in ("woodshed", "receipt"):
        parser.add_argument("--" + name, type=lambda s: Path(s).resolve(), required=True)
    for name in ("turnstone", "turnstone-root"):
        parser.add_argument("--" + name, type=lambda s: Path(s).resolve())
    parser.add_argument("--keeper", type=Path, help="tucket-custody-receipt-host for an isolated test identity")
    args = parser.parse_args()
    if args.mark_listening:
        args.listening = True
    elif not (args.turnstone and args.turnstone_root):
        parser.error("scripted runs require --turnstone and --turnstone-root")
    args.receipt.mkdir(parents=True, exist_ok=False)
    result = {"passed": False}
    try:
        run(args, result)
    except Exception as error:
        result.update(passed=False, error=str(error))
        raise
    finally:
        write_result(args.receipt, result)
