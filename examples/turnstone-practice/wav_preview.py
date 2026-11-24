"""Preview a local WAV for Turnstone using the Windows audio device's own cursor."""
import ctypes
from concurrent.futures import ThreadPoolExecutor
from ctypes import wintypes
import threading


class WavPreview:
    def __init__(self, path):
        self.path = path.resolve()
        self.worker = ThreadPoolExecutor(max_workers=1)
        self.lock = threading.Lock()
        self.closed = False
        self.opened = False
        self.observations = []
        self.mci = ctypes.WinDLL("winmm")
        self.mci.mciSendStringW.argtypes = [
            wintypes.LPCWSTR, wintypes.LPWSTR, wintypes.UINT, wintypes.HWND,
        ]
        self.mci.mciGetErrorStringW.argtypes = [wintypes.DWORD, wintypes.LPWSTR, wintypes.UINT]

    def command(self, command):
        out = ctypes.create_unicode_buffer(256)
        error = self.mci.mciSendStringW(command, out, len(out), None)
        if error:
            self.mci.mciGetErrorStringW(error, out, len(out))
            raise RuntimeError(f"WAV preview {command.split()[0]} failed: {out.value}")
        return out.value

    def __call__(self, request):
        with self.lock:
            if self.closed:
                raise ValueError("The preview has closed")
            return self.worker.submit(self.apply, request).result()

    def apply(self, request):
        action = request.get("action")
        if action not in ("play", "pause", "seek", "position"):
            raise ValueError("Unknown WAV preview action")
        if not self.opened:
            self.command(f'open "{self.path}" type waveaudio alias piano')
            self.opened = True
            self.command("set piano time format milliseconds")
        if action == "seek":
            target = request.get("position_ms")
            if type(target) is not int or not 0 <= target <= int(self.command("status piano length")):
                raise ValueError("Seek outside the recording")
            playing = self.command("status piano mode") == "playing"
            self.command(f"seek piano to {target}")
            if playing:
                self.command("play piano")
        elif action == "play":
            if self.command("status piano position") == self.command("status piano length"):
                self.command("seek piano to start")
            self.command("play piano")
        elif action == "pause" and self.command("status piano mode") == "playing":
            self.command("pause piano")
        state = dict(ok=True, position_ms=int(self.command("status piano position")),
                     playing=self.command("status piano mode") == "playing")
        # Retain a bounded sample of requests, including both endpoints of long sessions.
        if len(self.observations) == 128:
            self.observations.pop(1)
        row = {"action": action, **state}
        if action == "position" and request.get("marker") in ("in", "out"):
            row["marker"] = request["marker"]
        self.observations.append(row)
        return state

    def close(self):
        with self.lock:
            if self.closed:
                return
            self.closed = True
            try:
                if self.opened:
                    self.worker.submit(self.command, "close piano").result()
            finally:
                self.worker.shutdown()
