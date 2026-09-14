# Redshank

Redshank is the offline-first listening and timed-annotation port. This
incubating workspace is intentionally separate from Woodshed's root product
workspace, and every package is unpublished while public naming remains gated.

Current packages:

- `redshank-cache`: complete HTTP(S) enclosure downloads, content-addressed
  publication, complete representation receipts, and strict disk-budget
  admission;
- `redshank-feed`: host-neutral projection from Errand's RSS/Atom parser into
  durable subscriptions and GUID-stable episodes, including podcast artwork,
  duration, chapter, and transcript facts;
- `redshank-model`: durable library, queue, progress, settings, representation
  receipts, text notes, audio notes, and the capture-host trait;
- `redshank-storage`: a local JSON store using immutable numbered generations;
- `redshank-surfaces`: the Cambium surfaces from the endorsed 2026-09-13 design
  canvas: one fixed-height Player/Capture dock (family A) or left transport
  rail (family B), a header with Listen, Library, Notes, Mere, and Settings,
  the feed-node projections (chain, orrery, trail) for the Mere tab, and a
  tinct-derived stylesheet with wetland and brand-shell seeds in dark, light,
  and high-contrast modes. Wide, narrow, and phone widths are plain media
  queries in the sheet. The compact composition depends only on its
  presentation snapshot and command queue, so a host can mount it without the
  Library. See `../../design_docs/2026-09-13_redshank_gui_implementation_plan.md`;
- `redshank-web`: the same surfaces mounted on a browser canvas over
  `cambium-genet-web-host`, with an in-memory fixture and no audio; see
  `web/README.md`;
- `redshank-playback`: an MP3/AAC decoder worker using Symphonia and one
  host-owned Firewheel/CPAL output, admitted through Genet's player controller.
  Local files and Rustls-backed HTTP(S) byte-range sources use the same command
  vocabulary. HTTP ranges occupy at most 256 KiB in memory; host blobs still
  require an embedding host;
- `redshank-desktop`: the sovereign executable over Mere's Cambium/Genet winit
  host. It restores selection and per-item progress, opens local files, and
  saves text and microphone notes with frozen item/time/representation targets.
  Voice bodies are mono PCM WAV blobs in content-addressed local storage. Disk
  writes and the file picker run outside the UI thread.

The storage package writes and flushes a pending generation before publishing
it with a same-directory rename. Loading walks completed generations newest to
oldest and selects the first valid document. An interrupted or corrupt newer
write therefore leaves the last valid state readable.

The desktop waits for a storage acknowledgment before clearing a saved note.
Editing retains the original target. Changing playback selection or queue order
does not retarget an open draft. Closing saves a nonempty draft and progress;
a save failure leaves the window and draft open. `REDSHANK_DATA_DIR` overrides
the local storage directory for isolated runs.

Open a local recording using **Open local file**, `Ctrl+O`, or pass a file path
or direct HTTP(S) audio URL as the executable's first argument. Remote servers
must support byte ranges. Outside the editor, Space toggles playback,
Left/Right skip by the configured interval, and N begins a text note.
`Ctrl+Enter` saves the editor. Text capture supports Pause and Continue.
Hold the voice-note control while speaking and release it to save, or press R
once to start and again to finish. The standalone host uses the system default
microphone. A denied or lost device leaves playback and the rest of the
application usable. A voice-note row separates its source timestamp from its
Play/Stop action. Audition pauses a playing episode, uses the same host-owned
Firewheel output, and restores episode playback when the note finishes or is
stopped. Opening the timestamp returns episode playback to its frozen anchor.
The full application keeps the Player/Capture composition visible as its
bottom listening dock across Listen, Library, Notes, and Settings. This is the
same compact surface an embedding host can mount independently.
Hosts explicitly advertise voice-capture availability. The standalone host
does so when its default microphone has a supported input configuration; an
embedding host can keep the action visible but disabled rather than emitting
an unsupported command.

Paste an HTTP(S) RSS or Atom URL into **Podcast subscriptions** and choose
**Subscribe**. The standalone host fetches feeds off the UI thread with a 4 MiB
limit. Manual refresh updates episode metadata in place, adds newly published
episodes to the Library, and preserves a completed offline object when its
enclosure URL is unchanged. An embedding host can use `redshank-feed` with its
own fetch and network policy.

For an isolated headed local restart receipt, set `REDSHANK_DATA_DIR` to an
empty directory. Launch once with a local file argument and
`REDSHANK_HEADED_RECEIPT=seed`, then launch without the file argument using
`REDSHANK_HEADED_RECEIPT=verify`. Each process opens the ordinary desktop and
default output device, waits for durable storage acknowledgments, and prints a
single `redshank-headed-receipt ... PASS` line before exiting. This driver does
not run when the variable is unset.

For the offline headed receipt, use `cache-seed` for the first launch with an
HTTP(S) URL. Stop the server, then launch without an argument using
`cache-verify`. The second process requires the persisted content-addressed
object and its complete digest; it cannot fall back to the origin.

Remote library items expose **Download for offline listening**. Downloads run
off the UI and audio threads, publish only after the complete object is flushed
and hashed, and then switch playback to the validated local object. The saved
cache budget defaults to 2 GiB and can be adjusted in Settings. Identical bytes
share one object; exhaustion is reported without automatic eviction.
Cached items also expose **Remove offline download**. Redshank first persists
the item's return to its enclosure URL, then removes the object on a cache
worker if no other library item refers to that content-addressed path. Failed
model saves retain the object. The model exposes a deterministic least-recently
used candidate order for a later configurable reclamation policy.

The landed slices support local, bounded progressive HTTP(S), manual feed
subscriptions, manually cached offline listening, text notes, local voice
capture, and voice-note audition. Automatic refresh and download, automatic
eviction, duck/reaction-offset capture settings, W3C annotation export,
rate/volume controls, representation-drift warnings/remapping, and Turnstone
embedding remain open.
A local digest is computed before decode from the opened file; it does not make
a concurrently modified file immutable. Remote receipts retain validators but
do not claim a complete digest until every byte is durably cached.
See the [canonical plan](../../design_docs/2026-09-01_listening_annotation_port_plan.md)
for current validation receipts and the remaining done-conditions.

For a deterministic headed receipt of any surface state, the desktop drives
itself from a scenario file: set `REDSHANK_SCENARIO`, `REDSHANK_CAPTURE_DIR`,
`REDSHANK_WIDTH`, and `REDSHANK_HEIGHT`, or run
`Code/testing/woodshed/redshank-run-scenario.ps1 -Scenario <name> -Fixture <name>
[-Matrix]`. Captures are in-process readbacks of the presented frame. Scenarios,
fixtures, and the `act` vocabulary are documented in `scenarios/README.md`.

Run the isolated model and storage gates from outside the repository's parent
Cargo configuration:

```powershell
$env:CARGO_TARGET_DIR = 'C:\t\redshank-phase2'
cargo test --manifest-path C:\Users\mark_\Code\repos\woodshed\ports\redshank\Cargo.toml -j 1
cargo clippy --manifest-path C:\Users\mark_\Code\repos\woodshed\ports\redshank\Cargo.toml --workspace --all-targets -j 1 -- -D warnings
```

Playback substrate experiments remain in the independent
`spikes/playback` workspace.
