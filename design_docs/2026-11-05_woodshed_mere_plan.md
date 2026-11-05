# Woodshed's practice data in a Woodshed mere

**Status (2026-11-05):** draft for Mark's review. Nothing is scheduled. The
direction is ruled (ruling 6, re-ruled 2026-11-05), and so is the domain:
`music`, shared with Hocket (Mere adoption plan D5). The phases and the
migration are proposed. Mere's shared work comes first:
`mere/design_docs/mere_docs/implementation_strategy/2026-11-05_domain_adoption_plan.md`
(M1 to M5).

**For:** Mark's re-ruling of ruling 6 in the
[survey brief](2026-10-14_dsp_graphshell_hocket_survey_brief.md#rulings-mark-2026-10-14):
Woodshed's whole domain moves into a Woodshed mere in the reservoir and
follows every device of the persona by default. Mark: "I would, by default,
like for even my working state to be replicated to my phone ... the chords and
whatnot that I'm working with, the progression I'm working with."

**Not here:** Hocket's and Redshank's own moves (their rows are in Mere's
adoption plan; Hocket shares the `music` domain); Ringdown, which stores nothing of its own (its pedal bindings
are Woodshed settings); the mobile shell, which follows the web host.

## Today

Everything is two sealed muniment slots under
`<root>/personas/v1/<owner>/` (`crates/woodshed-genet/src/storage.rs`):
`session` holds one JSON `PersistedSession` and `settings` holds `AppSettings`.
Loop takes are content-addressed PCM blobs beside them, up to about 64 MiB each
and 256 takes. The key comes from custody; no practice bytes leave the app.
Window geometry (`desktop.json`) and authored themes (`themes.json`) are
outside the persona store.

## What moves and what stays (proposed)

| Moves into the mere | Stays on the device |
|---|---|
| Working Set and working Sets, retained Sets, catalog explorations, musical context | Window geometry |
| Song and looper configuration | Audio and MIDI device choices |
| Practice history | Workspace layout, scope navigation, Overview presentation |
| Personal relations | Catalog cursor indices (`root_idx`, `scale_idx` and the rest) |
| Loop takes, as blobs fetched on demand | Cached take bodies past the device budget |
| Settings that follow the person: theme choice, accessibility, tuning, pedal bindings | Settings tied to the machine |

## Phases

### W0. Domain id: `music` (ruled 2026-11-05)

Mark: "music, i guess". Woodshed and Hocket share one `music` domain, since
both record phrases through `hocket-model` (Mere adoption plan D5). A music
library, if one is ever built (Mark floated Redshank), joins it. The validator
therefore covers Hocket's records too: W1's crate composes with
`hocket-model`'s `ProjectBundle::validate`.
- Done: ruled.

### W1. A pure domain crate

`woodshedding` is already pure (serde and roxmltree). The record types and
checks the validator needs are spread through `woodshed-core`, which depends
on muniment, personae and chartulary: Set and Card ownership,
`saved_arrangements::validate_alignment`, loop-capture header and bound
checks, and `admit_card_ids`. Move them into a pure crate and write one
complete-candidate validator. Mere's resident will depend on it the way it
depends on `cleromancy-core`.
- Done when: the validator accepts every current fixture and refuses each
  known invalid case, with no I/O dependency.

### W2. Ids and revisions that survive two devices

- `CardId`, `SavedSetId` and `WorkingSetId` are per-device counters. Two
  devices minting at once collide. Move to globally unique ids, keeping the
  current values readable.
- Song bars are positional (`RecorderTarget::SavedBar(usize)`), and so are the
  catalog cursors. Bars need stable ids. The cursors stay local.
- The Overview stamps `Revision(0)` and positional `InstanceId`s
  (`crates/woodshed-core/src/session_overview/scene.rs`). It needs real
  revisions.
- Practice history serializes `HashSet` and `HashMap` fields in varying order
  (`fold_receipt.rs`). Make it canonical so two devices produce the same bytes.
- Done when: the same edits on two simulated devices produce merge-ready
  records with no id collision and byte-identical history.

### W3. Split the session

Separate `PersistedSession` and `AppSettings` into the authored part, which
becomes mere records, and the device part, which stays in a device-local
store. Follow the table above.
- Done when: the authored part round-trips through the validator, and the
  device part never enters a mere.

### W4. Register, attach and embed

Register the domain through Mere's registry (M1), with the validator required
(M2). Woodshed attaches to the resident when one runs and embeds it otherwise,
as Cleromancy does. Declare the `mere/<domain>` route in
`crates/woodshed-genet/src/storage/declaration.rs`; its tests now assert that
no mere route is declared, so they change with it.
- Done when: the reservoir's V5 checks hold for Woodshed: it opens its mere
  with Djinn absent, attaches as a client when Djinn runs, and fails clearly
  if both try to own it.
- Needs: Mere M1, M2 and M3 (sealing, so nothing is downgraded).

### W5. Migration (needs Mark's ruling)

The preservation plan's D1 forbids inventing a migration. Proposed: a one-time
import from the sealed slots into the mere on first open, verified by the
validator, with the slots kept read-only until Mark retires them.
- Done when: Mark rules the migration, and a real persona's store imports with
  nothing lost, compared record by record.

### W6. On a second device

- Done when: a Set and a progression edited on one paired device appear on a
  second; concurrent edits merge or surface as a branch; a take recorded on
  one opens on the other after a fetch.
- Needs: Mere M4 and M5; device pairing.

## Open questions

- Whether retained Sets and working Sets stay distinct once everything
  follows the person.
- Whether authored themes (`themes.json`, application-wide today) should
  follow the person too.
