# woodshedding

Portable musical operations: theory, string-instrument realization, progressions,
exercises, fretboards, voicings, practice sets, editable fretted MusicXML and exact
score timing. No I/O, UI, persistence or audio engine. Normal dependencies are
`serde` and `roxmltree`.

For musical documents, start with:

- `notation::MusicDocument`: MusicXML text, read with `score()` and written with
  `from_score()`.
- `notation::Score`: the editable musical model, including tuning, written tempo,
  bar time signatures, chord symbols, notes and rests.
- `timing::ArrangementSchedule::from_score(&score, bpm)`: exact integer onsets
  and durations, sounding pitches and physical string/fret contacts.
- `timing::ArrangementSchedule::event_at(bar_index, ticks)`: find the event or
  rest at a bar-local position. Event intervals exclude their ending tick.

```rust
use woodshedding::{notation::Score, timing::ArrangementSchedule};

let score = Score::rhythm_example();
let timing = ArrangementSchedule::from_score(&score, 73.5).unwrap();
assert_eq!(score.tempo, 96); // deriving timing preserves written tempo
assert_eq!(timing.measures[1].events[0].gate_ticks, 320); // triplet eighth
```

Timing uses 960 ticks per quarter note and represents every supported note value
exactly, including triplets. The selected timing tempo supports fractional BPM
within the existing 1–400 range. Hosts own file access, transport, device clocks,
audio preparation and engine capacity checks.

The existing editable MusicXML profile supports fretted notes/chords, rests,
supported note values and per-bar meter; this is not a general MusicXML importer.
Its score validation limits, including at most 1,024 measures, remain in effect.
Woodshed's playback limits (256 bars and 4,096 events/contacts) do not constrain
this crate's timing operations.

See [`examples/musicxml-timing`](../../examples/musicxml-timing/README.md) for a
standalone file-reading consumer with its own Cargo workspace and lockfile. It
demonstrates MusicXML-to-timing without resolving the surrounding application.
