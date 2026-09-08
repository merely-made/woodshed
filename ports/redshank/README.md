# Redshank

Redshank is the offline-first listening and timed-annotation port. This
incubating workspace is intentionally separate from Woodshed's root product
workspace, and every package is unpublished while public naming remains gated.

Current packages:

- `redshank-model`: durable library, queue, progress, settings, representation
  receipts, text notes, audio notes, and the capture-host trait;
- `redshank-storage`: a local JSON store using immutable numbered generations;
- `redshank-surfaces`: reusable Cambium Player and Capture surfaces, plus the
  full Library, Queue, Notes, and Settings composition. The compact
  composition depends only on its presentation snapshot and command queue, so a
  host can mount it without the Library;
- `redshank-playback`: an MP3/AAC decoder worker using Symphonia and one
  host-owned Firewheel/CPAL output, admitted through Genet's player controller.
  Local files and Rustls-backed HTTP(S) byte-range sources use the same command
  vocabulary. HTTP ranges occupy at most 256 KiB in memory; host blobs still
  require an embedding host;
- `redshank-desktop`: the sovereign executable over Mere's Cambium/Genet winit
  host. It restores selection and per-item progress, opens local files, and
  saves text notes with frozen item/time/representation targets. Disk writes
  and the file picker run outside the UI thread.

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

For an isolated headed local restart receipt, set `REDSHANK_DATA_DIR` to an
empty directory. Launch once with a local file argument and
`REDSHANK_HEADED_RECEIPT=seed`, then launch without the file argument using
`REDSHANK_HEADED_RECEIPT=verify`. Each process opens the ordinary desktop and
default output device, waits for durable storage acknowledgments, and prints a
single `redshank-headed-receipt ... PASS` line before exiting. This driver does
not run when the variable is unset.

This Phase 4 slice supports local and bounded progressive HTTP(S) listening.
Subscription, durable offline caching, voice capture, rate/volume controls,
representation-drift warnings/remapping, and Turnstone embedding remain open.
A local digest is computed before decode from the opened file; it does not make
a concurrently modified file immutable. Remote receipts retain validators but
do not claim a complete digest until every byte is durably cached.
See the [canonical plan](../../design_docs/2026-09-01_listening_annotation_port_plan.md)
for current validation receipts and the remaining done-conditions.

Run the isolated model and storage gates from outside the repository's parent
Cargo configuration:

```powershell
$env:CARGO_TARGET_DIR = 'C:\t\redshank-phase2'
cargo test --manifest-path C:\Users\mark_\Code\repos\woodshed\ports\redshank\Cargo.toml -j 1
cargo clippy --manifest-path C:\Users\mark_\Code\repos\woodshed\ports\redshank\Cargo.toml --workspace --all-targets -j 1 -- -D warnings
```

Playback substrate experiments remain in the independent
`spikes/playback` workspace.
