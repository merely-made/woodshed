"""Serve an existing VTT fixture on loopback with a deliberate response delay."""

import argparse
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import time


class DelayedTranscriptHandler(SimpleHTTPRequestHandler):
    def do_GET(self):
        if self.path.split("?", 1)[0].endswith(".vtt"):
            print(f"transcript_fetch_started delay_ms={self.server.delay_ms}", flush=True)
            time.sleep(self.server.delay_ms / 1000)
        super().do_GET()
        print("transcript_fetch_finished", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--port", type=int, default=8766)
    parser.add_argument("--delay-ms", type=int, default=2000)
    args = parser.parse_args()
    directory = args.directory.resolve(strict=True)
    if not 0 <= args.delay_ms <= 10000:
        parser.error("delay must be between zero and ten seconds")
    server = ThreadingHTTPServer(
        ("127.0.0.1", args.port), partial(DelayedTranscriptHandler, directory=str(directory))
    )
    server.delay_ms = args.delay_ms
    print(f"serving_transcript port={args.port} directory={directory}", flush=True)
    server.serve_forever()


if __name__ == "__main__":
    main()
