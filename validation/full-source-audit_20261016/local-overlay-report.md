# Frozen P5a local-overlay full semantic architecture review

Initial change review: 2026-10-16. Full assigned-file semantic read completed: 2026-10-17. Disposition: **request changes / qualification remains open**. This is a complete semantic read of the 33 assigned frozen local P5a files, separate from the published-main audit. The findings comprise the admission defect and test regressions already reported, three untouched follow-ups (history, Walk timing, and drawn-path setup), and the already supplied integrated-v2 compile blocker. No runtime failure below is presented as an executed test result.

## Authority, inputs, and limits

- Overlay: `/Users/markik/Code/testing/woodshed/full-source-audit-20261016/local-p5a-overlay`.
- Assigned inventory: `local-overlay-assigned.json`, 33 files / 34,861 lines. Every file was byte-read and its SHA-256 and line count matched the assignment. Exact hashes, changed hunks, reviewed context ranges, and unreviewed unchanged ranges are in `local-overlay-coverage.json`.
- `local-uncommitted.patch`: SHA-256 `6d9b088429359e2a2059977a473e19bce3132f3a47f8a3a8351759d17cb6c66f`, 2,203 lines, 31 tracked files, 155 hunks, 473 added / 349 deleted lines. The two new modules add another 296 lines; all 769 added/new lines and every patch hunk were reviewed.
- The patch is relative to supplied local `1bc4f6a`. Published context is `source-d12201bf`, which has two additional arrangement commits. The overlay was not applied to that later snapshot and is not represented as a tested combined tree. Published-only layout differences are not charged to P5a.
- Full semantic coverage is now **33 / 33 files and 34,861 / 34,861 assigned lines**, with **zero remaining assigned ranges**. The continuation actually read all 27,662 previously missing lines, including production code, tests, comments, and design documentation. The ledger preserves both the original 7,199-line read and every missing interval closed on October 17. Hash checks remain input-integrity evidence, separate from semantic reading. This completes the local assignment and does not replace the separate published-main source lanes.
- No Git, Cargo, build, test, native launch, product-process control, product-file change, or other-chat messaging was performed. Only the two requested external audit artifacts were written. The known integrated-v2 failure was not fixed.

All production references below are relative to the frozen overlay unless explicitly prefixed `source-d12201bf/`. Line numbers refer to those exact hashed files.

## Semantic coverage completion (2026-10-17)

The remaining assigned source was read as actual numbered text, with truncated outputs reread before coverage was accepted. The internal review split preserved disjoint file ownership and wrote no additional artifacts. The primary reviewer reconciled all exact ranges and hashes and checked the two new findings against both the overlay and published context.

| Continuation group | Files | Newly read lines |
| --- | ---: | ---: |
| Core and output-runtime tests | 9 | 5,370 |
| Genet audio, main, scenario, session, sync and layout tests | 6 | 8,528 |
| Stage view modules | 10 | 6,313 |
| Stage shell and complete design plan | 2 | 7,451 |
| **Continuation total** | **27** | **27,662** |

Six files were already fully read in the first pass. Together the two passes cover all 33 assigned files, totaling 1,490,015 bytes and 34,861 lines. Every file retains its assigned SHA-256 and line count; the patch SHA-256 is unchanged. All per-file semantic ranges are now `[1, file_line_count]`, and all per-file missing-range arrays are empty. The original first-pass and continuation interval records remain available in `semantic_read_passes`.

The full read confirms the separation of working Set/Card identity, retained artifacts, derived backing, transient requests, and the one host-owned audio runtime. It adds the two untouched path findings below; it does not establish any new test, desktop, native, reopen, publication, device, or acoustic acceptance. Historical qualification passages in the design plan were read as dated source claims and were not independently rerun or promoted to qualification of this overlay.

## Remaining introduced or exposed issues

### LO-1 — P2 — Do not admit an unavailable realization as intentional silence

**Primary site:** `crates/woodshed-genet/src/audio.rs:473-476`, especially the unconditional empty-pitch `Ok(())` at line 475. **Call/credit sites:** `crates/woodshed-views/src/stage.rs:2185-2194,2265-2281` and `stage/timing.rs:25-50`. **Source of ambiguity:** `woodshed-core/src/scale_realization.rs:178-185`, `chord_approach.rs:252-259`, and `lib.rs:1946-1952,1981-1982`.

A retained scale whose explicit tuning is missing, an approach with an unavailable saved setup, or a selected chord shape whose fingerprint can no longer resolve returns `Ok(([], 0, 0))`. A genuinely muted realization can return the same tuple. `admit_card_preview` calls both cases intentional silence and returns success merely because `self.output` is `Some`; it invokes neither runtime availability checks nor cancellation.

**Real execution trigger:** open a retained scale Card with a missing saved tuning (the existing layout fixture already exercises that source condition), keep a valid positive Bars hold, then press **Hear selected Card** or **Run**. Core timing succeeds, realization returns an empty success tuple, the new bound request is queued, and host admission returns `Ok(())`. `finish_card_preview_admission` clears the error, records Previewed or Rehearsed history, and for Run opens a measured span and permits timed progression. No corresponding preview reaches the output processor. The UI may separately show the setup-unavailable message, while history and the runner claim successful admission.

This is an **introduced selected-Card history regression**: the old `request_card_preview` returned on empty pitches before recording preview interest; the overlay now queues and credits it. Silent progression for an unavailable realization was already possible in the old rehearsal path, so that part is **remaining inherited behavior**, not a newly introduced realization bug.

Even valid all-muted Cards bypass actual runtime health and cancellation. If a preceding block preview is still sounding, moving to an admitted silent Card does not retire that voice. A concrete case is a 1.4-second block preview followed after the retained 0.5-second dwell by a fully muted Card. The old preview can continue into the interval being credited as the silent instruction. This tail behavior existed in the prior transport too; the new empty-success branch still does not provide the admission it asserts.

**Required correction:** distinguish a successfully resolved intentional silence from an unavailable realization before success credit. Preserve stored instructions and the explicit unavailable reason. If silence is admitted, make its required output/cancellation operation fallible rather than bypassing the runtime. `LoopPlan::lower` already demonstrates the useful distinction by checking unmarked availability before accepting marked silence (`loop_plan.rs:155-175`). This does not require a new device clock or pause policy.

### LO-2 — P2 — Finish the admission-fixture migration before trusting affected gates

These are **source-predicted test failures or lost coverage**, not results from running tests. They are independent of the known compile error below and would still matter after that single typo is corrected.

1. **Observation fixtures do not enter running state.** `stage.rs:3589-3593` constructs `UiState::new()` and stages a Card; it never sets `rehearsal_running`. Tests at `stage.rs:4449-4462` and `4495-4511` then directly call `record_rehearsal_cursor(); admit_previews_for_test()`. The new request has a run epoch, but `stage/timing.rs:19-21` rejects it because the runner is not running. The helper consumes the request anyway (`timing.rs:82-88`), so no span opens and expected totals of 9,000 ms and 4,000 ms remain zero. The connected interaction-clock test repeats this setup at `stage/connected.rs:317-328`; its expected `card_started_ms == Some(42000)` cannot hold. The no-clock and backwards-clock variants can now pass without opening any observation, so they no longer exercise the claimed measurement boundary.
2. **A post-drain assertion still expects the old request variant.** `stage/connected.rs:294-296` calls `toggle_rehearsal(); admit_previews_for_test()` and then requires a trailing `AudioRequest::PreviewPitches`. Production Run now queues `CardPreview`; the helper removes it, and no matching PreviewPitches is created. The assertion cannot establish inherited timing through the new host seam.
3. **The dwell return contract changed without changing the end-of-Set assertion.** `woodshed-genet/src/drive.rs:279` now returns `ui.rehearsal_running`. The two-Card test at lines 477-484 calls the second end-of-Set dwell under `assert!(...)`, then immediately asserts the runner stopped. In that execution path `advance_rehearsal_cursor` clears running and `rehearsal_dwell` returns false, so the first assertion fails. This is a stale test expectation, not evidence that the production stop behavior itself is wrong.

**Fixture distinction confirmed by the full host read:** the Genet helper `main.rs:482-488` clones the request queue and acknowledges without draining it. Its retained `CardPreview` assertions at `layout_tests.rs:1248-1251,1472-1476` are therefore not additional drain-related failures. The separate views helper `stage/timing.rs:82-88` consumes requests, which is the helper involved in the stale assertion described above.

**Required correction:** use a valid Run lifecycle before explicit fixture acknowledgement; assert the new request or its admitted frozen payload at the appropriate boundary; update the dwell test to its new continuation contract. Keep CPU fixture acknowledgement explicitly separate from real backend admission and native evidence. No product fixes or tests were run in this review.

## Known supplied blocker, kept separate

### LO-K1 — P1 — integrated-v2 test compilation is blocked by undefined `pitches`

`crates/woodshed-genet/src/layout_tests.rs:1081-1082` matches `AudioRequest::CardPreview(request)` but still reads `(pitches[0] - d4_hz)`. The binding named `pitches` was removed by the patch. This agrees with the supplied known integrated-v2 compile failure. This review confirms the source defect only; it did not rerun compilation, change the file, or claim that correcting this is sufficient for the test suite to pass. LO-2 explains additional source-level failures beyond that known typo.

## Remaining untouched behavior relevant to the architecture

### LO-3 — P2 — Discovery/catalog history still credits requests before fallible admission

`stage/connected.rs:74-90` records Previewed before enqueueing an unbound `PreviewPitches`; the same order remains in `connected_scales.rs:92-108`, `scale_patterns.rs:58-72,155-173`, `chord_approaches.rs:117-131`, and catalog `stage.rs:2178-2181,2208-2224`. The overlay adds timing-error early returns, but retains this existing history-before-host ordering. `woodshed-genet/src/sync.rs:154-163` now receives the actual exact-admission error and sets a notice, but it cannot revoke the history already written.

**Real trigger:** on a retained chord arpeggio with positive BPM 0.01, request a one-bar discovery audition at an ordinary 48 kHz output. Its 24,000-second duration fits `Duration` and resolves in core; the host's actual-rate PCM budget rejects it. A Previewed observation already exists. An unavailable output or backing playback refusal gives the same history/result mismatch. This is not evidence that the pure core should impose a seconds cap: the resource refusal belongs to audio.

This ordering **predates the overlay** and is not a new regression. It remains relevant if a later completion report claims all audition observations are gated by admission; the new bound selected-Card/Run seam alone does not establish that broader claim. Carry the admission result with the observation if that broader behavior is in scope.

### LO-4 — P2 — Untouched ordinary Walk previews do not consume resolved timing

**Sites:** `crates/woodshed-core/src/lib.rs:697-701,1967-1988` and `connected_catalog.rs:106-113`. `Material::Path` marks itself sequential, and an ordinary chord with `Touch::Walk` also takes the fixed `voicing_shape` route. That route uses 130 ms spacing and `(visit_count * 0.13 + 0.4).min(3.0)` seconds; the arpeggio adapter returns it unchanged because its touch is Walk. Timing validation succeeds, but the resolved timing never shapes those visits.

**Concrete trigger:** save or import a ten-note drawn-path Card with `Touch::Walk`, one Bars hold and authored BPM 400, followed by a voiced Card. Its validated dwell is 0.6 seconds, but its preview is 1.7 seconds with the tenth onset at 1.17 seconds. The dwell runner (`woodshed-genet/src/drive.rs:255-275`) advances at 0.6 seconds; the next voiced Card cancels the unfinished path. With a silent successor, LO-1 can instead leave the old tail sounding. The imported BPM is valid under the settled policy; this finding does not propose clamping it.

**Classification and disposition:** this fixed-shape behavior is already present in `source-d12201bf` (`lib.rs:1977-1998`) and is **untouched by P5a**. It is an additional existing ordered-articulation gap, separate from the newly routed scale/pattern/approach and chord-arpeggio paths. Track it explicitly as a follow-up if complete Walk timing is claimed; do not mislabel it as a new device-clock or pause-policy requirement. A future correction can use the existing resolved timing for these ordered previews without adding another timing authority.

### LO-5 — P2 — Untouched saved drawn paths ignore their retained setup at replay

**Save site:** `crates/woodshed-core/src/lib.rs:1319-1343` stores the path contacts together with the current instrument and tuning. The real view Save action uses this method (`woodshed-views/src/stage.rs:2255-2261`). **Replay sites:** `lib.rs:1395,1439-1463` draws against the Stage board, while `lib.rs:1971-1976` voices against `self.tuning()` and the live string/fret limits. Neither path uses the Card's saved instrument/tuning. The display comments explicitly describe current-tuning reinterpretation; that documents the existing behavior but does not reconcile it with a retained Card carrying explicit setup fields.

**Concrete trigger:** draw and save a path containing the open low-E contact on Standard guitar, switch the visible exploration to Drop D, then Hear or Run the saved Card. The saved Standard instruction remains unchanged while that contact now resolves to D. Switching to a four-string instrument can also silently omit saved contacts with string indices 4 or 5. The runner's captured Stage prevents further explorer changes during that run; it captures the later Stage setup rather than restoring this Card's retained setup, so it does not resolve the pre-Run mismatch.

**Classification and disposition:** both save and replay branches predate the overlay; the same path replay appears in published context at `lib.rs:1977-1987`. This is an **untouched retained-setup authority gap**, not a P5a-introduced mutation. Resolve contacts through the Card's retained setup and surface unavailable contacts, or explicitly establish a different product contract for reinterpretable physical paths. Stored setup should not appear authoritative while execution ignores it. This follow-up requires no historical PCM migration or replacement runtime.

### Execution and clock boundaries deliberately still open

- Success from `OutputRuntime::preview_card_pitches` is **control-side worker admission**, not completed synthesis or processor admission. The method queues cancellation, replaces the worker request, publishes a preview revision and returns (`output_runtime.rs:653-668`). `source-d12201bf/.../output_runtime/preview.rs:44-80` renders asynchronously; `output_runtime.rs:250-259` later forwards a completion. The processor separately accepts a current revision only when backing is stopped (`source-d12201bf/.../processor.rs:935-943`) and then increments `preview_starts`.
- **Concrete separation:** a second request or stop can supersede an accepted worker before any processing block sees its PCM. The first request can have a history observation without a `preview_starts` increment. Do not use the new acknowledgement as a sound/render receipt. P5a's admission boundary can be evaluated without changing it into the explicitly deferred device/event clock.
- Dwell uses `Instant`; observations use the host's wall/event clock and checked wall-time subtraction (`drive.rs:16-20,132,243-279`; `stage.rs:2285-2307`). Refresh resets dwell by serialized instruction/tempo/start facts (`drive.rs:185-208`). Existing frame quantization, tempo-edit/restart behavior, pause policy and sample-exact articulation remain open P5 work. They are not charged as newly introduced P5a defects here.
- No-backend progression policy is expressly outside the documented slice (`design_docs/...:1786-1788`). This review does not invent a no-audio run policy or treat missing native evidence as a new clock implementation requirement.
- `LoopPlan` retains its own explicitly different meter/backing admission rules, including the existing minimum 1 BPM (`loop_plan.rs:90-153`). The P5a resolver has not silently become the authority for all Looper or old-PCM clocks. No replacement audio runtime, capture-clock migration, or historical PCM rewrite is present in the reviewed changes.

## Issues fixed in the overlay — source verified, qualification not implied

| Concern in the prior code | Frozen overlay behavior and source |
| --- | --- |
| Infallible Duration conversion can panic on imported Seconds | `card_timing.rs:36-55` uses `try_from_secs_f32`, rejects zero/unrepresentable conversions, and `lib.rs:613-615` returns a typed Result instead of fake Manual. |
| Conflicting dwell/arpeggio/scale BPM clamps | Shared `resolve_card_timing` accepts finite positive authored values including 20/400; `connected_catalog.rs:154-156` and `scale_realization.rs:233-235` use its ordered preview calculation. `card_dwell` uses the same resolver. |
| Invalid authored BPM might inherit fallback | `card_timing.rs:42-44` and `lib.rs:2003-2008` use None-only inheritance; present invalid values are refused. The effective Card is a clone. |
| Hidden legacy minima | `card_timing.rs:45-53` reports HalfSecond/OneBar/OneRep without rewriting source; `stage/timing.rs:65-74` discloses the interpretation in the shared Card editor (`set_tray.rs:78`, mounted by `rehearsal.rs:172`). |
| Generic audition minimum stretches a short ordered Card preview | `output_runtime.rs:594-607` validates exact duration/spacing at actual output rate and calls the inner renderer without `duration.max(0.1)`. Resource constants remain owned by audio at lines 44-46,638-643. |
| A void audio trait or unrelated persistent error cannot establish this operation's result | New Cpal exact-admission method at `audio.rs:471-476`; the host uses the returned result in both `sync.rs:123-127` and `drive.rs:225-229`. It does not query a persistent error to infer success. LO-1 is the remaining empty-result exception. |
| Run history opens before host accepts the request | `stage.rs:2265-2281` enters pending state; `stage/timing.rs:25-50` opens the span only for a current successful request. Refusal clears runner state and queues cancellation at lines 53-63. |
| Stale run acknowledgements can affect the same Card started again in the same millisecond | Session pointer, owner, CardId, full instruction, inherited-tempo bits, pending flag and run epoch all participate in `timing.rs:12-23`. Run/stop/advance increment epochs; session load installs a new Arc, clears requests/pending and resets notices (`stage.rs:2427-2455`; `instances.rs:189-208`). |
| Background Set selection might redirect a runner request | Request lookup uses the frozen working owner and CardId, not current editor selection (`timing.rs:14-21`); `instances.rs:163-187` uses the runner's owner and captured Stage. Both execution sites validate before calling the backend. |

These findings are from the actual source paths and data dependencies, not from imports alone. The overlay preserves the stored Card at timing resolution and refusal; `CardPreviewRequest` is a transient frozen command and is not a second persisted Set. No new native-output ownership or durable Set authority is introduced.

## Acceptance disposition

The pure timing contract and the nonempty bound Card/Run admission design are materially improved. **The frozen overlay is not ready to be called qualified or complete.** Resolve LO-1, repair the known compile blocker and LO-2 fixture regressions, and then let the single authorized implementation/qualification owner run the affected gates, desktop build, native timed/manual/refusal/recovery flow, and exact fresh reopening described by P5a. LO-3 through LO-5 are separately identified existing behaviors; keep their dispositions explicit rather than claiming they were fixed by the new Card request. The full semantic read adds LO-4 and LO-5 without changing their inherited classification or charging them to the narrow P5a implementation.

No tests passed or failed in this audit, because none were executed. No desktop/native, processor-start, fresh-reopen, publication, device, or physical-audio acceptance is claimed. The exact cumulative ranges, preserved first-pass gaps, completed continuation intervals and unchanged input hashes are machine readable in `local-overlay-coverage.json`. There are no remaining unread ranges among the 33 assigned files.
