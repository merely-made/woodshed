# audio-pattern

The existing Woodshed `SequencerPattern`, `Track`, `Step`, `Subdivision`,
`TimeSignature`, `Sound` and `SampleBuffer` now have this shared owner.
`woodshed-audio` re-exports them for its existing consumers. Their serialized
representation and sound kernels are preserved.

This library has no file, device, Bluetooth, UI or Firewheel dependency.
`audio-primitives` remains dependency-free. `audio-packs` owns WAV loading and
pack admission; product runtimes own clocks and playback.

`SequencerPattern::validate` checks an authored one-bar pattern before pack
admission or MIDI export. It does not silently change existing constructor
behavior. Subdivisions count divisions of a notated beat; BPM counts quarter
notes. Each authored track contains exactly one bar of steps.

`midi::export_midi(&pattern, &mappings)` returns bytes and timing metadata.
Mappings correspond to existing tracks and supply 1–16 channel numbers,
note, normal/accent velocity and MIDI gate percentage. Muted tracks are skipped;
active tracks require distinct mapped note/channel pairs. Sample buffers are
not consulted. Samples, effects, sound level and tuning are not embedded in
MIDI; the mapping determines which receiving instrument note plays.

The format-0 file contains tempo, meter, note endings and an exact end-of-bar
event. PPQ is 480 where exact, increased when the grid needs it. Note-offs sort
before note-ons at shared boundaries. Unrepresentable MIDI tempos are refused.

`render::render_pattern` is the moved existing pure offline renderer, still
re-exported by `woodshed-audio::offline`. Hocket uses this owner to prepare
drum material for its existing recorder import; it adds no sequencer or audio
device. Native Woodshed playback still uses its production Firewheel processor.

MIDI parsing/writing uses pinned midly 0.5.3, `default-features = false` with
`std` and `strict`; Rayon is excluded. midly is Unlicense; its registry checksum
is retained in the root lockfile. This crate's source is MPL-2.0.

The [shared plan](../../design_docs/2026-09-09_audio_ports_rehome_plan.md#shared-data-defined-audio-packs-2026-10-19)
tracks implementation and the Mere composition direction.
