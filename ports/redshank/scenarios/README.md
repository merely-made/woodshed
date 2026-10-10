# Redshank scenarios

Self-drive receipts for the GUI. Each `.scn` uses Taproot grammar
(`repos/genet/components/taproot/scenario.rs`). Mesquite owns frame driving,
deferred selector clicks, captures and completion; `desktop/src/scenario.rs`
keeps Redshank commands, observations and asynchronous quiescence. Captures are
readbacks of the presented frame and do not depend on the foreground window.

The migration acceptance runner uses explicit isolated listener data, retains
receipts, and owns only its launched processes:

```powershell
../scripts/mesquite-receipt.ps1
```

The older external artboard runner remains available for existing workflows;
inspect its process and cleanup behavior before using it in a shared workspace.
Its historical invocation is:

```
C:\Users\mark_\Code\testing\woodshed\redshank-run-scenario.ps1 `
    -Scenario listen_playing -Fixture local-playing
```

Widths live in the driver, never in a scenario: `-Width`/`-Height`, or
`-Matrix` for the four artboard sizes (960x640 wide, 720x640 narrow,
412x892 phone, 820x560 rail).

## Verbs beyond the generic grammar

`act subscribe:<URL>` uses the shipping feed worker and generation store.
`../scripts/subscribe-podcasts.ps1` adds the five public podcast feeds to the
actual listener directory and records native and durable-model checks. It
preserves existing data; use `-DataDirectory` to select another listener store.
The feed size limit is configurable in Settings (8 MiB default, 1-32 MiB);
transcript reads retain their separate 4 MiB bound.

| Verb | What it does |
|---|---|
| `act <label>` | One named command — see the table below |
| `pointer-click <x> <y>` | A logical-space click, for a target the selector layout disagrees about |
| `ui-zoom <scale>` | Interface zoom from 0.5 to 4 (the 200 % / 400 % artboards) |
| `key <name>` | The shipping shortcut table: `space`, `left`, `right`, `enter`, `escape`, or one character (`n`, `r`, `o`) |

## `act` vocabulary

`play`, `pause`, `replay`, `skip-back`, `skip-forward`, `text-note`,
`text-note-begin`, `text-note-cancel`, `voice-begin`, `voice-finish`,
`voice-cancel`, `export-notes`, `tab:listen|library|notes|mere|settings`,
`scene:chain|orrery|trail`, `layout:dock|rail`, `notes:this|all`,
`seed:wetland|brand`, `mode:dark|light|hc-dark|hc-light`, `seek:<ms>`,
`select:<item-id>`, `feed:<url>` (bare `feed:` clears the selection).

## Snapshot fields an `assert snap` can read

`tab`, `transport`, `position-ms`, `duration-ms`, `buffered-percent`,
`recording`, `queue-size`, `note-count`, `marker-count`, `layout`, `scene`,
`seed`, `mode`, `item-count`, `feed-count`, `session-count`, `rate-percent`,
`volume-percent`, `voice-available`, `notes-filter`, `selected-feed`,
`item-id`, `item-title`, `text-capture`, `notice`, `unavailable`, `focused`.

## Events an `assert event` can match

`tab <name>`, `transport <WORD>`, `position <seconds>`, `recording <bool>`,
`queue-size <n>`, `note-count <n>`, `layout <name>`, `scene <name>`,
`seed <name>`, `mode <name>`, plus the loop's own `interaction-missed …` and
`wait-timeout …`.


## Shared appearance acceptance

`tabard_authoring.scn` starts from the exact Wetland fallback, opens the shared
Tabard editor, refuses unsaved Apply, saves a user copy, explicitly applies it,
and captures all four modes. `tabard_reopen.scn` loads the final saved mode in
a separate process. These scenarios use an empty listener profile and never
request playback, recording or a feed. Always isolate all three app paths:

```sh
python3 /absolute/mere/scripts/run_macos_scenario.py \
  --binary /absolute/redshank/target/debug/redshank-desktop --prefix REDSHANK \
  --scenario /absolute/redshank/scenarios/tabard_authoring.scn \
  --output /tmp/redshank-tabard-authoring \
  --env REDSHANK_DATA_DIR=/tmp/redshank-tabard-profile/listener \
  --env REDSHANK_APPEARANCE_STORE=/tmp/redshank-tabard-profile/appearance.json \
  --env REDSHANK_THEME_LIBRARY=/tmp/redshank-tabard-profile/themes.json
```

Repeat with `tabard_reopen.scn`, a new receipt directory and the same profile.
For explicit authored roles, copy `fixtures/tabard_role_library.json` into a
separate isolated theme library and run `tabard_authored_roles.scn`, followed
by `tabard_authored_reopen.scn` in a fresh process using that same profile.
These captures cover exact authored Dark roles and a custom Garden mode.
For a narrow editor run, use a separate fresh profile with `REDSHANK_WIDTH=420`
and `REDSHANK_HEIGHT=900`. A successful launcher alone is insufficient: require
`RESULT ok`, presentations, zero blank captures, reviewed PNGs and unchanged
listener state. The completed macOS qualification and exact image/receipt hashes
are recorded in [the acceptance ledger](validation/tabard_appearance_acceptance.json).
Earlier rejected receipts remain preserved; the final 420-pixel pair uses the
shared header repair. Mode controls use `data-appearance-mode` identities so
a Light mode cannot select the separate built-in Light theme.
