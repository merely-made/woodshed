# Hocket shared appearance acceptance — 2026-10-11

The ordinary Hocket desktop now composes Tabard application choices, Tinct modes,
the shared appearance workshop and preview bindings, native focused text,
export effects and dirty-close handling. Cambium supplies its title bar and
platform caption controls. Mesquite owns scenario input, retained clicks,
resize, capture, presentation and completion. Recorder session/history,
project storage, audio and custody retain their existing owners.

## Accepted evidence

Four finite LaunchServices processes on the Intel Xeon W / Radeon Pro Vega 56
macOS host completed with `RESULT ok`: 17 inspected original PNG captures,
zero blanks and 147 presented frames. The ordinary window is 1180×700 logical
pixels; the narrow workshop is 420×1000. The seed case shows the exact warm
missing-preference default, four canonical modes, readable authored CSS preview,
explicit Save/Apply and the narrow controls. The first reopen restores the blue
saved appearance. The same-identity case saves a brown definition, returns to
Hocket, reopens that definition and applies it without making another copy.
The last fresh process restores the brown appearance.

Mesquite's retained recorder viewport comparison passes before/after Save and
changes after Apply. Independent original-PNG analysis reports zero changed
pixels above the shared two-channel-unit tolerance after Save (maximum one
unit), then 3,277,240 changed pixels after Apply. Every seed recorder capture
retains the semantic red record dot. The persisted fixture library contains
one `theme:copy-1` with exactly the entered dark CSS; the persisted application
choice is that identity in dark mode. `summary.json` records all capture hashes,
case counts, recording-red pixels and the finite reset observation. No new
GPU restart/kernel/panic report appeared during the observed interval; this
receipt does not attribute the earlier shared-machine resets.

The final desktop suite passes 64 tests, with one keeper-specific custody test
explicitly ignored. The unchanged model and history suites pass 36 and two
checks respectively. Mounted tests exercise native text dispatch, usable field
geometry, dirty-close cancellation, Save-close without Apply, the full
Save→Back→Edit→Apply sequence, canonical record-dot style/geometry, corrupt-byte
preservation, failed preference writes and protected export/path aliases.

The accepted production binary SHA256 is
`43ca2156392c884eacb0366b6e8fda190ae86a8c646c67ad9c8f5e7e41ce744f`.
`source-freeze.json` records the source inputs against Woodshed base `6dfb2fe`.
The compiler is Rust 1.97.1; Hocket's package minimum remains 1.96 and a
minimum-version build is not claimed. The coherent Mere `e95326dc`, Genet
`7422e906`, Dramatis `c1d62076`, maintained Vello `10f01d6d` graph stays pinned.

## Reproduction and scope

Build/test the independent `ports/hocket` Cargo workspace with Rust 1.97.1.
Run `scenarios/tabard_appearance.scn`, `tabard_appearance_reopen.scn`,
`tabard_appearance_same_id.scn`, then the reopen fixture again through
`mere/scripts/run_macos_scenario.py`, prefix `HOCKET`, a separate output for
each process and a finite timeout. Use one private profile for all four runs,
with `HOCKET_SETTINGS`, `HOCKET_APPEARANCE`, `HOCKET_THEME_LIBRARY` and private
XDG data/config/runtime directories. `HOCKET_APPEARANCE_RECEIPT=1` requires a
scenario and suppresses audio hardware only in that acceptance lane. Ordinary
launch still enumerates and uses actual audio hardware.

These native receipts qualify application appearance and presentation. They
do not qualify recording/playback devices, keeper custody, Windows/Linux
native presentation or the earlier update/handoff scenarios. The pending
identity refusal is visible under the isolated unavailable-resident profile.

`rejected-attempts.json` preserves the timeout, missing viewport and moved-panel
comparison failures. Their evidence is not counted as accepted. The corrected
fixture closes the appearance panel around recorder comparisons; confirmation
notices remain in the product.
