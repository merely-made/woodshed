# Ringdown

An independent desktop controller for the HyVibe smart guitar, with performance
control and scripting as its next product target: prepare tones, trigger the
guitar from a keyboard or MIDI controller, and recall song cues. The existing
client speaks the instrument's JSON-RPC-over-Bluetooth app protocol; a software
Bluetooth stomp box and control of the new Beats rhythms are planned together.

A *ringdown* is how a resonating system decays once it stops being driven — the
tail of a struck string, or of a guitar body rung by the actuator inside it.
Controlling how long the instrument keeps sounding is much of what this thing
is for.

> **Status (2026-10-17):** reusable protocol/client libraries and a research
> probe exist. Earlier hardware sessions established status, metronome and
> effect control, recording retrieval and playable preset creation. The
> [desktop performance assessment and plan](design_docs/2026-09-02_client_surface_plan.md)
> covers MIDI pedal emulation, Beats, a desktop surface and scripting. Those
> features are not implemented or qualified against the owner's recent
> firmware update. The [founding plan](design_docs/2026-08-27_ringdown_founding.md)
> preserves the protocol map and historical hardware findings.

## Existing capabilities

The following have historical device evidence; current firmware needs a fresh
qualification run. The typed driver/profile path also has scripted-link
coverage, which is distinct from those hardware sessions.

- Read identity, battery, storage and both firmware versions.
- Read and drive the instrument's metronome.
- Read the guitar's own measured body resonances — the plate modes it derives
  by calibration and notches to prevent feedback.
- Edit a preset's effect chain and hear it: add, remove, reorder and bypass
  any of the thirteen effects, with every knob the vendor app exposes. Rename
  presets and switch between them. Read the recordings stored on the device.
  Create a playable preset with the bank shape established by H47; save a
  local shadow profile for restoration. The vendor app was observed to
  overwrite the guitar's profile when it connects.
- Reach methods the vendor's app did not call in the original investigation,
  five of which were confirmed to exist on that firmware.

The driver refuses `ReadConfig`: it hung the observed firmware's RPC handler
until power-cycled. Working calls reach portions of that information. The
performance path retains this guard unless a firmware fix is established.

## Design

The protocol was reverse-engineered for interoperability from the vendor's own
Android app. This repo carries no vendor code, no decompiled sources, and no
vendor firmware — only the recovered facts and interfaces, reimplemented
independently. Architecture follows the retinue template — a sans-io protocol
core, a thin Bluetooth I/O shell, and receipt-driven validation.
[Woodshed](https://github.com/merely-made/woodshed) is the first consumer and
repository host. Ringdown remains a separately versioned and released nested
workspace under `ports/ringdown`.

Ringdown is not affiliated with or endorsed by HyVibe; "HyVibe" is their
trademark, used here only to say what this interoperates with.

Start with [`design_docs/DOC_README.md`](design_docs/DOC_README.md).
