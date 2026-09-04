# Redshank

Redshank is the offline-first listening and timed-annotation port. This
incubating workspace is intentionally separate from Woodshed's root product
workspace, and every package is unpublished while public naming remains gated.

Current packages:

- `redshank-model`: durable library, queue, progress, settings, representation
  receipts, text notes, audio notes, and host adapter traits;
- `redshank-storage`: a local JSON store using immutable numbered generations;
- `redshank-surfaces`: reusable Cambium Player and Capture surfaces. The compact
  composition depends only on its presentation snapshot and command queue, so a
  host can mount it without the Library;
- `redshank-desktop`: the sovereign executable over Mere's Cambium/Genet winit
  host. It restores the first queued item into the compact surface. Playback
  commands currently degrade explicitly until the Symphonia adapter lands.

The storage package writes and flushes a pending generation before publishing
it with a same-directory rename. Loading walks completed generations newest to
oldest and selects the first valid document. An interrupted or corrupt newer
write therefore leaves the last valid state readable.

Run the isolated model and storage gates from outside the repository's parent
Cargo configuration:

```powershell
$env:CARGO_TARGET_DIR = 'C:\t\redshank-phase2'
cargo test --manifest-path C:\Users\mark_\Code\repos\woodshed\ports\redshank\Cargo.toml -j 1
cargo clippy --manifest-path C:\Users\mark_\Code\repos\woodshed\ports\redshank\Cargo.toml --workspace --all-targets -j 1 -- -D warnings
```

Playback substrate experiments remain in the independent
`spikes/playback` workspace.
