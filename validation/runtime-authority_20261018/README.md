# Runtime and material authority corrections

Date: 2026-10-18. Status: implemented and CPU qualified at `662cda5`; unpublished.
Baseline: published `6e6498a`. This bounded batch addresses B1 (CORE-4/CORE-6,
with LO-5 an alias), B2 (CORE-5) and B3 (AUDIO-01). It is separate from the
preceding H3/H5 and Q1 batch, now published through that baseline.

Three cheaper Luna workers owned disjoint source areas. The coordinator reviewed
the changes, handled receiving integrations and runs the final gates. Native apps,
resident custody services, microphone/device checks, browser acceptance and
fresh-process native reopening are outside this batch.

## Authority and complexity

- Path display, generic sound and managed lowering previously reconstructed notes
  separately. One retained-setup Path resolver now preserves complete ordered
  contacts and repeats. A shared contact seam feeds generic marks, managed mark
  filtering and retained-recipe validation. Display applies viewport clipping;
  valid out-of-view instructions remain playable. Unknown/unavailable setup is
  refused, and stale Solo never substitutes a full chord. Saved recipes reject
  mixed valid and unavailable marked contacts.
- SongBar retains one authored chord payload and a serde-defaulted mute flag.
  `is_silent()` preserves the legacy positive-root admission rule and is reused
  by playback and saved-song projections. Muting neither erases pitches nor
  stores a second chord copy. Legacy no-payload rests stay rests; the toggle
  cannot recover material already absent in an old save by inventing Major.
  Explicit chord edits while muted author new material without unmuting it.
- Future-bar capture Arms remain in the existing bounded boundary queue until
  their target is reached. Cancellation, pause/resume and ordered seeking retain
  their existing owners. One-shot termination exposes an unreachable arm through
  the existing invalid observation and reuses `stop_capture()` for retirement;
  no second scheduler, clock, queue or cleanup implementation was introduced.
  A new accepted arm clears the preceding attempt's invalid observation.

The source review removed incidental formatting, a duplicate proposed capture
cleanup helper and misleading input-loss-only status wording. Original captures,
clock policies and PCM are not migrated or normalized. CPU fixture serialization
and reopen are distinct from fresh-process native qualification.

## Evidence rules

Final acceptance uses coordinator process receipts with real exits and bound log
hashes. Lane reports are retained as development context; repeated focused runs
are not added to unique case counts. Failed receiving-test compile attempts and
superseded runs are excluded. No historical product receipt is rewritten as a
current native result.


## Final qualification

The [process-bound receipt](receipt.json) records **859 distinct parent/library
passing cases**, **five separately counted child invocations**, and **seven
existing ignores**:

| Gate | Passes | Ignored |
|---|---:|---:|
| Core with portable scenes | 309 | 0 |
| Views | 244 | 0 |
| Host | 246 | 7 |
| Output runtime, including selected capture and allocation guards | 59 | 0 |
| WAV/live parity across fractional clocks and chunk boundaries | 1 | 0 |

All accepted processes returned zero. Child invocations are nested in the host
log and excluded from the parent/library total. Focused or superseded reruns are
not counted again. Both failed receiving-test compile attempts are retained in
[excluded](excluded/), with actual process exits.

Final review extended the existing Card geometry path to retain the saved Path
instrument's string count while keeping the live viewport as display clipping.
A six-string saved Path remains a six-string board after a live switch to four
strings; unknown saved setup uses the shared empty geometry. This final geometry
change is covered by the core/views/host gates at `662cda5`. The runtime/WAV gates
use `2200350`; their source and dependencies are unchanged by the two subsequent
geometry files, as recorded in the receipt.

No native validation scenario, original media, capture clock policy, supplier
source or lockfile changed. The source audit remains open beyond this batch.
