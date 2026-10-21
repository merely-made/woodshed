# BLE peripheral supplier

This is the single maintained supplier used by Woodshed's standard BLE-MIDI
transport. Source imported from https://github.com/rohitsangwan01/ble-peripheral-rust
at `3135b029bd9f3e57c871039b087080bf511f6fe3` (upstream version 0.2.0).
The upstream MIT license and copyright are retained in LICENSE. This is not
HyVibe-derived code. Platform APIs remain CoreBluetooth, BlueZ/BlueR and WinRT.

Local changes are scoped to reusable peripheral lifecycle and delivery:
explicit shutdown/reopen, registration ordering, subscription generation and
targeted notifications, and actual send/congestion failures. MIDI encoding,
actions and guitar bindings remain outside this supplier. The API intentionally
requires a subscription generation rather than retaining the old broadcast
update method. No upstream examples or second runtime are product entry points.

The canonical assessment, qualification limits and implementation status live
in `ports/ringdown/design_docs/2026-09-02_client_surface_plan.md`.

Linux's BlueR 0.17.4 supplier is retained in vendor/bluer with a narrow
notification-encryption flag patch; see its PATCHES.md and original
BSD license. This avoids silently weakening CCCD security.

Windows cannot documentably enforce notification encryption through the API
used here. Notify/IndicateEncryptionRequired now fails registration rather
than silently becoming a plaintext notification. Explicit plain access remains
available. WinRT ATT handlers use deferrals, bounded response waits and normal
error propagation. The OS controls the advertised local name. Mac ATT callback
waits are bounded and multi-request write batches are rejected atomically.
