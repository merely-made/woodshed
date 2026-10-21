# Shared BLE-MIDI implementation — 2026-10-20

Canonical ownership, assessment and qualification cues live in the existing
[client surface plan](../../design_docs/2026-09-02_client_surface_plan.md#ble-midi-peripheral-transport-2026-10-20).
This is bounded working-tree evidence, not a release or standards certification.

## Results

| Surface | Observed result | Limit |
|---|---|---|
| Mac CLI | Locked/offline native build; one subscribed receiver; two CC 67 notifications accepted | Explicit unencrypted compatibility; one owner-configured Effect Next action |
| Guitar | Owner reports exactly one effect advance | No conclusion for other actions, Beats control or timing |
| Lifecycle | Explicit close succeeds; same-process reopen advertises with zero accepted messages; quit exits 0 | Not a congestion, sleep, radio-loss or reconnect fault campaign |
| Woodshed desktop | Locked/offline `cargo check -p woodshed-genet` passes | Existing picker/worker/status integration compiled; GUI hardware interaction not run |
| Windows | Rust target check passes for `x86_64-pc-windows-gnu` | No native link/runtime/pairing; encrypted notification requirement explicitly refuses |
| Linux | Rust target check passes for `x86_64-unknown-linux-gnu` | Real Debian D-Bus/ALSA/systemd development sysroot; no native link/runtime/pairing; native SSH unavailable |
| Profiles | File-only, OS-MIDI-only and private-RPC opt-in compile | Compiler evidence, not fresh behavior tests for those profiles |
| Default dependency boundaries | Probe graphs pass on all three targets; Woodshed Mac graph passes; file-only graph excludes MIDI/radio suppliers | Selected normal/build closure; no whole-source archive or legal-clearance claim |

There were two approved press/release attempts. The first was accepted before
the owner configured Effect Next, so it has no musical conclusion. The owner
requested another send, then reported one advance. Software did not write
persistent guitar files or settings. The peripheral has been closed.

## Reproduction

From `ports/ringdown`, with Rust 1.99.1 and cached dependencies:

```sh
cargo build --locked --offline -p ringdown-probe
cargo check --locked --offline -p ringdown-probe --target x86_64-pc-windows-gnu
cargo check --locked --offline -p ringdown-probe --target x86_64-unknown-linux-gnu
cargo check --locked --offline -p ringdown-probe --no-default-features
cargo check --locked --offline -p ringdown-probe --no-default-features --features midi
cargo check --locked --offline -p ringdown-probe --features hyvibe-rpc
```

The Linux cross check additionally needs target headers/libraries and
`PKG_CONFIG_ALLOW_CROSS=1`, `PKG_CONFIG_SYSROOT_DIR` and `PKG_CONFIG_LIBDIR`.
Exact scratch paths and commands are in `final-checks.json`; official package
URLs and SHA-256 hashes are in `linux-sysroot-packages.json`. No target packages
were installed into macOS. From the root, use
`cargo check --locked --offline -p woodshed-genet` for the existing host.

For the live path, select the instrument binding first, then keep this console
open while the guitar scans and subscribes:

```sh
ringdown-probe --ble-midi Ringdown --unencrypted-compatibility
```

The deliberate compatibility choice used in this trial is visible; encrypted
access is the default. Windows uses its OS-controlled advertised name and
refuses encrypted notification registration in this supplier. Linux must accept
the encryption flags or registration fails; minimum BlueZ version is unknown.
Console commands are `status`, `reopen`, `send 1 cc 67 127`, `send 1 cc 67 0`,
and `quit`. Startup/reopen send no MIDI; there are no automatic control retries.
OS acceptance does not establish an instrument action without observation.

## Evidence and provenance

- `receipt.json`: scope, compile outcomes, security/platform limits and final binary hash.
- `source-hashes.json`: final shared integration, packaging and vendored supplier hashes.
- `observed-trigger.json`: exact trigger binary and source hashes. The final CLI
  adds only an explicit reopen command to this surface; shared MIDI and Mac
  supplier hashes were checked identical. Later Windows-only refinements are
  compiled separately.
- `supplier.patch`: retained delta against MIT `ble-peripheral-rust` 0.2.0,
  upstream commit `3135b029bd9f3e57c871039b087080bf511f6fe3`.
- `bluer-security.patch`: two-file notification-security patch against the
  BSD-2-Clause BlueR 0.17.4 registry source; original licenses remain in vendor.
- `final-guitar-session.log`, `same-process-reopen.log`: native CLI observations;
  the owner's musical observation is recorded separately in the receipt.
- `probe-default-*.json`, `woodshed-default-macos.json`, `probe-file-only.json`:
  existing boundary checker output, with private RPC/btleplug excluded.
- Compile logs retain actual compiler outcomes. Existing Woodshed warnings are
  not BLE qualification failures. The final Windows check has no supplier warning.

No new automated test suite was added or run. Prior split/AP3 evidence is
historical and was not overwritten. Package scripts now retain supplier notices
and Mac Bluetooth usage metadata, but no package/signing/publication was run.
Native Windows/Linux execution, encrypted interoperability, disconnect and
congestion faults, latency/phase, other pedal bindings and GUI guitar operation
remain separate qualification cues. The vendor outreach draft remains unsent.
