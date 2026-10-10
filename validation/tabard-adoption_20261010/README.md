# Woodshed current-stack Tabard acceptance — 2026-10-10

The production Woodshed executable uses published Mere
`11236fd4f8da9b7e2ce3712b4f036fbf676b1c8d`, Genet
`7422e90613f9017e5bb790e3acb48f61776b2eda`, and the maintained renderer
`491c376cf2b01fc11132cf8f86419dec114ae032`. Full locked, offline Cargo metadata
has one Mere and one Genet identity. Woodshed's root explicitly patches the
maintained Vello triple; Cargo does not inherit that patch from Mere. The
unrenamed Vello compatibility tag remains for Sprigging's existing type bridge.
[source-closure.json](source-closure.json) records the resolved stack sources.

The locked CPU suite passes **444 tests**: 211 core, one integration, 81 native
and 151 views. [cpu-tests.log](cpu-tests.log) preserves the results. The real
production executable builds successfully; [native-build.log](native-build.log)
records the build. All four LaunchServices lanes use SHA-256
`89abee996338444d84fe16df1114caea5f03fe0d14617bc6edecbc7bdb63e53f`.

| Native lane | Presented frames | Captures | Blank | Outcome |
| --- | ---: | ---: | ---: | --- |
| 1180×800 seed, then same-window resize to 640×800 | 102 | 9 distinct | 0 | RESULT ok |
| 1180×800 fresh-process reopen | 19 | 1 | 0 | RESULT ok |
| 640×800 seed | 102 | 9 distinct | 0 | RESULT ok |
| 640×800 fresh-process reopen | 19 | 1 | 0 | RESULT ok |
| Total | 242 | 20 | 0 | All passed |

The shared `scripts/run_macos_scenario.py` runner launches a temporary application
bundle through LaunchServices, copying the production executable without a host
or renderer bypass. Each width has its own scratch practice, persona/settings,
and theme-library paths; reopening reuses that width's saved profile. The
explicit `WOODSHED_APPEARANCE_RECEIPT=1` plus scenario lane leaves audio and MIDI
devices inactive. The scenarios assert this state and unchanged empty practice,
song and transport snapshots. Private profiles and vaults are not copied here.

All 20 PNGs were visually inspected. They show the shared client title bar,
four canonical modes, exact authored role colors in the application preview and
Woodshed, Save → Apply → Back, the selected persisted authored definition,
narrow editor controls, reader glyphs, syntax spans, selected graph node and
fresh-process saved-choice reopening. The graph scroll captures show the nodes
and selection; lower graph-card content is outside those capture positions.
Startup surface occlusion recovered before scenario advancement. No failed lane
or new GPU reset occurred; the latest diagnostic remained the earlier
`Kernel_2026-10-10-014641_Q-PC.gpuRestart` throughout this serialized matrix.

This is client-area rendering and interaction evidence. It does not qualify
macOS traffic-light pixels, live accessibility/IME, native chooser panels,
Windows/Linux, or audio/MIDI hardware. Existing gold selected navigation labels
on pale settings surfaces remain a legibility follow-up. The deliberately
partial authored role sheet leaves link/code colors at their defaults in the
application preview; no formal contrast qualification is claimed.

Each lane retains `native.log`, `launcher.log`, `scenario.done`, `process.json`
and its PNGs. [acceptance.json](acceptance.json) records the reviewed matrix;
[artifact-sha256.json](artifact-sha256.json) verifies copied images. Raw receipts
retain the absolute original capture paths. October 8 receipts remain evidence
for their original binary and are not substituted for these runs.
