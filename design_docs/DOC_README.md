# design_docs Index

Canonical first-reference document for project documentation. Read this
before any other doc in this directory.

## Project Reference Docs

- [PROJECT_DESCRIPTION.md](PROJECT_DESCRIPTION.md) — Product goals,
  major features, scope. Maintainer-owned.
- [DOC_POLICY.md](DOC_POLICY.md) — Documentation governance.

## Active Plans

- [2026-09-04_musical_projections_plan.md](2026-09-04_musical_projections_plan.md)
  — Circle-of-Fifths context and the 24-triad Tonnetz are implemented: keyed background,
  separate focus/audition/Add actions, and stable expansion.
  The September 7-8 slices add exact pitch-motion comparison, nearby-candidate
  browsing, and explicit selected-shape resolution. S1, S2 and S4 remain partial;
  S3, an anchored pitch-motion reading, neck movement, and general fingering
  costs remain open. The September 6 subsystem research
  maps meaning, inference, analysis, generation, and comparison; a live
  enumeration probe records lookahead/texture tradeoffs and the re-entrant
  bass defect, corrected by the selected-shape slice. Bounded comparison and
  co-op proofs remain fixture evidence.

- [2026-09-01_listening_annotation_port_plan.md](2026-09-01_listening_annotation_port_plan.md)
  **Active; Phases 0-3 complete, Phase 4 in progress.** Redshank is a separate audio-listening and timed
  text/voice-annotation port incubated in this repository. The September 6
  local-file slice adds a Symphonia/Firewheel worker, full listening surfaces,
  and saved text-note editing. The September 7 controller-admission slice puts
  the real decoder, sole output, transport state, and sink clock behind Genet's
  controller. HTTP/cache playback, headed acceptance, and Turnstone as second
  host remain open. It is separate from the Woodshed product graph.
- [2026-07-11_stage_set_tools_plan.md](2026-07-11_stage_set_tools_plan.md)
  — **In progress; product authority.** P1-P3 remain partial. P4a/P4b landed;
  the scene canvas and single-Card expansion have receipts, while the full P4e
  catalog and P4f practice/sound join remain open. P5-P8 remain open. A repaired
  release baseline does not close the 1.0 practice proof.
- [2026-08-27_smart_instrument_plan.md](2026-08-27_smart_instrument_plan.md)
  — **W1 and W2 landed and hardware-verified; W3 open.** The
  `woodshed-instrument` connection and metronome authority model work in both
  directions. Retrieval and resonance reads exist in the crate, but the crate
  is not integrated into `woodshed-views`, so there is no product surface yet.
- [2026-08-12_persona_picker_plan.md](2026-08-12_persona_picker_plan.md)
  — **P1/P2 landed; P3 open.** Startup selection and a live Settings switch
  swap the sealed practice store without restart. Creating a persona inside
  Woodshed remains open.
- [2026-07-04_genet_host_cross_platform_plan.md](2026-07-04_genet_host_cross_platform_plan.md)
  — **Desktop migration landed; delivery work remains.** The September 7 audit
  addresses page-scroll ownership, expanded-Set sizing, and responsive
  Rehearsal/Settings components. The shared view tree
  has a Windows host and an unshipped browser host. Cross-desktop receipts,
  browser audio/storage/accessibility, packaging, and deployment remain open.
- [2026-07-18_accessibility_semantic_surface.md](2026-07-18_accessibility_semantic_surface.md)
  — **In progress.** AccessKit, genet-probe, and agents consume one semantic
  Cambium surface. Initial names and marker semantics landed; the wider Tier
  0-3 audit remains open.
- [2026-07-11_audio_material_analysis_plan.md](2026-07-11_audio_material_analysis_plan.md)
  — **Active research.** The model-neutral benchmark and scorer landed; the
  September 6 refresh specifies held-out evaluation, alignment, provenance,
  and ESP ownership. No transcription or reasoning model is selected.
- [2026-07-14_instruments_and_fretboard_rendering_plan.md](2026-07-14_instruments_and_fretboard_rendering_plan.md)
  — **Open.** Tuning-general catalog expansion and configurable note-region,
  spacing, extent, marker, and fill rendering.
- [2026-07-15_fretboard_marker_detail_plan.md](2026-07-15_fretboard_marker_detail_plan.md)
  — **Open.** Hover detail, multi-pin comparison, and audition for fretboard
  markers; not yet built.
- [2026-05-20_theme_system_design.md](2026-05-20_theme_system_design.md)
  — **Open proposal awaiting sign-off.** Seed-derived palettes and theme
  management semantics.

## Current Supporting Design

- [2026-07-15_material_and_touch_model.md](2026-07-15_material_and_touch_model.md)
  — Material and Touch reframe for the Stage/Set model; design direction, not a
  standalone implementation plan.
- [2026-05-15_midi_design.md](2026-05-15_midi_design.md) — Current MIDI
  transport and clock-sync reference for the landed audio implementation.
- [2026-06-15_redesign_plan.md](2026-06-15_redesign_plan.md) — Current visual
  reference; its product/navigation framing is superseded by Stage/Set/Tools.
- [2026-05-21_arpeggio_lens_plan.md](2026-05-21_arpeggio_lens_plan.md) — Partial
  arpeggio catalog/stepping record. The old lens framing is subordinate to the
  Stage plan.

## Completed Implementation Records

- [2026-07-08_personae_sealed_session.md](2026-07-08_personae_sealed_session.md)
  — Persona-derived sealing and device carry, completed 2026-08-08.
- [2026-08-06_settings_persistence_split_plan.md](2026-08-06_settings_persistence_split_plan.md)
  — Separate application-settings persistence and one-time legacy migration.
- [2026-08-09_cambium_desktop_host_migration.md](2026-08-09_cambium_desktop_host_migration.md)
  — Woodshed migrated onto Genet's shared Cambium desktop host.

## Historical and Superseded Documents

These files remain at their established paths for receipt and link stability.
They are not current product authority.

- [2026-04-30_initial_plan.md](2026-04-30_initial_plan.md) — Initial Iced-era
  scaffold and roadmap.
- [2026-05-15_polyphonic_pitch_spike.md](2026-05-15_polyphonic_pitch_spike.md)
  — Superseded by the model-neutral audio-material analysis plan.
- [2026-05-16_song_mode_integration.md](2026-05-16_song_mode_integration.md) and
  [2026-05-19_song_timeline_layers_plan.md](2026-05-19_song_timeline_layers_plan.md)
  — Historical Song/DAW framing, superseded by Set-derived Looper work.
- [2026-05-16_xilem_migration_plan.md](2026-05-16_xilem_migration_plan.md) and
  [xilem_fork_patches.md](xilem_fork_patches.md) — Retired Xilem migration and
  fork ledger; the live host is Genet/Cambium.
- [2026-05-18_xilem_popup_view_proposal.md](2026-05-18_xilem_popup_view_proposal.md)
  — Retained upstream proposal; not active Woodshed work.
- [2026-05-21_fretboard_canvas_lenses_plan.md](2026-05-21_fretboard_canvas_lenses_plan.md)
  — Historical lens-based product frame; shipped portions feed Stage.
- [2026-05-21_composable_instrument_surface_plan.md](2026-05-21_composable_instrument_surface_plan.md)
  — Superseded product composition; reusable tools carry forward without one
  configurable stack.
- [2026-05-22_rehearsal_redesign_plan.md](2026-05-22_rehearsal_redesign_plan.md)
  — Historical prototype-to-Card transition, superseded by Stage/Set/Tools.
- [2026-06-14_web_profile_plan.md](2026-06-14_web_profile_plan.md) — Superseded
  by the Genet host plan; retained for the original Tier-0 seam analysis.

## Archive

- `archive_docs/` — retired plans and superseded notes.
- `archive_docs/2026-05-18/2026-05-17_woodshed_daw_plan.md` —
  Original "sibling DAW project under the Woodshed umbrella" plan.
  Superseded same-week: the maintainer chose a separate sibling repo
  (`repos/strophe/`), and the project scope pivoted from "general
  DAW" to a Deeler-inspired collaborative loop recorder. See
  `repos/strophe/design_docs/` for the live plan.

## Working Principles for AI Assistants

These principles apply to AI-assisted work on this project. Update this
section whenever a durable working insight emerges from a session.

- **Theory model is owned**: do not depend on `rust-music-theory` or
  similar upstream theory crates. We need exotic scales, non-tertiary
  chords, and arbitrary tunings; the upstream models do not support
  those, and inheriting their data shape costs more than it saves. Build
  the theory crate from scratch.
- **Gerund crates name activity cores**: `woodshedding` follows the same
  convention as `murmuring` and `mooting`: the gerund names the portable
  operation core that makes the activity possible, where such a core
  exists. If a crate is tempting to describe generically as "data
  structures," explain the activity those structures enable. Here,
  woodshedding means turning musical material into playable practice:
  identify material, realize it on an instrument, arrange it into
  progressions/exercises/practice sets, and feed app shells that rehearse
  it.
- **Pure core, thin shell**: `crates/woodshedding` must remain pure data
  + math — no I/O, no UI, no audio. It may model the portable operations
  required for woodshedding, but app-specific rehearsal UI, persistence,
  and audio engines live in consuming crates.
- **The committed lock is the release graph**: the gitignored
  `.cargo/config.toml` is optional sibling-checkout plumbing and must not be
  required to resolve `Cargo.lock`. Generate and verify the lock from outside
  the repository config path; CI's clean-checkout metadata preflight is the
  gate that catches a lock accidentally written under local path patches.
- **Stage is a verb and Set is the spine**: catalogs supply material; Stage
  adds configured Cards to one ordered Set; Rehearsal and Looper consume it.
  Do not create parallel practice, song, or tool-owned material documents.
- **The Stage graph projects the Set**: each staged Card occurrence is a stable
  node, Set order derives `Next`, and theory, history, and learned suggestions
  are separately identifiable edge layers. Filtering changes the projection;
  staging or editing changes the one Set through explicit actions. A node may
  collapse from Card to summary to glyph without changing Card identity.
- **Tools project shared state**: Fretboard, Metronome, and Tuner have
  standalone homes and contextual forms. Settings owns their durable
  configuration; contextual controls edit that same canonical state.
- **Desktop first, mobile later**: ship to itch.io / Gumroad for desktop
  before attempting mobile. Mobile is a shell around the web build (see the
  genet-host plan's M track); building toward it is part of the project's
  broader value but does not
  block the music app.
- **Generalize across stringed instruments**: theory model parameterizes
  string count and tuning so bass, ukulele, and banjo fall out for free.
  Do not hard-code 6-string assumptions.
- **Catalog and generators are complementary, not redundant**: the
  catalog answers "what are the well-known tunings?" (preserves cultural
  names, voicing conventions, history); generators answer "what does
  this tuning become under transformation?" (transpose, drop a string,
  apply an interval pattern). The same pattern will apply to scales and
  chords: named-scale catalog + apply-formula-to-root algorithm. Don't
  pick one over the other — they answer different questions.
