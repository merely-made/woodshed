# Founding plan: a pure-Rust WavPack codec crate

**Status (2026-10-31): round-trip codec implemented, M0-M6 landed; bounded malformed-input arithmetic repair landed for Woodshed capture storage (2026-10-12). Open gates are listed under Open work.**
Endorsed by Mark 2026-07-15; name decided the same day (his pick: Eddington's
wave-particle portmanteau — a lossless codec makes audio both wave and bits,
one identity in both observations; crates.io was free, `waver`/`wavelet` taken,
`waveshaper` rejected as another technique's name). This repo is the crate; the
0.0.1 name claim is published. This doc migrated here from Hocket's design_docs
(where an archived copy remains). First consumer: Hocket's media codec, wired
2026-07-18 (M6).

This doc predates the name in places; read `<codec>` as `wavicle`.

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
round trip. Hocket's `hocket-engine` (now `hocket-project` in Woodshed) is the first
consumer, swapping its `hound` WAV media codec for `.wv` entries.

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
    decorr.rs        decorrelation: inverse (decode) + fixed forward (encode)
    entropy.rs       median-adaptive Rice-style model: decode + encode halves
    float.rs         f32 <-> WavPack float fields: decode + encode halves
    decode.rs        Decoder driver                       (feature = "decode")
    encode.rs        Encoder driver                       (feature = "encode")
    error.rs         error enum                                       (always)
  tests/conformance.rs   corpus round-trips + reference cross-check
  fuzz/                  cargo-fuzz target on the decoder (untrusted input)
```

The planned `sample.rs` was never built: `DecodedStream` lives in
`src/decode.rs` and `EncodeParams` in `src/encode.rs`, and `bitstream.rs` and
`format.rs` hold the bit reader/writer and format constants.

**Features.** One crate, two additive features, `decode` and `encode`. The framing
modules build unconditionally. `decorr`/`entropy`/`float` each hold both directions
behind `#[cfg(feature = ...)]`, because the forward and inverse transforms share
constants and state and are far easier to keep in sync side by side. Default
`["decode"]` during the decode-first ladder; promote to `["decode", "encode"]`
once the encoder is oracle-clean. Hocket enables both. Promoted 2026-10-31
(wavpack_codec D3).

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

Historical: the name was decided 2026-07-15 as `wavicle` (see Status).
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
  cross-check pass. *Landed 2026-07-18* (Hocket `0d7e26c`; the code is now
  `woodshed/crates/hocket-project/src/project_store.rs`); the `wvunpack` leg
  was run 2026-10-31 (Progress).

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

`hocket-engine`'s `project_store.rs` (now
`woodshed/crates/hocket-project/src/project_store.rs`) encoded media as WAV via
`hound` before M6.
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

- *Resolved 2026-07-15: endorsed and founded (Status).* **Whether to build it at all, and when.** Deflate already gives Hocket the easy
  size wins (silence), so this is not urgent. It is a real ecosystem-first artifact
  and a good fit for the reference-oracle method, but it is retinue-scale work.
- *Resolved 2026-07-15: `wavicle`.* **The crate name** (shortlist above), and a trademark/GitHub-org check before
  reserving it.
- *Settled 2026-10-31 (D1).* **Public sample API:** interleaved `i32`/`f32`, planar per-channel, or a
  streaming `Read`/`Write` interface. This shapes how `hocket-engine` feeds the
  BLAKE3 hash and should be pinned before the `Decoder`/`Encoder` signatures freeze.
- **Offer M1 (the decoder) to Symphonia** as an ecosystem contribution while
  keeping our own crate, or keep it entirely in-family (the retinue posture).
- *Settled 2026-10-31 (D2).* **Byte order for the float identity hash** (native vs little-endian), pinned to
  match what Hocket hashes, so the codec and consumer cannot disagree on identity
  while both being locally correct.

## Decisions

- **D1, the public sample API is interleaved slices (2026-10-31), settles an open decision:** Mark: "Ratify as shipped." Integer samples are interleaved `i32` and float samples interleaved `f32`: `decode_stream` returns `DecodedStream`, `encode_int(EncodeParams, &[i32])` and `encode_float(channels, sample_rate, &[f32])` take interleaved slices. Hocket already depends on this shape.

  Links: area codec API · plans wavpack_codec · summary The public sample API is interleaved i32/f32 slices, as shipped.

- **D2, the float identity hash is little-endian (2026-10-31), settles an open decision:** Mark: "Ratify as shipped." The BLAKE3 identity over decoded float samples hashes `to_le_bytes`, as wavicle's conformance tests and Hocket's `media.rs` both do.

  Links: area codec API · plans wavpack_codec · summary Float identity hashes little-endian sample bytes.

- **D3, both directions are default features (2026-10-31), refines the Features design:** Mark: "Promote to decode+encode." The encoder is oracle-clean, so `default = ["decode", "encode"]`; a reader-only build takes `default-features = false, features = ["decode"]`. Woodshed pins wavicle with explicit features and is unaffected.

  Links: area build · plans wavpack_codec · summary Default features are decode and encode; either builds alone.

## Open work (2026-10-31)

Gates this plan specifies that were never built, collected from the sections
above. None blocks the shipped codec.

- Property tests (`proptest`) and a fixture WAV reader; dev-dependencies are
  `blake3` only.
- A headless wasm decode-then-BLAKE3 test; CI only compiles for wasm.
- `wvunpack -mv` and `-ss` header assertions (bit depth, channels, float,
  version 0x410) on encoder output in the test suite, and a direct assertion
  that `wvx` opens when the float scan asks; the encode tests use
  `wvunpack -r` only.
- MD5 validation; `format.rs` defines the metadata IDs only.
- `wvtest --default` and a pinned `wavpack` version in CI (the reference job
  installs the unpinned distribution package).
- Resynchronization over leading tags, deferred at M0 (`src/block.rs`).
- A float-specials fuzz set; the fuzz target is decoder-only on arbitrary bytes.
- Fixtures come from the reference encoder over deterministic PCM
  (`tests/fixtures/gen.py`), not the seeded self-generated buffers planned.
- Whether to offer the decoder to Symphonia (Open decisions).
- `cargo clippy --all-targets -D warnings` fails on Rust 1.99.1
  (`chunks_exact_to_as_chunks`, ten sites in `decode.rs`, `decorr.rs`,
  `encode.rs`), so CI's clippy step is red since the 2026-10-18 toolchain move.

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
- 2026-07-15: Endorsed; founded as `wavicle` (repo pushed, crates.io 0.0.1
  published with Mark's one-time authorization).
- 2026-07-15: **M0 LANDED.** Oracle pinned: reference CLI 5.9.0 x64 at
  `Code/testing/wavicle/tools` (outside the repo, per the no-C rule). Ten
  fixtures generated by the reference encoder from deterministic raw PCM
  (`tests/fixtures/gen.py`): 16/24/32-bit int, f32, mono/stereo, false-stereo,
  silence, multiblock (`--blocksize=4096`, 5 blocks), and a hybrid-lossy
  rejection case; `wvunpack -ss` output checked in as goldens. Implemented
  `format` (constants, `Flags` accessors), `error` (typed scope rejection),
  `metadata` (sub-block framing + scope gate), `block` (header parse, block
  iterator, `StreamInfo::scan`). Gate green: scan reproduces the goldens'
  bits/float/channels/rate/modality/duration/version for all nine lossless
  fixtures (goldens parsed from the `.ss.txt` files, not transcribed); hybrid
  rejects as `OutOfScope(Hybrid)`; synthetic bad-version/DSD/magic reject;
  multiblock counts 5 contiguous blocks; `wasm32-unknown-unknown` check clean.
  Learned from the gate itself: the reference renders duration with
  round-to-nearest centiseconds (two candidate renderings each failed one
  fixture before the corpus disambiguated). Strict block alignment only;
  resynchronization over leading tags is deferred to M1 with the corpus.
- 2026-07-15: **M1 LANDED — the first pure-Rust WavPack audio decode, oracle-
  verified.** Ported from the 5.9.0-tag reference (matching the pinned oracle;
  sources kept outside the repo): the LSB-first bitstream reader with the
  reference's shift-register semantics (`read_code` and the unary/escape
  counts, portable variant), the median-adaptive entropy decoder
  (`get_words_lossless`, `wp_exp2s` + table, INC/DEC median updates), inverse
  decorrelation (terms 17/18/1..8/-1/-2/-3, dual-path `apply_weight`, the
  sign-trick `update_weight` and the +/-1024 clip), the joint-stereo un-mix,
  both CRC formulas (seed `0xffffffff`, confirmed in `open_utils.c`), the
  lossless shift fixup, and false-stereo expansion. Feature split live:
  `default = ["decode"]`; M0 parsing builds with no features. Gate green:
  decode is sample-for-sample identical to `wvunpack -r` for all five 16-bit
  fixtures (mono, joint stereo, false-stereo, silence/zero-run, 5-block
  multiblock); a corrupted payload fails the CRC as a hard error (deliberate
  divergence: the reference mutes, wavicle errors); wasm32 and
  no-default-features builds clean. One porting lesson recorded: within
  `read_code` the first-read bit is the code's HIGH bit even though bits
  stream LSB-first from bytes — a wrong test expectation caught by the
  implementation, not the reverse. PROVENANCE.md now records per-module
  lineage at tag 5.9.0.
- 2026-07-15: **M2 LANDED — 24/32-bit integer decode.** 24-bit needed nothing
  beyond wider samples (the M1 path already carries i32). 32-bit brought the
  real work: `ID_INT32_INFO` (sent_bits/zeros/ones/dups), the `wvx` extension
  bitstream in both forms (`0x0c` classic; `0x2c` "new" with the 5-bit
  max-width prefix), its leading 4-byte stored CRC, `getbits` (u64-accurate
  port of the reference macro), the per-sample restoration paths
  (bit-reinsertion with the max-width geometry; zeros/ones/dups transforms;
  the shift-fold case), and the second CRC (`crc_x`, seed `0xffffffff`)
  checked against the stream's `crc_wvx` as a hard error. New fixture
  `int32_shifted_mono` (12 low zero bits) exercises the zeros path; the 5.9.0
  encoder emitted a classic `0x0c` wvx for it. Gate green: decode
  sample-identical to `wvunpack -r` for all eight integer fixtures; wasm
  clean. The wvx machinery is the shared substrate float needs, so M3 starts
  from a working extension stream.
- 2026-07-15: **M3 LANDED — bit-exact float decode. This is the milestone the
  crate exists for.** Ported `float_values` and `float_values_nowvx` from
  `unpack_floats.c` plus the f32 sign/exponent/mantissa accessor macros: the
  six `float_flags` paths (SHIFT_ONES/SAME/SENT, ZEROS_SENT, NEG_ZEROS,
  EXCEPTIONS), the wvx float prefix (`min_shifted_zeros`/`max_shifted_ones`
  for the new format; 0 for classic), and the `crc_x` (seed `0xffffffff`,
  `crc*27 + mantissa*9 + exponent*3 + sign`) checked against `crc_wvx`. Float
  reconstruction is integer-only, producing exact IEEE-754 bit patterns; the
  block CRC still validates the aligned-mantissa integers before expansion,
  and float takes its own fixup branch (no header shift; `float_shift` applies
  inside). Gate green on three float fixtures including a new adversarial
  `f32_specials_mono` (+/-0.0, smallest/largest denormals, +/-inf, quiet and
  signaling NaN with payloads, max normal): **every sample's 32-bit pattern
  matches `wvunpack -r` exactly** (compared as bits, never float `==`), and
  **the BLAKE3 hash of the decoded f32 bytes equals the reference's** — the
  media-identity invariant Hocket keys on, proven end to end. wasm and
  no-default-features clean. The full decoder is now complete for the tiny
  profile: 16/24/32-bit int and 32-bit float, mono/stereo, all lossless.
- 2026-07-15: **M4 LANDED — the integer encoder. wavicle now writes .wv the
  reference decodes losslessly.** Ported the encode half from the 5.9.0
  reference: the `BitWriter` (putbit/putbits + the pad-to-even bs_close_write),
  the median entropy encoder (`send_words_lossless` + `flush_word`, the exact
  mirror of the decoder including the zero-run and LIMIT_ONES escape encodings),
  and the forward decorrelation passes (`decorr_mono_buffer` /
  `decorr_stereo_pass`, the exact inverse of the decode passes). The driver
  assembles a single block in the reference's metadata order (decorr terms/
  weights/samples, entropy vars, wv bitstream), computes the block CRC over the
  original samples, and stamps version 0x410. Two decisions from the plan held:
  the encoder ships ONE fixed decorrelation term (no adaptive search, no
  decorr_tables.h), and the gate is round-trip through the reference, not
  byte-identity. Gate green: seven generated buffers (16-bit mono/stereo/
  silence/full-scale, 24-bit stereo, 32-bit mono, 16-bit ramp) each round-trip
  losslessly through BOTH our own decoder AND the reference `wvunpack -r`.
  Feature split proven: default `decode`, `--features encode` adds the writer,
  and encode-only, decode-only, wasm, and combined builds all compile. Two
  lessons recorded: (1) the entropy coder needs small residuals, so zero
  decorrelation is not viable (a full-magnitude first residual emits ~2^N unary
  bits and overflows the u32 medians) which is exactly why decorrelation exists;
  (2) the magnitude header field must be the actual data magnitude, not the
  nominal bit depth, or a 32-bit stream's mute limit overflows and the reference
  rejects it. Made the median updates wrapping to match the reference's uint32_t
  exactly. Default features stay `decode` until the encoder is complete (float
  at M5). Next: M5, float encode (`pack_floats.c`: the scan/align + wvx
  residual), the last milestone before Hocket integration.
- 2026-07-15: **M5 LANDED — bit-exact float encode. wavicle is now a complete
  round-trip WavPack codec for the tiny profile.** Ported `scan_float_data`
  (converts IEEE floats to the integer mantissas WavPack compresses, computes
  the wvx `crc_x`, and picks the shift/zeros flags) and `send_float_data`
  (writes the residual bits over the saved-original floats) from
  `pack_floats.c`, plus the classic `0x0c` wvx sub-block with its 4-byte crc
  prefix (the reference uses classic, not the new form, for all float data).
  The driver saves the original float patterns before the scan converts them,
  computes the block CRC over the mantissa integers, and emits float_info + wv
  + wvx in the reference order. Refactored encode into a shared `assemble_block`
  used by both int and float. Gate green: four float cases (mono, stereo,
  silence, and an adversarial specials buffer with signed zero, smallest/
  largest denormals, +/-inf, quiet and signaling NaN with payloads, max normal)
  each round-trip bit-exact through our own decoder, the reference
  `wvunpack -r`, AND a BLAKE3 identity check over the decoded f32 bytes. Full
  matrix builds (decode-only, encode-only, both, wasm). **The codec is
  round-trip complete**: encode and decode, 16/24/32-bit int and 32-bit float,
  mono/stereo, all lossless and oracle-verified. Remaining before shipping to
  Hocket: multi-block encode (files over 131072 frames), then M6 integration
  (project_store media .wav -> .wv). Decorrelation is still a single fixed term,
  so files are larger than the reference's; a format-compatible compression
  follow-on, not a correctness gap.
- 2026-07-15: **Multi-block encode landed.** Long inputs (over 32768 frames)
  now split into independent blocks: each block carries its own complete
  starting state (zero medians, zero decorrelation), so the decoder needs no
  cross-block continuity, which keeps the encoder simple and definitely
  correct. Block size 32768 frames stays well under the 131072-frame and 1 MB
  block limits. The header now carries the running block_index and the file
  total_samples (on every block; decoders read it from block 0). Per-block
  re-adaptation overhead is negligible because large residuals are compactly
  escape-coded, not emitted as literal unary runs. Gate: 80000-frame integer
  and float inputs split into 3 contiguous blocks and round-trip losslessly
  through our own decoder and the reference `wvunpack`. The encoder now
  handles inputs of any length. The only remaining step before wavicle can
  back Hocket media is M6 (project_store .wav -> .wv wiring); the codec itself
  is feature-complete for the tiny profile.

> **Superseded on licensing, 2026-09-03.** The lines above that keep
> `MIT OR Apache-2.0` for our own SPDX record the July 2026 convention. The
> 2026-08-22 license posture brief (mere `design_docs/2026-08-22_license_posture_brief.md`)
> ruled MPL-2.0 by default with no exceptions, and the sweep landed on this
> repository on 2026-09-03: every owned source carries Exhibit A and
> `SPDX-License-Identifier: MPL-2.0`, the nine WavPack-derived modules keep
> David Bryant's notice, and the published 0.1.0 keeps the grant it shipped
> with. See `LICENSES.md`.


### 2026-10-12: Woodshed compressed-capture intake

Woodshed's bounded lossless capture profile exposed concrete malformed-input
arithmetic failures before CRC validation. Header parsing now checks `ck_size + 8`
without overflow, and the entropy reader refuses an escaped count that cannot
represent the added unary limit. Median updates and residual intervals use
explicit reference-compatible unsigned wrapping; signed history expansion uses
`unsigned_abs` and `wrapping_neg`, including the `INT32_MIN` result. These changes
preserve the encoded format and ordinary decoded sample identities.

Seven unit cases and fourteen existing fixture/round-trip cases pass with both
codec features, including four new arithmetic regressions and the existing
integer, float-bit-identity, multi-block, CRC and mutated-input cases. This is
bounded evidence for the repaired paths, not a claim that arbitrary streams
are fully certified. Runtime dependencies remain empty; inherited WavPack
notices and MPL headers remain intact.

The whole-stream decode API still allocates per audio block and does not enforce
a consumer's total decoded-frame budget or all cross-block format constraints.
Woodshed therefore preflights its own mono float32 profile, complete framing,
contiguous indices, declared/actual frame sum, rates and resource bounds before
calling the decoder. Its versioned companion envelope, namespace, sealing and
session-last publication belong to Woodshed, not this codec. Hocket's independent
pin is not changed by this intake.

### 2026-10-31: landings recorded, M6 cross-check, defaults promoted

Landings from 2026-07-18 that this log had not recorded: non-standard sample
rates via `ID_SAMPLE_RATE` (`9aa4688`); the five-pass fixed decorrelation
cascade (`b423f6f`, `FIXED_DECORR` in `src/encode.rs`), which supersedes the
M5 entry's single fixed term and brings sizes near the reference's, guarded by
`encoded_size_is_reasonable_versus_raw_and_reference`; CI, the decoder fuzz
target and robustness hardening (`faa3859`); release 0.1.0 (`4ac1134`). M6
landed in Hocket the same day (`0d7e26c`).

The M6 `wvunpack` leg was run against the pinned 5.9.0 CLI in
`Code/testing/wavicle/tools`: Hocket's exact call, `encode_float(1, rate,
samples)`, over a 4800-frame and a 200000-frame (multi-block) phrase at
48000 Hz, a 70000-frame phrase at 37800 Hz and 40000 frames of silence at
44100 Hz. Each verifies lossless under `wvunpack -mv`, reports 32-bit float
mono at its rate under `-ss`, and `-r` returns bytes identical to the input
`f32` little-endian samples. A copy with one corrupted byte fails `-v` with a
CRC error.

Default features promoted to `["decode", "encode"]` (D3); CI's bare
`cargo test` leg became `--no-default-features --features decode` so the
reader-only build stays covered. 7 unit and 14 conformance cases pass with
the defaults and 7 and 9 reader-only, with the reference CLI on `PATH`;
encode-only checks.


## H7: conformance source groups (2026-11-03)

**Status:** landed; 1/1 additional source path qualified.

This bounded house-style pass splits `tests/conformance.rs` into existing
fixture and round-trip responsibilities. It preserves reference fixtures,
pinned oracle tools, bit identities, error variants, feature guards and test
assertions. Mere's house-style plan owns H7's fifty-file aggregate and its
shared 100 MB retained receipt cap; this codec contributes one original.

The phase is done when the original and every child fit 600 physical lines,
the exact exception retires, matching baseline/post default and reader-only
suites and an encode-only check pass, the full local and actual commit hooks
pass, and the source is committed and pushed. Oracle legs run when the pinned
local CLI is available. These checks do not qualify physical audio devices
or broaden codec formats. Existing M0-M6 open gates retain their scopes.

### H7 findings

- 2026-11-03: the sole remaining exception has independent decode, encode
  and malformed/round-trip regression groups; original captured bytes stay.
- Compact run evidence appends to
  `archive_docs/receipts/2026-07-15_wavpack_codec_plan_receipts.md`.

### H7 progress

- 2026-11-03: selected the 882-line conformance source from clean main;
  baseline checks precede its edits.

- 2026-11-03: H7 conformance split lands: the original is 382 lines and encoding fixtures are 510; its exception retires. Default/reader-only inventories match baseline, encode-only and docs checks and full/commit hooks pass. Existing codec acceptance gates are unchanged.
