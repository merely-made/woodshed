#!/usr/bin/env python3
"""Regenerate the Redshank scenario fixtures.

Each fixture is a complete ``REDSHANK_DATA_DIR``: one ``redshank-storage``
generation file plus whatever blobs the model points at. The driver script
copies one into a fresh directory per run, so a scenario starts from a known
model without the desktop having to seed itself.

The shapes here are ``redshank_model``'s serde shapes (``model/src/lib.rs``).
Nothing in the model uses ``deny_unknown_fields`` and every field added since
the 2026-09-13 GUI work carries ``#[serde(default)]``, so a fixture stays
loadable as the model grows.

    python make_fixtures.py
"""

from __future__ import annotations

import json
import struct
import wave
from pathlib import Path

HERE = Path(__file__).resolve().parent

# The phase-4 codec fixture. Local audio fixtures point at it rather than
# copying bytes, so one file backs every local scenario.
STEREO_M4A = r"C:\Users\mark_\Code\testing\woodshed\redshank-phase4-20260906\stereo.m4a"

# A local range server started by the driver script serves this.
RANGE_BASE = "http://127.0.0.1:8765"

# Deliberately unroutable: the discard port refuses fast, so the unavailable
# fixture reaches its message without a long timeout.
DEAD_URL = "http://127.0.0.1:9/missing.m4a"

# `desktop/src/voice.rs` accepts a blob id of exactly 64 hex digits and reads
# `<data dir>/voice/<digest>.wav`. It does not verify the digest, so a fixture
# uses a readable constant rather than a real blake3 of the bytes.
VOICE_BLOB = "fixture" + "0" * 57
VOICE_ID = f"voice:{VOICE_BLOB}"

GENERATION = "state-{:020d}.json".format(1)


def receipt(**fields: object) -> dict:
    base = {
        "requested_url": None,
        "final_url": None,
        "media_type": None,
        "byte_length": None,
        "etag": None,
        "last_modified": None,
        "retrieved_at_ms": None,
        "complete_digest": None,
    }
    base.update(fields)
    return base


def target(item_id: str, offset_ms: int, end_offset_ms: int | None = None) -> dict:
    return {
        "item_id": item_id,
        "offset_ms": offset_ms,
        "end_offset_ms": end_offset_ms,
        "pressed_offset_ms": None,
        "representation": receipt(),
    }


def text_note(note_id: str, item_id: str, offset_ms: int, body: str,
              end_offset_ms: int | None = None, created: int = 1_757_000_000_000) -> dict:
    return {
        "id": note_id,
        "target": target(item_id, offset_ms, end_offset_ms),
        "body": {"kind": "text", "plain_text": body},
        "created_at_ms": created,
        "privacy": "private",
    }


def voice_note(note_id: str, item_id: str, offset_ms: int,
               duration_ms: int = 1_500, created: int = 1_757_000_000_000) -> dict:
    return {
        "id": note_id,
        "target": target(item_id, offset_ms),
        "body": {
            "kind": "audio",
            "blob_id": VOICE_ID,
            "media_type": "audio/wav",
            "duration_ms": duration_ms,
        },
        "created_at_ms": created,
        "privacy": "private",
    }


def local_item(item_id: str, title: str, path: str = STEREO_M4A) -> dict:
    return {
        "kind": "local_audio",
        "id": item_id,
        "title": title,
        "source": {"kind": "local", "path": path},
    }


def episode(item_id: str, feed_url: str, guid: str, title: str, url: str,
            published: str | None = None) -> dict:
    return {
        "kind": "feed_episode",
        "id": item_id,
        "feed_url": feed_url,
        "guid": guid,
        "title": title,
        "source": {"kind": "enclosure", "url": url},
        "facts": {
            "published": published,
            "summary": None,
            "duration": "0:32",
            "artwork": None,
            "enclosure_media_type": "audio/mp4",
            "enclosure_byte_length": 33236,
            "chapters": [],
            "transcripts": [],
        },
    }


def progress(position_ms: int, completed: bool = False,
             updated: int = 1_757_000_000_000) -> dict:
    return {"position_ms": position_ms, "completed": completed, "updated_at_ms": updated}


def model(library: list[dict], *, queue: list[str] | None = None,
          selected: str | None = None, progresses: dict[str, dict] | None = None,
          annotations: list[dict] | None = None,
          subscriptions: list[dict] | None = None,
          sessions: list[dict] | None = None,
          settings: dict | None = None) -> dict:
    return {
        "schema_version": 1,
        "library": {item["id"]: item for item in library},
        "subscriptions": {feed["feed_url"]: feed for feed in (subscriptions or [])},
        "queue": queue if queue is not None else [item["id"] for item in library],
        "selected_item": selected,
        "progress": progresses or {},
        "annotations": {note["id"]: note for note in (annotations or [])},
        "listening_sessions": sessions or [],
        "settings": settings or {
            "capture_playback": "pause",
            "skip_forward_ms": 30000,
            "skip_backward_ms": 15000,
        },
    }


def session(item_id: str, start_ms: int, stop_ms: int, started_at_ms: int,
            ended_at_ms: int, completed: bool = False) -> dict:
    return {
        "item_id": item_id,
        "start_ms": start_ms,
        "stop_ms": stop_ms,
        "started_at_ms": started_at_ms,
        "ended_at_ms": ended_at_ms,
        "completed": completed,
    }


def write_fixture(name: str, document: dict, *, with_voice: bool = False) -> None:
    root = HERE / name
    root.mkdir(parents=True, exist_ok=True)
    (root / GENERATION).write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8")
    if with_voice:
        voice = root / "voice"
        voice.mkdir(exist_ok=True)
        write_silent_wav(voice / f"{VOICE_BLOB}.wav", seconds=1.5)
    print(f"wrote {root}")


def write_silent_wav(path: Path, seconds: float, rate: int = 48_000) -> None:
    """A tiny mono 16-bit WAV. Silence is enough: the fixture exercises the
    voice-note *row*, not the decoder."""
    frames = int(rate * seconds)
    with wave.open(str(path), "wb") as handle:
        handle.setnchannels(1)
        handle.setsampwidth(2)
        handle.setframerate(rate)
        handle.writeframes(struct.pack("<h", 0) * frames)


# Long enough that every 120-character slice below is a real 120 characters.
LONG = (
    "A Very Long Episode Title That Has To Truncate Somewhere Because The Dock "
    "Row Will Not Grow For It Ever, And Neither Will The Roster Row Beside It, "
    "Nor The Note Card Under Either Of Them "
)


def main() -> None:
    # (a) local-playing: a local file mid-item with two text notes and a voice.
    write_fixture(
        "local-playing",
        model(
            [local_item("local-stereo", "Stereo phase-4 fixture")],
            selected="local-stereo",
            progresses={"local-stereo": progress(1_240)},
            annotations=[
                text_note("note-a", "local-stereo", 460, "The first thing worth keeping."),
                text_note("note-b", "local-stereo", 1_020, "And the second, a little later."),
                voice_note("note-c", "local-stereo", 1_160),
            ],
            sessions=[session("local-stereo", 0, 1_240, 1_757_000_000_000, 1_757_000_001_240)],
        ),
        with_voice=True,
    )

    # (b) remote-buffering: a subscription with three episodes off the local
    # range server, the first selected so the dock opens on a remote item.
    feed_url = f"{RANGE_BASE}/feed.xml"
    write_fixture(
        "remote-buffering",
        model(
            [
                episode("ep-1", feed_url, "one", "Range One", f"{RANGE_BASE}/stereo.m4a",
                        "Wed, 10 Sep 2026 09:00:00 GMT"),
                episode("ep-2", feed_url, "two", "Range Two", f"{RANGE_BASE}/stereo.m4a",
                        "Wed, 03 Sep 2026 09:00:00 GMT"),
                episode("ep-3", feed_url, "three", "Range Three", f"{RANGE_BASE}/stereo.m4a",
                        "Wed, 27 Aug 2026 09:00:00 GMT"),
            ],
            queue=["ep-1", "ep-2", "ep-3"],
            selected="ep-1",
            subscriptions=[{
                "feed_url": feed_url,
                "title": "Range Server Feed",
                "subtitle": "Three episodes served with byte ranges",
                "link": None,
                "language": "en",
                "artwork": None,
                "last_refreshed_ms": 1_757_000_000_000,
                "diagnostics": [],
            }],
        ),
    )

    # (c) unavailable: an enclosure nothing answers for.
    write_fixture(
        "unavailable",
        model(
            [{
                "kind": "direct_audio",
                "id": "gone",
                "title": "An episode the network cannot reach",
                "source": {"kind": "enclosure", "url": DEAD_URL},
            }],
            selected="gone",
        ),
    )

    # (d) completed: the replay state.
    write_fixture(
        "completed",
        model(
            [local_item("local-done", "Finished phase-4 fixture")],
            selected="local-done",
            progresses={"local-done": progress(32_000, completed=True)},
            sessions=[session("local-done", 0, 32_000, 1_756_900_000_000,
                              1_756_900_032_000, completed=True)],
        ),
    )

    # (e) cluster: ten notes inside one second, plus one span.
    cluster_notes = [
        text_note(f"cluster-{index:02d}", "local-cluster", 250 + index,
                  f"Clustered thought {index + 1}")
        for index in range(10)
    ]
    cluster_notes.append(
        text_note("span-1", "local-cluster", 900,
                  "A stretch worth replaying whole.", end_offset_ms=1_800)
    )
    write_fixture(
        "cluster",
        model(
            [local_item("local-cluster", "Ten notes in a second")],
            selected="local-cluster",
            progresses={"local-cluster": progress(2_400)},
            annotations=cluster_notes,
        ),
    )

    # (f) empty: the default model, for the empty-state artboards.
    write_fixture("empty", model([]))

    # (g) longnames: exactly 120 characters of title, everywhere it shows.
    long_feed = f"{RANGE_BASE}/long.xml"
    write_fixture(
        "longnames",
        model(
            [
                local_item("long-local", LONG[:120]),
                episode("long-ep", long_feed, "long", LONG[10:130],
                        f"{RANGE_BASE}/stereo.m4a", "Wed, 10 Sep 2026 09:00:00 GMT"),
            ],
            queue=["long-local", "long-ep"],
            selected="long-local",
            progresses={"long-local": progress(4_000)},
            subscriptions=[{
                "feed_url": long_feed,
                "title": LONG[20:140],
                "subtitle": LONG[5:125],
                "link": None,
                "language": "en",
                "artwork": None,
                "last_refreshed_ms": 1_757_000_000_000,
                "diagnostics": [],
            }],
            annotations=[text_note("long-note", "long-local", 2_000, LONG[:120])],
        ),
    )


if __name__ == "__main__":
    main()
