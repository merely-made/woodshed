# fundsp Inside Firewheel: Upstream Check and Adapter Shape

**Status (2026-10-19): research brief.** Nothing is implemented, and no
implementation is scheduled. Mark asked for this brief before deciding.

## Origin

On 2026-10-19 Mark asked for "fundsp + firewheel thoughts?", then "let's check
upstream!" The [April plan](2026-04-30_initial_plan.md) (line 638) had deferred
fundsp: a sine-burst click "adds zero dependency surface", with fundsp to come
"when we want richer drum sounds".

## Target versions

The adapter targets current releases, not the demo's versions:

- **Firewheel 0.14.0** (crates.io 2026-09-05; upstream `main` sits at that
  release). Woodshed's fork `2b03846` (2026-10-11) is 0.14.0 plus owned
  changes, so a 0.14 adapter builds on both.
- **fundsp 0.23.0** (2026-01-07; `master` unchanged since March, edition
  2024).

The demo below is on firewheel 0.7.1 and fundsp 0.20.0, so it shows the shape
only.

## The two layers

- **Firewheel** is the engine: node graph, scheduling, device I/O,
  sample-accurate events and node lifecycle. Woodshed, Hocket and Redshank pin
  the `mark-ik/firewheel` fork at `2b03846`. Its node set already covers SVF and
  fast filters, freeverb, convolution, a noise generator, sampler, mixing,
  volume/pan, meters and delay compensation.
- **fundsp** (0.23.0, 2026-01-07) is a sound-design library: oscillators,
  envelopes, filters and effects, composed with graph notation into
  `AudioUnit`s. It owns no device and no schedule.

They are complementary. fundsp would run inside a Firewheel node.

## Upstream check (2026-10-19)

- **No official integration.** Firewheel's docs, issues, PRs, discussions and
  code mention fundsp nowhere. Neither does bevy_seedling. On crates.io, none
  of Firewheel's 7 reverse dependencies uses fundsp, and none of fundsp's 29
  uses Firewheel.
- **One unpublished demo adapter.** CorvusPrudens/bevy-radio
  `src/fdsp_host.rs` was written by bevy_seedling's author and last pushed
  2025-09-30, on firewheel 0.7.1 and fundsp 0.20.0. It has **no licence**, so
  it is a reference only. olekspickle/bevy_new_3d_rpg carries a copy, and
  void-scape/annual pairs fundsp formant envelopes with Firewheel parameters.
- **Possibly relevant later:** `cochlea-synth` (an `Instrument` trait over
  fundsp, with presets and a typed parameter registry, 2026-08) and
  `midi_fundsp`, for voices rather than for the adapter.

## What the demo gets right and wrong

Its shape matches the fork's node API (`ProcInfo`, `ProcBuffers`, `ProcEvents`,
`ProcExtra`):

- a generic node wrapping `Box<dyn AudioUnit>`;
- channel counts taken from the unit;
- sample rate set and `allocate()` called on the control thread at
  construction.

Woodshed's version must not inherit its defects:

1. **Silenced generators.** `skip_silence` defaults to on and returns
   `ClearAllOutputs` whenever all inputs are silent. A zero-input generator is
   always "silent", and effect tails are cut the moment input stops.
2. **An outdated API.** It predates 0.14's `events()`/`process()` split and the
   `ProcStreamCtx` argument to `new_stream`/`stream_stopped`. In 0.14,
   `new_stream` runs on the main thread, and the docs call allocating there
   safe. So the demo's `allocate()` in `new_stream` is acceptable under
   current Firewheel. `stream_stopped` may run on the realtime thread, so it
   must stay non-allocating.
3. **Parameters that aren't sample-accurate.** `ProcEvents` is ignored, so
   parameters move only through fundsp's atomic `Shared` values.
4. **Slow processing.** It ticks one sample at a time through dynamic
   dispatch instead of using fundsp's block `process`.

## Adapter shape for Woodshed

- **Placement.** A small root crate, so Hocket can use it later. Not
  `audio-primitives`, which is ruled pure-std with zero dependencies, and not
  the Firewheel fork.
- **Construction.** Against the Firewheel 0.14 API: `AudioNode::info`,
  `construct_processor` and `update` on the control side. The unit is built,
  its sample rate set and its buffers allocated there. `new_stream` (main
  thread) re-sets the sample rate and reallocates.
- **Processing.** fundsp 0.23's block `process(size, &BufferRef, &mut
  BufferMut)`, in chunks of at most `MAX_BUFFER_SIZE` = 64 frames (SIMD width
  8). Copy through preallocated fundsp buffers, because Firewheel hands plain
  per-channel `f32` slices. Parameter events arrive through 0.14's
  `events()`, and blocks split at their frame offsets. Every output channel is
  filled, or the node returns `ClearAllOutputs`/`Bypass`, as 0.14 requires.
- **Silence policy.** Generators always run. Effects declare a tail length, or
  report silence from their own output.
- **Determinism.** Units are seeded and reset, so offline renders equal live
  ones.

## Where it fits and where it doesn't

- **Fits:** chord and tone audition (`woodshed-audio` `chord_audio.rs` and
  `sound.rs`, about 800 lines of hand-rolled synthesis), new practice voices
  (drones, richer metronome or drum sounds), and later designed effects on
  loops.
- **Doesn't fit:** capture, clocks, bar maths and analysis, which belong to
  the [DSP survey](2026-10-14_dsp_graphshell_hocket_survey_brief.md)'s pure
  state layer. Nor any byte-pinned output. Moving the click or the backing
  render to fundsp would change the WAV gold and force re-qualification for
  no musical gain.

## Gates for a first slice

- `woodshed-audio`'s counting-allocator zero-allocation test passes with the
  node in the graph.
- The block-size invariance test (127/509/211-frame blocks) passes.
- An offline render equals the live render.
- The first consumer is chord audition, where nothing is byte-pinned.

## Costs

- fundsp's typed combinator graphs are heavy on generics: longer builds and a
  larger binary.
- One more dependency to track against the shared 1.97.1 MSRV.
- Denormal behaviour in long reverb tails needs checking.

## Open for Mark

- Whether to adopt fundsp at all, and whether chord audition is the right
  first consumer.
- Whether the adapter crate should live in Woodshed's root `crates/`, or be
  offered upstream to Firewheel or bevy_seedling once it has passed the gates.
