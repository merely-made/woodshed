# H7 source-split receipts

Baseline: `60bc853ebfb0d9c3aefced7a5d3f6b0f7f0a5a1b`. Compact summaries append
here under H7's shared 100 MB cap. Existing captures and source fixtures remain.


## 2026-11-03: H7 conformance source split

`tests/conformance.rs`: 882 -> 382 physical lines. Existing encode tests move
to `tests/conformance/encoding.rs` (510), preserving their individual encode
feature guards and their original fixture helpers. The child module is also
encode-gated. The one exact ceiling exception retires; zero remain.

- Baseline/post `cargo test --locked --no-fail-fast`: 21 passed each.
- Baseline/post `cargo test --locked --no-default-features --features decode --no-fail-fast`:
  16 passed each.
- `cargo check --locked --no-default-features --features encode --lib`: pass.
- `cargo doc --locked --no-deps`: pass.
- `prek run --all-files` and the actual commit hook: ceiling and Clippy pass.
- Structural review matches all seven old/new changed function bodies,
  including nested tone generators, with identical signatures, operations
  and literals. Include macros still resolve to the original fixture bytes.
- The pinned local `Code/testing/wavicle/tools/wvunpack.exe` is present;
  reference round-trip/size branches run rather than the missing-tool path.
  Original goldens, binaries, manifests, lock and thresholds stay unchanged.
- Canonical DOC_README is founded and the local policy's old missing-index
  note reconciled. The maintainer-reserved description remains uncreated.
- Stable `C:/t/cargo-targets/wavicle`, two jobs, locked cached inputs.
  Tests remove their own generated oracle files. Compact evidence only;
  no isolated target, Cargo home or worktree, no physical-device claim.
