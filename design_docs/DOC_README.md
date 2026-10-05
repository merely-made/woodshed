# design_docs Index

Canonical first-reference document for project documentation. Read this
before any other doc in this directory.

## Project Reference Docs

- [PROJECT_DESCRIPTION.md](PROJECT_DESCRIPTION.md) — Product goals,
  major features, scope. Maintainer-owned.
- [DOC_POLICY.md](DOC_POLICY.md) — Documentation governance.

## Active Plans

- [2026-09-13_redshank_gui_implementation_plan.md](2026-09-13_redshank_gui_implementation_plan.md)
  — **Active.** Implements the endorsed Redshank design canvas: fixed-height
  dock in two families, three-width responsive rule, two tinct-derived seeds,
  Listen/Library/Notes/Mere/Settings, feed-node projections (chain, orrery,
  trail), model extension for the design's settings and span notes, a
  self-drive scenario/capture lane, and a browser host. Lanes and done-conditions per lane.

- [2026-09-09_audio_ports_rehome_plan.md](2026-09-09_audio_ports_rehome_plan.md)
  — **Landed.** Hocket and Ringdown are history-preserving nested `ports/`
  workspaces with local dependency seams, independent release boundaries, and
  MPL-2.0 repository licensing. Their former repositories are archived with
  relocation notices; inherited family-format and Hocket-Clippy drift is
  recorded as separate work.

- [2026-09-04_musical_projections_plan.md](2026-09-04_musical_projections_plan.md)
  — Circle-of-Fifths context and the 24-triad Tonnetz are implemented: keyed background,
  separate focus/audition/Add actions, and stable expansion.
  The September 7-8 slices add exact pitch-motion comparison, nearby-candidate
  browsing, explicit selected-shape resolution, an anchored pitch-motion
  reading, and per-string neck movement. S1 and S4 remain partial; S3,
  multi-card comparison and general fingering costs remain open. The September 6 subsystem research
  maps meaning, inference, analysis, generation, and comparison; a live
  enumeration probe records lookahead/texture tradeoffs and the re-entrant
  bass defect, corrected by the selected-shape slice. Bounded comparison and
  co-op proofs remain fixture evidence.

- [Timed transcripts](2026-09-26_timed_transcripts_plan.md): bounded WebVTT parsing,
  explicit fetch/save and offline reopening, active cue display and seek are
  implemented and tested. Stale download and oversized-response checks pass;
  native fixture fetch/save, exact cue seeking and offline restart pass. Five
  subscribed public feeds advertise no timed transcripts; Decoder links written
  website transcripts. Headed real-episode acceptance and full text-track
  rendering remain open.

- [2026-09-01_listening_annotation_port_plan.md](2026-09-01_listening_annotation_port_plan.md)
  **Active; Phases 0-3 complete, Phase 5 closed 2026-09-26, Phase 7 met 2026-09-22, Phase 4 in progress, Phase 6 identity/seek and synthetic alignment implemented; extraction to its own repository ruled 2026-09-26.** Redshank is a separate audio-listening and timed
  text/voice-annotation port incubated in this repository. The September 6
  local-file slice adds a Symphonia/Firewheel worker, full listening surfaces,
  and saved text-note editing. The September 7 controller-admission slice puts
  the real decoder, sole output, transport state, and sink clock behind Genet's
  controller. A two-process headed receipt now proves local output playback,
  text-note persistence, restart, and resume. The September 8 slice adds
  Rustls-backed, validator-aware progressive HTTP playback and a headed remote
  restart receipt. The September 9 slice adds a budgeted content-addressed cache
  and proves restart/playback with its HTTP server stopped. Turnstone hosts the
  compact dock and streams through its own fetch handle since September 22.
  Duck and a W3C export proved against the Web Annotation test suite closed
  Phase 5 on September 26. Phase 6 adds three-way identity, guarded desktop note
  seeking, private playback snapshots, configurable per-note fingerprints, and
  downloaded-copy derived positions. Turnstone adopted the coordinated graph
  and guarded note opens on September 29, with nine host regressions passing
  in its 612-test workspace gate. Real-recording validation, native embedded
  warning/audio acceptance, Turnstone's realignment worker, and its microphone
  and capture-behaviour settings remain open. The September 27 Mesquite migration
  centralizes the scenario lifecycle while keeping Redshank commands and async
  waits product-owned. A September 13 GUI evidence
  brief records current headed findings, reference screenshots, required
  states, and responsive wireframe done-conditions. It is separate from the
  Woodshed product graph.
- [2026-07-11_stage_set_tools_plan.md](2026-07-11_stage_set_tools_plan.md)
  — October 5 adds owner-qualified relational disclosure, a shared-compiler
  compatibility instrument, and fresh-process retention through the desktop
  session path; visible Woodshed recipe editing remains a follow-on.
  — **In progress; product authority.** P1-P3 remain partial. P4a/P4b landed;
  the scene canvas and single-Card expansion have receipts, while the full P4e
  catalog and P4f practice/sound join remain open. P5-P8 remain open. A repaired
  release baseline does not close the 1.0 practice proof. The September 30
  connected exploration/drilling direction specifies cross-catalog context,
  shared realization, ambient Mere actions, and integrated acceptance slices.
  The first chord/arpeggio connection passes automated discovery, host controls,
  and filesystem reopening checks. Native seed/reopen passes with six inspected
  captures. The editor layout correction also has a four-capture native receipt
  at recorded zoom. The bounded chord-to-scale slice adds formula containment,
  explicit audition/staging, progressive disclosure, and mixed-Set reopening,
  with 278 automated checks and three inspected native seed/reopen captures;
  instrument realization, progression recipes, keyboard, listening, and full
  articulation acceptance remain open.
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
  Rehearsal/Settings components. The September 8 selector integration uses
  retained host geometry, with 406 focused tests and five native passes;
  the September 27 Mesquite migration passes 223 product tests and both native
  Stage/nearby-candidate scenarios. Narrow-window controls now scroll into
  view; broader Stage layout acceptance remains open. The shared view tree
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

- [2026-09-29_redshank_persistence_diagnostics_receipt.md](2026-09-29_redshank_persistence_diagnostics_receipt.md)
  — **September 29 pilot and September 30 capture continuation published.** Bounded,
  redacted Apparatus observations follow real save dispatch, execution and
  reply handling; 63 desktop tests and native loss/default/failure controls.
  Presentation-paired read-only UI/persistence capture adds five source controls
  (68 desktop tests), strict Clippy, paired/default native captures and actual
  web Wasm compilation. Native pairing controls are recorded separately.
  The combined Mere `bd5912fb` / Genet `69a2383` graph repeats those gates with
  all 45 Redshank source blobs unchanged and a source-identity-only lock update.
  Compositor/physical visibility and human AT remain open.
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
- **Exploration and drilling are complementary**: lateral relationships across
  catalogs and deliberate Set rehearsal both help assemble material and act on
  it. Develop connected slices that carry discoveries into playable material
  and return qualified rehearsal observations to exploration. The ambient Mere
  makes relationships perceptible; explicit actions author the selected working Set.
- **Stage is a verb and Set is the spine**: catalogs supply material; Stage
  adds configured Cards to the selected ordered Set. Each working instance owns
  its material; shared views project that owner, and rehearsal remains bound to
  the instance that started it when another instance is inspected. Do not create
  parallel practice, song, or tool-owned material documents for the same Set.
- **Navigation preserves owner identity**: switching working Sets or configured
  catalog explorations restores that instance's content and context. It does
  not clone occurrences or transfer live rehearsal. Creation, duplication,
  saved-copy opening, and runner transfer are explicit product actions.
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
