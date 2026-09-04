#!/usr/bin/env python3
"""Serve one file with byte-range support and log every requested range."""

from __future__ import annotations

import argparse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path


def parse_range(value: str | None, length: int) -> tuple[int, int]:
    if not value:
        return 0, length - 1
    unit, _, spec = value.partition("=")
    if unit.strip().lower() != "bytes" or "," in spec:
        raise ValueError("only one byte range is supported")
    start_text, _, end_text = spec.partition("-")
    if not start_text:
        suffix = int(end_text)
        return max(0, length - suffix), length - 1
    start = int(start_text)
    end = min(length - 1, int(end_text) if end_text else length - 1)
    if start < 0 or start >= length or end < start:
        raise ValueError("range outside file")
    return start, end


class RangeHandler(BaseHTTPRequestHandler):
    server_version = "RedshankRange/1"

    def do_HEAD(self) -> None:
        self._serve(send_body=False)

    def do_GET(self) -> None:
        self._serve(send_body=True)

    def _serve(self, send_body: bool) -> None:
        source: Path = self.server.source  # type: ignore[attr-defined]
        length = source.stat().st_size
        requested = self.headers.get("Range")
        try:
            start, end = parse_range(requested, length)
        except (ValueError, TypeError):
            self.send_response(416)
            self.send_header("Content-Range", f"bytes */{length}")
            self.end_headers()
            return

        partial = requested is not None
        self.send_response(206 if partial else 200)
        self.send_header("Accept-Ranges", "bytes")
        self.send_header("Content-Type", "audio/mp4")
        self.send_header("Content-Length", str(end - start + 1))
        if partial:
            self.send_header("Content-Range", f"bytes {start}-{end}/{length}")
        self.end_headers()

        print(
            f"range_request start={start} end={end} bytes={end - start + 1}",
            flush=True,
        )
        if not send_body:
            return
        with source.open("rb") as handle:
            handle.seek(start)
            remaining = end - start + 1
            while remaining:
                block = handle.read(min(64 * 1024, remaining))
                if not block:
                    break
                self.wfile.write(block)
                remaining -= len(block)

    def log_message(self, format: str, *args: object) -> None:
        return


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("file", type=Path)
    parser.add_argument("port", nargs="?", type=int, default=8765)
    args = parser.parse_args()
    source = args.file.resolve(strict=True)
    server = ThreadingHTTPServer(("127.0.0.1", args.port), RangeHandler)
    server.source = source  # type: ignore[attr-defined]
    print(f"serving file={source} url=http://127.0.0.1:{args.port}/{source.name}", flush=True)
    server.serve_forever()


if __name__ == "__main__":
    main()
