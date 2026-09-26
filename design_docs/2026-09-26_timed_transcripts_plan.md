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
