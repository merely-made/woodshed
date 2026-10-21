# Song silence lane

Workspace: `/Users/markik/Code/repos/woodshed`
Baseline: `6e6498af7927a7a821f7c04215dddcaf0ddc25af` (published `main`)
Owned source: `crates/woodshed-core/src/song.rs`

## API decision

`SongBar` gains `silent: bool` with `#[serde(default)]` and `is_silent()`. The method returns true when explicitly muted or when `root_freq_hz` is not positive, matching prior host admission (`root_freq_hz > 0.0`) even for custom bars with empty formula/display strings. Toggling preserves all payload fields. Legacy no-root rests remain rests and are not replaced with Major. Chord edits while explicitly muted retain the marker.

## Commands and exits

- `rustfmt --edition 2024 crates/woodshed-core/src/song.rs` — exit 0; this was a direct single-file invocation before the recursive-formatting warning. It only changed import ordering in `song.rs` beyond the intended edits; that file is owned here and its substantive changes were preserved.
- `cargo test -p woodshed-core song::tests --lib --locked` — exit 101. Compilation reported missing `silent` in the out-of-lane `SongBar` literal at `crates/woodshed-core/src/session_overview.rs:828`; it also reported an unrelated `From<String>` conversion error at `crates/woodshed-core/src/lib.rs:2017` in concurrently modified code. The song constructor in this lane was then updated with `silent: false`.
- `rustfmt --edition 2024 --config skip_children=true crates/woodshed-core/src/song.rs` — exit 0 after the test correction.
- `git diff --check -- crates/woodshed-core/src/song.rs` — exit 0.

The focused test command must be rerun after parent-owned caller fixes and resolution of the independent core compile error. No native apps, audio devices, residents, or native validation scenarios were launched.

## Source hashes (SHA-256, after lane edits)

- `crates/woodshed-core/src/song.rs`: `c1a8ea97a41bab998464c42477233c4a91a2cfb1e6efeb01c717021755221e79`
- `crates/woodshed-genet/src/audio.rs`: `797cd49d2e63bf2b5e09071cbcae88783201082f1189eb803fb66ce93d0f69ae`
- `crates/woodshed-core/src/progression_scene.rs`: `4f4c12b27fa69af7b753d161d7ce28cabb680a3fd106a1515a52b6fd7818b721`
- `crates/woodshed-views/src/stage/looper.rs`: `05032334db44b592155cb61f9e5d95e74cd7b5aa64d8415383aa0613a283eb20`
- `crates/woodshed-views/src/stage/looper/recorder.rs`: `64a1273a0f0fd32cd6d37aef04ce6296335ad08c0773f1efc1836bf4c58184fa`
- `crates/woodshed-core/src/session_overview.rs`: `39c50f0311d216a90bd7cb878e6d27da38f342d9695eb53258252cc044e634ad`
- `crates/woodshed-genet/src/scenario.rs`: `5915ed0baee01630e5e50e6d5d63f5a2dc8c826e88c9da245a7291ee61132c3b`
- `crates/woodshed-views/src/stage/overview.rs`: `ae4a4955df3340686a49ee8f5cceed49d6f078428e96d93a1bb29543fc0904ad`
- `crates/woodshed-core/src/lib.rs` (concurrent, out of lane): `ba0aa1ce99f4a1facb71e713e5dcb2ddf1cdfcae8346316c53aa3627c9eedbe1`

## Final rerun and hashes

After parent-owned struct-literal/projection edits landed and the independent core compile error was resolved:

- `cargo test -p woodshed-core song::tests --lib --locked` — exit 0; 6 passed, 0 failed, 287 filtered.
- `git diff --check -- crates/woodshed-core/src/song.rs` — exit 0.
- `rustfmt --edition 2024 --config skip_children=true crates/woodshed-core/src/song.rs` — exit 0.

Final SHA-256 values at the pre-commit snapshot:

- `crates/woodshed-core/src/song.rs`: `997212cde8e1f5c88468aed52dcbe8627ecd4020893863ebfc2a76a0cc755abe`
- `crates/woodshed-genet/src/audio.rs`: `d21aaa87d7ab3dec281d0e39bf55f70ee23997528829832bbc3b1a62cd3c5165`
- `crates/woodshed-core/src/progression_scene.rs`: `ebe76838920fbb3ed303a980691df2be5dbaafbe0ac0da7b6cfc2895e50a04a3`
- `crates/woodshed-views/src/stage/looper.rs`: `bf66e446120ac54420b9e3fe31ac72e7105417b54a19e367b3702285ebc509ff`
- `crates/woodshed-views/src/stage/looper/recorder.rs`: `afbcfbf7982d12b8895a21726fed68d45bbbfc2785adc812c8fdb7cba6b832d9`
- `crates/woodshed-core/src/session_overview.rs`: `c8ab0a90fd4952740995d553836b4ce690a552dbd4441f2daaca7cbb1638c98e`
- `crates/woodshed-genet/src/scenario.rs`: `5915ed0baee01630e5e50e6d5d63f5a2dc8c826e88c9da245a7291ee61132c3b`
- `crates/woodshed-views/src/stage/overview.rs`: `ae4a4955df3340686a49ee8f5cceed49d6f078428e96d93a1bb29543fc0904ad`
- `crates/woodshed-core/src/lib.rs` (concurrent, out of lane): `3dd67972bdadc2768d323539da79158ea8416996fb3c301a5b71fb9a3756e3bf`

## Commit

Scoped local commit: `36e7ecc0e0e9e3ff8019c80204256d8f6409be3b` (`Preserve authored song voicing when muted`). Only `crates/woodshed-core/src/song.rs` was staged and committed. Its post-commit SHA-256 remains `997212cde8e1f5c88468aed52dcbe8627ecd4020893863ebfc2a76a0cc755abe`.

The shared checkout still contains unrelated/concurrent modifications in other lanes; they were left untouched and unstaged.
