# Hocket and Redshank appearance reopening — 2026-10-11

Both previously open port appearance reopen checks now pass from committed
source `977e594c03d79db0f296bf4ea44b9281c071e3ab`, based on published Woodshed
`48d4e5a`. These new production executables load the exact isolated profiles
written by the earlier [family adoption seeds](../stack-family_20261011/README.md).
No fixture choice or theme definition was replaced to make reopening pass.

| Fresh process | Presented frames | Reviewed captures | Saved choice |
|---|---:|---:|---|
| Hocket | 21 | 2 | `theme:copy-1`, HocketAcceptance, Dark |
| Redshank | 12 | 2 | `theme:copy-1`, Redshank copy, High Contrast Dark |

All four captures are nonblank and were visually inspected. Hocket also asserts
four tracks, 120 BPM, master clock on, no muted/solo tracks, no recording,
no project I/O and a closed workshop. Redshank asserts idle transport, empty
items/notes and a closed workshop before opening the appearance choices.
The before/after hash inventories show every existing fixture file unchanged.
[receipt.json](receipt.json) records the source, binary/scenario/image hashes,
resolved family, observations and complete receipts under [native/](native/).
No executables, app bundles, vault contents or profile payloads are committed.

The desktop checks pass 68 Hocket cases (one existing ignored) and 77 Redshank
cases. Both production builds pass. The strengthened final Hocket scenario
passes its focused grammar regression check. Exact results are in [logs/](logs/).
[Source freeze](source-freeze.json) covers 133 tracked port source, style,
scenario and Cargo files; [host.json](host.json) records Apple M4/arm64,
macOS 26.5.1 and 16 GiB RAM.

The port scenario adapters now expose their own
`HOCKET_PRESENTATION_TIMEOUT_SECS` and `REDSHANK_PRESENTATION_TIMEOUT_SECS`,
using Mesquite's existing deadline API. The default remains ten seconds and
an explicit allowance is clamped to 10–60 seconds. These runs used sixty
seconds; actual progress still requires presented frames. Both completed
before native-control's initial accessibility read returned, so it reported
that the finite app process was no longer running. Completion receipts and
captures establish success; neither app was relaunched or manually raised.
Each had one initial unpresented redraw, then real presentations. This does
not establish unattended background presentation on every launch.

The latest published Mere head inspected was `8f61cfe52`; Genet remains
`4bc75583`. Latest Mere still selects the same Dramatis, Netrender and maintained
Vello family, and its Mesquite source is unchanged from the consumer's qualified
`0e78fcff` pin. Locked/offline metadata verifies the chosen family in both port
workspaces, with the renderer triple at 0.11.0. Redshank does not resolve a
Dramatis package. No independent supplier head or unrelated mesh/presentation
work was introduced into this bounded qualification.

The Hocket appearance lane explicitly disables audio. Redshank uses the empty
listener fixture and requests no playback, recording or feed. The earlier
synthetic keeper is stopped; Hocket displays its expected identity-unavailable
state while reopening device-local appearance. This is not new custody,
handoff, media/project recording, microphone or physical audio qualification.
No narrow-start pair or other headed platform is added. The prior failed
attempts and their original receipts remain intact; these new receipts close
only the saved appearance reopen gates.
