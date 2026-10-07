# redshank-web

Redshank in a browser. `redshank_surfaces::surface` mounted over
`cambium-genet-web-host`, with `redshank_surfaces::sheet()` as its sheet —
the same view function and the same stylesheet the sovereign desktop draws,
rendered by the same Cambium, genet-layout, and netrender onto a canvas
instead of a window. Nothing about the interface is reimplemented here.

## What this host does

It seeds a built-in fixture (one feed, six episodes, one local item paused at
1:24 of 2:01 with two text notes and one voice note, a queue of three, three
listening sessions) and drains `CompactCommand` in `after_dispatch`. Every
command that is pure state is applied:

| applied | how |
|---|---|
| `SelectTab`, `SelectScene`, `SelectFeed`, `SetLayout`, `SetNotesFilter` | direct |
| `UpdateSettings` | settings, plus seed/mode scope class and skip intervals |
| `Enqueue`, `Dequeue`, `MoveQueue` | the in-memory queue |
| `Seek`, `SkipBackward`, `SkipForward` | `position_ms`, clamped to duration |
| `SetRate`, `SetVolume` | compact fields and settings |
| `Play`, `Pause`, `Replay` | `TransportState` only — see below |
| `BeginTextNote`, `CancelTextNote`, `SaveTextNote`, `SaveSpanNote`, `BeginEditNote`, `EditNote`, `DeleteNote` | in-memory `NoteSummary` list and seek-track markers |
| `SelectItem`, `OpenNote`, `PlaySpan` | reprojects now-playing / seeks to the anchor |
| `BeginVoiceNote`, `FinishVoiceNote`, `CancelVoiceNote` | shows and clears the recording row |

`Layout` is set from the frame hook's `ctx.logical_size` with the desktop's
rule: `Rail` when `width >= 700 && width > height * 1.15 && width < 1100`,
otherwise `Dock`. The page URL's `?seed=brand&mode=light` query picks the
opening seed and mode (`seed=brand|wetland`, `mode=dark|light|hc-dark|hc-light`).

## What this host does not do

The browser has no cpal, no file system, no microphone adapter, and no
network fetcher in this stack yet. Those commands are refused with
`state.notice = "Not available in the browser host yet: <command>"`:
`OpenLocalFile`, `CacheItem`, `RemoveCachedItem`, `RemoveLibraryItem`,
`RetryItem`, `PinItem`, `Subscribe`, `RefreshSubscription`,
`ExportAnnotations`, and voice-note audition
(`OpenVoiceNote` / `PlayVoiceNote` / `StopVoiceNote`).

Three further honesties, because a half-truth in a demo is worse than a gap:

- **There is no clock.** `Play` sets `TransportState::Playing` and posts a
  notice that the position does not advance. Only a command moves the
  playhead.
- **`FinishVoiceNote` saves nothing.** The recording row exists so the design
  can be read at that state; finishing clears it and says why.
- **Nothing persists.** A saved text note lives until the tab reloads, and
  says so in its notice.

## Building it

Build from `C:/t` by manifest path so optional local repository Cargo
configuration does not replace the qualified immutable Git sources:

```bash
cd /c/t
export CARGO_TARGET_DIR=C:/t/cargo-targets/woodshed
cargo build --manifest-path C:/Users/mark_/Code/repos/woodshed/ports/redshank/web/Cargo.toml \
    --locked --target wasm32-unknown-unknown --release -j 1
```

Then bindgen into the page's `pkg/`:

```bash
wasm-bindgen --target web --no-typescript \
    --out-dir C:/Users/mark_/Code/repos/woodshed/ports/redshank/web/www/pkg \
    C:/t/cargo-targets/woodshed/wasm32-unknown-unknown/release/redshank_web.wasm
```

The crate pins `wasm-bindgen = "=0.2.129"`, matching the current
`cambium-genet-web-host`. Use `wasm-bindgen-cli` 0.2.129 when producing the
page glue. A typed wasm32 check qualifies compilation; it does not qualify
bindgen output or a headed browser render.

Serve the page over HTTP — a module script will not load from `file:`, and
WebGPU wants a secure context, which `localhost` is:

```bash
cd C:/Users/mark_/Code/repos/woodshed/ports/redshank/web/www
python -m http.server 8080
```

Open `http://localhost:8080/`. Needs a WebGPU browser (Chrome/Edge, Safari 26,
recent Firefox). `www/pkg/` is build output and is not committed.

## Workspace wiring

This crate is a member of the port workspace (`ports/redshank/Cargo.toml`), so
it shares the workspace lockfile and builds with
`--target wasm32-unknown-unknown` from the workspace manifest. `cambium`,
`cambium-rootstock`, and `cambium-genet-web-host` are named at the workspace's
Mere pin (`57b4893db6909d5ed9c4ccae30216f0d8164201a`), aligned with Genet
`965b64e206a47d1c8808472de9aa461233638768`. The nested workspace retains the
same Genet revision for its in-process IPC patch. Woodshed root and Hocket
keep their independently qualified dependency families.

## Receipt

2026-09-14: `cargo build --target wasm32-unknown-unknown --release` finished
clean against `redshank-surfaces` mid-lane (10.9 MiB module); `wasm-bindgen
--target web --no-typescript` produced `www/pkg/redshank_web.js` exporting
`start` plus `redshank_web_bg.wasm`; `python -m http.server` served the page,
the module, and the wasm (`application/wasm`) at 200. A headed render receipt
in a WebGPU browser at phone and wide widths is Lane R's, not taken here.
