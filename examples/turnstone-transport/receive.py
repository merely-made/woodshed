"""Run the Woodshed transport handoff in two real desktop applications."""
import argparse
from collections import deque
from contextlib import ExitStack
import ctypes
from ctypes import wintypes
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import hashlib
import json
import os
from pathlib import Path
import secrets
import subprocess
import tempfile
import threading
import time


HERE = Path(__file__).resolve().parent


def until(probe, description, timeout=30):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        result = probe()
        if result:
            return result
        time.sleep(0.05)
    raise RuntimeError("timed out waiting for " + description)


def desktop_windows(pid):
    api = ctypes.windll.user32
    handles = []
    callback = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)

    @callback
    def visit(hwnd, _):
        owner = wintypes.DWORD()
        api.GetWindowThreadProcessId(hwnd, ctypes.byref(owner))
        if owner.value == pid:
            title = ctypes.create_unicode_buffer(256)
            api.GetWindowTextW(hwnd, title, len(title))
            if title.value == "Woodshed":
                handles.append({"hwnd": hwnd, "visible": bool(api.IsWindowVisible(hwnd)),
                                "minimized": bool(api.IsIconic(hwnd))})
        return True

    api.EnumWindows(visit, 0)
    return handles


def verify_loop_phase(points):
    # The fresh practice fixture repeats two 4/4 bars at 120 BPM.
    offsets = set()
    for point in points:
        rate = point["sample_rate"]
        position = point["bar"] * 2 * rate + point["sample_in_bar"]
        offsets.add((rate, (point["device_frames"] - position) % (4 * rate)))
    if len(offsets) != 1:
        raise RuntimeError("song position lost its sample offset against the audio device")


class Producer:
    def __init__(self, mailbox, process):
        self.mailbox = mailbox
        self.process = process
        self.sequence = 0

    def read(self):
        if self.process.poll() is not None:
            raise RuntimeError(f"Woodshed exited with {self.process.returncode}")
        try:
            return json.loads((self.mailbox / "status.json").read_bytes())
        except (FileNotFoundError, PermissionError):
            return {}

    def command(self, action):
        self.sequence += 1
        pending = self.mailbox / "pending.json"
        pending.write_text(json.dumps({"id": self.sequence, "action": action}))
        pending.replace(self.mailbox / "command.json")
        def ack():
            state = self.read()
            if state.get("last_command") == self.sequence and state.get("command_error"):
                raise RuntimeError(f"Woodshed rejected {action}: {state['command_error']}")
            return state if state.get("last_command") == self.sequence else None
        return until(ack, "Woodshed to acknowledge " + action)


class Handoff:
    def __init__(self, producer, report):
        self.producer = producer
        self.report = report
        self.lock = threading.Lock()
        self.attached = False
        self.returned = False
        self.owner = None
        self.frame = None
        self.hwnd = None

    def sample(self, label, view):
        before = self.producer.read()
        def progress():
            state = self.producer.read()
            if not state.get("playing") or state.get("view") != view:
                return None
            cursor = (state["bar"], state["sample_in_bar"])
            if state["device_frames"] <= before.get("device_frames", 0):
                return None
            if cursor == (before.get("bar"), before.get("sample_in_bar")):
                return None
            return state
        current = until(progress, "active playback in " + label)
        if current["streams"] != 1 or current["device_errors"] != 0:
            raise RuntimeError(f"unhealthy audio output at {label}: {current}")
        owner = (current["pid"], current["stream_id"], current["transport_revision"])
        if owner[0] != self.producer.process.pid or owner[1] is None:
            raise RuntimeError("invalid playback owner")
        if self.owner and (owner != self.owner or current["device_frames"] <= self.frame):
            raise RuntimeError("audio output changed or restarted at " + label)
        self.owner, self.frame = owner, current["device_frames"]
        def window_state():
            matches = desktop_windows(owner[0])
            return next((w for w in matches if (w["visible"] and not w["minimized"]) == (view == "window")
                         and (self.hwnd is None or w["hwnd"] == self.hwnd)), None)
        window = until(window_state, label + " native Woodshed window visibility")
        self.hwnd = window["hwnd"]
        self.report["checkpoints"].append(dict(stage=label, window=window, **current))
        return current

    def move(self, action):
        with self.lock:
            if self.returned or action not in ("mounted", "quiet", "detach"):
                return None
            if self.attached == (action == "mounted"):
                return None
            self.producer.command({"mounted": "attach", "quiet": "quiet", "detach": "detach"}[action])
            state = self.sample(action, "window" if action == "detach" else "pane")
            self.attached = action != "detach"
            self.returned = action == "detach"
            return state


def serve(handoff, token):
    page = (HERE / "pane.html").read_bytes()
    base = "/" + token + "/"
    class Pane(BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass

        def send(self, body, kind="application/json", status=200):
            self.send_response(status)
            self.send_header("Content-Type", kind)
            self.send_header("Cache-Control", "no-store")
            self.send_header("Referrer-Policy", "no-referrer")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def do_GET(self):
            if self.headers.get("Host") != self.server.authority:
                return self.send_error(403)
            if self.path == base:
                return self.send(page, "text/html; charset=utf-8")
            if self.path == base + "status":
                return self.send(json.dumps(handoff.producer.read()).encode())
            self.send_error(404)

        def do_POST(self):
            if (self.headers.get("Host") != self.server.authority
                or self.headers.get("Origin") not in (None, "http://" + self.server.authority)
                or not self.path.startswith(base)):
                return self.send_error(403)
            action = self.path[len(base):]
            try:
                state = handoff.move(action)
            except Exception as error:
                handoff.report["error"] = str(error)
                return self.send_error(500)
            if state is None:
                return self.send_error(409)
            self.send(json.dumps(state).encode())

    server = ThreadingHTTPServer(("127.0.0.1", 0), Pane)
    server.authority = f"127.0.0.1:{server.server_port}"
    return server, "http://" + server.authority + base


def launch(stack, binary, env, cwd, output, token):
    child = subprocess.Popen([str(binary)], env={**os.environ, **env}, cwd=cwd,
                             stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    tail = deque(maxlen=100)
    def drain():
        for line in child.stdout:
            tail.append(line.decode(errors="replace").replace(token, "<session>")[:1800])
    reader = threading.Thread(target=drain, daemon=True)
    reader.start()
    def finish():
        if child.poll() is None:
            child.terminate()
        try:
            child.wait(timeout=10)
        except subprocess.TimeoutExpired:
            child.kill()
            child.wait()
        reader.join(timeout=3)
        output.write_text("".join(tail), encoding="utf-8")
    stack.callback(finish)
    return child


def run(args, report):
    if os.name != "nt":
        raise RuntimeError("this receiver checks native window visibility on Windows")
    report["binaries"] = {}
    for binary in (args.woodshed, args.turnstone):
        with binary.open("rb") as executable:
            report["binaries"][binary.name] = hashlib.file_digest(executable, "sha256").hexdigest()
    token = secrets.token_urlsafe(32)
    with tempfile.TemporaryDirectory(prefix="woodshed-transport-") as temp, ExitStack() as stack:
        root = Path(temp)
        mailbox = root / "mailbox"
        mailbox.mkdir()
        wood_env = dict(WOODSHED_DATA_DIR=str(root / "woodshed"),
                        WOODSHED_TRANSPORT_DIR=str(mailbox))
        wood = launch(stack, args.woodshed, wood_env, HERE.parents[1],
                      args.receipt / "woodshed.log", token)
        producer = Producer(mailbox, wood)
        until(producer.read, "Woodshed transport", 120)
        handoff = Handoff(producer, report)
        producer.command("play")
        handoff.sample("standalone", "window")
        server, url = serve(handoff, token)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        stack.callback(server.server_close)
        stack.callback(server.shutdown)
        scenario = root / "handoff.scn"
        scenario.write_text((HERE / "turnstone.scn").read_text().replace("${WOODSHED_PANE_URL}", url))
        captures = args.receipt / "pane"
        captures.mkdir()
        env = dict(TURNSTONE_ROOT=str(root / "turnstone"), MERE_ROOT=str(root / "mere"),
                   TURNSTONE_CAPTURE_DIR=str(captures), TURNSTONE_SCENARIO=str(scenario),
                   TURNSTONE_GENET_ENGINE="bole", WOODSHED_PANE_URL=url,
                   TURNSTONE_SERVO_PROFILE="Woodshed", TURNSTONE_SERVO_PROFILE_DIR=str(root / "servo"))
        turn = launch(stack, args.turnstone, env, args.turnstone_root,
                      args.receipt / "turnstone.log", token)
        def finished():
            if report.get("error"):
                raise RuntimeError(report["error"])
            if turn.poll() is not None or (captures / "scenario.done").exists():
                return True
        until(finished, "Turnstone's real pane scenario", 180)
        done = captures / "scenario.done"
        if not done.exists() or done.read_text().splitlines()[:1] != ["RESULT ok"]:
            raise RuntimeError("Turnstone scenario failed; see pane/scenario.done and turnstone.log")
        stages = [point["stage"] for point in report["checkpoints"]]
        if stages != ["standalone", "mounted", "quiet", "detach"]:
            raise RuntimeError(f"unexpected Woodshed transport transitions: {stages}")
        if not report["checkpoints"][2]["quiet"]:
            raise RuntimeError("Woodshed did not apply the pane's quiet control")
        handoff.sample("returned", "window")
        verify_loop_phase(report["checkpoints"])
        producer.command("release")
        report["passed"] = True
        if args.leave_playing:
            # Release the pane capability while the normal Woodshed window is in use.
            server.shutdown()
            server.server_close()
            write_result(args.receipt, report)
            wood.wait()


def write_result(directory, report):
    pending = directory / "result.pending"
    pending.write_text(json.dumps(report, indent=2) + "\n")
    pending.replace(directory / "result.json")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("woodshed", "turnstone", "turnstone-root", "receipt"):
        parser.add_argument("--" + name, type=lambda s: Path(s).resolve(), required=True)
    parser.add_argument("--leave-playing", action="store_true",
                        help="keep Woodshed open after writing the passing receipt")
    args = parser.parse_args()
    args.receipt.mkdir(parents=True, exist_ok=False)
    report = {"passed": False, "checkpoints": []}
    try:
        run(args, report)
    except Exception as error:
        report["passed"] = False
        report["error"] = str(error)
        raise
    finally:
        write_result(args.receipt, report)


if __name__ == "__main__":
    main()
