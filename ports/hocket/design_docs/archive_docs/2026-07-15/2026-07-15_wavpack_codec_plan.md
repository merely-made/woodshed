# Founding plan: a pure-Rust WavPack codec crate

**Status (2026-07-15): ENDORSED by Mark; not started.** The build decision is
made ("I'm into this", 2026-07-15); no repo, no crate, no code yet. Founding is
gated on the name pick (candidates below). This doc scopes a standalone pure-Rust
WavPack encoder/decoder whose first consumer is Hocket. It spins out of the
deferred "FLAC vs WavPack" item in
[2026-07-14_open-project-format_plan.md](2026-07-14_open-project-format_plan.md).
Once the repo is founded, this doc moves into the new crate's own `design_docs/`.

Working name: undecided (candidates in the Architecture section). This doc uses
`<codec>` as a placeholder.

## Why

The open-format work made a `.hock` a zip of `manifest.cbor` plus one
`media/<hash>.wav` per phrase. WAV is universally importable, so the file opens
anywhere, but it does not compress dense audio. FLAC was evaluated and rejected:
it is an integer codec and cannot store the engine's `f32` audio both losslessly
(quantizing breaks the content hash) and importably (bit-reinterpreting reads as
noise in a DAW). WavPack is the codec that *can*: it stores 32-bit float
losslessly and is read by real DAWs.

The gap: no pure-Rust WavPack codec exists. The one crates.io `wavpack`
(irh/wavpack-rs 0.4.0) is FFI bindings to the C library and needs a C toolchain,
so it cannot serve Hocket's wasm target. Symphonia PR #429 is a draft
reader/demuxer only (block splitting, not sample decode) and is stalled. Building
our own is the same move already proven with retinue (own Reticulum stack) and
muniment (own persistence): implement a public format ourselves and verify it,
bit for bit, against an authoritative oracle.

## Goal

A standalone crate that losslessly decodes and encodes the WavPack v5 stream for
mono and stereo PCM (16/24/32-bit integer and 32-bit float), pure Rust, no C,
wasm-capable. Reference-compatible so real DAWs open the output, and bit-exact on
32-bit float so Hocket's BLAKE3-over-decoded-samples media identity survives a
round trip. Hocket's `hocket-engine` is the first consumer, swapping its `hound`
WAV media codec for `.wv` entries.

## The honest scope correction

An earlier framing called the needed subset a "tiny profile" and implied it was a
small slice. Deeper reading corrects that: the part Hocket actually needs, lossless
32-bit float, is not small. Lossless float pulls in the full float sub-format (six
`float_flags` paths, signed zero, denormals, NaN payloads, infinities), the `wvx`
extension bitstream that carries the residual low bits, and the median-adaptive
entropy coder. That is most of the codec, not a corner of it.

So the scope is reframed: the *integer* decoder is the genuinely small first
target, and **32-bit float is its own milestone with the float sub-format
enumerated explicitly**, not a flag flipped on the integer path. This is a real
project on the order of retinue, not a weekend, and its center of mass is the
float round-trip and the entropy coder, not the happy path.

## Reference discipline

The reference is [dbry/WavPack](https://github.com/dbry/WavPack), the C codec, and
its CLI tools `wavpack` (encoder) and `wvunpack` (decoder). The method mirrors
retinue: nothing is "done" until it agrees, bit for bit, with the reference over a
corpus.

One difference from retinue works in our favor. Reticulum's reference carries
post-2025 license clauses, so retinue implements from the public-domain spec and
treats the Python reference as a black-box oracle whose code it does not read.
WavPack is **BSD-3-Clause** (David Bryant / Conifer Software), which is permissive
and one-way compatible with redistributing under MIT/Apache. So we may read and
port from the reference, subject to attribution:

- Ship `ATTRIBUTION.md` with the verbatim BSD-3-Clause notice and copyright lines
  for the reference and any consulted ports (soiaf's Go/Haxe ports and
  Tianscar's Java "tiny" port are also BSD-3-Clause, Peter McQuillan / Conifer).
- Header-comment any file that is a close port ("Portions derived from WavPack,
  BSD-3-Clause, see ATTRIBUTION.md"). Our own `SPDX-License-Identifier` stays
  `MIT OR Apache-2.0`.
- Honor clause 3 (no endorsement) by not naming the crate `wavpack*`.
- Keep `PROVENANCE.md` listing which reference files each module derived from.

The reference CLI is a dev/CI tool, never a `Cargo.toml` dependency, so no C
enters the shipped crate or the wasm build.

## Crate architecture and home

Own repo, own crate, published standalone to crates.io. Edition 2024,
`MIT OR Apache-2.0`. Leaf infrastructure: it knows nothing about `.hock` archives,
project files, or BLAKE3. `hocket-engine` calls it over in-memory buffers.

```text
<codec>/
  Cargo.toml         edition 2024, MIT OR Apache-2.0
  ATTRIBUTION.md     BSD-3 notices (reference + consulted ports)
  PROVENANCE.md      which reference file each module derived from
  src/
    format.rs        magic, version bounds, flag bits, metadata IDs   (always)
    bitstream.rs     LSB-first word-oriented bit reader/writer        (always)
    block.rs         32-byte header parse/emit + block iterator       (always)
    metadata.rs      sub-block framing + ID dispatch                  (always)
    sample.rs        sample buffers, bit-depth enum, channel layout   (always)
    decorr.rs        decorrelation: inverse (decode) + fixed forward (encode)
    entropy.rs       median-adaptive Rice-style model: decode + encode halves
    float.rs         f32 <-> WavPack float fields: decode + encode halves
    decode.rs        Decoder driver                       (feature = "decode")
    encode.rs        Encoder driver                       (feature = "encode")
    error.rs         error enum                                       (always)
  tests/conformance.rs   corpus round-trips + reference cross-check
  fuzz/                  cargo-fuzz target on the decoder (untrusted input)
```

**Features.** One crate, two additive features, `decode` and `encode`. The framing
modules build unconditionally. `decorr`/`entropy`/`float` each hold both directions
behind `#[cfg(feature = ...)]`, because the forward and inverse transforms share
constants and state and are far easier to keep in sync side by side. Default
`["decode"]` during the decode-first ladder; promote to `["decode", "encode"]`
once the encoder is oracle-clean. Hocket enables both.

**Dependencies: none at runtime, std-only.** No `build.rs`, no `cc`, no `cmake`.
Dev-dependencies only: `blake3` for the identity gate, a minimal WAV reader for
fixtures, `proptest`, and an optional spawn to the reference `wvunpack`/`wavpack`
when present. The reference's hand-written x86/ARM assembly inner loops are NOT
ported; we write scalar Rust and rely on autovectorization, and audit that no
intrinsics or non-portable unsafe leak into the wasm build.

**wasm** falls out of the architecture: pure computation over in-memory slices,
I/O pushed to the caller, so `wasm32-unknown-unknown` builds on the same path as
native. Done condition: the crate compiles for wasm in CI and a decode-then-BLAKE3
round-trip runs in a headless wasm test.

**Name candidates** (crates.io-free as of 2026-07-15; Mark chooses, and a
trademark/GitHub-org glance should precede the pick). Leaning into the family's
medieval-music-and-manuscript strand where a codec is a compact written encoding
of sound: **plica** (a fold; a folded neume; compression is folding), **quaver**
(eighth note; to vibrate; note the unrelated Quaver Music brand),
**cantus** (the fixed given melody line), **punctum** (the single-note neume),
**sonance** (the act of sounding). Backups: descant, plaint, wrack. Taken, avoid:
neume, quire, minim, breve.

## Format scope and the boundary

Target the v5 stream. Every `.wv` is a flat series of self-describing blocks; the
first audio block fixes the whole file's format. A block is a 32-byte `wvpk`
header followed by metadata sub-blocks (typed, length-prefixed, skippable when
unknown-and-optional).

**Version.** `MIN_STREAM_VERS = 0x402`, `MAX_STREAM_VERS = 0x410`,
`CUR_STREAM_VERS = 0x407`. Absent the compatibility flag, the reference encoder
stamps `0x410` (the bump exists to make mono optimization the default). So:
**decode accepts `0x402..=0x410`; encode stamps `0x410`** to match the reference
default, the corpus, and DAW output.

**In scope:** mono and stereo (first two channels); lossless only; 16/24/32-bit
integer and 32-bit float; joint stereo (mid/side) and cross-channel decorrelation;
`FALSE_STEREO` (mono data with stereo output, which the 0x410 reference emits by
default, so a decoder that ignores it corrupts stereo); non-standard sample rates
(carried in a metadata sub-block when the rate index is `0xF`); the header CRC and
the optional block/MD5 checksums.

**Rejected, with a clear error, never silently ignored:** DSD; hybrid/lossy and
its noise-shaping variants; `.wvc` correction files; more than two channels; pre-4.0
legacy streams (the reference routes those through a separate `unpack3` path).
Detection is by the header flags and by the presence of out-of-scope metadata IDs.

**Distinguish lossless samples from lossless file.** WavPack can also store the
original RIFF/WAV header chunks so the source file reproduces byte for byte. That
RIFF passthrough is out of scope: Hocket hashes decoded samples, not file bytes,
so sample-lossless is what the identity needs. Noted so it is a deliberate
omission, not an oversight.

## Decoder design (the smaller half)

Pipeline, mapping to the reference files:

1. **Block parse** (`block.rs`, `metadata.rs`): walk `wvpk` blocks, read the
   sub-block run, dispatch by ID. Model the reference `read_next_header` resync as
   the rejection filter.
2. **Entropy decode** (`entropy.rs`, from `read_words.c`): the median-adaptive
   Rice-style model, three median predictors per channel nudged by INC/DEC, run-of-
   zeros handling, escape codes. Not Huffman, not arithmetic coding (WavPack avoids
   both on purpose). This is the hardest correctness surface.
3. **Inverse decorrelation** (`decorr.rs`, from `decorr_utils.c`): apply the terms,
   weights, and samples the stream carries, with the exact weight-update step, then
   un-mix joint stereo. The decoder reads the decorr terms from the stream, so it
   needs none of the reference's decorrelation lookup tables.
4. **Integer reconstruction and shift.**
5. **Float unpack** (`float.rs`, from `unpack_floats.c`): only for float files;
   see the next section.
6. **CRC check.** The header carries a running 32-bit CRC over the decoded integer
   samples. We recompute and compare. Critically, on mismatch we raise a hard
   error. The reference silently zeroes (mutes) the block on mismatch, which for
   Hocket would corrupt the media identity hash with no signal, so we diverge here
   deliberately.

## Encoder design (the larger half) and the byte-identity non-goal

The encoder mirrors the decode transforms so the reference decoder reads its
output: forward decorrelation and weight update, joint-stereo decision, entropy
encode (`write_words.c`, the mirror of `read_words.c`), and float pack
(`pack_floats.c`).

A key scoping decision keeps the encoder small and the license clean. The
reference spends most of its encoder complexity (roughly 130 KB across `pack.c`,
`extra1.c`, `extra2.c`) adaptively *searching* for the best decorrelation terms,
driven by the four `decorr_tables.h` constant arrays. We do not need that. Any
conforming decoder, including any DAW, reads whatever fixed decorrelation
configuration we stamp into the block. So **the encoder ships one fixed, lossless
decorrelation configuration and skips the adaptive search entirely.** This has two
payoffs: it removes the hardest encoder code, and it sidesteps porting
`decorr_tables.h` (BSD-3 copyrighted numeric tables), so the crate stays clean
MIT/Apache with no table-derivation obligation.

The corresponding decision on the verification gate: **byte-identity with the
reference encoder is a non-goal.** Producing bytes identical to `wavpack` would
require porting the term-search heuristics, the exact block-sizing policy, and the
table-driven weight seeding, all of which are encoder *policy*, not format-mandated.
The honest, load-bearing gate is: our output, decoded by the reference `wvunpack`,
is lossless and matches the input, with valid CRC and MD5. A `.wv` that any DAW
opens and that round-trips bit-exact is the product requirement; matching the
reference's exact bytes is not.

## Float sub-format (its own milestone)

This is the part that earns the whole project, and the part most likely to hide a
silent identity break. WavPack reconstructs the exact 32-bit IEEE-754 pattern using
integer math, governed by `float_flags`: `FLOAT_SHIFT_ONES=1`, `FLOAT_SHIFT_SAME=2`,
`FLOAT_SHIFT_SENT=4`, `FLOAT_ZEROS_SENT=8`, `FLOAT_NEG_ZEROS=0x10`,
`FLOAT_EXCEPTIONS=0x20`. The main `wv` bitstream carries the aligned mantissa
integers; a second `wvx` extension bitstream carries the residual low bits that
would otherwise be lost. Signed zero, denormals, infinities, and NaN payloads each
ride their own path (the exception path stores the 23-bit NaN mantissa and sign).

The structural trap: the reference has two decode routines, `float_values` (used
when the `wvx` stream is present, lossless) and `float_values_nowvx` (no `wvx`,
lossy fill). An encoder that fails to open the `wvx` stream when the float scan
requires it produces a lossy file with **no error raised**, and the BLAKE3 identity
changes silently. So the float milestone's gate is explicit and mandatory:

- `decode(encode(x))` equals `x` compared via `f32::to_bits()`, never `==` (float
  `==` gives `NaN != NaN` and `-0.0 == +0.0`, both wrong for identity).
- `blake3(decoded_f32_le_bytes) == blake3(original_f32_le_bytes)`, with the byte
  order pinned to match exactly what Hocket hashes.
- Fuzz coverage over the values that can hide a break: `-0.0`, denormals, `±inf`,
  quiet and signaling NaN, full-scale.

Note also that 32-bit *integer* audio with more than 24 significant bits uses the
same `wvx` extension (via `ID_INT32_INFO`), so the extension stream is not
float-only. Easy to miss because it "looks like plain int."

## Conformance oracle and harness

The reference CLI is the oracle, kept entirely in dev/CI, never a crate dependency.
`wavpack` (encoder) and `wvunpack` (decoder) ship in one package: `apt-get install
wavpack` on Linux, `brew install wavpack` on macOS, a prebuilt zip on Windows. Pin
a 5.x version in the fixture manifest so a reference bump cannot silently move the
goalposts.

Two gates plus the float identity gate:

- **Decode gate:** decode a reference `.wv` with our crate and with `wvunpack -r`;
  compare at the typed-sample level (sign-extended `i32` for int, `f32::to_bits()`
  for float), never as raw bytes (24-bit raw is 3-byte packed). Independently, run
  `wvunpack -mv` so the stream's own CRC and MD5 confirm internal consistency.
- **Encode gate:** encode a generated buffer with our crate, decode it with the
  reference `wvunpack -r`, diff against the input. This proves a real reference
  decoder, and therefore a real DAW, accepts our output. Add a self round-trip and
  a `wvunpack -ss` header-field assertion (bit depth, channels, float flag,
  version 0x410).
- **Float identity gate:** as above, the `to_bits` + BLAKE3 check.

**Property tests** (`proptest`): round-trip self-consistency over random
`i16/i24/i32/f32` buffers, mono and stereo, runs everywhere including wasm; plus a
reference cross-check in the reference CI job. Enumerated edge vectors: silence,
full-scale ±, single sample, odd lengths, false-stereo, low-bits-zero, and the
float special values.

**CI in two layers**, so the published crate never needs a C toolchain:

- **Layer 0, everywhere (incl. wasm):** decode checked-in golden vectors, run the
  round-trip and float/BLAKE3 property tests. No `wavpack` binary, no network.
- **Layer 1, reference job (Linux):** `apt-get install wavpack`, run the encode
  gate and the reference cross-check over a fetched corpus, and `wvtest --default`
  (192 tests) as a reference-health check. Only this layer regenerates golden
  vectors.

**Fixtures** are self-generated from deterministic seeded signal buffers (tone
sweeps and noise bands, mirroring the reference's own generators), so we own and
can license them and keep the whole set well under a megabyte. The 174 MB
`test_suite.zip` and FFmpeg FATE samples are fetched only in Layer 1, not vendored.
FFmpeg's decoder is a weak secondary oracle only (it downconverts above 16-bit by
default), never the bit-exact authority.

## Milestone ladder (decode-first, each gated)

Every milestone is done only when green on both its Layer 0 golden vectors and its
Layer 1 reference diff. No milestone is trusted on self round-trip alone, so a
shared bug across our own encode and decode cannot pass itself.

- **M0 Block parse.** `wvunpack -ss` fields reproduced by our reader for every
  corpus file; out-of-scope streams rejected with a clear error.
- **M1 Integer decode (16-bit).** Our decode equals `wvunpack -r` for all 16-bit
  fixtures and corpus files; all block CRCs validated. *M1 alone is the first
  pure-Rust WavPack decoder in existence, and could optionally be offered to
  Symphonia while we keep our own crate.*
- **M2 Integer decode (24/32-bit).** Same, including the `wvx` extension for >24-bit
  integers.
- **M3 Float decode.** Same, plus the `f32::to_bits()` and BLAKE3 identity checks
  over the float special-value set.
- **M4 Integer encode.** Our `.wv` decoded by `wvunpack -r` equals input; `-mv`
  clean; header fields correct. Fixed decorrelation config, no adaptive search.
- **M5 Float encode.** Same, plus `blake3(decode(encode(x))) == blake3(x)` over the
  float fuzz set, and an assertion that the `wvx` stream is opened whenever the
  float scan requires it.
- **M6 Hocket integration.** `hocket-engine` `project_store` writes `media/<hash>.wv`
  and reads it back, hash preserved; the `.hock` round-trip and `wvunpack`
  cross-check pass.

## Risk register

- **Entropy-coder bit-exactness** (both directions must agree with the reference).
  Mitigation: the median model is small and has three readable non-C ports
  (Go/Haxe/Java) to triangulate, plus the reference corpus catches any drift
  immediately.
- **Silent float identity break** (encoder skips the `wvx` stream, produces lossy
  float, no error). Mitigation: the mandatory round-trip BLAKE3 gate, plus a direct
  assertion that `wvx` opens when the float scan asks.
- **CRC silent-mute divergence.** Mitigation: we raise a hard error on CRC mismatch,
  unlike the reference.
- **`FALSE_STEREO` overlooked** (corrupts default-encoded stereo). Mitigation: a
  false-stereo fixture in Layer 0.
- **Hidden non-Rust dependency** (accidental intrinsics, or the reference leaking
  into the crate build). Mitigation: the reference is dev-only; wasm CI is a gate;
  audit for target-feature and unsafe.
- **`decorr_tables.h` license entanglement.** Mitigation avoided by design: the
  fixed-config encoder needs no tables; the decoder reads terms from the stream.
- **Scope creep toward `wvtest`'s 192 tests** (multichannel/hybrid/lossy).
  Mitigation: those are explicit non-goals; `wvtest` is a reference-health check,
  not our gate.
- **Reference version drift.** Mitigation: pin the `wavpack` version in the fixture
  manifest.

## Hocket integration (M6)

`hocket-engine`'s `project_store.rs` currently encodes media as WAV via `hound`.
M6 swaps that for `.wv` via `<codec>`: `media/<hash>.wv` entries, still hashed over
the decoded `f32` samples so `MediaRef` identity is unchanged. Open decisions for
that step:

- **WAV fallback or `.wv`-only.** Keeping a WAV path (for example on wasm until the
  wasm codec build is proven, or as a user-facing export) hedges risk; going
  `.wv`-only is simpler. Leaning `.wv`-only once M5 is oracle-clean, with WAV
  remaining an *export* option for users who want a plain file.
- **No on-disk migration needed:** no `.hock` file exists yet, so this is a clean
  cut, same as the earlier format changes.

## Open decisions for the maintainer

- **Whether to build it at all, and when.** Deflate already gives Hocket the easy
  size wins (silence), so this is not urgent. It is a real ecosystem-first artifact
  and a good fit for the reference-oracle method, but it is retinue-scale work.
- **The crate name** (shortlist above), and a trademark/GitHub-org check before
  reserving it.
- **Public sample API:** interleaved `i32`/`f32`, planar per-channel, or a
  streaming `Read`/`Write` interface. This shapes how `hocket-engine` feeds the
  BLAKE3 hash and should be pinned before the `Decoder`/`Encoder` signatures freeze.
- **Offer M1 (the decoder) to Symphonia** as an ecosystem contribution while
  keeping our own crate, or keep it entirely in-family (the retinue posture).
- **Byte order for the float identity hash** (native vs little-endian), pinned to
  match what Hocket hashes, so the codec and consumer cannot disagree on identity
  while both being locally correct.

## Prior art and references

- [dbry/WavPack](https://github.com/dbry/WavPack) (C reference, BSD-3-Clause):
  [wavpack.h](https://github.com/dbry/WavPack/blob/master/include/wavpack.h),
  [wavpack_local.h](https://github.com/dbry/WavPack/blob/master/src/wavpack_local.h),
  [read_words.c](https://github.com/dbry/WavPack/blob/master/src/read_words.c),
  [write_words.c](https://github.com/dbry/WavPack/blob/master/src/write_words.c),
  [decorr_utils.c](https://github.com/dbry/WavPack/blob/master/src/decorr_utils.c),
  [pack_floats.c](https://github.com/dbry/WavPack/blob/master/src/pack_floats.c),
  [unpack_floats.c](https://github.com/dbry/WavPack/blob/master/src/unpack_floats.c),
  [cli/wvtest.c](https://github.com/dbry/WavPack/blob/master/cli/wvtest.c).
- [WavPack 4 & 5 File Format spec (2020)](https://www.wavpack.com/WavPack5FileFormat.pdf).
- Readable non-C ports (BSD-3-Clause): [soiaf/Go-WavPack-encoder](https://github.com/soiaf/Go-WavPack-encoder),
  [soiaf/Haxe-WavPack-Decoder](https://github.com/soiaf/Haxe-WavPack-Decoder),
  [Tianscar/javasound-wavpack](https://github.com/Tianscar/javasound-wavpack).
- [Symphonia PR #429](https://github.com/pdeljanov/Symphonia/pull/429) (draft
  pure-Rust reader), [irh/wavpack-rs](https://github.com/irh/wavpack-rs) (the
  C-bindings crate this replaces).
- Oracle tooling: [wvunpack(1)](https://manpages.debian.org/testing/wavpack/wvunpack.1.en.html),
  [wavpack(1)](https://manpages.debian.org/testing/wavpack/wavpack.1.en.html).

## Progress

- 2026-07-15: Founding plan drafted from parallel research verified against the
  reference source. Scope reframed (float is its own milestone, not a tiny flag);
  encoder byte-identity set as a non-goal in favor of a fixed-config,
  reference-decodable gate; `decorr_tables.h` license entanglement designed out.
  Not started; awaiting a build decision.
