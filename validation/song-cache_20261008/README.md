# Song chord cache qualification — 2026-10-08

The mixer prepares chord PCM outside the callback and its mutex. Read-only and
cosmetic edits reuse matching buffers; changed pitch/duration/rate inputs
receive fresh PCM. Stale preparation and older replacement requests cannot
overwrite current audio. Count-in and measure trigger times are preserved.

Baseline source: `9e4b37238d5febfcfe73b4bc9e291ec93b2ee1cd`. Audio implementation:
`f8e40371d5ad95c087106b8fab15f32ae039155b`. Native candidate: `0812c67`, including
current renderer development-profile overrides and the corrected tempo selector.
The receipt records full revisions, binary, log and capture hashes.

- 133 audio and 79 desktop tests pass; the desktop gate passes again with
  the current renderer profile. Committed desktop and diagnostic builds pass.
- Two baseline hardware runs emit 13 device overload notifications (cold
  4+4, edited 2+3, warm 0+0). Two precursor fixed runs and the final committed
  run emit zero. Final maximum callback gap: 10.741 ms, on 48 kHz stereo
  iMac Speakers with 512-frame buffers. All differential output is muted.
- Native seed/reopen succeed: 514/178 presentations, three/one reviewed
  GPU-readback PNGs, zero blank captures and zero buffer-error logs.
- Four saved bars survive reopening; the first is C-sharp Major at 245 BPM.
  Actual Play/Stop/Rewind/root/tempo controls and live-bar progress are asserted.

Full local artifacts (including PNGs and failed preflights):
`/Users/markik/Code/testing/woodshed/song-cache-20261008/`.
`native-qualified` is the accepted cohort. Startup/layout attempts and the
ambiguous tempo-selector attempt are preserved and excluded from acceptance.
Both final app processes exited; neither app was relaunched after completion.

Reproduction: `cargo test -p woodshed-audio --locked`,
`cargo test -p woodshed-genet --locked`, `cargo build -p woodshed-genet --locked`,
and `cargo run -p woodshed-audio --example audio_ddx --locked -- song`. Use the
committed `scenarios/song_cache.scn` and `song_cache_reopen.scn` in sequential
native processes with the same isolated profile, initially selecting Looper.
The profile is unsealed; no microphone, vault, release signing or acoustic
listening qualification is claimed. Preparation still runs on the caller.
A low-level live edit skips an unready strike until the next measure; the
desktop replacement route publishes ready PCM atomically. Other callback
locks and recording-buffer allocations remain separate hardening.
