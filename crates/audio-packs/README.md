# audio-packs

Shared data-defined sample-kit admission, with one path for bundled assets,
manually installed directories and retained snapshots. Uses `audio-pattern`'s
existing pattern representation; owns no audio device, graph, UI or transport.

Entry points:

- `bundled_drum_kit()` admits the embedded original kick/snare/hi-hat kit.
- `LoadedPack::from_directory(path)` captures one `audio-pack.toml` or
  `audio-pack.json` and its referenced WAVs.
- `pack.pattern(name)` returns a hydrated copy for the existing audio runtime.
- `pack.midi(name)` exports the same authored rhythm through midly.
- `pack.snapshot().to_json()` retains exact manifest and asset bytes;
  `PackSnapshot::from_json` followed by `LoadedPack::from_snapshot` revalidates
  the identity, definition and samples before use.
- `PackSelection` retains a selected pattern and its edits alongside the exact
  immutable snapshot. Products store this value in their existing saves.
  Its cached prepared samples are skipped by serialization. Checked edits are
  revision-qualified; failed proposals leave the previous selection intact.
  Inactive pattern drafts are retained when switching patterns.
- `selection.at_tempo(app_bpm)` derives an admitted projection for audition or
  MIDI export from the same current app tempo. `hydrated_pattern`, `midi` and
  `render_bar` share that authored pattern and its assets.
- `files` supplies the same bounded open/create-new export workflow to host
  workers and CLI. It can reopen both AP1 snapshots and AP2 edited selections.

Prepare a candidate before replacing a product's active pack or submitting its
pattern to the runtime. Admission is fallible and does not mutate a previous
pack. The production runtime similarly prepares its replacement before sending
the existing bounded command. Parsing, hashing, decoding and serialization
belong on the control side.

Format 1 declares sample voices, their MIDI mappings, parameter metadata and
one-bar patterns. Its fixed mono sample-to-output route requires explicit
capabilities. Unsupported operations, unsafe references, missing assets and
invalid numeric values are errors. Referenced paths are portable relative
spellings; canonical directory containment is checked before bounded reads.

Bounds: 1 MiB manifest, 64 voices/patterns, 4 MiB per encoded WAV, 16 MiB total
encoded assets and 4,194,304 decoded mono frames. WAVs must have one or two
channels, 8–192 kHz sample rate and finite normalized PCM in [-1, 1]; stereo is
downmixed by the existing loader. Existing host `SampleBank::load_wav` keeps its
legacy format behavior; pack admission adds its own bounded boundary.

Snapshots bind a versioned BLAKE3 envelope over the original format, source
bytes and referenced encoded asset bytes. No installation path or decoded PCM
is serialized as a substitute for the source. Different TOML/JSON encodings
have different digests. Later edits, replacement or uninstall cannot change an
already captured snapshot; missing/corrupt/future snapshots refuse admission.
This format is an audio-pack snapshot, not a new Hocket project or MediaRef.

See the [kit examples](../../assets/audio-packs/basic-drums/README.md) for
authoring and runnable CLI/WAV examples. Shared Cambium kit controls live in
`audio-pack-views`; products retain selection/clock/history authority. Hocket
saves its local draft but hands off ordinary rendered layers. Editable instrument
handoff remains later work. The
[shared plan](../../design_docs/2026-09-09_audio_ports_rehome_plan.md#shared-data-defined-audio-packs-2026-10-19)
keeps Mere rhythm/phrase, routing and expressive scene directions explicit
without making this loader depend on Mere.
