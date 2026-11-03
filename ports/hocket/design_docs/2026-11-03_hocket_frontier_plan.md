# Hocket current frontier

Historical source: [Initial Plan](archive_docs/2026-11-03/2026-05-18_initial_plan.md).
Related historical sources: [host refactor](archive_docs/2026-11-03/2026-07-08_genet_host_refactor_plan.md) and [waveforms and meter ballistics](archive_docs/2026-11-03/2026-07-11_real_waveforms_meter_ballistics_plan.md).

Status (2026-11-03): **in progress; successor approved.**

## Product and authority
Hocket is a configurable loop-first composition tool organized around asynchronous turn-taking. Tracks contain layers: `Sum` plays unmuted layers together; `SelectOne { active }` picks a variation. Profile defaults are starting points rather than fixed limits. Session/model and history own musical facts; Firewheel projects them into runtime audio. Keep peer-local transport distinct from shared session state. The recorded collaboration direction is one capture owner per track per turn and deliberate reconciliation of branches; neither a transport nor a merge law is supplied by a signed envelope alone. [Initial plan:38–117; signed handoff plan.]

A full arrangement is a deliberate scope decision after the loop recorder, not an implicit promise to become a DAW. Plugin and plugin-GUI hosting remain outside v1. Data-defined packs configure supported Rust audio devices; source/asset custody and admissibility remain explicit. [Initial:89–100, 146–158.]

## Current bounded evidence and owners
The October 20, 2026 AP3 packet records native controls, project save/reopen, edited pack and asset snapshot, rendered media, MIDI/WAV parity and the stated bounded graph playback. Draft metadata is local to the project and outside recorder history; handoff carries rendered media and omits editable drafts. Editable pack handoff, richer rhythm scenes, typed routing and FunDSP vocabulary remain future work. Slow debug frames can lose input; physical latency, Windows/Linux and two-peer handoff remain unqualified. [Initial:3–25; `Code/testing/woodshed/validation/audio-packs-ap3_20261020/README.md`; root `../../../design_docs/2026-11-03_audio_ports_rehome_plan.md`.]

Shared recorder boundary and Woodshed adoption belong to the root audio ports rehome plan. Hocket's Firewheel plan owns device and physical audio acceptance; the signed handoff and UI plans own authenticated handoff, recipient exchange, transport, branch reconciliation and native scope. A bounded self-addressed native receipt does not establish two-peer delivery. [Host refactor:387–454; `2026-10-11_firewheel_adoption_plan.md`; `2026-07-10_signed-handoff-envelope_plan.md`; `2026-07-18_handoff-ui_plan.md`.]

## Open feature targets and done conditions
- **Performance and capture:** qualify physical input/output selection, restart/hot-plug, microphone permission, audible replay, count-in precision and round-trip calibration on named devices/platforms. Original FT3c specifies ten measurements with variance below 1 ms and click alignment within one sample after calibration; recheck feasibility and test method before asserting it. Confirm multi-layer/clock behavior by ear and preserve accepted schedules. [Initial:356–421; Firewheel F3c.]
- **Combinations and sections:** preserve the proposed named snapshot of active layer per track, next-bar selection and a visible distinction between the currently playing and queued slot. The historical done conditions include section save/load, sample-accurate changes without audible clicks/dropouts, and the stated ten-track/256-sample scenario; reconcile each with the current design and implementation before scheduling work. [Initial:498–526.]
- **Project continuity:** retained `.hock` project and native self-addressed acceptance are bounded. The original FT8 goal was to save and load the complete session, including captured audio, selections, sections and history, with autosave on every history commit. Cross-machine identical reopen and glitch-free autosave were also historical done conditions. Reconcile that trigger and scope with the current design and obtain separate evidence; local save alone does not establish them. [Initial:528–543; native-project plan.]
- **Two-peer handoff:** decide carrier and conflict reconciliation. Original FT9 seeks bidirectional LAN handoff, cross-peer recorded media, preservation of distinct simultaneous edits, and non-LAN handoff; qualify exact platforms and identities. [Initial:545–567; signed handoff plan.]
- **Arrangement, contingent on scope decision:** ordered named sections/bar counts, bounded editor and deterministic offline WAV; original gate is a 16-bar/four-section render, matching realtime output and expected duration. [Initial:569–586.]
- **Web:** original FT11 proposes backend abstraction, web audio, OPFS and shared geometry; recording and playback, combination scrubbing, native/web project round-trip, and per-browser latency remain the historical proof gates. A self-hosted demo was a recorded direction for a later hosting decision. Prior named APIs and threading/deployment assumptions are historical candidates, not a current dependency decision. [Initial:588–609.]
- **Distribution/mobile:** original FT12 proposes offline PWA, signed Windows/macOS installers and Android, with iOS deferred to initial Safari PWA; original gates are offline Chrome, installer verification on owner hardware and Android device execution. OS trust and current feed ownership belong to RELEASING; confirm target and current choices before implementation. [Initial:611–625.]

## Retained design possibilities
Historical extension ladder: gain envelopes, send/return, EQ, individual-track export, grid quantization, layer crossfade, MIDI capture, comping and eventual editable waveform are candidates rather than completed features. Historical implementation notes, CPU measurement, old 0.10 Firewheel workarounds and UI framework choices remain in the intact original; reconcile with current source before reviving them. The original calls for configurable defaults, not hard-coded ceilings. [Initial:146–181, 629–1714.]

## Closure carry-forward received 2026-11-03
FT9 retains Moothold wire-schema/transport coordination, two-peer exchange and signed per-turn work. The local self-addressed native project packet is narrower; it does not qualify network transfer, encrypted carrier or physical audio. A branch chooser must select an explicit checkout target; branch retention is not a merge policy. Device preference persistence, live settings editing/worker restart and hot-plug remain host work where applicable. Storing already-compressed WavPack ZIP entries without Deflate is an optional tidy, not a correctness condition. WAV remains the mix export and WavPack the implemented float-preserving media path; the older FLAC note is not an implementation mandate. Promotion of open/importable-format doctrine to PROJECT_DESCRIPTION remains maintainer-owned. [Initial closure carry-forward; signed-handoff and handoff-UI carry-forward 2026-11-03.]


## Display and recorder follow-up

- Offer raw-amplitude and visually normalized waveform display as host-local settings without altering stored media or mix gain.
- Expose or persist meter attack/release/hold timing if users need profiles.
- Add peak-file pyramids if project size outgrows inexpensive in-memory overviews.
- Promote the Hocket audio leaf only after Woodshed or another concrete Chisel waveform consumer exists.
- A zoomed editable waveform is a separate Path-B product feature.
[Waveforms plan:44–53.] These are deferred possibilities, not findings that a settings surface or second consumer already exists.

Host refactor's original unresolved choices at lines 233–241 (consumer-triggered shared audio leaf, coexisting migration bins, window decoration, and Path A/B rendering) must be checked against present implementation before re-opening; the completed migration does not by itself settle future leaf ownership. Physical device and runtime acceptance remain in existing Firewheel plan. Historical separate Deeler combination/settings spin-out at 257 is reconciled with the initial frontier rather than treated as a new plan.

Firewheel's 2026-11-03 carry-forward explicitly retains physical selection/restart, hot-plug, permissions, audible input/output, mixed-rate input/resampling and latency/calibration. Later bounded native project acceptance adds no manual audio demonstration. [Firewheel adoption, closure carry-forward.]
