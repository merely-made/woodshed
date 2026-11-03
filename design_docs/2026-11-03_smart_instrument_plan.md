# Smart instrument integration

Historical source: [Smart-instrument control: Woodshed drives a HyVibe guitar](archive_docs/2026-11-03/2026-08-27_smart_instrument_plan.md).

Status (2026-11-03): **in progress; successor approved.**

## Ownership and rulings
Ringdown owns device protocol and standalone control; Woodshed owns integration, Set, recorder and transport. WD1, 2026-08-27: “both, with an explicit toggle.” WD2, same date: “hold a session, and release on demand.” Preserve context and wording at original lines 401–412. Device writes are overwritten when the vendor app reconnects; show that limit and retain Woodshed-authored state locally (418–444).

## W3 and done conditions
Expose proven Ringdown actions through a Woodshed surface. Review bank import/export/diff, resonance reporting and checksum-verified background loop retrieval; existing protocol prerequisites do not constitute a Woodshed GUI. Keep local pack audition, MIDI export and device Beats distinct, with no automatic authority switch (155–220). Done when selected supported actions retain explicit device/session authority, errors and reconnect behavior, save/reopen, and platform-specific actual device acceptance. Assess `SetSpeakerBiquads` separately; the original destructive probe finding precludes assuming it is safe.

## Deferred and limits
W1/W2 are historical hardware evidence, not updated-firmware or phase-sync qualification. Cross-desktop, timing and remote Beats remain open. Whether unsolicited device changes occur and which loop metadata fields denote bars versus beats require dedicated observation (446–460). Adapter publication and rights evidence belong to Ringdown; no private RPC release claim follows.


## Findings and progress

No fresh build, test, headed, audio or hardware gate was run for this successor. Recheck open work against current source before implementation.

Current-source pointer: `ports/ringdown/crates/ringdown-ble/src/instrument.rs:243` requests analysis; this alone establishes neither a Woodshed product surface nor fresh hardware qualification.

## Coverage question for review

Which W3 product controls, bank operations and resonance/loop actions are selected for the first Woodshed surface? Keep destructive filter operations independently assessed.
