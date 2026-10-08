# Woodshed shared Tabard adoption — 2026-10-08

The implementation uses published Mere
`7d2a5f3cfe97174058368073d2bae809fffbf902`, matching Genet
`965b64e206a47d1c8808472de9aa461233638768`, and netrender
`9607d16f1907f6c2085648ae96abcaa30d7c3d41`.
[sources.txt](sources.txt) records the resolved native production closure:
536 packages, one identity for each shared stack crate, no external local-path
dependencies. Resolver counts describe source identity, not rendering speed.

## Passing qualification

Commands were run from the Woodshed workspace with `--locked`. Native commands
used `CARGO_TARGET_DIR=/Users/markik/Code/targets/woodshed-tabard`; the wasm
check used the separate `woodshed-tabard-web` target directory.

| Check | Command | Result / raw evidence |
| --- | --- | --- |
| Whole workspace before input deferral | `cargo test --workspace --locked` | 828 passed, zero failed or ignored; [workspace-tests.log](workspace-tests.log) |
| Final native package | `cargo test -p woodshed-genet --locked` | 79 passed, zero failed or ignored; [native-tests.log](native-tests.log) |
| Input request routes | `cargo test -p woodshed-genet audio:: --locked` | Five new device-free tests passed; [lazy-input-tests.log](lazy-input-tests.log) |
| Final native Clippy | `cargo clippy -p woodshed-genet --all-targets --no-deps --locked` | Succeeded with inherited diagnostics; [native-clippy.log](native-clippy.log), [classification](native-clippy-classification.txt) |
| Native binary | `cargo build -p woodshed-genet --locked` | Succeeded; [native-build.log](native-build.log) |
| Web compatibility | `cargo check -p woodshed-web --target wasm32-unknown-unknown --locked` | Succeeded; [wasm-check.log](wasm-check.log) |

The whole-workspace log precedes the desktop-only lazy input correction. The
final 79-test native run includes that correction and its five new tests. The
earlier combined core/views/native Clippy log is preserved as
[before-persona-test-cleanup-clippy.log](before-persona-test-cleanup-clippy.log);
its new test-only default-box warning was fixed before the final native run.
The final classification checks every remaining native diagnostic against the
baseline statements. Scoped formatting and `git diff --check` pass; existing
unformatted scenario fixture code outside the added snapshot fields was kept.

Before pushing, remote main had advanced with audio-preview commits `846c386`
and `e573072`. They were merged cleanly in `49eef4e`. The combined source passed
`cargo test -p woodshed-audio -p woodshed-genet --locked --offline`: **127 audio
tests and 79 native tests passed**, zero failed or ignored. The locked cache
was used after waiting for another build's package-cache lock. The
[integration log](push-integration-tests.log) records this final source check;
the historical failed native attempt below is preserved separately from the
subsequent passing visual acceptance.

Retained-host tests cover typed workshop fields, Save/Apply/Back, failure and
close guards, repeated embedding, persona restoration, corrupt libraries and
export cancellation. Title-bar composition is checked at wide and narrow
logical widths. These tests do not establish native visual acceptance.

## Passing macOS native visual acceptance

The [LaunchServices matrix](visual/README.md) qualifies the integrated production
binary from `2fa89ca61d75a9a03bf5e301eb658e696d74e44f` on macOS 15.8.1, Intel.
Wide and narrow seed/reopen runs all pass: **132 actual presentations, 14
nonblank captures**. The extended fixture resizes the same native window and
reveals the shared reader and selected graph through normal host scrolling.
All 14 images were inspected, with an independent second review. The earlier
background launch failure is superseded for this launch path, not erased.

## Historical background launch: no native presentation

The rebuilt binary ran `scenarios/tabard_appearance.scn` at 1180×800, with
`CAMBIUM_HOST_FRAME_TRACE=1`, a 45-second independent process deadline, and
scratch paths for `WOODSHED_STATE`, `WOODSHED_SETTINGS`,
`WOODSHED_THEME_LIBRARY` and `XDG_DATA_HOME`. A test-only
`PERSONAE_PASSPHRASE` and `PERSONAE_PROFILE=tabard-acceptance` isolated the
identity vault. `WOODSHED_CAPTURE_DIR` and `WOODSHED_RECEIPT` named this
receipt directory. No real practice profile was used.

The process reached native redraws and completed in 17.223 seconds. The
[native receipt](native.done) reports **RESULT fail: 147 redraws, zero
presented frames and zero captures**, after the independent 10-second
presentation stall deadline. The process itself exited zero; its exit code is
not sufficient to infer scenario success. [native.log](native.log) records
Metal refusing acquisition as `Occluded`, with the owned window visible and
not minimized, but inactive and without the visible occlusion bit.
[native-process.json](native-process.json) records process provenance and the
binary SHA-256; it is a runner observation, not a generated Mesquite receipt.

`tabard_appearance_reopen.scn` was not run: the preceding scenario did not
advance, save or apply an authored theme. There are no native images to inspect.
That attempt does not establish resize acceptance. The later matrix above does;
live accessibility and Windows/Linux headed acceptance remain open.

Before input deferral, the owned Woodshed process waited in microphone
initialization and never reached the scenario lane. Its [45-second run](before-input-fix-process.json)
and [sampled run](before-input-fix-sampled-process.json) are distinct evidence.
The [process sample](before-input-fix.sample.txt) identifies the main-thread
path through `CpalBackend::new`, `InputEngineBuilder::build` and CoreAudio.
After deferral, Woodshed gets past that startup wait; the compositor failure
above was the next native gate, subsequently qualified through normal macOS
application launching as recorded in the visual matrix.
