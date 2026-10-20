# Hocket first-wave source recheck

Date: 2026-10-20. Input revision: `9f80bb5eabc4b513e057b22af6f80d83ecb8e9ac`.
Checkout: `/Users/markik/Code/worktrees/woodshed-audit-first-wave-20261020`.
Owner: `hocket_first_wave`, solely covering A2, A3 and Hocket's A4 model/project/handoff seam. The integration owner controls Git, documentation and Cargo scheduling. P5a remains with its existing owner.

**Result: the requested Hocket implementations are already present. No product patch is justified by this recheck.** Existing negative-path tests exercise the relevant production boundaries. This report is a current-source review; the integration owner must record any new CPU run separately. No Cargo, build, native, device, network or Git mutation was performed by this agent.

## A2: project lifecycle and availability

`state.rs:69-94` captures the complete `ProjectBundle`, append-only media count and missing-media facts. Dirtiness at `state.rs:1063-1065` compares the full saved revision rather than only the active history head. `InMemoryStore` has private buffers, immutable reads and insertion by content hash (`crates/hocket-project/src/media.rs:53-85`), supporting the media-count assumption without copying PCM every frame.

Saving freezes the submitted bundle/media revision (`state.rs:789-819`, `state.rs:871-887`). A correlated successful reply acknowledges that submitted revision and verifies the returned saved head (`state.rs:1016-1033`); edits or added media after dispatch remain dirty. Failure or Save-dialog cancellation retains the outgoing project and preservation decision.

Open is checked both at request time (`state.rs:826-854`) and after its matching reply (`state.rs:1034-1041`). Close, Open, Accept and updater restart share `resolve_preservation` (`state.rs:1155-1195`). Its active-capture and admitted-I/O checks precede even explicit Discard. Open payloads and incoming handoffs survive cancellation/refusal. Host adoption validates the bundle before retirement/replacement (`state.rs:715-719`, `state.rs:1169-1176`). No recorder retirement change is needed.

Native close, caption/dispatch, scenario/frame continuation and updater restart all reach this same guard through `main.rs:154-185` and `main.rs:218-246`. Appearance approval is distinct from recorder-project approval; an appearance cancellation clears close approval.

Same-session handoff acceptance derives missing references from the accepted complete phrase pool and actual imported media store (`state.rs:732-738`), then installs those facts with the accepted bundle. It preserves unresolved local media and retained branches rather than carrying forward stale availability.

Meaningful existing regressions in `native_lifecycle_tests.rs`:

- `dirty_close_cancel_and_explicit_discard_preserve_the_live_revision_until_exit` (line 28).
- `cancelled_and_failed_save_keep_work_and_success_alone_continues_close` (line 47).
- `close_during_in_flight_save_waits_and_acknowledges_only_submitted_revision` (line 78).
- `cancellation_while_preservation_save_is_pending_does_not_close_on_success` (line 97).
- `unchanged_head_media_added_during_save_remains_dirty` (line 113).
- `dirty_accept_failed_save_keeps_both_projects_until_explicit_discard` (line 159).
- `unchanged_saved_head_imports_retained_branch_and_media_as_dirty` (line 189).
- `same_session_import_restores_one_missing_phrase_and_keeps_local_missing_facts` (line 222).
- `accept_during_free_capture_keeps_ticket_payload_and_playable_outgoing_take` (line 335).
- `close_during_free_capture_cannot_discard_its_stop_token` (line 358).
- `matching_open_rechecks_edits_made_after_dispatch_even_without_capture` (line 375).
- `open_dispatched_then_capture_starts_completion_is_retained_for_explicit_retry` (line 391).
- Mounted production-hook close, separate appearance permission and updater cancellation cases at lines 446, 462 and 481.

The fake-audio capture cases use the real device-free recorder service and return the admitted capture token through normal promotion. They are stronger than direct field toggles but do not establish physical capture or native-dialog acceptance.

## A3: exact identity consent

The host stages the offered family profile, target contact token and losing token (`state.rs:554-578`), renders the exact target in the confirmation (`view.rs:325-364`), and forwards every staged fact to `join_family` (`state.rs:604-615`).

`LocalIdentity::family_offer` explicitly connects as the cached family profile and checks the returned profile (`identity.rs:249-273`). Confirmation connects explicitly as the shown profile, observes its actual key and current Hocket key, and passes those facts to `admit_family_offer` (`identity.rs:278-315`). That helper requires the current Apart home, cached family profile, staged/actual profile, staged/actual target key and losing-token equality before writing consent (`identity.rs:129-166`). Only a successful write permits installing the new speaker/home. Refusal leaves the staged offer and existing identity intact. Startup consent reuse also requires both actual profile and key (`identity.rs:216-230`).

The two existing regressions at `identity.rs:476-538` cover profile/key/losing-token drift, unchanged consent bytes on refusal, exact accepted profile/key persistence, and consent-write failure retaining prior consent and Apart state. They exercise the production helper with temporary files. Real custody IPC and fresh-process persona reopening remain a separate acceptance lane.

## A4: structural project and handoff admission

`History::validate` owns complete graph checks (`crates/hocket-model/src/history.rs:404-451`): designated root/head presence, root semantics, map key/node identity, parent references, one rooted parentless node, cycle refusal and nonroot Genesis refusal. Traversal is iterative and memoized. `checkout` validates both paths before applying/inverting edits and bounds each ancestry walk by map size (`history.rs:475-539`). Full validation belongs at admission; unrelated branches are not rescanned during every scrub.

`integrate` validates a temporary combined graph before publication (`history.rs:547-570`). It deliberately accepts valid partial node sets whose omitted ancestors exist locally. The fragment itself remains invalid as a durable complete snapshot. This distinction is covered by `integrate_accepts_fragments_using_local_ancestors_without_moving_head` and `invalid_fragment_union_is_atomic_even_with_a_valid_new_node` (`history.rs:914-968`).

`ProjectBundle::validate`, encode and decode all enforce schema and graph validity (`persistence.rs:90-115`). `ProjectStore::save` validates before any backend operation (`project_store.rs:127-133`), and load decodes/validates before materialization (`project_store.rs:201-229`). Invalid save/load tests compare archive bytes and retained keys (`project_store.rs:330-364`).

Signed handoff create validates before signing (`handoff.rs:158-165`); receive validates the authenticated bundle after signature/address checks (`handoff.rs:233-270`); same-session acceptance validates both local/incoming bundles before temporary graph union/checkout/media import (`handoff.rs:309-385`). The hostile-peer test actually re-signs malformed graphs, proving that refusal occurs after authentication rather than merely because tampering invalidates a signature (`handoff.rs:458-491`). Additional cases verify malformed local graphs and missing public media are refused atomically (`handoff.rs:495-526`). Host direct adoption is separately covered by `host_rejects_invalid_loaded_and_direct_handoff_replacements_without_consuming_payloads` (`native_lifecycle_tests.rs:128-156`).

No new domain authority, reconstruction layer, scheduler or recorder mutation is introduced.

## Current source versus earlier receipts

The SHA-256 comparison ledger is [recheck.json](recheck.json).

The current model history/persistence, portable handoff/project-store, host main close-hook file and worker-correlation regressions match the October 16 and 17 preservation receipt hashes exactly. Current `identity.rs` and `native_lifecycle_tests.rs` match the later H3/H5 receipt exactly. Compared with its implementation revision `0effba1`, current `state.rs` only removes the duplicate audio-status wrapper and current `view.rs` only removes its duplicate status span. None of the A2/A3 implementation bodies changed in that comparison.

Historical evidence remains historical: preservation integration reports 90 Hocket cases and its build; the later H3/H5 receipt reports 93 Hocket passes and two ignores, plus its separately counted evidence controls. Neither count is asserted as a new current-head result. Current dependency/lock hashes are retained in the ledger.

## Focused checks requested from integration

From the shared checkout root:

```sh
cargo test --locked -p hocket-model -p hocket-project
```

From its `ports/hocket` workspace:

```sh
cargo test --locked -p hocket-genet --bin hocket-genet
```

The model/project targets are narrow enough to exercise complete topology, valid fragment compatibility, portable archive byte preservation and authenticated handoff admission. The Hocket host target adds the actual lifecycle, worker-correlation, consent-helper and mounted close/update-hook regressions. The integration owner may narrow further to the named existing modules if shared build contention requires it.

No fresh native OS close, Save/Open dialog, update restart, capture, custody IPC, fresh-process Hocket preservation/identity reopen or physical-audio acceptance follows from this review. Updater staged-artifact identity and wider compatibility/policy decisions are outside A2/A3/A4; P5a stays with its existing owner.
