# Genet host and platform acceptance

Historical source: [Genet host: one codebase for desktop + web (mobile downstream)](archive_docs/2026-11-03/2026-07-04_genet_host_cross_platform_plan.md).

Status (2026-11-03): **in progress; successor approved.**

## Scope and findings
Desktop migration and S5 parity cut are recorded complete; browser host, platform-specific desktop acceptance and publishing remain separate. Current host imports `cambium_genet_winit_host` at `crates/woodshed-genet/src/main.rs:57`. Original lines 9–11, 61–116.

## Work and done conditions
- Desktop: address ancestor clipping/occlusion, shell construction and geometry together, responsive scroll/selector reachability and cross-platform runs. Done when measured layouts preserve usable board, scrolling reaches controls and graph paint aligns with hit targets, supplemented by presented native captures (580–605, 690–705).
- Web: establish host input/resize/fonts/storage; OPFS or an explicitly chosen current storage contract; audio-clock output, live-mic worklet tuner, feature-detected MIDI and browser accessibility. Done when Stage interaction, click and tuner run in an actual supported browser, then deployment/PWA/license packaging are separately qualified (61–116). Recheck browser support instead of inheriting old compatibility statements.
- Release and platform: platform archive and extracted launch with source-bound evidence; native manual assistive technology and audio quality are independent (708–737).

## Deferred
Mobile shell is downstream of web deployment, not scheduled by this plan. Strophe owns stewardship of its own Xilem-derived components. Shared renderer defects belong to Genet, while app shell sizing stays Woodshed-owned.


## Findings and progress

No fresh build, test, headed, audio or hardware gate was run for this successor. Recheck open work against current source before implementation.

### Dependency relock: crates still locked at several versions (2026-11-12)

Finding, 2026-11-12. The root `Cargo.lock` was relocked onto Mere f4aef3e68's successor (mere 52d740f8) with one scoped `cargo update` naming every package held from a family repository (mere, graphshell, dramatis, eidetic, murm, genet, wgpu-graft, distillery, inker, armillary, emblem, netrender, seiche, wavicle, wgpu-cue, firewheel); every one ends at a single commit. After it, 72 crates are locked at more than one version. Three are listed here with the packages that hold each version: the family-held `getrandom` 0.3 first (pandect, personae, sporran, woodshed-web), then `windows-sys` and `libloading`, which the relock moved edges between.

The lock builds (`cargo check --locked --all-targets` on the workspace and on `validation/relationship-recipe`, Windows, including wgpu-hal's dx12 path) and passes the prek hooks (`cargo clippy --locked --all-targets`), so none of these is a conflict under D137. They are duplicates to clear when their holders move to a common version; nothing here blocks.

- **getrandom**: `0.2.17` (rand_core, redox_users); `0.3.4` (ahash, pandect, personae, rand_core, sporran, woodshed-web); `0.4.3` (crypto-common, inker, jobserver, personae, rand, tempfile, uuid, uuid-rng-internal, woodshed-genet, woodshed-web)
- **windows-sys**: `0.45.0` (jni); `0.48.0` (dirs-sys); `0.52.0` (android-build, winit); `0.59.0` (cap-fs-ext, cap-primitives, fs-set-times, io-extras, rustix, winx); `0.61.2` (async-io, async-signal, bevy_platform, cambium-desktop, dbus, dlopen2, errno, light-file-dialog, mio, polling, rfd, rustix, socket2, stacker, tempfile, tokio, uds_windows, wasmtime, wasmtime-internal-cache, wasmtime-internal-fiber, wasmtime-internal-jit-icache-coherence, wasmtime-wasi, winapi-util, zbus)
- **libloading**: `0.7.4` (dlib, jni); `0.8.9` (ash, khronos-egl, wgpu-hal, x11rb)

69 more crates are locked at several versions; the full list with holders is in `Code/testing/engine-split/p4/woodshed_duplicates_2026-11-12.md`.

## Closure carry-forward check (2026-11-03)

Completed Cambium desktop-host migration does not close desktop matrix, accessibility, narrow layout, web or mobile gates. Preservation D1 supersedes the historical settings split; do not reinstate singleton paths or plaintext migration from that historical record. Original Closure carry-forward.

## Prototype web and mobile intent

Browser/mobile reach and accessible custom widgets remain product interests; keep audio I/O, OPFS/storage, time/scheduling, available Web MIDI and browser accessibility seams explicit. Touch-screen reorder and screen-reader behavior require their own acceptance. The October 11 web ruling calls for explicit local build and publication, hosting choice later; historical GitHub deployment, Iced/Xilem and output-only tuner restrictions do not become present mandates. Reconcile earlier store/installer ideas with the current release owner. Original 2026-11-03 prototype carry.

## Coverage question for review

Which desktop, browser storage/audio/input and accessibility seams are implemented on the current supplier cohort? Browser and platform qualification remain distinct even if code is present.
