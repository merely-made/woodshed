# Native Woodshed MusicXML editing — 2026-10-18

The bounded notation-led Arrangement prototype is qualified on the local Mere
supplier cohort. Music belongs to the working Set and saves through its ordinary
session workflow. Chart, Tab, Score and the linked fretboard edit the same musical
reading; the Set stores the canonical MusicXML. This packet records current-source
proof, not canonical-lock integration or release qualification.

## What works

- Explicitly write a compatible chord Set as music, preserving saved tuning,
  voicings, capo and authored bars where available. The fallback action explicitly
  writes each Card as a 4/4 bar. It refuses incompatible material or timing rather
  than flattening it. New exercises start with an explicitly chosen 4/4, 3/4 or
  6/8 and the current instrument/tuning and whole-number tempo.
- Select music in Chart/Tab/Score, switch views, split quarters into eighths or a
  triplet, make rests/notes, change individual pitches, edit written chord symbols,
  choose catalog voicings and audition the selection.
- Click empty fretboard space to add a simultaneous tone. Click an existing tone
  to select it; drag it to move it. Preview does not author; release is one undo
  step; Escape cancels. Moving onto a string occupied by another tone refuses
  without deleting it. Space/Enter can activate a focused fret. Remove is explicit.
- Preserve a written chord when notes disagree, flag the mismatch and offer chord
  suggestions. Suggestions do not replace authorship automatically.
- Choose quarter-note ascending/descending or eighth-note up/down picking,
  inspect pitches and coverage, then Apply or Cancel. Apply is one undo step.
- Save music with its working Set, retain its editing history across Save, and
  reopen the exact MusicXML in a new process. Original Cards and inactive Sets
  remain intact. Unsaved notation participates in the existing close/persona gate.
- Export `.musicxml` through the native writer and OS destination picker. The
  automated native lane selects a receipt destination through a scenario-only
  override; it does not automate or claim qualification of the OS save dialog.

## Evidence

[Machine receipt](receipt.json) records **60 passing focused cases**:

| Scope | Cases |
|---|---:|
| Musical model and bounded codec | 9 |
| Set-to-score adapter | 4 |
| Native paint, clefs, ranges, wrapping and shared staff alignment | 7 |
| Editing, history, owners, fret gestures and picking previews | 17 |
| Production native pointer/key/save/dirty-close routes | 1 |
| Existing Card arrangement routes | 3 |
| Existing persona routes | 19 |

The edit and fresh-reopen processes pass their scenarios and produce **12 reviewed
captures**, eight edit and four reopen, at 1180×1100 and 420×1100 logical pixels
(2× macOS captures). The edited file [native-edited.musicxml](exports/native-edited.musicxml)
is byte-identical to the active Set's saved XML. Preview checkpoints prove that
proposals do not alter its authority. Checkpoints also prove that original Cards,
the inactive progression Set, and the musical reading survive the new process.

Six pure Rust-emitted exports and the actual native-edited export pass the W3C
MusicXML 4.0 XSD and independent `musicxml` 1.1.2 reader. ElementTree checks compare
**101 pitched staff pairs, three rest pairs and 24 measures** across written pitch,
transpose, sounding TAB, tuning/fret, duration, rest, triplet ratio and brackets.
The fixtures cover guitar progression/rhythm, Bass, re-entrant Ukulele, Viola and
Cello. [Fixture checks](export-qualification.json) and
[native export checks](native-export-semantic.json) preserve exact inputs.
This does not establish third-party engraving or playback fidelity.

The binary SHA-256 is
`ae01825d8cd0518ed983dee148e124b13490532868773cc42b00f959ec480be9`.
All **955 source inputs** match before build, after build and after both runs.
[Provenance](cohort/provenance.json) identifies the dirty worktree bases; the
source ledgers identify the actual tested files rather than claiming HEAD alone
contains the implementation.

## Captures

- [Progression chart](captures/seed/01_progression_chart.png),
  [tab](captures/seed/03_progression_tab.png),
  [score](captures/seed/02_progression_score.png),
  [picking preview](captures/seed/04_picking_preview.png).
- [Rhythmic score](captures/seed/05_rhythm_score.png),
  [edited tab](captures/seed/06_rhythm_tab.png),
  [narrow music](captures/seed/07_narrow_music.png),
  [scrolled narrow fretboard](captures/seed/08_narrow_fretboard.png).
- [Freshly reopened score](captures/reopen/01_reopened_score.png),
  [narrow score](captures/reopen/02_reopened_narrow.png),
  [scrolled fretboard](captures/reopen/03_reopened_fretboard.png),
  [wide return](captures/reopen/04_reopened_wide.png).

Review corrected per-measure staff-height drift, replaced repeated triplet
numbers with group brackets, made room for TAB rhythm, and retained full-bar
wrapping. The final captures have no clipped staff/fingering in their declared
viewport; the complete narrow fretboard is reached through vertical scrolling.

## Build and reproduction boundary

The canonical locked command currently fails dependency resolution: the pinned
`scenomise` has `authoring` but lacks the prototype's required `djot` feature.
[Canonical check](logs/canonical-check.log) records that refusal. This change adds
only the already-locked `roxmltree` and `ttf-parser` dependency edges to the
canonical lock; it does not repin Mere or Genet.

The external harnesses point at the actual Woodshed source and the local
`mere-portable-scene-backdrops` supplier cohort. The shared native input change
is [five lines in Rootstock](cohort/rootstock-key-layout.patch): apply pending DOM
layout before post-key focus restyling, so inserted contextual controls have
computed styles. The production native regression exercises this fix, and the
existing arrangement/persona cases pass with it. Musical types/commands remain
in Woodshed; no superset music grammar was added to Mere.

The original build/run root is:
`/Users/markik/Code/output/woodshed-native-musicxml-20261018`.
Its `native-run-v2` holds the frozen binary and isolated profile. The fixture
Keeper used a dedicated synthetic identity and never opened the user's vault.
The native processes ended after their scenarios; no background monitoring is
installed. Receipt paths name those original run files; packet copies preserve
their bytes. The copied scripts/manifests are audit inputs and retain those paths.

To rerun checks against the unchanged original run:

```sh
python3 /Users/markik/Code/output/woodshed-native-musicxml-20261018/verify_native.py
python3 /Users/markik/Code/output/woodshed-native-musicxml-20261018/qualify_native_export.py
```

For builds/tests, use the copied manifest's original location and pass its
`.cargo/config.toml` explicitly. This toolchain uses a separate shared build-dir
and harness target-dir; running from the repository without that configuration
would start a different dependency/build cohort.

Remaining work includes canonical supplier integration, general external
MusicXML import, arbitrary polyphony and engraving features, fractional tempos,
meter-map rewriting, whole-arrangement practice playback and hardware audio
qualification. Current listening uses the existing selection audition path.
All work is still prototype source; no commit, push or release is claimed.
