# Remaining ports/protocol reconciliation — 2026-10-17

Local HEAD: `a42477297b692feb7df4d8be74fba4cb83bcaf8c`. Frozen baseline: `d12201bfe80cc94345c6308c46caaebf4a9fe42e`. This completes the requested archive/index reconciliation; it does not start implementation.

**Result: 40 scoped rows, 39 canonical findings. All scoped source mechanisms remain; no trigger-specific current qualification closure was verified. Current qualification is unassessed for every row.** This is completed reconciliation, not product acceptance.

Read the five frozen reports and the plan’s current-source/instrument reconciliation. All 328 assigned frozen hashes match their assignments; compared every corresponding current path. Selected changed host/container files were diffed and relevant callers reread. This is not a second full current-source audit. [ports.json](ports.json) retains full hashes, frozen/current unchanged-segment mappings, named receipt candidates and qualification boundaries.

| Partition | Assigned | Same current bytes | Changed current files |
|---|---:|---:|---:|
| hocket | 73 | 63 | 10 |
| shared | 33 | 23 | 10 |
| redshank | 178 | 174 | 4 |
| ringdown | 23 | 23 | 0 |
| theory | 21 | 20 | 1 |

All 303 unchanged files preserve source evidence only; **file/line hash equality is not current runtime qualification**. The 25 changed files include separate preservation/topology/recovery work, view timing labels and dependency/configuration inputs. Hocket H8 has new validation but still prepares bulk media synchronously. Redshank’s changed desktop file retains the scoped scheduling/export mechanisms.

## Scoped ledger

Each row is source-only, with focused regression, build, native, fresh-process, physical/device and publication qualification **unassessed**. Owners are existing component responsibilities. Links name current source lines; JSON records frozen ranges and matching segment hashes.

| Finding | Priority / package | Remaining source issue and owner | Current source |
|---|---|---|---|
| hocket:H6 | P2 · D8 | Persisted channel/cadence have no execution effect. **Owner:** Hocket updater host/settings. | [ports/hocket/crates/hocket-genet/src/update.rs:231](/Users/markik/Code/repos/woodshed/ports/hocket/crates/hocket-genet/src/update.rs:231) |
| hocket:H7 | P2 · B11 / D8 | Apply re-resolves feed instead of consuming staged offer. **Owner:** Hocket Velopack transport. | [ports/hocket/crates/hocket-genet/src/update/velopack_transport.rs:116](/Users/markik/Code/repos/woodshed/ports/hocket/crates/hocket-genet/src/update/velopack_transport.rs:116) |
| hocket:H8 | P2 · B11 | Handoff hashes, clones and serializes media on frame thread. **Owner:** Hocket project I/O host / portable preparation. | [ports/hocket/crates/hocket-genet/src/state.rs:526](/Users/markik/Code/repos/woodshed/ports/hocket/crates/hocket-genet/src/state.rs:526) |
| shared:S2 | P2 · B5 | Capture/import/unmute bypass SelectOne eligibility. **Owner:** Shared Recorder model-to-audio projection. | [crates/hocket-recorder/src/lib.rs:1292](/Users/markik/Code/repos/woodshed/crates/hocket-recorder/src/lib.rs:1292) |
| shared:S3 | P2 · D5 then B5-based correction | Live BPM nudge stops accepted loops without Resume. **Owner:** Shared Recorder transport policy. | [crates/hocket-recorder/src/lib.rs:1199](/Users/markik/Code/repos/woodshed/crates/hocket-recorder/src/lib.rs:1199) |
| shared:S4 | P2 · B7 | Reusable wrapper drops backend failure status. **Owner:** Public hocket-views Recorder adapter. | [crates/hocket-views/src/recorder_adapter.rs:141](/Users/markik/Code/repos/woodshed/crates/hocket-views/src/recorder_adapter.rs:141) |
| shared:C1 | P2 · D13 | Compatibility conversion drops frozen action targets. **Owner:** Public compatibility action contract. | [crates/hocket-views/src/recorder_adapter.rs:227](/Users/markik/Code/repos/woodshed/crates/hocket-views/src/recorder_adapter.rs:227) |
| redshank:R02 | P2 · B8 | Preview output can ignore listener mute. **Owner:** Redshank shared output owner / worker. | [ports/redshank/playback/src/backend.rs:613](/Users/markik/Code/repos/woodshed/ports/redshank/playback/src/backend.rs:613) |
| redshank:R03 | P2 · B8 | Worker recreation resets effective controls but retains published values. **Owner:** Redshank playback recovery. | [ports/redshank/playback/src/worker.rs:507](/Users/markik/Code/repos/woodshed/ports/redshank/playback/src/worker.rs:507) |
| redshank:R04 | P2 · B10 | Native idle predicate omits alignment/export jobs. **Owner:** Redshank desktop scheduling. | [ports/redshank/desktop/src/main.rs:2216](/Users/markik/Code/repos/woodshed/ports/redshank/desktop/src/main.rs:2216) |
| redshank:R05 | P2 · B9 | Dock projection removes pointer release target. **Owner:** Redshank capture host / gesture owner. | [ports/redshank/surfaces/src/dock.rs:369](/Users/markik/Code/repos/woodshed/ports/redshank/surfaces/src/dock.rs:369) |
| redshank:R06 | P2 · B8 | Integrity refusal can nest mutable borrow and panic. **Owner:** Redshank cached-admission worker. | [ports/redshank/playback/src/worker.rs:561](/Users/markik/Code/repos/woodshed/ports/redshank/playback/src/worker.rs:561) |
| redshank:R07 | P2 · B8 | Exact-end seek can remain Buffering without first frame. **Owner:** Redshank backend EOF admission. | [ports/redshank/playback/src/backend.rs:754](/Users/markik/Code/repos/woodshed/ports/redshank/playback/src/backend.rs:754) |
| redshank:R08 | P2 · B10 | Sanitized ItemIds collide and overwrite export. **Owner:** Redshank export destination owner. | [ports/redshank/desktop/src/main.rs:916](/Users/markik/Code/repos/woodshed/ports/redshank/desktop/src/main.rs:916) |
| redshank:R09 | P2 · B9 / D3 | Ten-minute sample cap silently drops tail. **Owner:** Redshank capture capacity owner. | [ports/redshank/desktop/src/voice.rs:177](/Users/markik/Code/repos/woodshed/ports/redshank/desktop/src/voice.rs:177) |
| redshank:R10 | P2 · C4 | Explicit Local audio selection falls back to feed. **Owner:** Redshank navigation scope. | [ports/redshank/surfaces/src/lib.rs:690](/Users/markik/Code/repos/woodshed/ports/redshank/surfaces/src/lib.rs:690) |
| redshank:R11 | P2 · C4 | Rail Seek slider has no seek handlers. **Owner:** Redshank rail seek interaction. | [ports/redshank/surfaces/src/rail.rs:15](/Users/markik/Code/repos/woodshed/ports/redshank/surfaces/src/rail.rs:15) |
| redshank:R12 | P2 · C4 | Connections card invents cached match. **Owner:** Redshank representation projection. | [ports/redshank/surfaces/src/scene/geometry.rs:229](/Users/markik/Code/repos/woodshed/ports/redshank/surfaces/src/scene/geometry.rs:229) |
| redshank:R13 | P2 · C4 | All notes merges offsets under selected-item timeline. **Owner:** Redshank item-scoped notes projection. | [ports/redshank/surfaces/src/tabs/notes.rs:318](/Users/markik/Code/repos/woodshed/ports/redshank/surfaces/src/tabs/notes.rs:318) |
| redshank:R14 | P3 · Q2 | Subscription proof requires pruned generations to survive. **Owner:** Redshank subscription receipt owner. | [ports/redshank/scripts/subscribe-podcasts.ps1:134](/Users/markik/Code/repos/woodshed/ports/redshank/scripts/subscribe-podcasts.ps1:134) |
| redshank:R15 | P3 · C4 | Retained total is mislabeled this week. **Owner:** Redshank Trail time scope. | [ports/redshank/surfaces/src/scene/mod.rs:39](/Users/markik/Code/repos/woodshed/ports/redshank/surfaces/src/scene/mod.rs:39) |
| redshank:R16 | P3 · K2 | Production range history grows without bound. **Owner:** Redshank HTTP instrumentation. | [ports/redshank/playback/src/http_range.rs:179](/Users/markik/Code/repos/woodshed/ports/redshank/playback/src/http_range.rs:179) |
| redshank:R17 | P3 · B8 | Rate changes bypass admitted controller. **Owner:** Redshank controller rate admission. | [ports/redshank/playback/src/worker.rs:670](/Users/markik/Code/repos/woodshed/ports/redshank/playback/src/worker.rs:670) |
| ringdown:R1 | P2 · K3 / D14 | Restore loses negative re-push receipts. **Owner:** Ringdown public restore contract. | [ports/ringdown/crates/ringdown-client/src/lib.rs:754](/Users/markik/Code/repos/woodshed/ports/ringdown/crates/ringdown-client/src/lib.rs:754) |
| ringdown:R2 | P2 · B12 | Even-byte Unicode hex can panic. **Owner:** Ringdown file hex admission. | [ports/ringdown/crates/ringdown-client/src/lib.rs:204](/Users/markik/Code/repos/woodshed/ports/ringdown/crates/ringdown-client/src/lib.rs:204) |
| ringdown:R3 | P2 · B12 / D9 | Sparse status becomes confident observed defaults. **Owner:** Ringdown status decoder / instrument projection. | [ports/ringdown/crates/ringdown/src/rpc.rs:294](/Users/markik/Code/repos/woodshed/ports/ringdown/crates/ringdown/src/rpc.rs:294) |
| ringdown:R4 | P2 · B14 | Failed requested file save still returns success. **Owner:** Ringdown probe persistence/outcome. | [ports/ringdown/apps/probe/src/main.rs:884](/Users/markik/Code/repos/woodshed/ports/ringdown/apps/probe/src/main.rs:884) |
| ringdown:R5 | P2 · B12 | ACK accepts wrong or extra envelope. **Owner:** Ringdown Ack2 parser. | [ports/ringdown/crates/ringdown/src/llt2.rs:122](/Users/markik/Code/repos/woodshed/ports/ringdown/crates/ringdown/src/llt2.rs:122) |
| ringdown:R6 | P2 · B14 / D9 | ACL/ATT decoding loses connection identity. **Owner:** Ringdown probe capture provenance. | [ports/ringdown/apps/probe/src/pklg.rs:106](/Users/markik/Code/repos/woodshed/ports/ringdown/apps/probe/src/pklg.rs:106) |
| ringdown:R7 | P2 · B14 | Uncheckable/skipped metadata becomes all-match claim. **Owner:** Ringdown probe index summary. | [ports/ringdown/apps/probe/src/main.rs:759](/Users/markik/Code/repos/woodshed/ports/ringdown/apps/probe/src/main.rs:759) |
| ringdown:R8 | P3 · K3 | Wedging-method fact duplicated in instrument host. **Owner:** Supplier firmware metadata / stricter host policy. | [ports/ringdown/crates/ringdown-client/src/lib.rs:215](/Users/markik/Code/repos/woodshed/ports/ringdown/crates/ringdown-client/src/lib.rs:215) |
| ringdown:R9 | P3 · K3 / D9 | Driver/probe disagree on extensionless names. **Owner:** Ringdown recording-name grammar. | [ports/ringdown/crates/ringdown-client/src/lib.rs:187](/Users/markik/Code/repos/woodshed/ports/ringdown/crates/ringdown-client/src/lib.rs:187) |
| ringdown:X1 | P2 · B13 | Readback mismatch does not fail hardware proof. **Owner:** woodshed-instrument proof example. | [crates/woodshed-instrument/examples/metronome_link.rs:94](/Users/markik/Code/repos/woodshed/crates/woodshed-instrument/examples/metronome_link.rs:94) |
| theory:T1 | P2 · B13 | Cached sent request is called a matched metronome. **Owner:** woodshed-instrument policy over Ringdown receipts. | [crates/woodshed-instrument/src/session.rs:128](/Users/markik/Code/repos/woodshed/crates/woodshed-instrument/src/session.rs:128) |
| theory:T2 | P2 · B13 | Mismatch can certify success; intervening error skips restore. **Owner:** woodshed-instrument proof example. | [crates/woodshed-instrument/examples/metronome_link.rs:94](/Users/markik/Code/repos/woodshed/crates/woodshed-instrument/examples/metronome_link.rs:94) |
| theory:T3 | P2 · D11 | Pentatonic Box Shift uses approximate translated geometry. **Owner:** woodshedding exercise meaning / realization. | [crates/woodshedding/src/exercise.rs:248](/Users/markik/Code/repos/woodshed/crates/woodshedding/src/exercise.rs:248) |
| theory:T5 | P3 · B12 | Defaults and unchecked narrowing invent measurements. **Owner:** woodshed-instrument numeric conversion. | [crates/woodshed-instrument/src/lib.rs:241](/Users/markik/Code/repos/woodshed/crates/woodshed-instrument/src/lib.rs:241) |
| theory:T6 | P3 · C5 | Same-tones copy contradicts root-preserving Related action. **Owner:** Formula graph explanation / keyed core action. | [crates/woodshed-graph/src/lib.rs:1165](/Users/markik/Code/repos/woodshed/crates/woodshed-graph/src/lib.rs:1165) |
| theory:T7 | P3 · C5 | Quartal Tetrad final tone violates ascending compound register. **Owner:** Canonical chord catalog. | [crates/woodshedding/src/chord.rs:510](/Users/markik/Code/repos/woodshed/crates/woodshedding/src/chord.rs:510) |
| theory:T8 | P3 · D12 | Tenor (fifths) data is ADGC fourths. **Owner:** Canonical tuning catalog / saved-name compatibility. | [crates/woodshedding/src/tuning.rs:1161](/Users/markik/Code/repos/woodshed/crates/woodshedding/src/tuning.rs:1161) |

## Aliases and reachability boundaries

- **Ringdown X1 = theory T2:** one B13/P2 defect. T2 includes false confirmation after mismatch and restoration skipped after an intervening error.
- **Overlay LO-5 = CORE-4:** retain the parent B1 alias; neither is an additional scoped finding.
- **Ringdown R8 stays P3/K3.** Ordinary case misses still hit the driver’s case-insensitive guard. No default guarded-path hardware bypass was established; preserve any intentional stricter host prohibition.
- Shared C1 is a public queued/retained-callback contract gap. Native Hocket and the two-mount example dispatch immediately; no native stale-callback race was established.
- Ringdown R1 is a public-library restore receipt/field-scope gap. Current source caller search found only a test restore caller. Instrument T1/T2/T5 remain library/example findings; no current GUI consumer found.
- Ringdown R3 and theory T5 share B12 but are distinct: status presence versus strict numeric conversion. R9’s extensionless-name disagreement is not established as a hardware regression; observed fixtures use .wav.
- Theory T3’s live generator and T6’s root-preserving Related action remain. D11/D12 require explicit meaning and compatibility decisions; no silent exercise redefinition or saved-tuning retune is implied.
- Hocket H1–H5, shared S1, Redshank R01 and theory T4 are outside this assignment; their current safety implementations and acceptance remain with the existing reviewer.

## Named existing receipts

No reviewed receipt closes a scoped trigger. These are retained historical candidates with explicit limits; full receipt hashes and available embedded source comparisons are in JSON.

| Candidate | Relevance and limit |
|---|---|
| [hocket-recorder_20261013/receipt.json](/Users/markik/Code/repos/woodshed/validation/hocket-recorder_20261013/receipt.json) | Historical controller/synthetic mounts/native controls. Does not establish SelectOne parity, nonempty tempo continuity, real-wrapper DOM refusal or retained-action rejection. |
| [hocket-looper_20261013/receipt.json](/Users/markik/Code/repos/woodshed/validation/hocket-looper_20261013/receipt.json) | Historical Woodshed adapter/renderer acceptance is separate from public Recorder wrapper findings. Selected retained test logs inspected for case scope. |
| [hocket-project-correlation_20261016/receipt.json](/Users/markik/Code/repos/woodshed/validation/hocket-project-correlation_20261016/receipt.json) | Request/session correlation and CPU/build scope; no media-size responsiveness or memory bound. |
| [hocket-native-project_20261016/receipt.json](/Users/markik/Code/repos/woodshed/validation/hocket-native-project_20261016/receipt.json) | Small self-addressed handoff and reopen fixture; no large-project responsiveness. Embedded source hashes compared below. |
| [redshank-current-stack_20261007/qualification.json](/Users/markik/Code/repos/woodshed/validation/redshank-current-stack_20261007/qualification.json) | Historical typed/tests/wasm scope; excludes native presentation/audio. No specific scoped negative-control closure verified. |
| [redshank-browser-a11y_20261007/qualification.json](/Users/markik/Code/repos/woodshed/validation/redshank-browser-a11y_20261007/qualification.json) | Historical Windows tests/wasm compile; excludes browser runtime, human AT and native audio. Does not close rail/worker/capture findings. |
| [port-reopen_20261011/receipt.json](/Users/markik/Code/repos/woodshed/validation/port-reopen_20261011/receipt.json) | Appearance-only reopen: Hocket audio disabled, Redshank no media/playback/record/feed. No updater/feed-drift acceptance. |
| [stack-family_20261011/receipt.json](/Users/markik/Code/repos/woodshed/validation/stack-family_20261011/receipt.json) | Historical family/product evidence; no matching Ringdown negative controls or corrected metronome proof verified. |
| [preservation-input_20261017/integration.json](/Users/markik/Code/repos/woodshed/validation/preservation-input_20261017/integration.json) | Separate safety topology/transition/recovery lane. Shared host source identity does not qualify other functions in those files. |

Selected retained controller/two-mount logs were inspected for actual case scope; their tempo/control/refusal cases do not cover the scoped negative controls. Broad source inventories and stored green summaries do not establish behavior. No exhaustive exit/binary/supplier/capture/hardware requalification was performed. Unreviewed receipts remain **unassessed**, not asserted absent.

## First unaddressed issue and parent handoff

**Within this assignment: `shared:S2 / B5`, owner: shared Recorder model-to-audio projection (`crates/hocket-recorder`), consuming `hocket-model::PlaybackMode` eligibility.** This is the first remaining scoped package in existing plan order. Capture/import/unmute must agree with canonical SelectOne eligibility. Focused actual-controller parity and separately applicable native gates remain unassessed. This is an archive handoff, not authorization to implement or run them.

The parent plan’s A0 safety frontier and proposed A3/Hocket H3 offered-identity correction stay with the excluded safety reviewer. This scoped ordering does not certify or override that lane.

HEAD stayed a424772; tracked worktree/index stayed clean. The checkout was initially clean; concurrent parent work later added untracked `validation/full-source-audit_20261016/` and completion artifacts. Those were left untouched. All compared source and frozen input hashes were stable at final pre-write verification.

No tests, builds, native launches, hardware operations, network access, Git mutations or product edits. Only `completion-20261017/ports.md` and `completion-20261017/ports.json` were written.
