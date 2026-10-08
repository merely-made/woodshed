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

## Status (2026-09-30)

Desktop alpha with macOS, Linux, and Windows host CI. Source builds are the
supported way to try it. Manual workflows have produced checksummed macOS app
and Linux portable archives; the Windows tag workflow can publish a portable
ZIP. No public GitHub release is currently recorded. A downloadable candidate
needs packaging and extracted-app validation at its exact revision. Signing,
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
- Nine root workspace crates: `woodshedding` (pure theory), `audio-primitives`
  (shared pure-std DSP), `woodshed-audio` (audio, pitch, MIDI, looping),
  `woodshed-core` (portable state and host seams), `woodshed-graph` (theory
  catalog as a content graph), `woodshed-views` (Cambium product views),
  `woodshed-genet` (desktop application), `woodshed-web` (unshipped browser
  host), and `woodshed-instrument` (hardware-verified smart-instrument control).
  The instrument crate is not yet connected to the product views.
- `ports/` holds the independently released Hocket, Redshank, and Ringdown
  products. Each is an embeddable nested Cargo workspace with its own lockfile,
  package versions, release tags, and validation commands. Shared audio code
  stays in the root `crates/` directory.

Current plans and completed records are indexed in
[design_docs/](design_docs/DOC_README.md). The release baseline is a clean
locked checkout, green core and Windows-host CI, and a checksummed tagged ZIP.
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
cargo test --workspace
```

The committed manifest and lockfile resolve the Genet family from the same
revision Mere uses, without local sibling checkouts. A gitignored
`.cargo/config.toml` may redirect those sources to sibling paths for local
development, but it is optional development plumbing rather than part of the
release graph.
`scripts/package-windows.ps1`, `scripts/package-macos.sh`, and
`scripts/package-linux.sh` produce checksummed platform archives. The macOS
archive contains an unsigned, architecture-specific app; Linux packaging is
portable archive packaging, with host library requirements. See the
[release baseline and candidate assessment](design_docs/2026-07-11_stage_set_tools_plan.md#release-baseline-and-10-product-proof)
for the remaining validation and catalog roadmap.

## License

MPL-2.0. See [LICENSE](LICENSE).

---

*This README was generated by AI and will be edited by the author upon
release.*
