# Scenario fixtures

Each directory here is a complete `REDSHANK_DATA_DIR`: one `redshank-storage`
generation file (`state-00000000000000000001.json`) plus whatever blobs the
model points at. `redshank-run-scenario.ps1 -Fixture <name>` copies one into a
fresh data directory under the run's output folder, so a run starts from a
known model and never touches the machine's real listener state.

The JSON is `redshank_model`'s serde shape, written by `make_fixtures.py`.
Regenerate with:

```
python make_fixtures.py
```

Nothing in the model uses `deny_unknown_fields`, and every field added by the
2026-09-13 GUI work (`TimedTarget::end_offset_ms`, `Annotation::privacy`,
`RedshankModel::listening_sessions`, the new `ListenerSettings` fields) carries
`#[serde(default)]`. So these documents stay loadable as the model grows, and
a fixture may name a new field before every consumer reads it.

## Inventory

| Fixture | What it seeds | Artboard states it serves |
|---|---|---|
| `local-playing` | One `LocalAudio` item pointing at the phase-4 `stereo.m4a`, progress 1 240 ms, two text notes (460 ms, 1 020 ms), one voice note (1 160 ms) with its WAV blob, one listening session | Listen, Notes, Mere, dock Playing/Paused, seek ticks |
| `remote-buffering` | One subscription with three `Enclosure` episodes off the local range server, first selected | Library roster and episode list, dock Buffering, CLOUD badges |
| `unavailable` | One `DirectAudio` item whose enclosure URL nothing answers | dock Unavailable with the item-scoped message and retry |
| `completed` | One local item with `completed: true` progress at 32 000 ms | dock Completed, Replay as the primary action |
| `cluster` | One local item with ten text notes at 250–259 ms and one span note (900 → 1 800 ms) | Notes timeline cluster chip, span wash, span card |
| `empty` | The default model — no library, no queue, no notes | Empty state across every tab |
| `longnames` | Item, episode and feed titles of exactly 120 characters | Truncation at every width, and at 200 % / 400 % `ui-zoom` |

## The voice blob

`desktop/src/voice.rs` resolves a blob id of exactly 64 hex digits to
`<data dir>/voice/<digest>.wav`, and does **not** verify that the digest is a
real blake3 of the bytes. `local-playing` therefore uses the readable constant
`voice:fixture000…0` and ships a 1.5 s silent mono WAV under that name. It is
there so the voice-note *row* renders with a real duration and a resolvable
blob; it is not a content address and no receipt should read it as one.

## The range server

`remote-buffering` and `longnames` point at `http://127.0.0.1:8765/…`, served
by the spike's range server:

```
python ../spikes/playback/scripts/range_server.py \
    C:\Users\mark_\Code\testing\woodshed\redshank-phase4-20260906\stereo.m4a 8765
```

`redshank-run-scenario.ps1 -Fixture remote-buffering` starts and stops it for
you (`-NoRangeServer` opts out when one is already running). The server ignores
the request path, so all three episodes resolve to the same bytes — which is
what the buffering and roster artboards need, not three distinct recordings.

## Not seeded here

- **Recording.** An active voice capture is process state, not model state, so
  no fixture can seed it. `recording.scn` reaches it with
  `act voice-begin`, which needs a real input device; on a machine without one
  the dock reports voice capture unavailable and the scenario's
  `assert snap recording == true` fails honestly rather than faking the state.
