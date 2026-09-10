# Ringdown

An independent desktop client for the HyVibe smart guitar. It speaks the
instrument's own JSON-RPC-over-Bluetooth protocol directly, so the guitar can be
configured — eventually past what the vendor's phone app exposes — from a
computer, with that capability owned rather than rented from an app.

A *ringdown* is how a resonating system decays once it stops being driven — the
tail of a struck string, or of a guitar body rung by the actuator inside it.
Controlling how long the instrument keeps sounding is much of what this thing
is for.

> **Status (2026-09-01):** the protocol is **hardware-verified**. The client
> connects, reads the instrument's status, drives its metronome, reads the
> body resonances the guitar measured of itself, and edits a preset's effect
> chain audibly. Both transports are implemented, including the compressed
> one this firmware selects, and a compressed round trip has been confirmed on
> a real instrument. The plan and the full protocol map live in
> [`design_docs/2026-08-27_ringdown_founding.md`](design_docs/2026-08-27_ringdown_founding.md).

## What it can do

- Read identity, battery, storage and both firmware versions.
- Read and drive the instrument's metronome.
- Read the guitar's own measured body resonances — the plate modes it derives
  by calibration and notches to prevent feedback.
- Edit a preset's effect chain and hear it: add, remove, reorder and bypass
  any of the thirteen effects, with every knob the vendor app exposes. Rename
  presets and switch between them. Read the recordings stored on the device.
  How the vendor app creates a new preset is not yet worked out, and the app
  overwrites every change when it next connects.
- Reach methods the vendor's app never calls, five of which are confirmed to
  exist on current firmware.

One method is refused rather than exposed: `ReadConfig` hangs the instrument's
RPC handler until it is power-cycled. Its contents are reachable by composing
calls that work.

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
