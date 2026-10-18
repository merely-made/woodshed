# Core B1 receipt — 2026-10-18

Repository: `/Users/markik/Code/repos/woodshed`
Requested baseline: `6e6498af7927a7a821f7c04215dddcaf0ddc25af`
Concurrent local HEAD at final check: `26c599241a8bcd823e2b9846eb1f3416f7586cf4` (`Reuse capture cleanup for unreachable arms`). The intervening commits were preserved; work below is staged against the current HEAD.

## Owned source hashes

- `crates/woodshed-core/src/lib.rs` — `fc450b653f254c954d5fe7f84181b5b87cda204516a8c218b68b6a0f41a655f5`
- `crates/woodshed-core/src/practice_clock.rs` — `ccdc1ef239be1c6d777569ba181b5df7ff3f8880adc2db5c62d098c9fe609d61`
- `crates/woodshed-core/src/captured_arpeggio.rs` — `c2f1c208dc98fd4d50bd92592d4b21a3052cc8f95d18a3fbd1b55b438bb0be77`

## Checks

- `cargo check -p woodshed-core` — exit 0.
- `cargo test -p woodshed-core saved_path_uses_retained_setup_and_viewport_only_clips_dots` — exit 0, 1 passed.
- `cargo test -p woodshed-core stale_solo_marks_do_not_fall_back_to_the_full_chord` — exit 0, 1 passed.
- `cargo test -p woodshed-core set_lowering_preserves_ordered_repeated_physical_visits_and_seconds` — exit 0, 1 passed.
- `cargo test -p woodshed-core unavailable_saved_path_contacts_refuse_display_sound_and_managed_lowering` — exit 0, 1 passed.
- `cargo test -p woodshed-core path_solo_marks_share_full_ordered_contacts_outside_viewport && cargo test -p woodshed-core chord_recipe_rejects_mixed_available_and_stale_solo_marks && cargo test -p woodshed-core captured_scale_patterns_replay_exact_order_repetition_and_marked_setup` — exit 0; each selected test passed.
- `cargo test -p woodshed-core path_mute_marks_share_full_ordered_contacts_outside_viewport && cargo test -p woodshed-core path_contact_setup_supports_legacy_inheritance_and_refuses_unknown_identity && cargo test -p woodshed-core --lib` — exit 0; the final library run passed 298/298 tests.
- `git diff --check` — exit 0.
- `git diff --cached --check` — exit 0.

A first full library run after stricter chord-contact validation exposed an old test fixture marking a contact outside its saved window. The fixture now obtains a real contact from the shared resolver; the final 298-test library run passes.

## Formatting cleanup

The first direct `rustfmt` invocation on the `lib.rs` module root also formatted child modules. Its formatting-only hunks were manually reversed in `history.rs`, `loop_capture.rs`, `search.rs`, `storage.rs`, `arpeggio.rs`, `audio.rs`, `card_shapes.rs`, `chord_approach.rs`, `connected_catalog.rs`, `scale_pattern_discovery.rs`, and `scale_realization.rs`. `loop_plan.rs` was verified to have only formatting changes and restored to HEAD as requested. No edits were made to `song.rs`, `progression_scene.rs`, or `session_overview.rs` during cleanup. The shared integration edits in `lib.rs` remain unstaged and excluded from this lane commit.

## Commit

Local commit: `f0b2e61` (`Resolve retained Path and marked contacts`). It contains only `captured_arpeggio.rs`, `lib.rs`, and `practice_clock.rs`. The remaining worktree modifications shown by `git status` belong to concurrent lanes, including separate unstaged integration declarations in `lib.rs`; they were preserved and excluded.
