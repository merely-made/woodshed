# MusicXML timing consumer

This standalone Cargo workspace reads a MusicXML file and prints bars, exact
integer note/rest timing, MIDI pitches, frequencies and string/fret contacts.
Its only direct dependency is `woodshedding`. It does not resolve Mere,
Woodshed's application crates, persistence, UI or an audio engine.

From the Woodshed repository root:

```sh
cargo run --locked --manifest-path examples/musicxml-timing/Cargo.toml -- \
  examples/musicxml-timing/tests/fixtures/rhythm.musicxml
cargo run --locked --manifest-path examples/musicxml-timing/Cargo.toml -- \
  examples/musicxml-timing/tests/fixtures/progression.musicxml 73.5
cargo test --locked --manifest-path examples/musicxml-timing/Cargo.toml
cargo tree --locked --manifest-path examples/musicxml-timing/Cargo.toml -e normal
```

The optional BPM overrides timing only; it does not rewrite the file or its
written tempo. Omitting it uses the written tempo. Tempo follows the existing
1–400 BPM profile; fractional overrides are supported. This is the existing
editable fretted MusicXML profile, not a general MusicXML importer.

The fixtures are copies of the progression and rhythm exports qualified in
`validation/native-musicxml_20261018/`. Tests exercise real file input, simultaneous
chord tones, triplet boundaries, rests, failure without partial output, and
timing a MusicXML round-trip with 257 bars. That last score is valid musical
data, while Woodshed's current playback adapter admits at most 256 bars.

`woodshedding::timing` validates the existing score profile and derives a
disposable `ArrangementSchedule`. Output hosts convert `TICKS_PER_QUARTER` to
their own clock. Woodshed's native adapter separately enforces its 256-bar and
4,096-event/contact limits before creating an output program.
