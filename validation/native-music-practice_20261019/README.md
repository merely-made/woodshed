# Canonical arrangement practice — 2026-10-19

The native MusicXML arrangement now drives the existing shared sample-clock
output program. This closes the earlier prototype's external-manifest dependency
boundary: the application builds from its own canonical manifests and lockfile.
Mere supplier `77d820026ff0c1c5fa08b12eee7039f49206a028` publishes the opt-in
Djot transport, free/unknown-stress presentation and keyboard relayout repair.
[Dependency record](canonical-dependencies.json) identifies the real Git sources
and confirms that no non-workspace local package paths enter this build.

## Product behavior

- Practice arrangement plays the visible music, including unsaved edits and
  visible picking previews, without saving or rewriting the Set.
- Exact note gates, rests, simultaneous chord tones, triplets, physical tuning
  and variable bar lengths lower to the existing 960-tick quarter-note clock.
- Edits queue for the next natural bar boundary. The current bar retains its
  prepared music; only a matching processor revision clears the pending state.
  If the edited version has no following bar, playback returns to its start.
- The Set's Off/All/Card repeat setting appears as Off/Arrangement/Bar. Bar
  repetition starts at the selected bar; changing the setting while playing
  preserves the current bar and device clock.
- One shared practice tempo across Woodshed covers 1–400 BPM and restores
  through normal application settings. Practice speed changes leave the
  authored MusicXML tempo untouched; it remains a reference that can be matched
  explicitly. The reopened capture shows practice at 92 BPM and written music
  still at 96 BPM.
- Editing selection stays independent of playback. Follow playback shows the
  notes across the arrangement or a chosen bar range, highlighting the currently
  played notes. Rests and playback outside the chosen range clear the highlight.
- Normal Set Save and fresh reopen preserve the exact authored MusicXML.
  Playback reopens stopped. Card-order practice remains available and shares
  the output lane through processor-acknowledged handoff.

## Qualification

[Receipt](receipt.json), [native invariant verification](native-qualification.txt),
and [source ledger](source.json) bind this qualification to the actual files and
frozen native binary, rather than claiming the old branch HEAD contains them.
The final headed run uses **254 unchanged source inputs**, two fresh native
processes, and ten captures at 1180×1100 and 420×1100 logical pixels.

The canonical checks cover 732 distinct Woodshed cases:

| Gate | Passed |
| --- | ---: |
| MusicXML model | 9 |
| Complete core with portable scenes, storage and transport | 322 |
| Saved arrangements, including fresh CLI processes | 5 |
| Complete view library | 301 |
| Complete OutputRuntime suite | 69 |
| Native music binding and PCM adapter | 10 |
| Existing Set/live practice clock | 11 |
| Existing native arrangement controls/gestures | 3 |
| Isolated receipt controls | 2 |

The supplier adds 13 passing cases: 4 document/Djot, 8 backdrop, and 1 Rootstock
keyboard relayout regression. [Logs](logs/) retain the receiving checks;
[supplier logs](supplier/) retain its focused canonical gates.

The audio engine's tests compare PCM across the old/new boundary, gates, rests,
chords, sample rates and buffer partitions. They also cover latest-update wins,
pause, explicit stop/seek/advance cancellation, observer pressure, session fences,
shortened arrangements, bar repeat and a batched tempo change across 256 bars.
Its callback allocation/deallocation guards stay active.

The native checkpoints record exact rendered stereo frame/sample counts, PCM
float-bit hashes and nonzero counts, processor epochs/revisions and positions,
and independent draft/applied/saved/stored XML hashes. The verifier proves that
paused and stopped advances are silent, a queued edit has not changed the saved
source, the edited version applies at bar 2, practice retiming does not author a
tempo change, and fresh reopen preserves the exact saved XML before repeat wraps.
PCM hashes are recorded from the real processor; this packet does not retain
raw PCM for independent hash recomputation.

## Captures and review

- [Chart with contextual playback fretboard](native/seed/02_contextual_fretboard.png)
- [Tab](native/seed/03_playing_tab.png) and [score](native/seed/04_playing_score.png)
- [Selected-bar context, with playback outside it](native/seed/02b_selected_bar_context.png)
- [Narrow music](native/seed/04b_narrow_playback.png) and [scrolled narrow fretboard](native/seed/04c_narrow_fretboard.png)
- [Freshly reopened music](native/reopen/01_reopened_music.png)

Review distinguished written-tempo controls from practice speed, labeled the
bar range, made the active repeat choice visible, and replaced missing sharp
text glyphs with conventional ASCII pitch names. Translucent context disappeared
in the scrolled narrow view; this component now uses opaque theme dim colours,
which preserve the surrounding notes and grid after scrolling. This does not
claim a general repair of the framework's opacity/scrolling interaction.

## Reproduction and limits

Build and tests run at the Woodshed workspace root, without local supplier
patches or external build manifests. The narrow receipt backend is enabled only
when both `WOODSHED_MUSIC_PRACTICE_RECEIPT=1` and a scenario are present; ordinary
startup retains the native device backend. The receipt lane renders the same
OutputRuntime processor in blocks no larger than 8192 frames. It does not open
a CPAL output stream or microphone, and elapsed UI time does not advance music.

[Launcher](launch_native.py), per-process environments, checkpoints and scenario
receipts document the isolated synthetic persona setup. No user vault is used.
The frozen executable and synthetic fixture keeper remain in the local output
folder; they are not checked into this repository. The verifier accepts that
run folder and `--source-root` for the matching checkout. Earlier external-cohort
receipts remain historical records.

This qualifies the supported editable MusicXML profile and software output.
General MusicXML import, arbitrary notation/polyphony, physical audio device
qualification and the normal OS export dialog are separate boundaries.
