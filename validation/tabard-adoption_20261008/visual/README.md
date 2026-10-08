# Woodshed native visual acceptance — 2026-10-08

**Accepted on macOS 15.8.1, x86_64, device scale 2.** All four runs report
`RESULT ok`; all 14 PNGs are nonblank and were visually inspected, including an
independent second review. These are real native GPU readbacks after successful
presentations, using the production host and renderer.

Source: Woodshed `2fa89ca61d75a9a03bf5e301eb658e696d74e44f`, with the extended
`scenarios/tabard_appearance.scn` fixture in this acceptance change. The locked,
offline build uses published Mere `7d2a5f3cfe97174058368073d2bae809fffbf902`.
Binary SHA-256:
`c84a26221fb4da8e453f896df173fa4a56c0bf38195bff1dcee44bfa34a0e25f`.

| Lane | Initial logical size | Presentations | Redraws | Captures | Blank |
| --- | --- | ---: | ---: | ---: | ---: |
| [Wide seed](wide-seed/scenario.done) | 1180×800, resized to 640×800 | 52 | 54 | 6 | 0 |
| [Wide fresh-process reopen](wide-reopen/scenario.done) | 1180×800 | 14 | 16 | 1 | 0 |
| [Narrow seed](narrow-seed/scenario.done) | 640×800 | 52 | 54 | 6 | 0 |
| [Narrow fresh-process reopen](narrow-reopen/scenario.done) | 640×800 | 14 | 17 | 1 | 0 |

Each width uses a fresh temporary practice/settings/theme library and identity
vault. Reopen uses the same files in a separate process. The files' hashes are
retained in each lane's `saved-profile-hashes.json`; private profile contents
are not copied. Capture dimensions are doubled by the display's device scale.

Normal macOS application launching is the qualification requirement. The
shared [LaunchServices runner](https://github.com/merely-made/mere/blob/codex/tabard-boundary/scripts/run_macos_scenario.py)
copies the actual executable into a temporary unsigned `.app` and uses
`open -n -W`, forwarding scenario and scratch-profile environment variables.
No host, drawable-acquisition or occlusion bypass was used. The first two or
three unpresented startup redraws do not advance the scenarios. Both the
launcher exit and the Mesquite receipt must pass; exit zero alone is insufficient.
The historical background-child launch failure remains in the parent receipt.
This is debug-binary acceptance, not signed release-package qualification.

## Visual findings and limits

Settings shows the authored `WoodshedAcceptance` selection and High contrast
light mode before and after fresh-process reopen. The workshop's Save/Apply/Back
route returns to Settings and can open the editor again without stale close
state. Wide controls fit; narrow buttons wrap within their bounds, and the
fixed action headers remain intact. The same-window resize produces a stacked
reader, syntax and graph layout, with clear reader glyphs and paragraph wraps,
distinct syntax colors and an aligned Notes selection ring and connecting edges.

The scroll captures show the reader paragraphs and graph nodes, but the graph
card's lower help text/bottom and embedded library/CSS editor controls are outside
these Woodshed capture positions. Standalone Tabard separately captures authored
CSS. No blocking visual defect was found. Existing gold selected navigation
labels against pale Settings surfaces remain a nonblocking legibility follow-up;
this visual inspection is not a formal contrast audit.

Native OS decorations, live screen-reader/IME interaction, native chooser panels,
Windows/Linux and browser execution are outside these renderer readbacks. The
scenario verifies real control/input routes and persisted state; visual review
assesses their resulting frames.

## Reproduction

Build `cargo build -p woodshed-genet --locked --offline`. From the Woodshed root,
set `MERE_RUNNER` to the shared runner, `BINARY` to the built executable and
`EVIDENCE` to a new output parent. Then run this matrix in an interactive macOS
session. Each width needs a new scratch profile; each reopen reuses its seed.

```sh
for width in 1180 640; do
  scratch=$(mktemp -d)
  for stage in seed reopen; do
    scenario=tabard_appearance.scn
    if [ "$stage" = reopen ]; then scenario=tabard_appearance_reopen.scn; fi
    python3 "$MERE_RUNNER" --binary "$BINARY" --prefix WOODSHED \
      --scenario "scenarios/$scenario" --output "$EVIDENCE/$width-$stage" \
      --env "WOODSHED_STATE=$scratch/practice.json" \
      --env "WOODSHED_SETTINGS=$scratch/settings.json" \
      --env "WOODSHED_THEME_LIBRARY=$scratch/themes.json" \
      --env "XDG_DATA_HOME=$scratch/identity" \
      --env PERSONAE_PASSPHRASE=isolated-tabard-visual \
      --env PERSONAE_PROFILE=tabard-acceptance \
      --env CAMBIUM_HOST_FRAME_TRACE=1 \
      --env "WOODSHED_WIDTH=$width" --env WOODSHED_HEIGHT=800 || exit 1
  done
  rm -rf "$scratch"
done
```

Every lane retains complete `native.log`, `launcher.log`, `scenario.done` and
`process.json`. The JSON records runner provenance and binary/scenario hashes;
it is not a separately generated Mesquite outcome. Absolute capture paths in
raw receipts identify the original output location. All copied artifacts are
byte-identical to those originals; [artifact-sha256.json](artifact-sha256.json)
records their SHA-256 values.
