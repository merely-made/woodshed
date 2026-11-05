# Stage, Set, Tools and practice receipts

Plan: [Stage, Set, Tools and practice](../../2026-11-03_stage_set_tools_plan.md).

## 2026-11-04: Overview label

Baseline: `0ae6a96c`, uncommitted working tree. macOS, CPU tests only.

- `cargo test --locked --no-fail-fast -p woodshed-views`: lib 308 passed,
  1 failed; `tests/midi_state.rs` 5 passed; doctests 0. The failure is the
  existing `stage::overview::tests::navigation_and_graph_inspection_do_not_start_or_pause_live_processes`
  (`overview.rs:1690`, left 2, right 3), recorded before this change under
  "Overview process-count assertion"; it does not read workspace labels.
  New `workspace::tests::layout_saved_with_the_former_mere_label_restores_as_overview`
  and renamed `legacy_four_panel_snapshot_restores_exactly_then_adds_overview_on_request`
  pass.
- `cargo test --locked -p woodshed-genet layout`: 29 passed, 0 failed,
  337 filtered out. All twelve former `containing("Mere")` workspace-panel
  clicks now select `containing("Overview")`; no other panel label contains
  "Overview".
- `rustfmt --edition 2024 --check` on the two edited `workspace` sources and
  `git diff --check`: clean.
- Wider rename (Overview title, accessible graph name, notices, relationship
  Back action, Related graph arrangement setting, `scenarios/*.scn` panel
  clicks), then `cargo test --locked --no-fail-fast -p woodshed-views
  -p woodshed-genet -p woodshed-core`: woodshed-core 329 passed;
  woodshed-genet 358 passed, 8 ignored (plus five single-test binaries, each
  1 passed); woodshed-views lib 308 passed, 1 failed (the same existing
  process-count case, left 2, right 3), `tests/midi_state.rs` 5 passed.
  Scenario files were edited but not executed; native GUI acceptance of the
  label is not claimed.

## 2026-11-05: Overview process-count fixture

- `navigation_and_graph_inspection_do_not_start_or_pause_live_processes`
  fixture now reports host-observed tuner listening (admitted input,
  `tuner_active`). `cargo test --locked -q -p woodshed-views`: lib 309 passed,
  0 failed; `tests/midi_state.rs` 5 passed.
