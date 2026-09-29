# Timed transcripts

**Status (2026-09-26): bounded implementation verified; headed acceptance open.** Native Redshank
transcript slice; broad WebVTT conformance and headed acceptance remain open.

## Ownership and scope

The pinned reference is [WebVTT CRD, 20 May 2026](https://www.w3.org/TR/2026/CRD-webvtt1-20260520/),
file structure and cue timing sections. `crates/timed-text` is a pure Rust
format leaf with no fetching, storage, application, or rendering authority.
It incubates in Woodshed for Redshank's first consumer; revisit shared custody
when a second consumer actually arrives, in accordance with Mere's net-media
planning. This is a transcript subset, not an HTML text-track implementation.

Preserved facts include cue identifiers, millisecond intervals, source order,
raw payload, and settings. Display projects cue text as inert text; it never
inserts source tags into the document. The projection strips cue tags and
decodes the six WebVTT character references. It does not implement the full
cue tree/tokenizer, ruby/voice presentation, timestamp-node activation,
regions, CSS, or video subtitle layout. Malformed blocks are counted and
skipped. Recovery from a missing separator is not a full implementation of
the normative parser state machine.

## Phases and done-conditions

1. **Format leaf:** BOM, newline normalization, identifiers, timestamps,
   checked overflow, overlaps, cue text, and malformed block tests pass.
2. **Native consumer:** a listener explicitly saves an advertised `text/vtt`
   transcript. The existing host Fetch handle reads it on a background thread
   with the existing 4 MiB text-resource bound. No autonomous transcript fetch
   occurs. The saved raw source, original resource/language, final URL, and
   retrieval time live in Redshank's existing generation store. Reopening
   needs no origin. Removal of the library item removes its transcript.
3. **Projection:** the Listen notes pane shows cues, marks every interval
   containing the playhead, and issues the existing seek command on a cue
   click. Selection changes clear the previous item's transcript. A harness
   test verifies inert text and the emitted seek offset.
4. **Acceptance, open:** headed playback of a real episode, cue click seeking,
   active state after external seek, and restart with the origin stopped.
   Verify long-transcript scrolling and keyboard navigation before claiming
   full product readiness. Add multi-language replacement/removal controls,
   configurable resource limits, and a paged transcript for large resources.

## Findings (2026-09-26)

- Feed import already retains transcript URL/type/language/relation in
  `ports/redshank/feed/src/lib.rs`; no existing WebVTT parser was found in the
  inspected Mere or Genet components.
- `desktop/src/main.rs` already owns one Fetch handle and asynchronous
  persistence. The implementation reuses both. Native transcript saving is
  separate from the browser demo, whose unsupported command reports its
  missing fetch/store adapter explicitly.
- The saved source is a snapshot, not a claim that a publisher's resource is
  immutable. It is intentionally not silently replaced on feed refresh.
- Transcript intervals use start-inclusive/end-exclusive activation and may
  overlap. Existing `CompactCommand::Seek` keeps audio authority in the host.

## Validation

Validation on 2026-09-26: 2 parser tests passed; final model/storage run passed
14 model and 6 storage tests, including explicit saved-source restart and item
removal. The native loopback fetch/store/offline reopen test and surface
active-cue/inert-text/seek-command test passed. Native-target
`cargo check -p redshank-web --offline` passed; this is not a WASM target
receipt. Commands used the Redshank workspace manifest and stable target.
No headed receipt or real publisher episode has been claimed.

The stable reusable Cargo target is `C:/t/cargo-targets/woodshed`, owned by
Woodshed builds. No isolated Cargo home or worktree was created.

## Response and persistence review (2026-09-26)

The existing Fetch `read_all` checks Content-Length and bounds drained body
bytes. Transcript source is rendered through Cambium text/button values;
source tags never become document elements. A concrete deletion/re-add race
was fixed: an in-flight request carries its item identity and a removal
marker, so deleting then recreating the same ID cannot resurrect a transcript.
Completion also revalidates the advertised resource. Switching selection
alone permits the requested save to finish for its original item, without
projecting it onto the newly selected item. Pending saves hold close until
completion; the existing serial generation writer orders persistence.

Four native transcript tests and one surface transcript test pass after review.
Additional regression tests cover oversized responses, response ownership
after selection changes, and deletion followed by re-addition. Large-document
pagination remains an explicit product acceptance gate; the input byte bound
does not imply an efficient view for every accepted document.

## Public podcast survey, 2026-09-29

The public RSS feeds for Bad Faith, TrueAnon, House of Bob, Decoder with
Nilay Patel, and Regulation Podcast were retrieved and inspected. None of the
2,256 served entries advertises `podcast:transcript`. This is a feed-availability
finding, not proof that a publisher has no transcripts elsewhere. Decoder has
94 public HTML transcript links in its episode descriptions; these are untimed
website pages and cannot be imported as timed WebVTT. No transcription or
conversion was attempted.

Official feed locations: `https://badfaith.libsyn.com/rss`,
`https://www.patreon.com/public-rss/2963533?show=875184`,
`https://feeds.megaphone.fm/houseofbob`,
`https://feeds.megaphone.fm/recodedecode`, and
`https://feeds.megaphone.fm/fface`. Retrieval hashes, byte counts and advertised
resource counts are retained in
`Code/testing/woodshed/redshank-podcasts/rss-research.json`. Raw RSS stays outside
the repository because public media URLs can contain expiring signatures.

Decoder's RSS is 5,203,522 bytes. Redshank's separate feed read bound now
defaults to 8 MiB and is configurable from 1 to 32 MiB in Settings; network
reads clamp edited persisted values to that range. Transcript reads remain
bounded to 4 MiB. The native subscription runner uses the shipping worker and
merges into the listener store, preserving existing generations and entries.

The final native receipt saved all five subscriptions and 2,255 audio episodes
in the normal listener store, with no advertised transcript resources. A local
two-cue WAV/WebVTT fixture separately verifies delayed fetch, explicit durable
saving, exact five-second seeking, and offline reopening after the origin stops
(with cue changes from zero to five seconds). Presented-frame captures and
assertions are in `Code/testing/woodshed/redshank-podcast-adoption/transcript/`.
This is synthetic native evidence; real publisher WebVTT, long-list scrolling
and keyboard behavior, and physical assistive-technology acceptance remain open.

After the optional persistence-observation pilot (`a57085b`, integration receipt
`b7f8c61`), the same native transcript case was rerun against the new save worker.
Delayed fetch/save, exact 5,000 ms seeking, and offline restart with zero-to-five
second cue changes all pass. The origin was stopped before reopening. Receipt:
`Code/testing/woodshed/redshank-persistence-transcript/transcript/summary.json`;
binary SHA-256:
`DFB8DBB840AC6413FDABAAC43D14444CEF6EA1C3B05FE580A5FAE4D46DA5E580`.
