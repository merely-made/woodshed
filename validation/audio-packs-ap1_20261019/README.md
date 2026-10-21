# AP1 shared audio packs — 2026-10-19

The shared file/API slice is implemented locally on baseline
`9ab3f6be29027321725b385961e8b323a2bba2c0`, with prior AP0/planning changes
already present. The [canonical plan](../../design_docs/2026-09-09_audio_ports_rehome_plan.md#ap1--admit-portable-definitions-and-assets)
owns scope and next steps. This receipt records builds and actual file
demonstrations; no test suites were added or run.

## Implemented owners

- `audio-pattern` owns the moved existing sequencer/sound modules and a midly
  format-0 projection of the same authored pattern.
- `audio-packs` owns the moved sample bank/WAV decoder, bounded TOML/JSON
  admission and exact-source/asset snapshots. Bundled bytes, installed
  directories and reopened snapshots share final admission and decoding.
- `woodshed-audio` re-exports the shared types. Its existing production
  Firewheel processor consumes the hydrated pattern in `examples/drum_pack.rs`.
- The original generated kick/snare/hat kit lives in
  [assets/audio-packs/basic-drums](../../assets/audio-packs/basic-drums/README.md).
  Its fixed sample-to-mono route uses the existing sampler. No FunDSP runtime
  or generic patch graph is introduced in AP1.

## Observed file workflows

The final binaries ran ten fresh command processes, recorded in
[commands.json](commands.json). The complete artifacts remain under
`/Users/markik/Code/testing/woodshed/audio-packs-ap1_20261019/final-current/`.

1. Exported bundled MIDI and its self-contained snapshot, then exported from
   the unchanged on-disk kit. Identity, MIDI bytes and rendered WAV bytes agree.
2. Loaded an externally installed JSON kit with its own identity/version,
   tempo changed from 100 to 90 BPM and snare gain from 0.7 to 0.4. Exported its
   MIDI, snapshot and WAV, then moved the installation directory away.
3. Reopened that snapshot in fresh MIDI/export and render processes while its
   original installation path was absent. Output bytes agree with the captured
   installed kit.
4. Loaded an equivalent authored TOML definition and reopened its own snapshot.
   MIDI and WAV output agree with the JSON version. The digest differs because
   identity binds the exact original format/source and encoded assets.

All exports contain 12 notes, PPQ 480 and a final bar boundary at tick 1920.
The 100 BPM WAV contains 115,200 frames at 48 kHz (2.4 seconds); the edited
90 BPM kit contains 128,000 frames (2.666667 seconds). PCM is finite and the
observed peak is approximately 0.7618. Details and comparisons are in
[observations.json](observations.json), with artifact SHA-256 values in
[receipt.json](receipt.json). WAVs were rendered through `OutputRuntime::offline`;
no audio device or guitar was opened, and physical audition is not claimed.

## Build scope and reproduction

Host: Intel macOS 15.8.1, build 24H32. All final Cargo commands ran from `/tmp`
with an absolute manifest, `--locked --offline`, explicit rustup executables,
separate target directories and `profile.dev.debug=0`.

- Rust/Cargo 1.99.1: `audio-pack` CLI and `woodshed-audio`'s `drum_pack`
  example build. The runtime emits four pre-existing `fetch_update`
  deprecation warnings.
- Rust/Cargo 1.99.1: both new libraries compile for
  `wasm32-unknown-unknown`. This qualifies compilation, not browser file APIs
  or browser audio.
- rustc 1.97.1 with Cargo 1.99.1: both libraries compile natively with the
  preserved root lockfile. Cargo 1.97.1 itself requests a workspace lockfile
  rewrite and refuses `--locked`; the failed invocation is retained in
  `logs/msrv-cargo-1.97.1.log`. Full Cargo-1.97.1 workspace compatibility is
  not claimed.
- The normal dependency tree for `audio-packs` contains no CPAL, MIDI device,
  Firewheel, GUI or Rayon closure. Root lock changes add the two local crates,
  midly 0.5.3 and their new dependency edges; prior external package versions
  and edges remain intact. Port lockfiles are unchanged.

Exact build commands, tool versions, source hashes, binary hashes and known
limits are in [receipt.json](receipt.json). Build logs and the portable normal
dependency tree are retained alongside it. Use the kit README's CLI examples
to author/export/render; output paths must be new.

## Remaining boundaries

AP2 adopts these shared APIs into existing Woodshed/Hocket views and project
workers, with retained snapshots for saved material and an explicit rendered
versus editable handoff choice. Ringdown still needs product integration and
independent guitar format/playback evidence. Native Windows/Linux execution,
GUI operation, browser audio, realtime device playback and callback allocation
measurements were not performed in this slice. Existing AP0 receipts retain
their original source and scope.

Mere composition remains in the canonical plan: source-qualified rhythm/phrase
editing via Scenomise, later typed routing, and intentionally bound scene
dynamics proposing musical controls under audio-owned timing. No Mere source
was changed. No commit or publication was performed.
