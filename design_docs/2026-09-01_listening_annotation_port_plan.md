# Redshank: listening and annotation port plan

**Status (2026-09-07): ACTIVE.** The product direction and **Redshank** name are
endorsed. Phase 0 is complete: Symphonia plus a bounded range source is the
shipping default, and Genet/GStreamer is the retained browser-conformance
fallback. Phase 1 is complete: its general Genet boundary, deterministic player
core, and unified source vocabulary are landed. Phase 2 is complete with the
unpublished model and generation store. Phase 3 is complete with shared podcast
feed facts and GUID-stable Turnstone projection. Phase 4 is active with the
reusable Cambium controls, Mere-hosted sovereign shell, and local listening
workflow landed. Genet controller admission is also landed; subscriptions,
HTTP/cache playback, and headed acceptance remain open.

## Ruling

Build this as a separate port incubated in the Woodshed repository, not as a
Woodshed feature:

```text
woodshed/
  ports/redshank/          listening model, actions, and Cambium surfaces
  ports/redshank/desktop/  sovereign Mere/Cambium host over Genet
```

This topology is illustrative, not compile-ready. The committed Phase 0
workspace lives at `ports/redshank/spikes/playback` without entering Woodshed's
root workspace. Shipping package names remain gated by the public-name check.

Redshank is an offline-first audio listener that can subscribe to podcasts or
open ordinary audio, remember progress, and attach text or recorded voice to
exact moments. Podcast support is a profile over the general listener.

The ownership split is strict:

- Genet owns the general media-player contract and decoder/backend work.
- The port owns the durable listening workflow: library, queue, progress,
  settings, timed notes, and its reusable product surfaces.
- The standalone host owns one audio runtime and device connection.
- Turnstone is the planned second host and supplies browsing, graph, fetching,
  storage, and audio services when it embeds the port.
- Wavicle remains a WavPack codec leaf. It is neither the application home nor
  the player architecture.

Playback by itself is a media handler, not a port. The long-lived library and
annotation workflow are what earn the port boundary.

## Why Woodshed is the temporary home

Woodshed already exercises the family's Genet/Cambium desktop host and carries
the shared pure-DSP `audio-primitives` crate. That makes its workspace a useful
incubator. Its product description is specifically a fretted-instrument
practice toolkit, so the new packages must not depend on `woodshed-core`,
`woodshed-views`, or `woodshed-audio`, and the new application must not appear
inside Woodshed navigation.

Co-location is temporary source custody. It does not make the listener part of
the Woodshed product.

Wavicle is a particularly poor home. Its founding boundary is a zero-runtime-
dependency codec that knows nothing about projects or storage. WavPack may
eventually be an archival option for recorded notes, but lossless storage is not
an automatic fit for compact voice notes. The format choice follows measured
size, quality, seek, and web-support results.

## Product surface

The initial UI vocabulary stays plain:

- **Player**: title, position, scrubber, play/pause, skip, volume, and status.
- **Library**: feeds and local audio.
- **Queue**: the upcoming listening order.
- **Notes**: timestamped text and voice notes for the current item.
- **Capture**: one text action and one press-and-hold voice action.
- **Settings**: playback, capture, storage, microphone, and privacy choices.

The compact Player and Capture controls must compose without the Library. That
is the embeddable shape for a Turnstone pane, a document viewer, or another
Merely product.

### Capture interaction

Voice capture follows one ordered transaction:

1. On press, synchronously take a playback snapshot.
2. Apply the selected capture behavior: pause, duck, or continue.
3. Record from the host-provided input lane while the button remains held.
4. On release, persist the audio body and timed target together.
5. Restore playback according to the resume setting.

Text capture takes the same snapshot before opening or focusing the editor.

The note keeps both the actual press position and the chosen anchor position.
The latter applies a configurable reaction offset, since the thought usually
arrives after the passage that prompted it. Keeping both values preserves what
happened and allows a note to be re-anchored later without rewriting history.

Capture behavior is configuration, including:

- reaction offset;
- pause, duck, or continue during recording;
- resume after capture;
- skip-back and skip-forward intervals;
- playback rate where the backend supports pitch-preserving rate changes;
- microphone and recording format;
- cache budget and automatic-download policy;
- private or shareable default for new notes.

## Initial scope

The first complete slice includes:

- local files and direct HTTP audio;
- subscription by RSS or Atom URL;
- MP3 and AAC-LC playback, with progressive download and range seek where the
  server permits it;
- library, queue, playback progress, and offline cache;
- text and press-and-hold voice notes;
- open timed-annotation serialization and export;
- a native standalone host and the same surface mounted by Turnstone.

Deferred product work includes directory search, accounts, hosted sync, public
discussion, automatic transcription, recommendations, video, car surfaces, and
publisher analytics. Gemot can later share selected annotation documents; it
does not enter the first local workflow.

## Existing stack seams

### Genet media player

Genet already has `servo-media-player`. Its `Player` trait covers play, pause,
stop, seek, buffered ranges, volume, mute, playback rate, streamed input, and
events. This is the starting contract for the general stack player.

It is not yet capture-grade:

- position is delivered as an event, and the GStreamer backend currently asks
  for updates every 500 ms;
- the trait has no synchronous playback snapshot;
- `AudioRenderer` receives sample data and a channel mask, but not an explicit
  sample rate or presentation timestamp;
- the current native backend is GStreamer, and progressive downloading is
  disabled on Windows in that backend;
- GStreamer is a broad browser backend, but it is too heavy to assume as the
  default for a deliberately small listener.

The GStreamer backend can already send decoded float samples to a caller-
provided `AudioRenderer` instead of opening its own output sink. That makes it a
valuable conformance backend and proves the host-owned-audio direction. The
shipping default should be selected by the Phase 0 measurement rather than by
inheritance.

The likely lightweight path is a Rust decoder built from Symphonia plus a
bounded progressive source/cache, with decoded frames handed to the host's
Firewheel graph. Termusic is useful design evidence for this source-to-decoder
shape, but its Rodio sink and application architecture do not come with it.

### One audio authority

Decoder and player code must not open CPAL. It emits decoded frames into a sink
supplied by the host.

The sovereign desktop application may own exactly one Firewheel context and its
CPAL input/output authority. When Turnstone embeds the port, Turnstone supplies
the live audio service. A competing device runtime inside either process fails
the architecture gate.

`audio-primitives` remains pure DSP. Driver state, live streams, and locks stay
in consuming host adapters.

Hocket remains the reference pressure vessel for Firewheel runtime work, not a
product dependency of this port. If the decoded-stream adapter earns a second
consumer, promote that adapter to a shared audio crate rather than making
Turnstone or Woodshed depend on `hocket-engine`.

### Feeds

Genet's Errand parser and Turnstone's current feed model retain title, link,
date, and summary. Podcast playback also needs stable episode and enclosure
facts. The shared parser should retain, without applying product policy:

- feed identity and episode GUID;
- enclosure URL, media type, and declared byte length;
- duration, artwork, chapters, and transcript references;
- alternate enclosure and source relations where supplied;
- namespace-qualified Podcasting 2.0 facts needed by the listener.

Turnstone remains responsible for network policy, redirects, credentials,
scheduling, and graph projection. The standalone host supplies simpler adapters
to the same port actions.

### Timed annotations

Knot already serializes W3C `SpecificResource` targets for text anchors while
leaving body, motivation, persistence, and source identity to its caller. Add a
generic media `FragmentSelector` there rather than creating a podcast-only
annotation format.

The port owns the audio-specific document:

- stable logical source identity;
- W3C Media Fragments time target;
- text or audio body;
- `commenting` motivation;
- capture receipt and representation receipt;
- privacy and sharing state.

## Product model

The following shapes are illustrative and not compile-ready:

```rust
struct PlaybackSnapshot {
    source: MediaSourceId,
    representation: RepresentationId,
    state: PlaybackState,
    position: Duration,
    duration: Option<Duration>,
    rate: f32,
    sequence: u64,
}

struct TimedTarget {
    source: MediaSourceId,
    representation: RepresentationId,
    pressed_at: Duration,
    anchor_start: Duration,
    anchor_end: Option<Duration>,
}

enum NoteBody {
    Text(String),
    Audio {
        blob: BlobRef,
        media_type: String,
        duration: Duration,
    },
}

struct ListeningAnnotation {
    id: AnnotationId,
    target: TimedTarget,
    body: NoteBody,
    created: Timestamp,
    visibility: Visibility,
}
```

`PlaybackSnapshot` is read synchronously from the decoder/runtime boundary. UI
timers may display position, but they cannot author annotation targets.

The product model exposes typed commands and snapshots. A minimal command set is
Load, Play, Pause, Stop, Seek, Skip, SetVolume, SetRate, BeginCapture,
FinishCapture, AddTextNote, and DeleteNote. Backends report unsupported
capabilities explicitly.

## Representation identity and dynamic ads

A podcast episode has a logical identity and one or more delivered audio
representations. Timestamp-only notes become misleading when an enclosure URL
later returns a different ad insertion or edit.

Each played representation therefore gets a receipt containing what is
available:

- feed URL and episode GUID;
- requested and final enclosure URLs;
- media type and byte length;
- ETag and Last-Modified validators;
- retrieval time;
- complete-content digest once the download finishes.

An annotation targets the logical episode and the representation heard during
capture. If a later request cannot prove the same representation, the UI shows
the mismatch before seeking. It must not silently pretend that the old time is
still exact.

Alignment or transcript fingerprints can later remap notes between known
representations. `podcast-annotations-js` is a useful donor for dynamic-ad
alignment fixtures and gap behavior, subject to a license and provenance review.

## Package and dependency direction

```text
Errand feed facts ───────────────┐
Genet media Player ─────────────┤
Knot media selector ────────────┤
Muniment/blob adapters ─────────┤
                                v
                         Redshank model
                                |
                     reusable Cambium surfaces
                          /              \
                standalone host       Turnstone
                Firewheel adapter      host services
```

Allowed dependency direction is from the port toward stable shared contracts.
Woodshed product crates do not enter this graph. Turnstone consumes the port
model and surfaces rather than recreating them.

## Phases and done-conditions

### Phase 0: playback substrate spike

**Complete (2026-09-03).** The spike selects Symphonia plus a host-supplied
Firewheel sink as the shipping default. Genet/GStreamer remains a conformance
and broad-format fallback for hosts that already carry its runtime.

Compare two native paths behind the existing Genet player semantics:

1. Genet's GStreamer player with a caller-provided decoded-audio renderer.
2. Symphonia plus a bounded progressive source/cache feeding Firewheel.

Termusic's Rust playback code may be studied for the second path. Any borrowed
code requires a per-file license and provenance entry before landing.

Done when:

- one local MP3, one local AAC-LC file, and one HTTP episode play through a
  host-owned Firewheel graph;
- an HTTP range seek succeeds before the complete file is downloaded;
- press-time snapshots are within 50 ms of the decoder playhead in a
  deterministic probe;
- every CPAL stream belongs to one host audio runtime, with no competing output
  stream;
- binary size, transitive dependencies, startup behavior, supported formats,
  and Windows packaging burden are recorded for both paths;
- the plan names the shipping default and the retained fallback.

Receipt from Windows 11, Rust 1.97, GStreamer MSVC 1.0, one shared `-j 1`
target directory, and synthetic mono 48 kHz fixtures:

| Measure | Symphonia path | Genet/GStreamer path |
| --- | --- | --- |
| Local formats exercised | MP3, AAC-LC in M4A | MP3, AAC-LC in M4A |
| HTTP/range path | 450 s seek in a 3,261,184-byte episode; 639,745 bytes fetched (19.6%); 256 KiB peak cache | Genet's Windows progressive-download path remains disabled; retain only as a host-fed compatibility backend |
| Firewheel ownership | One CPAL output stream, zero dropped frames | One CPAL output stream through caller renderer, zero dropped frames |
| Release first-audio samples | MP3 502 ms; AAC 628 ms | MP3 1,496 ms; AAC 1,094 ms with a warm registry; one cold debug run was 26,989 ms |
| Release executable | 3.66 MiB | 3.40 MiB launcher plus external GStreamer DLLs and plugins |
| Unique normal dependency packages | 86 | 276 |
| Clean release build on the loaded host | 7m10s | 24m54s |
| Windows packaging | Rust decoder and CPAL dependencies; the spike's HTTP client intentionally omits TLS, so shipping adds a rustls-backed transport | Requires a curated GStreamer runtime/plugin bundle; the installed full development tree measured 2.33 GiB across 4,983 files and is not a proposed bundle size |

The deterministic press-time unit probe derives presented time from accepted
source frames minus Firewheel's queued duration and passes the 50 ms bound. The
live one-second probes ended at 0.98–1.06 seconds because draining and the
GStreamer callback are block-quantized; annotation authority must use the
frame/queue snapshot, not the 500 ms GStreamer position event.

The Genet path also emitted a repeatable GLib signal-disconnect warning during
shutdown. Phase 1a added sample rate, whole-stream channel layout, presentation
time, and a synchronous playback snapshot at the general player boundary.

### Phase 1: capture-grade player contract

**Complete (2026-09-04).** Genet now exposes capture-grade decoded chunks and
synchronous snapshots plus a deterministic, product-neutral controller over
caller-supplied source and sink traits.

Extend or wrap the Genet contract with a synchronous snapshot and a decoded
audio chunk carrying channel layout, sample rate, and presentation time. Keep
source, decoder, cache, and sink separable.

Done when:

- a fake source and fake sink drive deterministic play, pause, seek, buffering,
  end-of-stream, and error tests;
- annotation capture reads an authoritative snapshot without consulting UI
  state;
- local, HTTP, and host-blob sources use one player command vocabulary;
- the player and decoder packages contain no CPAL or product-library code;
- unsupported rate or seek operations return typed capability errors.

Progress receipt, 2026-09-04:

- Genet commit `eb1b03686b7` adds the additive `PlaybackSnapshot` and
  `DecodedAudioChunk` contracts. Legacy renderers and external `Player`
  implementations retain default adapters.
- Focused `servo-media-player` tests, `servo-media-dummy` tests, and the
  `servo-media-gstreamer` check pass on Windows with an isolated target.
- The Redshank GStreamer probe received 45 whole-buffer callbacks and zero
  legacy callbacks or dropped frames. It retained 48 kHz, one channel, channel
  position `0`, and a final presentation timestamp of 1.056 s.
- The same probe synchronously read a playing snapshot at 1.0801978 s with a
  60.024 s duration and sequence `1`; the host frame/queue snapshot read 1.06 s.
- The pre-existing GLib signal-disconnect warning still occurs at shutdown.
- Genet commit `f089da339f4` adds the fake-driven controller. Seven focused
  tests cover play, pause, seek, buffering, end-of-stream, errors,
  sink-authoritative snapshots, and one `Load` command across local, HTTP, and
  host-blob sources.
- Unsupported seek and playback-rate operations return typed errors. The
  controller's normal dependency graph contains neither CPAL nor Woodshed,
  Redshank, or Hocket product code.
- Focused player tests, dummy-backend tests, the GStreamer backend check, and
  strict Clippy all pass with an isolated target. Clippy still prints Genet's
  pre-existing unreachable-type warning from `.clippy.toml`.

### Phase 2: port model and standalone storage

**Complete (2026-09-04).** The isolated `ports/redshank` workspace now contains
unpublished model and storage packages without a headed UI or Woodshed product
dependency.

Create the separate Woodshed `ports/redshank` packages. Implement the library,
queue, progress, settings, annotations, and host adapter traits without a headed
UI.

Done when:

- the port packages build without `woodshed-core`, `woodshed-audio`, or
  `woodshed-views`;
- a library with local items and feed episodes survives restart;
- progress and queue mutations are deterministic and covered by model tests;
- text and audio annotation documents round-trip with their representation
  receipts;
- storage failure leaves the previous durable state readable.

Receipt:

- `redshank-model` owns local audio and feed episodes, queue order, progress,
  listener settings, timed text and audio notes, representation receipts, and
  playback/audio-capture host traits.
- `redshank-storage` publishes immutable numbered JSON generations. Restart
  tests round-trip the complete model; injected pre-publication failure and a
  corrupt newer generation both preserve the previous readable state. A later
  successful save advances past an abandoned pending generation.
- Two model tests and three storage tests pass on Windows. Workspace-wide
  strict Clippy passes, and the normal dependency tree contains no
  `woodshed-core`, `woodshed-audio`, or `woodshed-views` package.

### Phase 3: podcast facts

**Complete (2026-09-04).** Errand now retains product-neutral podcast feed
facts, and Turnstone resolves, stores, and projects them without taking on
listener policy.

Extend Errand's feed facts and adapt Turnstone's feed projection without adding
listener policy to either layer.

Done when:

- RSS, Atom, iTunes, and Podcasting 2.0 fixtures retain GUID, enclosure, media
  type, length, duration, artwork, chapter, and transcript facts;
- relative URLs resolve through a caller-supplied base;
- unknown namespaces survive or degrade explicitly rather than disappearing
  without a diagnostic;
- duplicate suppression prefers GUID and has a documented fallback;
- ordinary non-podcast feed tests stay green.

Receipt:

- Mere commit `5630e256cdd` adds RSS and Atom GUIDs and enclosures, iTunes
  duration and artwork, Podcasting 2.0 chapter and transcript references, feed
  artwork, declared enclosure lengths, and explicit diagnostics for unknown
  namespace extensions or invalid lengths.
- Errand's focused suite passes 37 tests with one live-network test ignored;
  its doctest also passes. The corpus includes ordinary RSS and Atom alongside
  podcast RSS and Atom fixtures.
- Turnstone commit `a0b91a97cba` resolves relative podcast URLs against the
  fetched feed URL, persists the retained facts, suppresses duplicates by GUID
  with URL as the fallback, and migrates earlier URL-keyed stored entries when
  a GUID arrives.
- Five focused feed-model tests and three application projection tests pass.
  A changed episode link with a stable GUID navigates the existing graph member
  in place rather than creating a second node. The broad test build emits 51
  pre-existing unused-code warnings under `--no-default-features`.

### Phase 4: sovereign and embeddable surfaces

**In progress (2026-09-06).** The local listening implementation now adds a
Symphonia/Firewheel worker and full Library, Queue, Notes, and Settings
composition around the existing compact Player/Capture surfaces. This is a
bounded local-file slice; the Phase 4 done-conditions below still apply.

Build Player, Library, Queue, Notes, Capture, and Settings surfaces in Cambium,
then mount them in Mere's standalone Cambium/Genet winit host.

Done when:

- the standalone application can subscribe, play, seek, resume, queue, and
  cache an episode;
- the compact Player and Capture surfaces run without the Library surface;
- screen-reader names and keyboard actions exist for every transport and
  capture control;
- missing output devices, offline sources, and cache exhaustion appear as
  useful degraded states;
- a headed scenario proves playback, restart/resume, and one text annotation.

Progress receipt:

- `redshank-surfaces` depends on Cambium and `redshank-model`, not on Woodshed
  product crates. Its compact state contains current-item presentation facts,
  transport state, configured skip intervals, capture state, and a typed command
  queue. It contains no library, storage, fetch, decoder, or device authority.
- Player and Capture mount separately or as one compact surface. Native buttons
  carry screen-reader names and shortcut metadata for play/pause, skipping,
  text notes, and voice-note start/finish. An unavailable output state is spoken
  and rejects transport commands.
- `redshank-desktop` consumes `cambium-genet-winit-host` from Mere commit
  `9f8f44047ed`, restores the first queued item and its progress from the
  generation store, and mounts the same compact surface. Until the live
  Symphonia and capture adapters land, commands produce an explicit unavailable
  state.
- Cambium, Rootstock, Sprigging, Workbench, and the desktop host resolve from
  Mere. Genet remains the owner of the lower DOM, Livery, render, and winit
  engine contracts at Mere's exact `115d348dedd` pin. The standalone workspace
  restates Mere's Parley and in-process IPC patches because Cargo does not
  inherit root patches through git dependencies.
- Three headless surface tests and three desktop boot/degradation tests pass.
  The complete isolated Redshank workspace passes eleven tests, and
  workspace-wide strict Clippy passes.

#### Local listening implementation, 2026-09-06

The preceding Phase 4 receipt describes the September 4 shell. The September 6
working implementation replaces its unavailable-playback placeholder:

- A dedicated decoder worker owns local-file I/O and a single Firewheel/CPAL
  output runtime. It publishes load-token-correlated clocks and representation
  receipts. Queue order and the selected recording are independent; stale
  snapshots cannot supply another item's progress or capture target.
- The full surface composes the existing compact controls. The library opens
  local files; the queue supports selection, insertion, removal, and reordering.
  Text notes have an editable Cambium field and explicit save/edit/delete/cancel
  actions. Settings control skip intervals and Pause/Continue text capture.
- The standalone host routes text focus and keyboard shortcuts, freezes note
  anchors before applying capture playback behavior, and retains those anchors
  across playback selection changes. Selection and per-item progress survive
  restart. The optional `selected_item` field reads older schema-1 documents.
- Persistence runs on a serial worker with revision acknowledgments. A failed
  save retains the draft and dirty model. Closing saves the nonempty draft and
  final progress before exiting; save failure leaves the window open. Editing
  during an outstanding close/save keeps the newer draft open.
- A worker-generated BLAKE3 digest replaces capture-time file metadata reads.
  Hashing then rewinding an ordinary file is not an immutable-byte snapshot
  under concurrent modification. Phase 6 identity/mismatch behavior remains
  unimplemented, and no annotation remapping claim follows from this digest.

The worker retains product commands, load tokens, representation receipts, and
snapshots while delegating playback transitions to Genet's
`PlaybackController`. Its real Symphonia decoder and sole Firewheel output are
private source and sink adapters. Redshank therefore has one product facade and
one admitted generic player contract.

#### Controller admission ruling, 2026-09-07

`PlaybackRuntime` remains Redshank's product-facing facade because it owns load
tokens, representation receipts, and worker isolation. Its worker must delegate
playback state transitions to Genet's existing `PlaybackController`; the local
decoder and sole Firewheel/CPAL output become private source and sink adapters.
The persisted clock continues to come from the sink snapshot after queued audio
is subtracted. Redshank does not introduce another generic player contract.

The source mapping is one vocabulary at the adapter boundary: Redshank `Local`
maps to Genet `Local`, `Enclosure` maps to `Http`, and `HostBlob` maps to
`HostBlob`. This slice admits local playback. HTTP and host-blob loads must
degrade explicitly through that same path until the cache and host-blob work is
implemented. Load identity remains product state, so the desktop token gate must
reject stale snapshots before they can change progress, note targets, or UI.

This slice is done when the controller drives local play, pause, seek, ready,
end-of-stream, and error transitions; sink snapshots remain authoritative;
typed capability errors survive the adapter; all three source variants use the
shared mapping; stale-token, resume, and representation tests pass; and the
workspace remains green. HTTP/cache playback, headed acceptance, voice capture,
representation drift, and Turnstone embedding remain outside the slice.

Phase 4 remains open for subscriptions, HTTP/progressive cache playback and its
error cases, and the headed playback/restart/text-note scenario. Voice input,
ducking, rate/volume controls, media-fragment export,
representation drift, and Turnstone as a second host also remain open in their
respective phases. No physical listening or microphone receipt is implied by
the source changes.

Validation on the final September 6 working tree:

- The isolated workspace passes **29 tests**: desktop 6, model 5, playback 7,
  storage 5, and surfaces 6. Two codec-fixture tests are ignored by default and
  were run explicitly with `REDSHANK_PHASE4_FIXTURES`; both pass.
- The original generated fixtures are two-second, 48 kHz stereo tones (440 Hz
  left, 660 Hz right), encoded to MP3 and AAC/M4A with FFmpeg 8.1.1. They prove
  distinct decoded channels and decoder-clock resume near one second. M4A's
  absent track channel layout exposed and fixed a real issue: the adapter now
  obtains channel count from the first decoded frame when necessary.
- Desktop tests exercise the actual host keyboard/editor path, stale load
  tokens, queue/selection independence, frozen draft targets, failed save and
  retry, and edits arriving while an earlier save completes. Storage includes
  literal schema-1 compatibility and preservation of existing notes/progress.
- `cargo clippy --workspace --all-targets -- -D warnings` passes. Formatting
  and `git diff --check` pass. Commands run from `C:/t` with the isolated
  Redshank manifest, `--offline --locked`, and target
  `C:/t/redshank-phase4-mere`; this avoids the parent checkout's local patches.
- `cargo build -p redshank-desktop` passes on the same route, producing
  `C:/t/redshank-phase4-mere/debug/redshank-desktop.exe`.
- Fixture provenance, SHA-256 values, and test logs are in
  `Code/testing/woodshed/redshank-phase4-20260906/`. These are software and
  windowless host receipts. Output-device playback, acoustic timing, headed
  layout, and the full headed restart scenario remain unverified.

Controller-admission validation on September 7:

- The real Symphonia decoder and Firewheel sink now sit behind Genet's
  `PlaybackSource` and `PlaybackSink`; the old parallel worker authority is
  removed. `Ready`, `Error`, and end-of-stream signals and all transport
  commands pass through `PlaybackController`.
- A deterministic device-free worker test covers load, ready, pause, seek and
  its buffering projection, play, and end-of-stream. Adapter tests cover all
  source variants, typed seek/backend failures, and the sink clock. A failed
  new load after an existing item clears the prior source, duration,
  representation receipt, and clock before publishing its new-token error.
- The isolated workspace passes **28 default tests**: desktop 6, model 5,
  playback 6, storage 5, and surfaces 6. The two existing codec-fixture tests
  remain explicitly gated. Strict workspace Clippy, formatting, and
  `git diff --check` pass. The resolved dependency graph carries one Genet git
  revision, `9e8f9dc2f3ddc0af1658580bb51964462a03923f`.
- This is a software admission receipt. Output-device playback, acoustic
  timing, headed layout, HTTP/cache playback, and the full headed restart
  scenario remain unverified.

### Phase 5: voice capture and open annotation target

Add host-provided microphone capture and Knot's generic media
`FragmentSelector` serialization.

Done when:

- press captures the snapshot before playback behavior changes;
- release durably commits the recorded body and target as one recoverable
  operation;
- pause, duck, continue, reaction offset, and resume behavior follow settings;
- denied permission, a lost device, and an interrupted recording preserve a
  usable application state;
- reopening either a text or voice note seeks to the captured representation
  and anchor;
- exported targets validate against the chosen W3C Annotation and Media
  Fragments profile.

### Phase 6: representation drift

Exercise dynamic-ad and edited-enclosure cases.

Done when:

- two byte-distinct responses at one enclosure URL produce distinct
  representation identities;
- a note reopens directly when validators or a completed digest prove identity;
- a mismatch produces a visible warning and preserves the original target;
- alignment fixtures cover inserted gaps, shifted content, and low-confidence
  refusal;
- remapping, if implemented, creates a derived target and retains the original.

### Phase 7: Turnstone as second host

Mount the same model and Cambium surfaces in Turnstone. Turnstone supplies feed,
fetch, graph, blob, settings, and audio adapters.

Done when:

- Turnstone opens a podcast enclosure through handler routing;
- the port's compact surface appears without a copied Turnstone implementation;
- one library item, progress update, and timed note project into Turnstone's
  graph and reopen after restart;
- the embedded path uses Turnstone's existing device and network authorities;
- standalone and embedded conformance tests run against the same action and
  snapshot contract.

After this phase, decide whether the port remains co-located or moves to its own
repository. Extraction requires an independent audience or release identity, a
stable one-way dependency boundary, a proven second host, and a build that does
not rely on Woodshed's workspace configuration.

## Validation matrix

| Gate | Evidence |
| --- | --- |
| Model | Deterministic unit tests over fake player, clock, source, and store |
| Decode | Local and HTTP MP3/AAC fixtures with seek and error cases |
| Audio ownership | Runtime receipt proving one host-owned device authority |
| Feed | RSS, Atom, iTunes, and Podcasting 2.0 fixture corpus |
| Annotation | W3C serialization fixtures plus restart round trips |
| Drift | Same URL with distinct representations and alignment gaps |
| Surface | Genet headed scenario with accessibility assertions |
| Composition | Same model and Cambium surface in standalone and Turnstone |

## Prior-art ruling

A whole-application fork would import the wrong UI, runtime, persistence, and
device assumptions. Use donors by seam:

- [PodNotes](https://github.com/chhoumann/podnotes) is the strongest open UX and
  testing donor for library, progress, timestamp links, and note workflows. Its
  Obsidian/TypeScript shell is not a stack fit.
- [Termusic](https://github.com/tramhao/termusic) demonstrates the Rust
  progressive-source, Symphonia, ring-buffer, and multi-backend shape. Its sink
  and full application stay outside the architecture.
- [Margin](https://margin.fm/) independently validates the exact press, speak,
  release, and resume mechanic. It is a behavior reference, not a code donor.
- [podcast-annotations-js](https://github.com/carcurious/podcast-annotations-js)
  is useful for dynamic-ad mapping and transcript-gap fixtures.
- The [W3C Web Annotation Data Model](https://www.w3.org/TR/annotation-model/)
  and [Media Fragments URI](https://www.w3.org/TR/media-frags/) specifications
  define the interoperable target vocabulary.

The first implementation pass must create a provenance ledger naming files or
algorithms consulted, their licenses, and what was independently written.

## Naming

### Decision: Redshank

**Redshank** is the chosen product name. The Retinue host-tier founding note at
`retinue/design_docs/2026-08-06_signalman_founding.md` banked it as the sentinel
shorebird, Turnstone's pair, for a future application. This use earns that
relationship: Redshank is the sovereign listening tool, while Turnstone becomes
its first embedded host and can project selected timed notes into the browsing
graph.

Use `ports/redshank` for the source home. Exact Cargo package names still wait
for the Phase 0 dependency graph. Scratch playback probes remain unpublished.

A fresh 2026-09-02 namespace check found two active software uses that the
earlier bank did not account for:

- [`greysquirr3l/redshank`](https://github.com/greysquirr3l/redshank), a Rust
  investigation application whose published packages include `redshank-cli`
  and `redshank-core`, and whose executable is named `redshank`;
- [Redshank: Private Web Browser][redshank-android], an Android application
  from Redshank Studios.

[redshank-android]: https://play.google.com/store/apps/details?id=com.redshankstudios.securebrowser

The family product name remains Redshank. Public release identity, Cargo
package names, executable name, and ordinary legal clearance are therefore an
explicit gate before publishing. Incubation must not reserve or publish those
names accidentally.

Earlier working names `Audiendum` and `Diple` are released. `Wavicle` remains
the codec; `Wavelet`, `Margin`, `Hearmark`, `Scholion`, and `Notula` remain
rejected for the reasons recorded in the 2026-09-01 planning pass.

## Findings

- **2026-09-01:** Woodshed's product authority limits it to fretted-instrument
  practice. Incubation must use packages outside the Woodshed product graph.
- **2026-09-01:** `audio-primitives` explicitly admits only pure sample and
  timing kernels; stream ownership stays in consumer drivers.
- **2026-09-01:** Wavicle's founding plan defines an independent codec leaf with
  zero knowledge of project or storage formats.
- **2026-09-01:** Genet already owns a useful `Player` contract and can deliver
  decoded float audio to a supplied renderer. Snapshot precision, chunk timing,
  Windows progressive download, and default backend weight remain open.
- **2026-09-01:** Errand and Turnstone currently discard podcast enclosure and
  episode-identity facts.
- **2026-09-01:** Knot's current W3C target support covers text quote and text
  position selectors; media fragments require an additive selector.
- **2026-09-01:** Mere's port law requires one sovereign tool, one embeddable
  surface, and the same product model in the second host. Playback alone belongs
  at the media-handler layer.
- **2026-09-02:** The maintainer selected Redshank, previously banked as
  Turnstone's sentinel-shorebird pair, as the product name.
- **2026-09-02:** The name has low-footprint but active software collisions: a
  Rust investigation application and an Android private browser. This does not
  change the internal product decision, but it makes external package and
  release naming a deliberate gate.
- **2026-09-03:** The isolated Phase 0 workspace proved local MP3 and AAC-LC on
  both candidate paths through one Firewheel/CPAL output authority. Symphonia
  also proved a 450-second HTTP seek after fetching 19.6% of a 3.11 MiB episode
  with a 256 KiB cache.
- **2026-09-03:** Symphonia is the shipping default. Genet/GStreamer has 276
  normal dependency packages versus 86, requires an external Windows runtime,
  starts more slowly, and cannot supply capture-grade timing through its
  current renderer contract. Its strength is conformance with the browser's
  broad media backend.
- **2026-09-03:** The GStreamer caller-renderer path works but emits a repeatable
  GLib signal-disconnect warning at shutdown. Treat that as backend debt rather
  than adding a Redshank workaround.
- **2026-09-04:** Errand and Turnstone now retain podcast enclosure and stable
  episode-identity facts. GUID is the primary duplicate key; resolved entry URL
  is the documented fallback. Unknown namespace extensions produce diagnostics.

## Progress

- **2026-09-01:** Founding plan written from live Woodshed, Wavicle, Hocket,
  Genet, Mere, and Turnstone seams. This planning pass changed no manifests or
  implementation code.
- **2026-09-02:** Redshank selected and the illustrative source home changed to
  `ports/redshank`. No package was created: Phase 0 still determines the player
  dependency graph before manifests change.
- **2026-09-03:** Added the unpublished nested workspace at
  `ports/redshank/spikes/playback`, including a shared Firewheel sink, local and
  range-backed Symphonia probe, Genet/GStreamer caller-renderer probe,
  deterministic press-time tests, synthetic fixture generator, range server,
  and provenance ledger. Kept it outside Woodshed's root manifest and lockfile.
- **2026-09-03:** Phase 0 gates passed. The full nested workspace checks cleanly;
  the press-time tests pass 2/2; both local formats and the bounded HTTP seek
  ran successfully; release sizes, dependency counts, startup behavior, and
  Windows packaging constraints are recorded above.
- **2026-09-04:** Phase 3 landed in Mere `5630e256cdd` and Turnstone
  `a0b91a97cba`. Parser, model, restart, duplicate-suppression, relative-URL,
  diagnostics, and graph-member continuity tests pass. Phase 4 is next.
- **2026-09-04:** Phase 4 began with the reusable `redshank-surfaces` compact
  Player/Capture slice. Independent mounting, semantic controls, command
  emission, and unavailable-output degradation pass headlessly.
- **2026-09-04:** Corrected the post-move ownership boundary: Redshank's
  Cambium packages and sovereign winit host come from Mere, while Genet retains
  their lower engine contracts. `redshank-desktop` now restores queued progress
  and mounts the compact surface. Live audio, headed acceptance, and the
  remaining full surfaces are still open.
- **2026-09-07:** Landed the bounded Phase 4 controller-admission slice.
  `PlaybackRuntime` remains the product facade while the real decoder, sole
  Firewheel/CPAL output, transport transitions, terminal errors, and sink clock
  are admitted through Genet's existing controller. Device-free conformance,
  stale-token and failed-load metadata regressions pass. HTTP/cache, voice,
  headed acceptance, and Turnstone work remain outside the slice.
