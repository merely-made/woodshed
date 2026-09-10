# Open, Non-Lock-In Project Format

## Doctrine (maintainer direction, 2026-07-14)

A `.hock` file is a container, not a destination. The audio a musician makes in
Hocket is theirs, and the format must never be the thing that traps it. The
failure mode to avoid is the DAW-industry norm: a specialty project format that
is useless without the originating app or a bespoke converter. That is lock-in,
and Hocket rejects it as a matter of product identity, the same way it rejects
plugin-hosting gravity and DAW-shaped scope creep.

Two obligations follow:

1. **The container should be inspectable and un-lockable without Hocket.** A
   third party, a future web build, or the user with a zip tool should be able
   to open a `.hock` file and get at the material.
2. **The material inside should be standard, importable formats** — audio as
   ordinary WAV/FLAC, structure as a documented, non-proprietary serialization.
   "Export" should be a thin operation over what is already stored, not a
   lossy re-derivation.

This is a stated product value, not just an implementation preference. Surface
it into `PROJECT_DESCRIPTION.md` when the maintainer wants it at goal level.

## Realized shape (LANDED 2026-07-14)

A `.hock` file **is** a zip archive — the genre-standard answer (Renoise `.xrns`,
`.docx`, `.odt`, `.epub`, Scratch `.sb3`) — with these entries:

```text
mysession.hock            (a zip)
  manifest.cbor           session + history, CBOR
  media/<blake3-hex>.wav  one mono 32-bit-float WAV per phrase, content-addressed
```

The OS still associates `.hock` with Hocket, but any zip tool opens it and the
audio imports anywhere — verified by opening a saved file with .NET's zip reader
(an implementation independent of the Rust `zip` crate that wrote it) and
confirming a valid `RIFF`/`WAVE` payload. No `meta.json` yet; add one if a
version/app-provenance record proves useful.

### How the Muniment tension resolved

The earlier draft framed this as "zip file vs. KV seam," with the KV seam
(browser OPFS/IndexedDB) as the thing at risk. **That was a false choice, and the
maintainer caught it: a zip is just another `Backend`.** So the seam is kept and
the archive lives *under* it — Muniment gained a `ZipBackend` (`zip` feature)
whose entry names are the store's keys. `ProjectStore` stays backend-agnostic;
only the media-value codec and the key names changed on the Hocket side. The
browser story is intact (a future OPFS-backed store still slots into the same
seam), Muniment earned a second real backend, and no dependency was dropped.

`ZipBackend` is snapshot-oriented: it holds the archive in memory and rewrites
the whole file atomically (temp + rename) on each mutating call. That fits
Hocket's whole-project `apply` exactly; it is the wrong backend for
high-frequency incremental appends, which stay on redb. Documented as such in
muniment.

### Decisions locked in

- **Content addressing over decoded audio.** `MediaRef` is BLAKE3 over samples +
  sample rate. The WAV file is a carrier: on load the reference is re-verified
  against the *decoded* samples, not the file bytes, so `MediaRef` identity is
  unchanged by the format move.
- **WAV now, FLAC later.** Mono 32-bit-float WAV via the `hound` dependency
  Hocket already had (its exporter uses it). FLAC is lossless and smaller but
  needs a new codec dependency and has no strong pure-Rust encoder yet; deferred
  as a size optimization, not correctness.
- **`Stored` (no zip compression).** Keeps Muniment's `zip` dependency free of a
  codec (no flate2). Revisit if archive size matters; FLAC media would help more.
- **Clean break, no shim.** No `.hock` file existed on disk, so the old
  redb/`HOCKMED\0` framing has no read path (DOC_POLICY section 3).

## The CBOR half (structure serialization)

The manifest (`ProjectBundle` = session + history) is serialized today with
[postcard](../crates/hocket-model/src/persistence.rs). Postcard is compact but
Rust-only and *not self-describing*: you cannot decode a postcard blob without
the exact Rust types. That is itself a lock-in property, which is why the
already-planned move to CBOR (ciborium) serves this doctrine, not just Moothold
alignment.

Why CBOR specifically:

- **Self-describing and standard.** CBOR (RFC 8949) is an IETF standard with
  implementations in every language; a `.hock` manifest becomes inspectable by
  any CBOR tool, no Hocket required.
- **One format across storage and sync.** Moothold speaks CBOR (blobs,
  IndexCommits). If the at-rest manifest is already CBOR, the FT9 hand-off can
  put the same bytes on the wire with no transcode step.

The move is small because everything is serde-derive: swap
`postcard::to_allocvec` / `from_bytes` for ciborium's writer/reader calls and
the `PersistenceError` inner type. Two things are *not* mechanical and must be
handled:

- **Deterministic encoding is a hard requirement, not a nicety.** The bundle is
  meant to be content-addressable, so equal state must encode to equal bytes.
  Postcard gets this free from the `BTreeMap` collections. CBOR does *not* by
  default — it needs RFC 8949 §4.2 core-deterministic rules (sorted keys,
  shortest-form integers, definite lengths). Verify ciborium emits canonical
  CBOR, or add a canonicalization step, before relying on hashed bundles.
- **Encoding discriminator.** `format_version` is the first field but you cannot
  read it without already knowing the encoding. A clean CBOR-only cut sidesteps
  this; a read-both transition would need an out-of-band magic byte.

Because no `.hock` file exists on disk, this was a **clean break** to
CBOR-only with no read-both shim (DOC_POLICY section 3).

**LANDED 2026-07-14 (ahead of the rest of FT8).** The manifest
(`ProjectBundle`) and the hand-off envelope both serialize as CBOR via ciborium;
postcard is removed from the workspace. `FORMAT_VERSION` stayed at 1 on purpose:
it tracks the payload *schema*, which did not change, and it cannot discriminate
encodings anyway (you cannot read the version without already knowing the
encoding). The hand-off signature is computed over the CBOR bytes of
`UnsignedHandoff`; signing and verification re-serialize the same value, and CBOR
over the envelope's `BTreeMap`/`Vec` types is deterministic, so verification
reconstructs identical bytes — covered by the existing round-trip, tampering, and
determinism tests. Pulling this out of FT8 early was safe because the manifest
and envelope are Hocket's own types, not Moothold's message schema; only the
FT9 wire protocol needs Moothold coordination. Still open for FT8: the media
half (zip container of standard audio) and whatever Moothold-schema alignment
FT9 needs.

## Why not FLAC (decided against, 2026-07-15)

The earlier follow-on list named FLAC as a size optimization. It does not fit
this audio model. **FLAC is a lossless codec for *integer* PCM**; Hocket's audio
identity is `f32` (`hash_buffer` hashes the raw little-endian float bytes). Two
ways to force float into FLAC, both bad:

- **Quantize `f32` -> integer.** Lossy: `f32` carries precision below any fixed
  integer LSB, so the decoded samples differ and `hash_buffer(decoded)` no longer
  equals the reference — content-addressing breaks.
- **Reinterpret the `f32` bit pattern as `i32`.** Bit-exact, but a DAW opening
  that FLAC reads the *integers*, i.e. noise, not the original audio — which
  defeats the whole importability point of the doctrine.

So FLAC can be lossless *or* importable for float audio, not both. It only
becomes an option if the capture pipeline moves to integer PCM, which is an
audio-model change, not a format tweak.

The size intent was met differently: the archive is **Deflate-compressed**
(miniz_oxide via the zip `deflate` feature). A probe on representative `f32`
loop audio showed the real shape — dense audio compresses ~8% (high-entropy
mantissa), but silence compresses ~99%, and loop phrases routinely have silent
tails and sparse sections. An end-to-end save of a 1-second tone-then-silence
phrase shrank its 192 KB WAV entry to 35 KB. WAV stays the on-extract format, so
importability is untouched.

## What remains

- **FT9 Moothold-schema alignment** for the hand-off wire protocol — separate
  from at-rest format; the CBOR manifest already shares Moothold's encoding.
- Consider promoting the doctrine into `PROJECT_DESCRIPTION.md` at goal level
  (maintainer-owned; not edited here).
- Genuinely-smaller-than-Deflate lossless audio would need a float-capable codec
  (WavPack float mode) or an integer audio model; neither is on the near path.

## Progress

- 2026-07-14: Doctrine captured from maintainer direction during the
  Strophe -> Hocket rename. Extension set to `.hock`.
- 2026-07-14: **CBOR half LANDED** (structure) — manifest + hand-off envelope on
  ciborium; postcard removed.
- 2026-07-14: **Media/container half LANDED** — `.hock` is now a zip of
  `manifest.cbor` + `media/<hash>.wav`, over a new Muniment `ZipBackend` (seam
  kept, not dropped). WAV via `hound`; hash still over decoded samples. Verified
  openable by an independent zip reader. Engine + host + muniment suites green.
- 2026-07-14: **Hardened after an adversarial multi-agent review** — ZipBackend
  fsyncs temp+parent around the rename, put/delete roll back on write failure,
  `scan` guards inverted ranges, content keys ending in `/` round-trip; Hocket
  `save()` self-heals a corrupt/tampered `.wav` and prunes orphaned media;
  `encode_media` guards the 4 GiB WAV limit. Cross-process locking documented as
  a known single-writer limitation, not fixed.
- 2026-07-15: **Polish LANDED** — `meta.json` provenance entry (human-readable,
  informational) and Deflate compression on the archive. FLAC evaluated and
  **rejected** for float audio (see above). FT8's open-format goal is met.
- 2026-07-18: **Media is now lossless WavPack (`.wv`), not WAV.** The
  "genuinely-smaller lossless audio needs a float-capable codec" note above (the
  WavPack line) was carried through: a pure-Rust WavPack codec, **wavicle**
  (`repos/wavicle`), was built for exactly this, and `project_store` now encodes
  media with `wavicle::encode_float` and decodes with `wavicle::decode_stream`.
  `MediaRef` is unchanged (BLAKE3 over the decoded f32 samples), and wavicle
  verifies its block CRCs internally. Doctrine check: WavPack is an open format
  real DAWs and `wvunpack` read, and wavicle is pure Rust and wasm-capable, so
  "openable/importable without Hocket" still holds while media now compresses
  losslessly (dense audio, which Deflate could not shrink). WAV survives only as
  the offline mix-export format. One consequence to note: the archive's Deflate
  now does little for the `.wv` media (already compressed) and mostly helps
  `manifest.cbor`; making the `ZipBackend` store already-compressed entries
  uncompressed is an optional future tidy, not a correctness issue.
