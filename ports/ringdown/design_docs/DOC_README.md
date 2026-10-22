# Ringdown — documentation index

`DOC_README.md` is the sole canonical index for this repo (DOC_POLICY §6). Read
this first, then `DOC_POLICY.md`, before starting work.

## What ringdown is

An independent cross-platform desktop controller for the HyVibe smart guitar,
targeting live performance and scripting through reusable instrument actions. Existing code
speaks the instrument's JSON-RPC-over-Bluetooth app protocol; software MIDI
pedal emulation and the new Beats rhythms are assessed together in the client
surface plan. The protocol was
reverse-engineered for interoperability from the vendor's Android app; ringdown
reimplements it independently from the recovered facts and interfaces. Woodshed
is the first consumer. Structural template is retinue (sans-io core, thin I/O
shell, receipt-driven validation).

## AI-assistant working principles

- **Prove the instrument.** A protocol fact read from the decompiled app is a
  hypothesis, not a fact, until it is confirmed against the physical guitar in
  the same run as a positive control (a `GetStatus` that returns real values).
  Mark the confidence of every Finding: static-read, fixture-verified, or
  hardware-verified. This is the local addendum's provenance rule; honor it.
- **Expression boundary.** No vendor code, decompiled sources, or firmware
  binaries in this repo. No vendor trademark in a crate name. Findings record
  facts and interfaces and paraphrase behaviour, citing class names for
  provenance only. Say "reverse-engineered", never "clean-room" — the personnel
  split that word denotes is not ours, and DOC_POLICY's addendum explains why
  the distinction is worth keeping straight.
- **Separate conformance, provenance and permission.** A standard transport
  does not standardize a vendor application profile. Missing terms strings do
  not prove no agreement applies; hardware success does not authenticate the
  research artifact. Keep release decisions tied to the actual implementation,
  source/terms record and supported capabilities. The current bounded assessment
  and unsent outreach live in the client plan.
- **Cross-platform product.** Windows, Linux and macOS are first-implementation
  targets. Shared Rust actions, sessions and views sit above thin platform I/O
  adapters. A platform probe establishes one adapter's evidence, not a native
  product direction. Use the existing Cambium host and family lifetime contracts;
  background residency belongs in the Djinn assessment, not a new app daemon.
- **Sans-io first.** Protocol logic is pure functions over bytes, replayable
  against fixtures, `no_std`-capable — I/O lives in a separate shell. This is
  what keeps the future-firmware option open without committing to it, and it
  is the retinue discipline the repo is modelled on.
- **Reuse shared audio authoring (2026-10-19).** Data-defined packs and assets
  share one root-owned contract with Hocket and Woodshed. Reuse the existing
  sequencer and assess Rust suppliers before building; Ringdown owns guitar
  MIDI compatibility and delivery. Export needs no audio engine. Pack support
  on a computer does not establish the guitar's custom-pattern capability.
- **Qualify each performance path.** App RPC, pedal MIDI and Beats support
  need evidence tied to the actual instrument, processor versions and platform.
  Keep untested capabilities and unknown outcomes explicit; no stale live
  trigger is replayed after reconnect. A positive status reply does not prove
  another method or a musical action.
- **Nothing irreversible without sign-off.** Publishing to crates.io, pushing,
  or writing to the guitar's persistent config are Action-tier: they enter
  deliberately, with Mark's go, never as drift from exploration.
- **Follow the workspace method.** New objectives (notably alternative
  firmware) get their own Assess pass and plan before Assemble/Action, rather
  than being folded into an open plan.

## Active documents

| Doc | What's there |
|---|---|
| [`DOC_POLICY.md`](DOC_POLICY.md) | Documentation governance (canonical core + ringdown addendum). |
| [`2026-08-27_ringdown_founding.md`](2026-08-27_ringdown_founding.md) | Product direction and protocol/hardware history; current Phase 3 targets desktop performance control and scripting. |
| [`2026-09-01_effects_handoff.md`](2026-09-01_effects_handoff.md) | Historical H27–H38 session and experiment discipline; bank-creation limits were superseded by H47. |
| [`2026-09-02_client_surface_plan.md`](2026-09-02_client_surface_plan.md) | Current cross-platform performance plan: stack/ownership review, joint software stomp-box/Beats assessment, MIDI versus RPC paths, P0–P4 device/CLI/desktop/cue/scripting targets; independent default MIDI/Beats build with opt-in private RPC/PacketLogger, bounded runtime/release/MIDI-loopback and read-only guitar status acceptance; shared BLE-MIDI transport with Mac guitar trigger and three-platform compiler evidence; bounded actual desktop/endpoint-hotplug/guitar reconnect acceptance with 36 focused cases and 14 native captures; shared configurable Effect Next/Previous controls and same-owner desktop JSONL scripting with 57 focused cases, actual macOS save/reopen/independent partial-send capture and owner-confirmed guitar actions/reconnect; native Windows/Linux and timing remain open; bounded provenance/terms and standards assessment, A1–A7 evidence frontier and an unsent outreach draft; retains A–D typed/profile foundation. |
| [Shared data-defined audio packs](../../../design_docs/2026-09-09_audio_ports_rehome_plan.md#shared-data-defined-audio-packs-2026-10-19) | AP2 offline pack export plus AP3 approved custom-file trial: exact copy and unchanged factory hashes; owner confirms Beats discovery and repeated playback, including after a power cycle. The five-bar 4/4 intro/loop and stop/start rule is confirmed; broader lengths/meters and remote Beats control stay in P3. Shared controls/project acceptance and Mere/FunDSP direction are owned by the root plan. |

Current production-transport slice: [shared BLE-MIDI implementation](2026-09-02_client_surface_plan.md#ble-midi-peripheral-transport-2026-10-20)
reuses one supplier and MidiOut across all three platforms. Mac has one observed
Effect Next advance with explicit compatibility; Windows/Linux have target
compiler evidence and still need native runtime qualification. The supplier
refuses an encryption requirement it cannot enforce. The frozen earlier
experiment below remains diagnostic evidence.

Current [keyboard/controller mappings](2026-09-02_client_surface_plan.md#keyboard-and-incoming-controller-mappings-2026-10-20)
reuse that panel, input and dispatcher. Sixteen focused cases/build checks are
recorded separately from device acceptance. Ordinary repeats are suppressed;
the shared Mere host rejects synthesized focus presses; the coordinated pool
intake retires Woodshed's temporary source snapshot. The [input acceptance packet](../validation/pedal-input-acceptance_20261020/README.md)
records callback/None-intent repairs, 48 focused cases and three fresh native
processes with 35 independently received messages. Separate real-key/guitar
checks finish with 12 OS-accepted messages, quiet reconnect/holds, five observed
single changes and one unresolved Note-triggered no-change. Windows/X11 focus
interaction remains unqualified.
The [Previous comparison](2026-09-02_client_surface_plan.md#previous-source-diagnostics-2026-10-21)
adds opt-in bounded input/writer/BLE diagnostics to investigate that observation
through the same writer. Its 38 focused cases and new eight-message native
comparison pass with identical output bytes. Real P, sparse Note and held Note
each return Rev to Off in the traced guitar comparison (12 accepted messages,
zero disconnects). The earlier miss did not reproduce and remains unexplained.
[Desktop diagnostics](2026-09-02_client_surface_plan.md#desktop-diagnostic-controls-2026-10-21)
add session-only recording controls, recent event visibility and versioned JSON
export through the same status/history owner. Mac/core/shared Windows/Linux
compilation passes. The separate [diagnostic acceptance packet](../validation/midi-diagnostics-acceptance_20261021/README.md)
records 39 focused passing cases and native macOS controls/dialog/reopen results:
retained history, default-off reopen, full Status/export equality and both pending
host-command close branches. Native Quit's modal refusal is a separate observation;
Windows/Linux dialog runtime remains unqualified.


Earlier device session: [Q-PC MIDI pedal experiment](../validation/midi-pedal_20261017/README.md)
records the first successful software-pedal trigger: a direct CoreBluetooth
subscription and one owner-confirmed Effect Next change using the configured
channel-1 CC-67 binding. Firmware/RPC baseline, Beats, timing, and broader
performance qualification remain open. The owner-reported MIDI menu has no
standalone Beats action; the diagnostic trial has ended and its helper is closed.

USB follow-up: [storage and Beats inspection](../validation/usb_20261018/README.md)
observed two configuration files, nine tone banks, ten drum MIDI files and
stored Beats selection keys. Private configurations were captured outside Git;
that October 18 snapshot did not qualify custom patterns or live RPC control.
Later AP3 custom-file and intro/loop results are linked below. The drive was ejected.

`PROJECT_DESCRIPTION.md` (maintainer-owned, DOC_POLICY §7) is not yet written;
the founding plan carries the product intent until it is.

AP3 follow-up: [native/project and guitar evidence](../../../validation/audio-packs-ap3_20261020/README.md)
records the explicitly approved `11-Ringdown_AP3.mid` trial and owner-confirmed
repeated playback, including after restarting the guitar with USB OFF. It does not establish a complete note map or remote Beats
control. Factory files were retained and the volume safely ejected. A
distinguishable-bar follow-up confirms one authored intro bar followed by
bars 2–5 repeating, with intro replay after stop/start. Playback begins directly
with the file; no separate count-in control was found. The five-bar 4/4 result
informs a future guitar export profile; other lengths/meters remain open.
The receipt retains an unidentified additional directory-entry observation.

Publication follow-up: [bounded provenance/terms and standards audit](2026-09-02_client_surface_plan.md#provenance-terms-and-standards-audit-2026-10-20)
records a 16-file source inventory and 124 package declarations, separates
MIDI/SMF/audio from private RPC/codec/framing, qualifies the older legal
claims and lists A1–A7 evidence gaps. The outreach draft is saved in that plan
and has not been sent. This assessment is not a legal-clearance verdict.
