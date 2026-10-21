# Pedal input implementation and focused checks — 2026-10-20

**Keyboard/Note/CC mappings and ordinary-repeat suppression are implemented.
The consumed shared host rejects synthesized focus presses before dispatch.**
See the [canonical contract](../../design_docs/2026-09-02_client_surface_plan.md#keyboard-and-incoming-controller-mappings-2026-10-20).
The preceding native/device [pedal acceptance](../pedal-acceptance_20261020/README.md)
is unchanged and does not qualify these new input controls.

## Results

| Check | Result |
| --- | --- |
| Instrument focused `pedal` cases | 10 pass: four existing encoding cases, four new mapping/edge cases, two new input-history/parser cases. |
| Desktop focused `pedal_inputs` cases | 4 new cases pass through the production keyboard interceptor and retained app controls: 64 repeats, new fresh press, target retention, learn/duplicate refusal, command-chord preservation, arm/clear/output-edit disarming. |
| Shared-host focus gate | 1 new case passes against the exact production `src/key_admission.rs` via `rustc --test`; 64 synthesized snapshots are refused. |
| Settings focused `pedal_preparation` | 1 extended case passes: legacy defaults and keyboard/controller preparation round trip; live arming, learners, pending request and target are absent from settings. |
| macOS `woodshed-genet` build | Pass, Rust 1.99.1, debug info off, four build jobs. |
| Windows GNU / Linux GNU | Shared `woodshed-instrument --features ble-midi` target checks pass; Linux uses the existing ALSA cross sysroot. The exact shared-host gate also compiles for both targets. Full desktop linking/native input behavior is not claimed. |
| Native input/controller/guitar | Not exercised in this turn. No devices opened for a trial, musical messages sent, or guitar files/settings written. |

The 16 focused executions contain 11 new cases and one extended existing case;
they are a focused subset, not a workspace-suite result. The supplier gate test
compiles that dependency-free production module directly; it is not a full
shared-host package suite or an injected native focus event. Earlier native/
guitar observations remain tied to their earlier source. Logs include existing
warnings. Cargo commands are locked/offline after one offline resolution that
substitutes the host package's source; no dependency version changes. Final
desktop tests/build consume that exact patch. Scratch attempts retain the
unpatched-host build/tests, an interrupted redundant `cargo check`, and a rejected
package-test invocation (supplier is excluded from workspace membership).

## Ownership and input behavior

- `woodshed-instrument::pedal::input` owns saved input data and release-gated
  Note/CC edges; no device or writer lives in that module.
- The existing MIDI input parser/history now supplies sequence-consumed events
  and endpoint liveness. Same-timestamp events remain distinct. Gaps, removed
  endpoints and events older than 100 ms disarm and discard; no replay queue.
- The existing settings panel supplies Learn/Clear and live Arm controls.
  Learning sends nothing. Notes release on Note Off/zero velocity; momentary
  CC uses <64 release / >=64 press. A release is required before the initial
  controller press after learning/reset. Relative encoders and Program Change
  input are not supported in this slice.
- Keyboard mappings accept logical ASCII letters/digits and navigation keys,
  optionally Shift/Alt. Ctrl/Cmd and text/menu editing remain with the app;
  repeat-flagged matching presses are consumed without another pedal request.
- Mapping/output-binding edits and connection changes disarm. Only mappings
  persist. Input requests capture the UI-observed output revision/generation
  and use the same dispatcher/writer as buttons and stdio. Busy/refused and
  uncertain press/release behavior retain that owner's existing contract.
- Polling continues for a selected input even while disarmed so a controller
  release cannot be left in display history when the host otherwise goes idle.

## Shared-host focus correction and reuse

Source inspection of Mere `d1a200b1` and winit 0.30.13 establishes that Windows/
X11 may synthesize a non-repeat key press when focus returns with the key still
held. The original host destructured `KeyboardInput { event, .. }`, losing
`is_synthetic` before the app's interceptor. Both native synth paths can carry
ordinary text, so filtering character/text fields cannot solve this reliably.

[shared-host-focus.patch](shared-host-focus.patch) records the exact correction
at the existing shared event owner: synthesized presses never reach `self.key`;
releases retain the original Tab-navigation path. The test compiles the actual
production gate module, and the final Mac desktop consumes that host source.
[Single-host metadata](single-host.json) confirms **one** resolved package; all
surrounding Mere/Genet identities remain unchanged. There is no second event
loop, app-local keyboard adapter or copied output/control implementation.

The MPL-2.0 [supplier provenance](../../../../vendor/cambium-genet-winit-host/PATCHES.md)
and `UPSTREAM.json` pin original source/license/hashes and describe the manifest
expansion from workspace/path entries to the same Git identities. The root Cargo
source patch substitutes this package alone. Only its event gate/module and
manifest portability differ; other source files remain byte-identical. This
same fix belongs at the Mere owner. Once consumed from a corrected owner revision,
remove this patch and snapshot together; do not accumulate application policy
in the supplier. No supplier cache or Mere primary checkout was edited.

**Qualification limit:** code/focused-gate/retained-control evidence covers the
admission rules. Real Windows/X11 focus switches with held keys, real controller
input/hotplug and musical/hardware timing remain native runtime qualification.
The local supplier package is part of the product's source closure now; the old
Git host without this patch does not establish this safety behavior.

## Preservation

`source.sha256` and `task.patch` identify final input work against the preserved
pre-task files, including new files. The primary checkout was already dirty;
no staging, commit, push, publication, outreach or memory update occurred.
Existing validation packets and the drive/scenario lane remain unchanged. The
root manifest/lock add the one supplier source substitution; supplier version
and surrounding package versions remain unchanged. Scratch captures and original baseline are at
`Code/testing/woodshed/pedal-inputs_20261020/`.
