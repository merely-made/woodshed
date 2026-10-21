# Basic drums

One bar of kick, snare and closed hi-hat at 100 BPM. The manifest is an
editable example of audio-pack format 1. Samples are original synthetic
16-bit mono WAVs at 48 kHz; `generate.py` is their source. Manifest, generator
and samples use the repository's MPL-2.0 license. No third-party samples are
bundled.

Copy this directory anywhere to install your own pack. Edit `audio-pack.json`
and keep the referenced `samples/` files beside it. Give a changed kit its own
`id`/`version`; exact source and asset bytes also determine its retained digest.
Use one `audio-pack.json` **or** `audio-pack.toml`, never both. TOML and JSON
decode the same definition; they retain their original bytes and format.

The `patterns` object contains the existing `SequencerPattern` representation.
Each track has one bar of `Empty` or `Active { accent }` steps and references a
declared sample voice. A subdivision counts divisions of each notated beat;
BPM counts quarter notes. MIDI channels are numbered 1–16. Note assignments
are General MIDI percussion choices; they are not a qualified HYVIBE map.

`parameters` describes the supported sample level, accent level and tuning
controls. Each track's sound holds their current values. Format 1 uses a
fixed mono sample-to-output route; a patch graph or executable extension is
not part of this format. Required capabilities are explicit.

From the repository root, export MIDI and a self-contained snapshot to new
paths (existing files are preserved):

```sh
cargo run -p audio-packs --bin audio-pack -- bundled backbeat /tmp/basic.mid /tmp/basic.rhythm.json
cargo run -p audio-packs --bin audio-pack -- /path/to/your-kit backbeat /tmp/custom.mid
cargo run -p audio-packs --bin audio-pack -- /tmp/basic.rhythm.json backbeat /tmp/reopened.mid
```

The file workflow has no CPAL, Bluetooth, Firewheel or GUI dependency.
The saved selection contains the editable pattern plus its snapshot's original
manifest and referenced WAV bytes, so reopening does not need the installation
folder. AP1 snapshot-only files also remain readable. Changed or corrupt base
snapshots are rejected when their identities are revalidated.

Render through Woodshed's existing production Firewheel graph, without opening
a device:

```sh
cargo run -p woodshed-audio --example drum_pack -- bundled backbeat /tmp/basic.wav
cargo run -p woodshed-audio --example drum_pack -- /path/to/your-kit backbeat /tmp/custom.wav
```

Woodshed's Metronome view and Hocket use the same kit/step controls. Both use
their existing tempo and saves. In Hocket, arm a track and add a drum loop to
audition it through ordinary track playback; recorder Undo removes that layer.
The kit draft is saved as local project metadata; handoff carries rendered audio.
Woodshed uses the admitted kit in its existing click and backing lanes.

Ringdown's existing probe has an offline branch that shares the file workflow:

```sh
cargo run --manifest-path ports/ringdown/Cargo.toml -p ringdown-probe -- \
  --audio-pack bundled backbeat /tmp/ringdown.mid /tmp/ringdown.rhythm.json
```

This branch does not connect to the guitar. MIDI output is not a qualified
HYVIBE beat format. Mere's quantized span operation serves one-step hit moves
in the shared controls; richer rhythm scenes/routing and expressive scene
bindings remain in the canonical plan.
