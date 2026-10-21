# AP2 application adoption — 2026-10-20

**Implemented; compilation and file workflow evidence.** The
[canonical plan](../../design_docs/2026-09-09_audio_ports_rehome_plan.md#ap2--share-authoring-audition-and-midi-export)
owns scope. Baseline main was `0452bcea1ff227924631ac190744e2bd9282fba5`, with
the prior AP0/AP1 work already local and uncommitted. This receipt records the
AP2 source and its qualification limits; it does not replace older receipts.

## Product paths

- **Woodshed:** Tools → Metronome contains the shared kit controls. Choose
  Basic drums or a kit folder, edit steps/accent/mute/sample level, and use the
  existing Play/tempo controls. Selection, edited patterns and exact assets are
  retained in the existing sealed practice session. The existing output
  processor consumes the kit in metronome, practice-program and backing lanes;
  Looper WAV export freezes that selection in its existing worker too. Count-in
  retains the conventional click. Kit file work is session-qualified,
  runs off dispatch and participates in close preservation.
- **Hocket:** the same panel appears with the recorder. Arm a track and add a
  drum loop: the existing correlated project worker renders at project tempo
  and matching meter, then the existing import commits a normal recorder layer
  for playback, media storage and Undo. The editable kit draft is local project
  metadata and is included in dirty/save acknowledgement/reopen paths. Draft
  controls are outside recorder history. Handoff carries normal rendered media
  and omits the local draft, so a receiving player needs no pack engine.
- **Ringdown:** `ringdown-probe --audio-pack <source> <pattern> <new.mid>
  [new-rhythm.json]` returns before Bluetooth discovery and calls the same
  portable file workflow. It writes no instrument files and leaves the protocol
  core unchanged.

`audio-pack-views` emits actions over product-owned selections using existing
Cambium buttons. One-step hit moves use Mere's `SpanEdit::Move`; the music
owner validates the revision, source and destination. Larger rhythm/phrase
scenes, duration editing, typed routing, FunDSP and deliberately bound dynamics
remain in the canonical plan.

The existing pure offline renderer moved to `audio-pattern::render`, with the
old Woodshed API re-exported. Hocket uses that same implementation with exact
frame counts and a complete-phrase render. Existing private renderer cases
moved with their owner; no new cases or suites were added or run.

## Evidence

Host: x86_64 macOS 15.8.1. Explicit rustup executables, offline locked builds,
debug information disabled, Cargo invoked from `/tmp` with absolute manifests.

- Rust/Cargo 1.99.1 `cargo check` passed for `woodshed-genet`, Hocket's
  `hocket-genet`, and `ringdown-probe` in their owning workspaces.
- Shared `audio-pattern` and `audio-packs` libraries compile for
  `wasm32-unknown-unknown` with Rust/Cargo 1.99.1, and natively with rustc
  1.97.1 using Cargo 1.99.1. These are library compiler checks, not browser audio
  or whole-GUI MSRV qualification.
- Built the portable CLI and production-runtime WAV example. The ten actual
  file commands in [commands.json](commands.json) exported bundled/installed
  kits, reopened an edited retained selection after its installation moved,
  switched patterns and returned to the edited one, then rendered through the
  existing production processor.
- Returning to the edited pattern preserved its MIDI and WAV bytes. Its 90 BPM
  bar contained 128,000 finite mono frames at 48 kHz; peak approximately 0.6786.
  [observations.json](observations.json) records comparisons and artifact hashes.
  Artifacts remain under
  `/Users/markik/Code/testing/woodshed/audio-packs-ap2_20261020/artifacts/`.
- Existing external package blocks were preserved in all three lockfiles.
  Hocket's resolver initially rewrote seven unrelated package edges; their
  original blocks were restored before its final successful locked check.
  New local packages and required new dependencies remain recorded normally.

Exact source hashes, commands, build results, artifacts and lock review are in
[receipt.json](receipt.json), with logs alongside it. The native targets emit
existing deprecation/dead-code warnings; this task does not alter those owners.

## Scope still open

No native GUI or save-dialog interaction was performed. Fresh-process sealed
practice/`.hock` interaction, project handoff interaction, audible device
playback, callback allocation measurements, Windows/Linux execution, browser
audio and guitar format/playback acceptance remain AP3 work. The file
demonstration exercises retained selections and the shared production renderer;
it does not establish those application/device outcomes. No test suites ran.

All implementation remains local and uncommitted; no publication occurred.
