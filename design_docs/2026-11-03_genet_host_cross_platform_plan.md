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

## Closure carry-forward check (2026-11-03)

Completed Cambium desktop-host migration does not close desktop matrix, accessibility, narrow layout, web or mobile gates. Preservation D1 supersedes the historical settings split; do not reinstate singleton paths or plaintext migration from that historical record. Original Closure carry-forward.

## Prototype web and mobile intent

Browser/mobile reach and accessible custom widgets remain product interests; keep audio I/O, OPFS/storage, time/scheduling, available Web MIDI and browser accessibility seams explicit. Touch-screen reorder and screen-reader behavior require their own acceptance. The October 11 web ruling calls for explicit local build and publication, hosting choice later; historical GitHub deployment, Iced/Xilem and output-only tuner restrictions do not become present mandates. Reconcile earlier store/installer ideas with the current release owner. Original 2026-11-03 prototype carry.

## Coverage question for review

Which desktop, browser storage/audio/input and accessibility seams are implemented on the current supplier cohort? Browser and platform qualification remain distinct even if code is present.
