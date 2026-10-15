# Bounded preview worker qualification — 2026-10-08

Implementation and native replay: `c97be580c177cc866e71de96b1225b8eb86ceb40`.
Baseline: `0f104028523b22cd2f13ee0ff73f3cc889b3d873`. Current Mere/Genet pins
are preserved. Each SongEngine owns one renderer, with one in-flight job and
one replaceable pending request. New Hear supersedes old work and sounding
previews. Step notes retain sounding envelopes, but supersede unfinished work.
Stop, Rewind, song replacement/playback, rehearsal Pause, exercise/arpeggio/
scale Stop, and shutdown cancel obsolete previews. Publication validates its
request revision under the mixer lock; synthesis runs on neither UI nor callback.

- 577 checks pass: 138 audio, 212 core, 148 views, 79 desktop. Committed desktop
  and diagnostic builds pass. Paused-render regressions prove caller/callback
  progress, bounded latest pending work, stale-publication rejection, transport
  cancellation and joined shutdown with a surviving external handle.
- Muted 32-pitch scale control: baseline caller work is 478.970–631.302 ms.
  Committed enqueue calls are 0.014–0.061 ms across six trials. Synthesis still
  takes 534.926–1910.830 ms in these runs; moving it off the caller is the result,
  rather than faster DSP. The diagnostic waits for work completion each trial.
- First committed scale replay records one startup event at 0.196 s in the
  monitor-only phase, before Woodshed engines exist, and no preview-phase
  events. Preserve this observation: the repeat emits zero events (maximum
  callback gap 12.537 ms). The committed song cold/warm/edited regression also
  emits zero events (maximum gap 10.734 ms). Device timing is not a hard deadline
  guarantee, and startup-event elimination is not claimed.
- Native seed/fresh-process reopen report `RESULT ok`, 162/165 presentations,
  three captures each, no blank images, and zero buffer-error log lines.
  Repeated Hear leaves one sounding preview; Rewind clears it. The saved
  92 BPM Thirds recipe survives removal of its source Card, retaining its pin,
  background role and Orbits atmosphere. Fresh-process Hear/Add and rehearsal
  Run/Pause pass; the final Set has four Cards and Pause leaves zero voices/work.
- All six PNGs were inspected. Inspector captures show a scrolled portion of
  the graph; fitted scene captures contain the retained recipe and connected
  nodes. No new responsive-layout claim is made. Both apps exited. Initial UI
  state queries returned AXError.cannotComplete for the short-lived launches;
  they were not retried or relaunched. Native result/presentation receipts and
  GPU-readback captures establish the replay.

Full local artifacts, PNGs and initial test attempts:
`/Users/markik/Code/testing/woodshed/preview-worker-20261008/`.
The receipt records source revisions, binary/log/scenario hashes and reviewed
PNG hashes. Concise gate and hardware/native logs are committed beside it.

Reproduce with `cargo test -p woodshed-audio --locked`,
`cargo test -p woodshed-core -p woodshed-views -p woodshed-genet --locked`,
`cargo build -p woodshed-genet --locked`, and muted
`cargo run -p woodshed-audio --example audio_ddx --locked -- scale` / `song`.
Run `scenarios/preview_worker.scn` and `preview_worker_reopen.scn` sequentially
against the same isolated state/settings profile, initially selecting Looper
at 1280×900. No microphone or personal session is used.

Song chord preparation remains synchronous on the caller. Callback locks,
recording allocations, broad startup/layout performance, acoustic/device-wide
quality and release packaging remain separate work. This is unsealed fixture
persistence qualification, not vault or release-signing evidence.
