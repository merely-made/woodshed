# AP3 native pack/project and guitar qualification — 2026-10-20

**Bounded macOS acceptance and physical guitar playback; desktop matrix open.**
The [canonical plan](../../design_docs/2026-09-09_audio_ports_rehome_plan.md#ap3--qualify-product-and-instrument-paths)
owns scope. All AP1/AP2 and AP3 changes remain local and uncommitted. No commit,
push, supplier fork or release was requested. This receipt supersedes AP2's
unqualified native paths without rewriting its historical evidence.

## Product results

| Path | Native acceptance and retained evidence |
|---|---|
| Woodshed | Actual Tools → Metronome controls load bundled and installed kits, cycle steps, mute/unmute Hat, reduce Snare sample level, refuse an occupied move, move Kick, switch patterns without losing edits, change tempo, Play/Stop, and export through a native Save dialog. The existing sealed practice session is read back and compared to the live selection. Missing kick sample refusal preserves the prior selection/save. |
| Woodshed fresh process | The installed kit directory is moved away before a new process starts with the same private sealed profile. Selection and inactive drafts compare exactly, pattern switching preserves the edit, Play/Stop works, and a new native MIDI export is byte-identical to the first. |
| Hocket | Actual shared kit controls load/edit an installed kit, arm a track, mute the click and Add drum loop. The production graph moves the layer from queued to assigned and reports nonzero output independently of the muted click. Stop all removes accepted playback. Native Save writes a `.hock`; Export mix and Export MIDI write files; missing-asset replacement refuses without dirtying/replacing the saved project. |
| Hocket fresh process | The installation is moved away before a new process opens the saved `.hock` through the actual Open dialog. Exact project bundle, kit snapshot, inactive drafts, media references and 377,712-frame / 48 kHz rendered layer return. A full step cycle toggles dirty then restores clean state. MIDI and WAV exports compare byte-for-byte before/after. |
| Guitar | Following explicit approval for one new file, exact copy/readback and unchanged hashes for all ten factory beats, storage was safely ejected. The owner confirms the new file appears in Beats and plays repeatedly. |

Native process receipts, presented captures, synthetic input/profile metadata
and compressed exact checkpoints are under [native/](native/). Each app has two
independently launched processes and normal scenario completion/close. Actual
native file panels are selected by `dialogs.py`, scoped to the owned PID.
Mesquite performs retained input, assertions and presented-window captures;
the domain checkpoint actions only observe state/storage, without injecting a
kit or saved project. Keeper and app profiles are private synthetic fixtures,
with no normal user profile or peer transport used. Fixture binaries/vaults
remain in scratch, excluded from this receipt.

[Observations](observations.json), [receipt](receipt.json),
[source hashes](source-hashes.json), [AP2 source delta](source-delta-ap2.json),
[attempt history](attempts.json) and [evidence hashes](evidence-hashes.json)
retain the comparisons and provenance. Delta entries with no AP2 baseline
identify files absent from the AP2 hash inventory, not necessarily new files.
Checkpoint JSON files above 100 kB are
stored as `.json.gz` with deterministic gzip timestamps.

## Fixes found through the actual application

- Shared step cells use explicit flex layout; controls have track-qualified
  accessible names. Hocket's kit panel has its own bounded scroll area and
  product styles so recorder/transport remain visible.
- Hocket's former kit notice promised an Undo button absent from the current
  native view. It now describes normal audition, layer mute and rendered-media
  handoff. Model history remains; draft controls stay project metadata.
- Repeated mic input overflow previously returned from `Engine::tick` before
  output events were flushed. Incomplete capture still aborts and reports loss,
  while independent playback/stop graph changes are now serviced.
- Pending playback previously needed a phase drop larger than half a bar to
  recognize a boundary. With this debug UI's roughly 1.5-second frames, every
  wrap could be missed and a layer stayed queued forever. A fallback deadline
  uses Firewheel's existing monotonic processed-sample clock, invalidated on
  tempo restart. Existing capture wrap semantics are preserved. This prevents
  starvation; starts still have control-tick resolution.

The debug native host remains slow and can lose microphone input between UI
ticks. These repairs establish playback progress and preserve explicit capture
failure. They do not establish acceptable live-performance latency or native
capture under those stalls. No extra engine, sequencer or musical clock was
introduced. The only Mac-specific additions are acceptance launch/dialog tools.

## Software checks

Locked/offline Rust/Cargo 1.99.1, explicit matching rustdoc, debug information
disabled, four build jobs. Selected owning-package tests: **701 passed, zero
failed, two intentionally ignored existing native-display tests**. This is not
a whole-repository/full-family suite.

| Packages / selected tests | Passed |
|---|---:|
| `audio-packs` | 13 |
| `audio-pattern` | 35 |
| `woodshed-audio` | 160 |
| `hocket-project` | 38 |
| `hocket-recorder` | 28 |
| `woodshed-core` plus its two integration tests | 295 |
| `hocket-engine` plus integration tests | 42 |
| `hocket-genet` | 90 (2 ignored) |

Thirteen new regressions cover installation deletion, exact retained MIDI/PCM,
inactive drafts, stale/invalid/occupied edits, corrupt/missing retained assets,
bounds, destination collision, recorder CBOR and practice-save round trips,
production backing WAV parity, callback allocation behavior, repeated input
loss with playback/stop, sparse ticks, and tempo-restart deadline replacement.
The callback test measures the production prepared-pack swap/render helper;
it is not a blanket allocation claim for the complete native host.

Native desktop builds pass. Logs are in [logs/](logs/). All three product
lockfiles match the frozen AP2 hashes. AP1/AP2 Wasm/MSRV compiler evidence is
historical, not a new AP3 browser or Windows/Linux execution result.

## Guitar artifact and observation

- Read-only event/timing aggregates for all ten factory files:
  [guitar-factory-midi.json](guitar-factory-midi.json). No factory MIDI bytes
  are redistributed. Current limited storage/settings observation is in
  [guitar-before.json](guitar-before.json); no configuration was written.
- Actual Woodshed native export: SMF 0, PPQ 480, one 4/4 bar, 12 notes, 125 BPM.
  The app MIDI before/after hashes agree. A diagnostic helper repeats its
  events five times, omits tempo like the factory material, and emits the
  reviewable [11-Ringdown_AP3.mid](artifacts/11-Ringdown_AP3.mid): 519 bytes,
  9,600 ticks, channel 10, note 36 kick / 38 snare / 42 hat, balanced note-offs.
  SHA-256 `bb81218e90b12fc757ccd6d02f4aab85cf187cd3bf8a0021156f668cce7618db`.
  This is a trial artifact, not a shipping guitar export profile.
- The user explicitly approved adding that one file to `HYVIBE/Drums`.
  [Write receipt](guitar-trial-write.json) records exact readback and all ten
  unchanged factory hashes. The volume was safely ejected before audition.
- [Owner observation](guitar-playback-observation.json): **“It appears and
  plays repeatedly.”** After stopping Beats and restarting the guitar with
  USB OFF, the owner separately confirmed: **“It still appears and plays after
  restarting.”** Saved-file discovery and playback therefore survive a power
  cycle on this instrument.

Five identical bars do not reveal which bars the guitar loops or whether it
uses a count-in. Factory patterns vary in length; stored `nbbars` is not
reliable evidence of a particular file's loop range. Notes 36/38/42 are a
working subset, not a complete drum voicing/velocity map. MIDI carries note
instructions, not the pack's WAV samples. Remote Beats start/stop/pattern
selection, firmware coverage and performance scripting remain Ringdown work.
The earlier pedal menu had no standalone Beats action; no guessed command was
sent in AP3.

## Reproduction and retained failures

`native.py` launches each exact frozen app with an isolated no-wallet Keeper,
private environment, unique temporary executable name and a 240-second bound
per process. It moves `installed-kit` before phase two. Frozen binaries,
original scenarios and synthetic profiles remain under
`/Users/markik/Code/testing/woodshed/audio-packs-ap3_20261020/`.
`verify.py woodshed hocket` checks both successful process pairs, exact state
and export comparisons. `collect.py` freezes bounded receipts. The diagnostic
`inspect_midi.py` parses event aggregates; `midi_trial.rs` is the midly helper.

Earlier attempts are retained in scratch and summarized in `attempts.json`:
ambiguous temporary process names in the dialog helper, slow/unbounded initial
panel layout, dirty-close preservation in a smoke run, an early output check,
input-overflow starvation, and the missed-wrap playback failure. The separate
`hocket-audio-probe` cohort isolated queued=1 / assigned=0 while the audio clock
advanced. These are failed/diagnostic attempts, not native acceptance passes.

Cargo runs from `/tmp` with absolute manifest and target paths to reuse warm
artifacts. An initial wrong relative edit path, rustc/rustdoc mismatch, and a
second cold target invocation were corrected; the WAV inspector was adjusted
for the actual standard extensible float header; only task-owned canceled Cargo
processes were stopped. No unrelated work was stashed or discarded.

## Remaining qualification

Windows/Linux native execution and browser audio, live-performance latency,
physical desktop-speaker audition, two-peer handoff,
broader guitar loop/count-in behavior across lengths/meters and firmware/note
mapping remain open. The five-bar 4/4 startup/repeat/stop-start rule is now
owner-confirmed below. Mere's composition surfaces and FunDSP adapter remain planned in the
canonical document; this slice adds neither a graph editor nor a supplier
fork. This bounded receipt does not close AP3's full target-desktop matrix.

## Distinguishable-bar loop/count-in trial

**2026-10-20: owner-confirmed intro/loop behavior and stop/start; baseline trial complete.**
[Trial definition and procedure](loop-count-in/trial.json) and
[169-byte MIDI artifact](loop-count-in/12-Ringdown_Bars.mid) retain the new
experiment separately from the earlier accepted/reboot-qualified file.
SHA-256 `a3a0de7aadb6dc745991874ea65c188b222d72a215e938c787c209bffb9325e1`.

| Bar | Signature in four beats |
|---|---|
| A | One kick on beat 1; rest |
| B | Two snares on beats 1, 2; rest |
| C | Three hats on beats 1, 2, 3; rest |
| D | Four kicks on beats 1, 2, 3, 4 |
| E | Five quick snares on beats 1, 1-and, 2, 2-and, 3; rest |

All bars are four seconds at 60 BPM. Independent MIDI inspection confirms
1/2/3/4/5 hits, channel 10, balanced 60-tick note gates, PPQ 480, 4/4 and a
9,600-tick end. There are no tempo, program-change or controller events; the
guitar's tempo setting governs playback. The five-bar format/meter/length
matches the previous working trial. Only previously used notes 36/38/42 occur.
`prepare.rs` reuses `audio-packs` admission, the existing shared pattern model,
MIDI export and offline renderer; it introduces no product sequencer or engine.
A 20-second, 60 BPM reference WAV using bundled computer samples remains at
`/Users/markik/Code/testing/woodshed/audio-packs-ap3_20261020/loop-count-in-artifacts/reference-60bpm.wav`.
Its hash/specification is in the trial definition. It represents one complete
A–E pass, not observed guitar voicing or loop behavior.

The owner approved the additive file with the guitar connected in USB mode.
[Write receipt](loop-count-in/write-receipt.json) records exact readback and
unchanged hashes for all existing regular files, including the ten factory
beats and prior trial, followed by successful safe ejection. A stricter
directory-entry assertion detected an additional new entry whose name was
not retained before ejection. macOS metadata is a plausible explanation, not
a confirmed identification; inspect at the next normal USB connection.
The assistant intentionally wrote only the approved MIDI file.

[Physical listening observations](loop-count-in/listening-observations.json):

1. Initial start: owner reports **“A B C D E once, then B C D E repeats.”**
2. Stop, wait, start again with the same file/settings: **“A returns once,
   then B C D E repeats.”**
3. Panel check: **“No count-in setting found; playback begins directly with A.”**

For this five-bar 4/4 trial, the first authored bar is a one-time lead-in on
each fresh start; bars 2–5 form the repeating body. From the known file, those
regions are ticks [0, 1920) and [1920, 9600). These are derived from heard bar
identities, not sample-accurate hardware timing. There was no reported extra
click count-in before A. The owner did not find a separate count-in control;
this does not establish its absence in all firmware/menu contexts.

**Export consequence:** keep the generic shared exporter unchanged. A
Ringdown guitar-specific profile should explicitly provide a first-bar intro
followed by the intended repeat body. This is a plan direction, not shipping
export behavior. Test other lengths/meters and relevant firmware/settings
before generalizing the five-bar result. 60 BPM was requested during this
trial; actual panel tempo and processor firmware were not independently read
back. Mid-bar stop behavior and phase accuracy remain unqualified.

Preparation is device-free and uses a separate scratch Cargo package/lock;
product lockfiles are untouched. The first helper compile rejected a borrowed
MIDI-event lifetime; reconstructing owned MIDI messages fixed it. Both compile
logs and the successful local verification inputs are retained in this subfolder.
