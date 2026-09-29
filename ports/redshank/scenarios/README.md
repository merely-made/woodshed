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
