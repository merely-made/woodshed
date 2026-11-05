# design_docs Index

Canonical first-reference document for project documentation. Read this
before any other doc in this directory.

Rust policy (2026-10-13; working version reconciled 2026-11-03): working toolchain 1.99.1 and shared MSRV 1.97.1, qualified separately. Canonical policy: `mere/design_docs/2026-08-12_family_composition_thesis_brief.md`, section "Shared Rust toolchain and minimum version".

Murm source adoption (2026-10-13) is tracked in
`murm/design_docs/archive_docs/2026-11-02/2026-10-13_murm_family_move_plan.md`. Active consumer roots
use the imported transport supplier and coherent Mere/Dramatis/p2panda
composition patches. Compile and software-test receipts are scoped to this
cutover; previous physical and headed receipts retain their original inputs.

Foundation source adoption (2026-10-19): canonical promoted-family revisions and consumer source validation are recorded in `mere/design_docs/mere_docs/implementation_strategy/2026-10-19_family_promotion_plan.md`. Existing application and domain authority remain with their owning components.

## Project Reference Docs

- [PROJECT_DESCRIPTION.md](PROJECT_DESCRIPTION.md) — Product goals,
  major features, scope. Maintainer-owned.
- [DOC_POLICY.md](DOC_POLICY.md) — Documentation governance.
- Port doc roots: [Hocket](../ports/hocket/design_docs/DOC_README.md) and
  [Ringdown](../ports/ringdown/design_docs/DOC_README.md) keep their own index
  and `DOC_POLICY.md`. Redshank's plans live in this index.

## Active Plans

- [Custody client receiving](2026-11-03_preservation_input_admission_plan.md#custody-client-receiving-2026-11-04) — Woodshed, Hocket and the maintained recipe fixture migrate to Ostiary and Castellan-client; owner-granted application declarations and independent receiving checks.

- [Contextual navigation in Woodshed](2026-10-23_contextual_navigation_plan.md)
  — Native Back/Forward and Up/Down navigation across the session Overview, working
  Set, and owner-qualified Card routes is qualified. Session-persistent history
  passed fresh two-process Windows runs on the portable lock through isolated
  unlocked Keepers: the exact owner/Card route and view reopened, Back restored
  distinct Overview cameras, and playback remained stopped. That run used the
  2026-10-24 portable pins, Mere `7632bbd32a` and Genet `2b5c01811e`;
  `Cargo.lock` now resolves Mere `c6290142` and Genet `565458ea`
  (2026-11-04), not requalified here. The Card 2 G2 tone
  admission fallback is covered by 174 passing Woodshed audio tests and a
  headed one-shot Card 2/full-Set run at the actual 44.1 kHz device rate.
  Physical audio quality, latency, and live-user-persona qualification remain
  open. Playback, edits, and audio capture remain
  separately owned.

- [Timed transcripts](2026-09-26_timed_transcripts_plan.md): bounded WebVTT parsing,
  explicit fetch/save and offline reopening, active cue display and seek are
  implemented and tested. Stale download and oversized-response checks pass;
  native fixture fetch/save, exact cue seeking and offline restart pass. Five
  subscribed public feeds advertise no timed transcripts; Decoder links written
  website transcripts. Headed real-episode acceptance and full text-track
  rendering remain open.

- [2026-08-12_persona_picker_plan.md](2026-08-12_persona_picker_plan.md)
  — **P1/P2 landed; P3 open.** Startup selection and a live Settings switch
  swap the sealed practice store without restart. Creating a persona inside
  Woodshed remains open.

- [2026-07-18_accessibility_semantic_surface.md](2026-07-18_accessibility_semantic_surface.md)
  — **In progress.** AccessKit, genet-probe, and agents consume one semantic
  Cambium surface. Tier 0 and Tier 2 landed 2026-07-18; the shared Cambium
  desktop host (`cambium-desktop`) now owns the AccessKit sync, not
  `woodshed-genet`. Tier 1 beyond the fretboard and Tier 3 keyboard access
  remain open.

- [2026-07-11_audio_material_analysis_plan.md](2026-07-11_audio_material_analysis_plan.md)
  — **Active research.** The model-neutral benchmark and scorer landed; the
  September 6 refresh specifies held-out evaluation, alignment, provenance,
  and ESP ownership. No transcription or reasoning model is selected.

- [2026-07-15_fretboard_marker_detail_plan.md](2026-07-15_fretboard_marker_detail_plan.md)
  — **Partly landed.** Click-to-pin essentials cards (Phase 1) and Play plus
  close-all (Phase 3) landed 2026-07-15. Hover peek has code but no recorded
  verification; the reusable detail popover (Phase 4) is open.

### Current successor plans (2026-11-03)

Each successor carries the open work of a whole archived plan; its historical
source keeps the original body, rulings and receipts.

- [Stage, Set, Tools and practice](2026-11-03_stage_set_tools_plan.md) — **In
  progress; product authority.** One Set owns Card occurrences; the session
  Overview and tools project it. Open: P1–P3 shell, settings and staging;
  connected-practice paths; P4 relationships and semantic zoom; P7
  Set-derived Looper, saved backing/PCM and physical capture; P8 polish;
  release acceptance and catalog expansion. A 2026-11-04 finding (Overview
  process-count test, 2 vs 3) is owned here. Historical source:
  [2026-07-11 plan](archive_docs/2026-11-03/2026-07-11_stage_set_tools_plan.md).
- [Instrument and fretboard](2026-11-03_instruments_and_fretboard_rendering_plan.md)
  — **In progress.** Spacing, extent, marker, fill and fretless-region
  settings, instrument fret counts and a filtered tuning picker; recheck
  against current source before assigning work. Course pairs and non-12-TET
  need separate modeling. Historical source:
  [2026-07-14 plan](archive_docs/2026-11-03/2026-07-14_instruments_and_fretboard_rendering_plan.md).
- [Musical projections](2026-11-03_musical_projections_plan.md) — **In
  progress.** Supports Stage P4e/P4f without authority over the Set. Circle,
  Tonnetz, selected-shape and anchored-motion slices are bounded; S1, S2 and S4
  are partial and S3 is planned. Historical source:
  [2026-09-04 plan](archive_docs/2026-11-03/2026-09-04_musical_projections_plan.md).
- [Audio foundation and packs](2026-11-03_audio_ports_rehome_plan.md) — **In
  progress.** Rehome landed; AP1/AP2 implemented and AP3 has bounded
  macOS/guitar evidence. Open: shared DSP and F4/F5 seams, desktop matrix,
  latency, broader instruments and pack restoration. Historical source:
  [2026-09-09 plan](archive_docs/2026-11-03/2026-09-09_audio_ports_rehome_plan.md).
- [Genet host and platform acceptance](2026-11-03_genet_host_cross_platform_plan.md)
  — **In progress.** Desktop migration and S5 parity cut are complete. Open:
  desktop clipping and scroll reachability with cross-platform runs; the
  browser host (input, storage, audio, MIDI, accessibility) and its
  deployment; platform archive acceptance. Mobile is downstream of web.
  Historical source: [2026-07-04 plan](archive_docs/2026-11-03/2026-07-04_genet_host_cross_platform_plan.md).
- [Smart instrument integration](2026-11-03_smart_instrument_plan.md) — **In
  progress.** W1/W2 hardware evidence is historical. W3 exposes proven
  Ringdown actions through a Woodshed surface with explicit device/session
  authority; which actions come first is an open review question. Historical
  source: [2026-08-27 plan](archive_docs/2026-11-03/2026-08-27_smart_instrument_plan.md).
- [Redshank listening and annotation](2026-11-03_listening_annotation_port_plan.md)
  — **In progress.** Redshank owns listening/annotation semantics and storage.
  Open: Phase 6 representation identity and drift (refuse unsafe
  realignment), real-episode and Turnstone host qualification, voice-capture
  custody, and native note readability after supplier repair. Historical
  source: [2026-09-01 plan](archive_docs/2026-11-03/2026-09-01_listening_annotation_port_plan.md).
- [Redshank GUI](2026-11-03_redshank_gui_implementation_plan.md) — **In
  progress.** The endorsed Redshank surface under the listening plan's
  authority. Recheck S2 panels and S3 feed projections against current code;
  finish design completions and the native export chooser; qualify keyboard,
  human accessibility and a headed Turnstone receipt. Narrow note-row layout
  is covered on Genet `01d3f528`. Historical source:
  [2026-09-13 plan](archive_docs/2026-11-03/2026-09-13_redshank_gui_implementation_plan.md).
- [Preservation and admission](2026-11-03_preservation_input_admission_plan.md)
  — **In progress.** D1 owner-specific stores are qualified on their cohort;
  custody client receiving landed 2026-11-04 (above). Open: recheck
  preservation, recovery and custody-refusal seams through production hooks.
- [Source authority remediation](2026-11-03_source_authority_audit_plan.md) —
  **In progress.** The audit is complete; B11–B14, C1–C5, Q2–Q5 and K1–K3
  remediation remain, each rechecked against current source.
- [Appearance and native title bars](2026-11-03_tabard_adoption_plan.md) —
  **In progress.** Tabard owns definitions and workshop; Woodshed owns
  selection and title-bar composition. Windows/Linux presentation, screen
  reader and contrast review remain.

### Historical qualification entries

Entries below that link to `archive_docs/` refer to intact historical source sections and retain their original qualification.

- [Canonical arrangement practice](archive_docs/2026-11-03/2026-07-11_stage_set_tools_plan.md#canonical-music-integration-and-arrangement-practice-2026-10-19) — Qualified from real Git suppliers: 732 Woodshed cases, 13 supplier cases, two native processes and ten captures. MusicXML drives sample-clock practice, next-bar edits, shared 1–400 BPM tempo, Set repeat and contextual fretboard views; exact XML reopens stopped. [Receipt](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/native-music-practice_20261019/README.md).

- [Native MusicXML arrangement editing](archive_docs/2026-11-03/2026-07-11_stage_set_tools_plan.md#native-notation-led-music-editing-2026-10-18) — Bounded native Chart/Tab/Score and linked fretboard, click/add and drag/move, explicit picking previews, Set save/reopen and MusicXML export. Qualified on the local supplier cohort: 60 focused cases, two native processes, 12 reviewed captures, seven schema/reader/semantic export checks and exact saved-source reopening. Canonical integration and practice are qualified in the entry above. [Receipt](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/native-musicxml_20261018/README.md).

- [Independent progression document blocks](archive_docs/2026-11-03/2026-07-11_stage_set_tools_plan.md#shared-scene-prototype-review-2026-10-17) — Current-source second-consumer proof for the shared Djot scene-document codec: real Card/rest/voicing facts round-trip without changing a Set, and Graphshell receives the actual export. Local supplier cohort only; native Woodshed block authoring and canonical dependency integration remain separate. [Receipt](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/document-blocks_20261017/receipt.json).

- [Shared data-defined audio packs](archive_docs/2026-11-03/2026-09-09_audio_ports_rehome_plan.md#shared-data-defined-audio-packs-2026-10-19)
  — **AP1/AP2 implemented; AP3 bounded macOS/guitar acceptance (2026-10-20).** Shared
  `audio-pattern`/`audio-packs`, midly export, original drum kit, common bundled/
  directory admission and exact-source snapshots work through CLI and the
  production runtime. [AP1 evidence](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/audio-packs-ap1_20261019/README.md)
  covers AP1 file/reopen/render and Wasm/MSRV checks.
  [AP2 evidence](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/audio-packs-ap2_20261020/README.md) records locked
  product compilation and retained edited-selection file work. AP2 adds shared kit controls,
  retained edited selections and existing runtime/project paths.
  [AP3 evidence](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/audio-packs-ap3_20261020/README.md) adds actual
  native controls/save/reopen without installed kits, exact MIDI/WAV parity,
  701 selected tests and owner-observed custom guitar beat playback through a
  power cycle. The target
  desktop matrix, latency and broader instrument qualification remain open. Mere composition and FunDSP patch directions stay
  in the plan; [AP0](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/audio-packs-ap0_20261019/README.md) retains the
  earlier supplier assessment.
  [Adapter provenance/terms assessment](../ports/ringdown/design_docs/2026-09-02_client_surface_plan.md#provenance-terms-and-standards-audit-2026-10-20)
  records standards-based compatibility and the implemented default MIDI/Beats
  build boundary with opt-in private diagnostics,
  unresolved publication evidence and a local unsent vendor outreach draft. The
  [split acceptance](../ports/ringdown/design_docs/2026-09-02_client_surface_plan.md#split-acceptance-2026-10-20)
  adds bounded real-CLI/release tests, native virtual MIDI capture, default
  artifact exclusion and a read-only live guitar status check. The
  [shared BLE-MIDI transport](../ports/ringdown/design_docs/2026-09-02_client_surface_plan.md#ble-midi-peripheral-transport-2026-10-20)
  reuses MidiOut and the desktop output path, with three-platform compiler
  evidence and one owner-observed Mac guitar trigger; native Windows/Linux
  and timing qualification remain open. [Desktop/reconnect acceptance](../ports/ringdown/design_docs/2026-09-02_client_surface_plan.md#desktop-path-and-reconnect-qualification-2026-10-20)
  adds actual picker/clock controls, exact receiver parity, explicit hotplug
  refusal/rearm, and a zero-message guitar lifecycle check (36 focused cases,
  fourteen native captures).
  [Desktop diagnostics on Windows/Linux](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/ports/ringdown/validation/midi-diagnostics-cross-platform_20261021/README.md)
  adds complete native control/reopen/export and pending-close cohorts on
  Windows and Linux Xwayland, full Status/JSON parity, and the same 43 desktop
  plus one isolated journal case passing on each host. Linux independently
  receives six ALSA messages; Windows exercises no-output refusal diagnostics.
  Native Wayland and Windows MIDI/BLE delivery remain open; final Windows
  preservation metadata collection is pending in the packet.

- [A1–A5 and Q1 current-source recheck](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/first-wave-recheck_20261020/README.md) — October 20 parallel recheck from fetched `9f80bb5e`: existing first-wave fixes retained, selected-persona admission and strict audio diagnostic result published through `cb4ed31` on October 21. 1,020 distinct CPU passes, nine existing ignores and five child invocations counted separately; 50 focused publication reruns and fresh remote source/evidence verification also pass. [Publication](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/first-wave-recheck_20261020/publication.json). That historical first-wave receipt retained its P5a owner reservation; P11 retires the completed reservation. No new native/device qualification from that receipt. Current phase and ownership are in the [source authority plan](archive_docs/2026-11-03/2026-10-16_source_authority_audit_plan.md#p10-recheck-a1a5-and-q1-against-current-source-2026-10-20).

- [Shared notation readings in Graphshell](archive_docs/2026-11-03/2026-07-11_stage_set_tools_plan.md#adr-shared-notation-readings-in-graphshell-2026-10-19) — Accepted pure-model/shared-leaf boundary; canonical main source integration, explicit MusicXML creation, and selectable Chart/Tab/Score. Qualification and publication limits are recorded with the implementation.

- [Source authority audit and remediation](archive_docs/2026-11-03/2026-10-16_source_authority_audit_plan.md) — Frozen collective read complete: 937 files / 231,190 lines, plus separate 33-file P5a overlay. All 93 raw finding entries reconciled against later work with qualification limits; remediation remains open. [Audit record](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/full-source-audit_20261016/README.md). Coordination closure published through `8204688`. H3/H5 and Q1 now have a separate CPU-qualified [implementation receipt](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/admission-evidence_20261017/README.md): 114 distinct passes, published through `6e6498a`; native custody/reopen remains separate. B1–B3 have a separate published [runtime receipt](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/runtime-authority_20261018/README.md): 859 parent/library passes plus five child invocations, published through `b1c9dfd`; native qualification remains separate. B4 finite PCM, onset accounting and calibration support are CPU qualified at `5dd01ae`, published through `8da953b`: 308 distinct passes across primitives, Woodshed audio and Hocket engine; [receipt](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/pcm-timing_20261018/README.md). B5 shared Recorder SelectOne admission is CPU qualified at `b804d901`, published through `4fab895`: 176 distinct passes, two existing ignores and three reproduced baseline bypass failures; [receipt](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/recorder-select-one_20261019/README.md). Requested B6 tempo replay and wrapper diagnostics are CPU qualified at `7b161a9`, published through `fd9fb82`: 171 distinct passes and two existing ignores; [receipt](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/recorder-runtime_20261019/README.md). The original B6 receipt records its shared-view unit paint API blockage on that tested cohort. Historical B6 MIDI H6–H9 are scoped CPU qualified at `480c0d6`, published through `20bcc1c`: 42 distinct passes; [receipt](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/midi-admission_20261019/README.md). Supplier intake and native validation remain separate. Rechecked current main `3ce93675` on October 23: completed P5a reservation retired, D1/shared Recorder closure and idle-predicate replacement reconciled; [P11](archive_docs/2026-11-03/2026-10-16_source_authority_audit_plan.md#p11-recheck-the-remaining-frontier-and-repair-playback-admissioncontrols-2026-10-23) qualifies B8 Redshank cache refusal, effective controls/recovery and exact-end completion at `046b0bf4`: 167 distinct CPU passes, eight existing ignores, seven new regressions and a locked desktop build. The [B8 receipt](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/audit-playback_20261023/README.md) retains three reproduced baseline failures, corrected ledger dispositions and scope limits; no new native/device claim. [P12](archive_docs/2026-11-03/2026-10-16_source_authority_audit_plan.md#p12-capture-preservation-export-identity-and-observed-audio-availability-2026-10-25) refreshes main to `91b2dd5b` and qualifies the approved R05/R09 capture and R08 export identity at `951c7822`, plus V03 observed availability at `770f5443`: 1,153 distinct CPU passes, 16 existing ignores, five child invocations separately, 24 new executed tests, two desktop builds and a Wasm check. Two native processes/eight reviewed captures qualify absent/offline availability presentation only. The [P12 receipt](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/capture-export-availability_20261025/README.md) preserves failures, exact source/process bindings and native/device limits. Hocket staged Apply/feed and handoff is the next product slice to recheck; other-computer synchronization remains blocked by SSH access.

- [Related graph label placement](archive_docs/2026-11-03/2026-07-11_stage_set_tools_plan.md#p8-adaptive-polish-and-release-acceptance) — Locally qualified shared Cambium label collision repair: 640 affected passes, desktop build and 16 native captures; published through `8204688` (Mere `c8d316b60`). Narrow toolbar/125% graph clipping remains P8 work. [Receipt](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/related-labels_20261017/README.md).

- [Production Mere scroll adoption](archive_docs/2026-11-03/2026-07-11_stage_set_tools_plan.md#p7-rebuild-song-as-looper-over-set) — Main-based clipping/observation supplier, coherent renderer closure and retained selector migration; software gates, 31 native captures, exact sealed reopen and three-way WAV parity pass on committed source. Published through Mere `1eba417eb` and Woodshed `b01c922`. [Receipt](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/mere-scroll-adoption_20261017/README.md).

- [CoreAudio input admission](archive_docs/2026-11-03/2026-07-11_stage_set_tools_plan.md#p7-rebuild-song-as-looper-over-set) — Explicit macOS microphone consent before HAL setup; consent waits preserve pending capture authority and have a separate hardware deadline. 700 affected cases plus five child invocations pass; committed native 15.066-second Card 2 recording, sealed fresh reopen and three-way WAV parity pass; published at `7b68b4b`. [Receipt](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/coreaudio-input_20261017/README.md).

- [Looper follow-ups](archive_docs/2026-11-03/2026-07-11_stage_set_tools_plan.md#p7-rebuild-song-as-looper-over-set) — Published compact/copy/waveform/recipe and selected-Card fixes. The historical native input-opening stall is resolved and qualified in the CoreAudio receipt above. The production Mere scroll adoption above is qualified and published. [Receipt](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/looper-followups_20261017/README.md).

- [P7b Seconds-held Looper material](archive_docs/2026-11-03/2026-07-11_stage_set_tools_plan.md#p7b-seconds-held-looper-material-2026-10-17) — Qualified October 17: absolute Seconds backing, retained captured clocks and exact sealed PCM reopen; 1,015 affected cases plus five child invocations, desktop/shared-leaf checks, eight reviewed native captures and byte-identical worker/reference WAV. [Receipt](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/stage-p7b_20261017/README.md).

- [P7a mixed Bars Looper backing](archive_docs/2026-11-03/2026-07-11_stage_set_tools_plan.md#p7-rebuild-song-as-looper-over-set) — Qualified October 17: ordered scale/path/arpeggio backing over the explicit Bars clock; 925 cases plus five child invocations, desktop build, a separate sealed-WAV probe and eight reviewed native captures. Exact Set/take reopen and byte-identical 160,800-frame worker/reference WAVs. [Receipt](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/stage-p7a_20261017/README.md). Full P7 remains open.

- [P6 guided Rehearsal runner](archive_docs/2026-11-03/2026-07-11_stage_set_tools_plan.md#p6-finish-rehearsal-as-the-guided-set-runner) — Qualified October 17: previous/current/next context, output-clock Card progress, focused Card looping, paused navigation, keyboard controls and edit recovery. Final merged source with Mere `4c4ea11` passes 911 cases plus five child invocations, desktop build and eight reviewed native captures; exact sealed Set, owner and 22 history rows reopen stopped. [Merged receipt](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/stage-p6-merged_20261017/README.md); [original receipt](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/stage-p6_20261017/README.md).

- [Preservation and input admission](archive_docs/2026-11-03/2026-10-16_preservation_input_admission_plan.md) — First audit-remediation pass: outgoing-work preservation, checked history/Set/project input, and nonwriting failed recovery. Integrated onto P5 October 17; 1,355 passing cases plus five child invocations and three desktop builds. Bounded Woodshed persona/close/quarantine native qualification adds 12 reviewed captures; The historical Hocket/Redshank integration receipt is CPU/build only. [Native receipt](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/preservation-input_20261017/native/README.md). [Current integration receipt](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/preservation-input_20261017/integration.json); [isolated implementation receipt](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/preservation-input_20261016/receipt.json). Current-persona selection is published at `01c4b3af` with a separate [D2 receipt](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/persona-selection-noop_20261021/README.md): 28 passing persona cases and an observed pre-change failure; native/custody acceptance remains separate. D3 explicit recovery exit/copy has a separate [receipt](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/redshank-recovery-copy_20261021/README.md): 221 focused CPU passes, one existing ignore, desktop/Wasm checks and eleven completed audit chats archived. D3 is published through `f4904a27`. The subsequent [macOS native recovery receipt](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/redshank-recovery-native_20261021/README.md) qualifies chooser cancellation, failed/successful local-media copies and independent reopening on committed `94583610`, with ten accepted captures and 92 desktop passes/one existing ignore, published through `428dd7ad`. Physical audio and other platforms remain separate. [D1 persona-store cutover](archive_docs/2026-11-03/2026-10-16_preservation_input_admission_plan.md#p7-d1-persona-store-cutover) is qualified at `8bf59b31`: 678 distinct CPU parent/integration passes, five separate nested invocations, five successful native product processes and 12 independently reviewed captures. The [D1 receipt](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/persona-store_20261022/README.md) records exact owner-specific Cards/settings/PCM readback, fresh A/B reopening, corrupt incoming refusal/Retry/Cancel and nonwriting locked custody. Woodshed is unreleased, so the new cryptographic-owner roots replace development singleton stores without a compatibility/migration subsystem. P5a retains its existing owner.

- [P5 shared clock and visual articulation](archive_docs/2026-11-03/2026-07-11_stage_set_tools_plan.md#p5-unify-the-clock-and-visual-articulation) — Qualified October 17: one processor clock for Card dwell, click, articulation and Fretboard event projection; preserved pause/resume and tempo phase, absolute Seconds, Manual Next and accepted-start history. 881 cases plus five child invocations, desktop build and five reviewed native Rehearsal/reopen captures with exact sealed Cards, owner and six history rows. [Receipt](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/stage-p5_20261017/receipt.json).

- [Catalog/discovery audition admission history](archive_docs/2026-11-03/2026-07-11_stage_set_tools_plan.md#catalogdiscovery-audition-admission-history-2026-10-16) — Qualified: frozen accepted-only observations, exact source/session binding and refusal/retry; 676 affected cases plus five child invocations, desktop build and four independently reviewed native seed/reopen captures with exact sealed Card/history preservation. [Receipt](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/catalog-audition_20261016/receipt.json).

- [Session Overview selection folds](archive_docs/2026-11-03/2026-07-11_stage_set_tools_plan.md#session-mere-selection-folds-2026-10-16) — Qualified on macOS against current native-arrangement main: member folds, view-owned v3 retention and undo/redo; 671 cases plus five child invocations, two native processes, eight reviewed captures and exact sealed fold reopening. [Receipt](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/mere-folds_20261016/README.md).

- [Shared span-edit intents with metre](archive_docs/2026-11-03/2026-07-11_stage_set_tools_plan.md#shared-span-intents-with-authored-metre-2026-10-17) — Adopted on shared Mere `4c4ea11d8`; nine locked arrangement-editor route/model cases pass with forged-intent and stale gesture controls. Domain validation and explicit Save remain in Woodshed. [Receipt](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/shared-span-intents_20261017/receipt.json).

- [Native arrangement editor and panel](archive_docs/2026-11-03/2026-07-11_stage_set_tools_plan.md#native-arrangement-editor-and-panel-2026-10-16) — Qualified on macOS: per-Set drafts, role/Card/rest editing and linked saved voicings over the shared scene surface; 393 parent-suite cases plus five child invocations, desktop build, two sealed native processes and 12 reviewed wide/narrow captures with exact Set/document reopening. [Receipt](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/native-arrangement_20261016/receipt.json). The shared-height spacing follow-up passes 214 view and two host cases, with three reviewed native captures. [Spacing receipt](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/native-arrangement-spacing_20261016/receipt.json). Direct span manipulation is also qualified: captured move/resize, keyboard controls and cancellation; 14 focused cases, two native processes, five reviewed captures and exact sealed Set/document reopening. [Span editing receipt](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/native-arrangement-spans_20261016/README.md).

- [Saved progression arrangements](archive_docs/2026-11-03/2026-07-11_stage_set_tools_plan.md#saved-progression-arrangements-and-portable-export-2026-10-16) — Qualified: Set-owned musical alignment, identity-safe snapshot copies and explicit session exchange export to Graphshell; 656 suite cases, feature-disabled retention, fresh CLI processes and browser receiving.

- [Stage P5a Card timing and admission](archive_docs/2026-11-03/2026-07-11_stage_set_tools_plan.md#p5a-admit-one-resolved-card-timing-contract-2026-10-16) — Bounded slice qualified: shared fallible dwell/ordered-preview timing, literal authored BPM, visible refusal and retry; 804 cases plus five child invocations, desktop build and eight reviewed native seed/reopen captures with exact sealed Card/resolution preservation. Full P5 is qualified above. [Receipt](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/stage-p5a_20261016/receipt.json).

- [Stage P4f continuation](archive_docs/2026-11-03/2026-07-11_stage_set_tools_plan.md#p4f-join-graph-understanding-to-sound-and-practice) — Bounded practice-flow gate qualified: honest save/close outcomes, visible frontier → voicing → exact directed audition → numbered Rehearsal → sealed fresh reopen; 789 cases plus five child invocations, desktop build and 14 reviewed captures. [Receipt](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/stage-p4f_20261016/receipt.json).

- [Hocket native project/handoff acceptance](../ports/hocket/design_docs/archive_docs/2026-11-03/2026-10-16_native_project_handoff_plan.md) — Two native processes/seven reviewed captures qualify Save/Open/WAV export, self-addressed signed handoff review/accept/adopt and exact project/media reopening; physical audio and two-peer transport remain separate.


- [2026-10-08_tabard_adoption_plan.md](archive_docs/2026-11-03/2026-10-08_tabard_adoption_plan.md) — Shared appearance authoring and native title-bar adoption in Woodshed; October 12 root font roles are qualified on the agreed Mere `8568f42` family with 556 combined CPU cases, three native font processes and 19 reviewed captures, preserving application requests, authored precedence, domain geometry and exact WAV bytes. Separate normal-output capture migration/reopen passes on the same build. Earlier qualification preserves named themes and persona settings; the October 10 coherent stack and repaired renderer pass 444 CPU tests and four native lanes with 20 reviewed captures. The October 11 Classic 0.11 family is integrated across the three independent UI workspaces: 1,454 active tests and eight native runs with 38 reviewed captures; Woodshed Set, preview/rehearsal and appearance reopening pass, and the later Hocket/Redshank appearance reopen follow-through passes with four additional reviewed captures.
  The October 19 repaired foundation supplier intake is recorded in [the stack-family validation receipt](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/stack-family_20261011/README.md#foundation-supplier-intake--2026-10-19), including locked graph identities and focused check results.
  The [October 22 pool receiving record](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/validation/stack-family_20261011/pool-cohort-20261022.json) covers all four roots and the return of the synthesized-focus guard to Mere, retiring the temporary host snapshot.

## Current Supporting Design

- [2026-10-19_fundsp_firewheel_brief.md](2026-10-19_fundsp_firewheel_brief.md)
  — Research brief: no upstream fundsp/Firewheel integration exists (one
  unlicensed bevy_seedling-author demo); proposes an own adapter crate with
  block processing, event-split params and a generator-safe silence policy,
  gated by zero-alloc, block-size invariance and live/offline parity, with
  chord audition as first consumer. Not scheduled.
- [2026-10-14_dsp_graphshell_hocket_survey_brief.md](2026-10-14_dsp_graphshell_hocket_survey_brief.md)
  — Verified survey of shared frame-driven DSP between Hocket and Woodshed
  (divergent capture schedules and bar-length rules, Hocket guard gaps), what
  "Graphshell in Woodshed" can mean against the custody-only dependency and
  the second-owner rule, and Hocket handoff over Mere. Records Mark's six
  rulings (Hocket bar-length rule canonical, Woodshed-native graph first,
  repin forward to Mere main, per-turn signed Hocket operations, Woodshed
  practice data to a Woodshed mere in the reservoir, Graphshell lift
  coordinated with the Knot lane) and three follow-ups started 2026-10-14.
  The practice-data ruling was reopened on 2026-11-04 and re-ruled by Mark on
  2026-11-05: the whole domain moves into a Woodshed mere whose lifetimes the
  reservoir store owns (Mere D19), replicated to the persona's devices by
  default. Nothing is scheduled; the move needs its own plan.
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

- [2026-09-29_redshank_persistence_diagnostics_receipt.md](https://github.com/merely-made/woodshed/blob/445b7deca620f598b9ff3141d65c5549ba544d22/design_docs/2026-09-29_redshank_persistence_diagnostics_receipt.md)
  — **September 29 pilot and September 30 capture continuation published.** Bounded,
  redacted Apparatus observations follow real save dispatch, execution and
  reply handling; 63 desktop tests and native loss/default/failure controls.
  Presentation-paired read-only UI/persistence capture adds five source controls
  (68 desktop tests), strict Clippy, paired/default native captures and actual
  web Wasm compilation. Native pairing controls are recorded separately.
  The combined Mere `bd5912fb` / Genet `69a2383` graph repeats those gates with
  all 45 Redshank source blobs unchanged and a source-identity-only lock update.
  Compositor/physical visibility and human AT remain open.
- [Source splits, H6 and H8](2026-11-03_source_splits_plan.md) — Thirty-five H6 and thirty-two additional H8 root-workspace splits landed; 37 exceptions remain. Mere owns the aggregates; independent ports and existing product acceptance keep their own qualification.

## Historical and Superseded Documents

These files remain at their established paths for receipt and link stability.
They are not current product authority.

- [2026-05-18_xilem_popup_view_proposal.md](2026-05-18_xilem_popup_view_proposal.md)
  — Retained upstream proposal; not active Woodshed work.
- [2026-05-20_theme_system_design.md](2026-05-20_theme_system_design.md)
  — Seed-derived palettes and theme management; initial implementation shipped
  May 2026. Tinct and Tabard now own the work; see the
  [Tabard adoption plan](2026-11-03_tabard_adoption_plan.md).

## Archive

### Archived checkpoint: 2026-11-03

Completed bounded work and superseded prototypes are preserved whole.
Open product and qualification work retains its current owner; archival
adds no native, device or cross-platform acceptance.

- [Mere take acceptance](archive_docs/2026-11-03/2026-10-16_mere_take_acceptance_plan.md)
- [Shared capture accumulator](archive_docs/2026-11-03/2026-10-16_shared_capture_accumulator_plan.md)
- [Arrangement authoring observations](archive_docs/2026-11-03/2026-10-19_arrangement_observation_plan.md)
- [Relationship passage selection construction](archive_docs/2026-11-03/2026-10-19_passage_selection_plan.md)
- [Relationship source return](archive_docs/2026-11-03/2026-10-19_relationship_source_return_plan.md)
- [Personae-Sealed Practice Session](archive_docs/2026-11-03/2026-07-08_personae_sealed_session.md)
- [Woodshed Settings Persistence Split](archive_docs/2026-11-03/2026-08-06_settings_persistence_split_plan.md)
- [Woodshed onto the shared Cambium desktop host](archive_docs/2026-11-03/2026-08-09_cambium_desktop_host_migration.md)
- [Initial Plan](archive_docs/2026-11-03/2026-04-30_initial_plan.md)
- [Polyphonic Pitch Detection — Research Spike](archive_docs/2026-11-03/2026-05-15_polyphonic_pitch_spike.md)
- [Song Mode — Integration Design](archive_docs/2026-11-03/2026-05-16_song_mode_integration.md)
- [Song Timeline — Layered, Bar-Quantized View + Sampler](archive_docs/2026-11-03/2026-05-19_song_timeline_layers_plan.md)
- [Xilem Migration Plan](archive_docs/2026-11-03/2026-05-16_xilem_migration_plan.md)
- [Carried xilem fork patches](archive_docs/2026-11-03/xilem_fork_patches.md)
- [Fretboard-as-canvas, tabs as lenses](archive_docs/2026-11-03/2026-05-21_fretboard_canvas_lenses_plan.md)
- [Composable instrument surface (Phase 3 → 1.0)](archive_docs/2026-11-03/2026-05-21_composable_instrument_surface_plan.md)
- [Rehearsal redesign — from prototype to designed UI](archive_docs/2026-11-03/2026-05-22_rehearsal_redesign_plan.md)
- [Web profile — scope for a browser demo](archive_docs/2026-11-03/2026-06-14_web_profile_plan.md)
- [Receipt relocation and maintained validation paths](archive_docs/2026-11-03/2026-11-03_receipt_relocation_plan.md) — landed locally 2026-11-03; frozen evidence moved to `Code/testing/woodshed/`, maintained tools and fixtures repaired.
- [Independent custody client receiver](archive_docs/2026-11-03/2026-10-29_custody_projection_receiver.md) — `graphshell-endpoint/custody-client` receiving, 2026-10-29; superseded 2026-11-04 by custody client receiving in the preservation plan.
- [Codex handoff](archive_docs/2026-11-03/2026-10-15_woodshed_codex_handoff.md) — Woodshed, Hocket, Redshank and Firewheel after the Graphshell lift, with the binding rulings and four ordered tasks; all four done, archived 2026-11-04. Ruling 6 (custody to a Woodshed mere in the reservoir) was re-ruled 2026-11-05: the whole domain moves into a Woodshed mere.

- `archive_docs/` — retired plans and superseded notes.
- `archive_docs/2026-05-18/2026-05-17_woodshed_daw_plan.md` —
  Original "sibling DAW project under the Woodshed umbrella" plan.
  Superseded same-week: the maintainer chose a separate sibling repo
  (`repos/strophe/`), and the project scope pivoted from "general
  DAW" to a Deeler-inspired collaborative loop recorder. See
  `repos/strophe/design_docs/` for the live plan.

## Working Principles for AI Assistants

- **Preserve fixture literal bytes**: extracting an inline module may change code indentation while raw/multiline JSON stays byte-identical. Compare literal tokens against the original source; the rehearsal migration fixture demonstrates why successful decoding alone is insufficient. Pair body comparisons with signature, type-layout and facade-export review when moving public items.

- **Bind execution to the checkout**: baseline/control checkouts need separate build artifacts, or explicit local-package freshness verification before reusing a build directory. Stable source hashes alone do not prove that Cargo executed the intended binary; check compilation paths and the expected test inventory. Keep stale-artifact probes out of qualification.

- **Unreleased product (maintainer ruling, 2026-10-22)**: Woodshed has no deployed user data or released-format compatibility obligation. Prefer clean contracts and update development fixtures together; do not add migration wizards, legacy format retention or compatibility layers without a concrete current need. This does not relax correctness of new saves, ownership or failure reporting.

- **Prototype through source and canonical plans**: publish directly to `main`
  after refreshing remote state and preserving concurrent work. PRs are used
  only when requested; GitHub workflows are not part of this execution path.
  Use focused local checks to answer concrete questions about a change.
  Broader conformance and automation follow behavior deliberately stabilized
  for evaluation, rather than every interim prototype commit.

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
  the repository config path; the local clean-checkout metadata preflight is the
  gate that catches a lock accidentally written under local path patches.
- **Exploration and drilling are complementary**: lateral relationships across
  catalogs and deliberate Set rehearsal both help assemble material and act on
  it. Develop connected slices that carry discoveries into playable material
  and return qualified rehearsal observations to exploration. The session
  Overview makes relationships perceptible; explicit actions author the selected
  working Set.
- **Stage is a verb and Set is the spine**: catalogs supply material; Stage
  adds configured Cards to the selected ordered Set. Each working instance owns
  its material; shared views project that owner, and rehearsal remains bound to
  the instance that started it when another instance is inspected. Do not create
  parallel practice, song, or tool-owned material documents for the same Set.
- **Retained readings preserve captured evidence**: rendering or selecting a
  relationship reading does not recompute it or edit a Set. Explicit rebind
  captures new facts; exact source actions validate owner, Card and full ordered
  instructions. Invalid optional readings survive session capture without
  preventing access to valid Sets.
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
