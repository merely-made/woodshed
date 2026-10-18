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
- **Cross-platform product.** Windows, Linux and macOS are first-implementation
  targets. Shared Rust actions, sessions and views sit above thin platform I/O
  adapters. A platform probe establishes one adapter's evidence, not a native
  product direction. Use the existing Cambium host and family lifetime contracts;
  background residency belongs in the Djinn assessment, not a new app daemon.
- **Sans-io first.** Protocol logic is pure functions over bytes, replayable
  against fixtures, `no_std`-capable — I/O lives in a separate shell. This is
  what keeps the future-firmware option open without committing to it, and it
  is the retinue discipline the repo is modelled on.
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
| [`2026-09-02_client_surface_plan.md`](2026-09-02_client_surface_plan.md) | Current cross-platform performance plan: stack/ownership review, joint software stomp-box/Beats assessment, MIDI versus RPC paths, P0–P4 device/CLI/desktop/cue/scripting targets; retains A–D typed/profile foundation. |

Current device session: [Q-PC MIDI pedal experiment](../validation/midi-pedal_20261017/README.md)
records the first successful software-pedal trigger: a direct CoreBluetooth
subscription and one owner-confirmed Effect Next change using the configured
channel-1 CC-67 binding. Firmware/RPC baseline, Beats, timing, and broader
performance qualification remain open. The owner-reported MIDI menu has no
standalone Beats action; the diagnostic trial has ended and its helper is closed.

`PROJECT_DESCRIPTION.md` (maintainer-owned, DOC_POLICY §7) is not yet written;
the founding plan carries the product intent until it is.
