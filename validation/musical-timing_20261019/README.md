# Portable musical timing, 2026-10-19

Musical timing moved from `woodshed-core::music_playback` to
`woodshedding::timing`. Existing schedule types and timing behavior are preserved;
callers import the domain crate directly. No compatibility facade or additional
production crate was introduced.

The native `audio::music_program` adapter owns the 256-measure, 4,096-logical-event
and 4,096-sounding-contact admission limits. Capacity is checked before allocating
an output program. The portable timing module instead validates the existing
score profile, including its 1,024-measure limit. Written MusicXML, the existing
1–400 BPM range and ordinary saving are unchanged.

## Evidence

All commands used `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0`.
Exact commands and counts are in [receipt.json](receipt.json).

| Check | Passed | Evidence |
|---|---:|---|
| Complete woodshedding unit suite | 208 | [library.log](library.log) |
| woodshedding documentation examples | 5 | [library.log](library.log) |
| Standalone MusicXML consumer | 4 | [consumer.log](consumer.log) |
| Native music adapter and clock | 13 | [native.log](native.log) |
| Editor practice state | 7 | [editor.log](editor.log) |
| Consumer Clippy, warnings denied | Pass | [consumer-clippy.log](consumer-clippy.log) |

The [standalone consumer](../../examples/musicxml-timing/README.md) has its own
workspace and lockfile. Its [normal dependency tree](consumer-tree.txt) contains
only woodshedding, XML parsing, serialization and their supporting dependencies.
It reads the qualified progression and rhythmic exercise files. Tests verify
simultaneous chord tones, exact triplet onsets/durations, rests, an independent
73.5 BPM timing override, unchanged input bytes and failures without partial
output. A 257-bar MusicXML round-trip can produce portable timing.

The native tests accept exact capacity boundaries and refuse valid music beyond
each capacity. Native host tests additionally verify that a refused start leaves
the editable music intact, and that a refused update keeps the old music playing
while a later valid edit can still apply at the next bar boundary. Existing
software-output checks cover gates, rests, chord simultaneity, mixed meter,
repeat modes, sample rates, buffer partitioning, pause/resume and ownership.

[rhythm-output.txt](rhythm-output.txt) captures the small consumer reading the
rhythmic exercise at 73.5 BPM. [source-fingerprints.json](source-fingerprints.json)
identifies the domain sources, changed adapters, example and lockfiles for this
local working-tree result. The main workspace manifest and lockfile are unchanged.

Changed Rust implementation/example files passed scoped rustfmt checks and
`git diff --check` passed. A whole-workspace format check encountered existing
formatting differences outside this change; those files were left alone.
The core library's existing formatting was preserved around its one-line module
removal.

This packet qualifies the data path and native software playback. It does not
claim a new headed UI run, physical audio/MIDI run or expanded MusicXML support.
