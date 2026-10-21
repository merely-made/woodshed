# AP0 audio-pack embedding evidence — 2026-10-19

This is a bounded assessment, not a product pack implementation. Decisions live
once in the [shared plan](../../design_docs/2026-09-09_audio_ports_rehome_plan.md#ap0-assessment-results-2026-10-19).
The baseline is Woodshed `9ab3f6be29027321725b385961e8b323a2bba2c0` plus the
uncommitted planning changes. No product Rust source, manifest or lock changed.

## Observed results

- Native macOS x86_64, Rust 1.99.0: the portable wrapper imports the actual
  `sequencer.rs`, `sound.rs` and `samples.rs`; no copied pattern schema.
  Four synthetic single-lane patterns (4/4, 3/8, 7/8, triplets) serialize and
  reopen, export through midly 0.5.3, and strictly parse back with expected
  note-on ticks, note endings and final bar tick.
- The runtime loads an original float32 WAV through the actual `SampleBank`,
  rehydrates the exported pattern and renders 96,000 frames through the actual
  production `OutputRuntime`. Sixteen sound onsets agree with MIDI positions
  after accounting for the synthetic sample's one-frame initial zero. Stop,
  silence and restart pass. The measured render calls have zero allocations
  and deallocations on the rendering thread.
- Disposable custom nodes run FunDSP 0.23.0 and RustySynth 1.3.7 directly in
  Firewheel 0.14 at `2b03846d2e0dcc6ded2a85367bb8c5071ae99cdc`.
  Each renders 65,536 finite, non-silent frames at 48 kHz and 44.1 kHz, with
  zero observed rendering-thread allocations/deallocations. FunDSP exercises
  a dynamic sine/gain unit prepared with `allocate`; RustySynth uses an
  original synthetic SF2, 16 voices, effects disabled, note-on/off, a 40-note
  burst and all-notes-off. This does not qualify arbitrary instruments/effects.
- Rustup Rust 1.99.1: the portable wrapper, midly, FunDSP and RustySynth pass
  `wasm32-unknown-unknown` compilation. Rustup Rust 1.97.1: the same portable
  package with candidate libraries passes native checking. Neither command
  includes the production device runtime or qualifies a browser audio host.
- Product bounds refuse zero subdivisions and NaN BPM. Missing sample buffers
  currently produce no voice; shared pack admission must report this failure.

Machine-readable [receipt](receipt.json), [MIDI results](portable-report.json),
[runtime results](runtime-report.json), [source hashes](source-inputs.json) and
[registry checksums/licenses](supplier-provenance.json) accompany the probes.
The registry inventory retains the initially inspected RustySynth 1.3.6;
the built and selected optional candidate is 1.3.7. Mere opportunities are
source-inspected only, with hashes in `mere-source-inputs.json`.
Normal dependency trees describe the actual feature closure; optional crates
appearing in `Cargo.lock` do not mean those features were used. The combined
runtime probe includes CPAL/midir via the actual `woodshed-audio` dependency;
it opens no device. Neither portable closure includes them or Bevy ECS.

## Reproduction

`portable/` and `runtime/` are probe templates. `@WOODSHED_ROOT@` is substituted
in the wrapper and path dependencies, never in product source. From this
directory, choose a new scratch destination:

```sh
python3 prepare.py /absolute/path/to/new-ap0-scratch
cd /absolute/path/to/new-ap0-scratch
cargo run --locked -p ap0-portable -- artifacts
cargo run --locked -p ap0-runtime -- artifacts
cargo check --locked -p ap0-portable --features candidates --target wasm32-unknown-unknown
cargo check --locked -p ap0-portable --features candidates
```

Select Rust 1.99.1 and installed target libraries for the Wasm command, and
1.97.1 for the final MSRV command. On this host Homebrew cargo/rustc bypassed
rustup selection, so successful qualification explicitly selected the rustup
cargo binary and `RUSTC` path. The initial missing-target failure remains in
`logs/portable-wasm.log`; `logs/candidates-wasm.log` records the corrected run.
The initial Homebrew `cargo +1.97.1` invocation was also rejected; the corrected
MSRV result is in `logs/msrv.log`.

Preparation/render timings are instrumented unoptimized observations, not
benchmarks. Callback counters observe only the calling thread and measured
fixtures. No product suites, Windows/Linux execution, browser audio, physical
audio, guitar playback, general SF2 compatibility, patch editor, pack schema or
asset-retention workflow was qualified. The MIDI note mapping and gate/velocity
choices are probe-local; they are not a guitar compatibility profile.
