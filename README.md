# Woodshed

Woodshed is an offline-first practice toolkit for guitar and other fretted
instruments. Its catalogs stage chords, scales, arpeggios, progressions, and
exercises into an ordered practice Set; Rehearsal streams that Set as guided
practice, and the Looper repeats it for live-input recording. A fretboard,
tuner, metronome, MIDI clock, and latency calibration round out the practice
environment. The theory model supports arbitrary string counts and tunings.

<p align="center">
  <img src="assets/screenshots/stage-related-chord.png" alt="Woodshed Stage showing a dominant seventh chord, its fretboard, and related material" width="880"><br>
  <sub>Stage: inspect a playable chord across the neck, then stage related practice material.</sub>
</p>

## Status (2026-10-11)

Desktop alpha. Source builds are the supported way to try it. Local packaging
scripts produce checksummed macOS app, Linux portable, and Windows ZIP archives.
A downloadable candidate needs packaging and extracted-app validation at its
exact revision. Signing,
notarization where applicable, third-party notices, and installer work remain
part of broader release preparation.

- Migrated onto the shared Cambium desktop host
  (`cambium-genet-winit-host` in Mere) on 2026-08-09. Woodshed was
  the donor of that host and is its first consumer; the app binary shrank
  from 1728 to 211 lines of host code, and both semantic scenario receipts
  pass unchanged.
- Practice sessions are sealed at rest under a persona-derived key with
  carry across devices (2026-08-08). Startup choice and live persona switching
  are landed; creating a persona inside Woodshed remains open. Application
  settings have their own file and one-time migration (2026-08-07).
- Window frame moved onto the host seam 2026-08-10; window-chrome controls
  are named for screen readers.
- Ten root workspace crates: `woodshedding` (pure theory), `audio-primitives`
  (shared pure-std DSP), `timed-text` (portable transcript parsing),
  `woodshed-audio` (audio, pitch, MIDI, looping),
  `woodshed-core` (portable state and host seams), `woodshed-graph` (theory
  catalog as a content graph), `woodshed-views` (Cambium product views),
  `woodshed-genet` (desktop application), `woodshed-web` (unshipped browser
  host), and `woodshed-instrument` (hardware-verified smart-instrument control).
  The instrument crate is not yet connected to the product views.
- `ports/` holds the independently released Hocket, Redshank, and Ringdown
  products. Each is an embeddable nested Cargo workspace with its own lockfile,
  package versions, release tags, and validation commands. Shared audio code
  stays in the root `crates/` directory.
- Root desktop audio uses one Firewheel graph and CPAL output stream for click,
  preview, saved backing and Set loops. Prepared commands, bounded voices and
  exclusive capture buffers replace the previous callback mixer locks. The
  shared tuner/onset/capture input remains lazy; capture currently requires
  equal nominal input/output rates. See the
  [runtime implementation record](design_docs/2026-07-11_stage_set_tools_plan.md#october-11-follow-up-single-output-runtime)
  for the source-specific checks and remaining capture boundaries.

Current plans and completed records are indexed in
[design_docs/](design_docs/DOC_README.md). The release baseline is a clean
locked checkout, passing local CPU and intended-platform host checks, and a
checksummed package validated after extraction.
That baseline is not the 1.0 product proof. The 1.0 claim requires one
persisted practice flow that connects musical material, a playable instrument
route, honest practice history, and an intelligible next step.

## Appearance authoring

Open **Settings → Appearance** to select Woodshed's named themes, saved authored
copies, and presentation mode. **Edit appearance…** opens the shared Tabard
workshop. Editing a builtin makes a user copy; save the definition, choose
**Apply to Woodshed**, then **Back to Woodshed**. Unsaved work receives the same
Save/Discard/Keep editing guard on Back and native window close.

The authored library defaults to `themes.json` in Woodshed's application
configuration directory. `WOODSHED_THEME_LIBRARY` selects a separate library;
when `WOODSHED_STATE` is set, its sibling `.themes.json` isolates scenario
runs. Application settings retain the selected theme ID and mode. Existing
named-theme settings remain readable. A missing theme shows a fallback notice,
and an unreadable library disables editing without replacing the file.

Woodshed uses the shared Cambium title bar and native caption controls. The
[adoption plan](design_docs/2026-10-08_tabard_adoption_plan.md) records ownership
and validation, including the remaining fresh-window native presentation gate.

## Use

```sh
cargo run -p woodshed-genet     # the desktop app
scripts/validate-workspace.sh lockfile woodshed
scripts/validate-workspace.sh test woodshed
```

The committed manifest and lockfile resolve the Genet family from the same
revision Mere uses, without local sibling checkouts. A gitignored
`.cargo/config.toml` may redirect those sources to sibling paths for local
development, but it is optional development plumbing rather than part of the
release graph.

Run the local lockfile and scoped CPU checks independently for each workspace.
`scripts/validate-workspace.sh <lockfile|test> <workspace>` runs from a temporary
directory outside repository Cargo configuration, using the selected workspace's
toolchain or the root toolchain when the port has none. It preserves each
workspace's own manifest and lockfile. Select `woodshed`, `hocket`, `redshank`,
or `ringdown`:

| Workspace | CPU-test scope |
| --- | --- |
| Woodshed | Theory, DSP, timed text, audio, core, graph, views, portable instrument state, and desktop application tests |
| Hocket | Model, portable project/media operations, engine, desktop-host unit tests, and device-free manual CLI tests |
| Redshank | Model, storage, cache, feeds, playback, surfaces, and desktop-host unit tests |
| Ringdown | Protocol, client, BLE helpers, and probe unit tests |

Linux builds need ALSA, Fontconfig, D-Bus development headers, and `pkg-config`
(`libasound2-dev libfontconfig1-dev libdbus-1-dev pkg-config` on Ubuntu).
These commands run the suites with their default ignored-test policy; they do not
open audio/Bluetooth devices or qualify native capture, fresh-process reopen,
or hardware behavior. Those acceptance lanes retain their explicit fixtures
and device requirements.

`scripts/package-windows.ps1`, `scripts/package-macos.sh`, and
`scripts/package-linux.sh` produce checksummed platform archives. The macOS
archive contains an unsigned, architecture-specific app; Linux packaging is
portable archive packaging, with host library requirements. See the
[release baseline and candidate assessment](design_docs/2026-07-11_stage_set_tools_plan.md#release-baseline-and-10-product-proof)
for the remaining validation and catalog roadmap.

Release tags are product-qualified: `woodshed-v*`, `hocket-v*`, `redshank-v*`,
and `ringdown-v*`. Packaging is an explicit local command with a version
argument; a port's tag does not admit a Woodshed package or change a port's
independently managed updater feed. This repository has no GitHub Actions
workflows. Qualified changes are committed and pushed directly to `main`;
pull requests are used only when the maintainer asks for one.

## Portable fretboard recipes

`woodshed_core::fretboard_scene::stage_fretboard_scene(&stage, reading_id)`
exports the current resolved Scale or Chord lens as the shared `SceneInput`.
It reads the real tuning and `StageState::dots()` without changing the Stage,
audio transport or retained material. Displayed strings are reversed into the
shared highest-first convention while occurrence keys retain the original
string/fret identity. Pitch-class resources remain shared across octaves;
third/fifth roles preserve the source's degree spelling, including altered
fifths. Unresolved transport/exercise lenses are refused.

An explicit command-line Stage uses the same model and catalog:

```sh
cargo run --features portable-scenes -p woodshed-core --bin woodshed-fretboard-scene -- \
  C Major 0 5 fretboard.scene.json
```

The arguments are root name, exact chord catalog name, tuning catalog index,
highest fret and a new output path. The output imports into Graphshell's
Portable scenes or another shared-recipe host. This adds the model adapter and
file exporter; existing native fretboard controls keep their current renderer.

This branch requires Mere's portable-backdrop implementation. Until the shared
change is integrated, use
`python3 /path/to/mere/support/design-studies/backdrops/with_local_scene_source.py /path/to/mere test -p woodshed-core --features portable-scenes --lib fretboard_scene`.
The temporary patch keeps the scene contract, compiler, authoring and timing
crates in one cohort without committing local dependency paths.

## Portable progression templates

With `portable-scenes`, `progression_scene::progression_scene(&stage, &song,
reading_id)` presents the selected progression's relative roles and a supplied
`SongDoc` arrangement separately. Each role aligns by ordinal to one song
block; blocks retain their beats, bar lengths, tempo disclosure and rests.
Relative roles survive transposition while concrete chord identities change.
The function borrows both sources and performs no playback or harmonic analysis.

```sh
cargo run --features portable-scenes -p woodshed-core --bin woodshed-progression-scene -- \
  C 'ii-V-I (Jazz)' 100 1,1,2 progression.scene.json
```

The block lengths above give ii and V one bar each, and I two bars. Graphshell's
Portable scenes includes exports of this arrangement in C and G alongside
metre. The shared template owns lanes, nested groups, tick spans, expected
slots and supplied occurrence alignments. Woodshed owns the chord meaning.
The adapter currently requires one song block per role; arbitrary role-to-song
mapping and native progression panels remain separate work. Use the local
scene-cohort helper above until the shared API is integrated and repinned.

## License

MPL-2.0. See [LICENSE](LICENSE).

---

*This README was generated by AI and will be edited by the author upon
release.*
