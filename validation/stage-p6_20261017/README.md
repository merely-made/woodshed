# P6 guided Rehearsal — 2026-10-17

Qualified bounded macOS runner on committed source
`d955f5dc3b99ec1efd6ec9a56dd3ba007755409e`, following preservation integration
`cec221348154fc8e6abe3f6136f00b14f9860b78`. The main source, views, audio/core
semantics and scenarios are committed before native acceptance. This evidence
commit changes documentation and artifacts only. Supplier locks are unchanged.

Rehearsal now shows previous/current/next context and Card progress from the
accepted output clock. Off, Set and focused Card looping share the existing
transport. Automatic Card repetition and explicit Next have distinct meaning;
Manual continues to wait for Next. Pause survives repeated navigation and direct
Card selection. An edit stops the old source; a fresh explicit Run admits the
corrected material. Shared Card editing and Tools stay available.

The final affected gate contains **910 distinct passing cases**, five separate
capture child invocations and seven existing ignores. `integrated-final` covers
176 audio, 285 core unit, one Card-loop integration, five CLI, one portability,
204 desktop and 237 view cases. The guard correction adds one case; the whole
desktop rerun passes 205. Repeated tests are not summed into the total.
`candidate-build` and `final-fixture-check` pass after the final shipping fixture
additions. Earlier failed compile/test-fixture runs are retained and excluded.

| Native phase | Presented frames | Reviewed captures | Result |
| --- | ---: | ---: | --- |
| Mixed Set seed | 294 | 6 | passed |
| Separate fresh process | 111 | 2 | passed |

The real 48 kHz output run covers mixed chord/scale/arpeggio timing, Set and Card
loops, paused Next and direct selection, Tools, explicit Stop, material-edit
recovery, Manual waiting and keyboard operation. Wide 1180×1000 and compact
420×1100 logical viewports are captured at zoom 1 and 2× raster scale. The actual
sealed Set (including all Cards, order, cursor and Card loop), owner and all
**22 history rows** reopen exactly. Playback reopens stopped. The seed snapshot
is copied only after its process exits, and original seed hashes remain intact.

All eight captures received independent visual review. The coordinator reviewed
critical captures, source/guard routing and runtime/byte checks separately. The
compact view can require scrolling to reach a third filmstrip Card; current
context remains clear. The old edit notice asks to stop when the edit has already
stopped the run; Run is available. Those copy/layout refinements, three-width
whole-product acceptance, touch and reduced motion remain P8. Physical audibility,
latency, microphone capture and other operating systems are not claimed.

The [preservation native receipt](../preservation-input_20261017/native/README.md)
is separately scoped. It contains seven historical `cec2213` persona/reopen
captures and five current `d955f5d` close/reopen captures. Its two assertion-syntax
fixture attempts are explicitly excluded; the corrected reopen passes.

`source.json` and `source-hashes.json` bind 331 tracked Rust/manifest/lock paths
to historical Git. `receipt.json`, `comparison.json`, process exits, checkpoint
JSON, raw native/test logs and PNGs record the result. `sha256.json` binds every
published artifact except itself. Isolated Keeper/profile state and frozen
executables remain in `/Users/markik/Code/testing/woodshed/stage-p6-20261017`;
no vaults, keys or sealed profile blobs are published. All owned processes exited;
the four original Keeper fixture files remained unchanged.

Run the read-only historical verifier from any directory:

```sh
python3 validation/stage-p6_20261017/verify-native.py
```

Raw Cargo/native frame logs retain their original trailing whitespace and blank
EOF lines. Non-log source, docs and structured artifacts pass whitespace checks.
Publication is established separately by Git refs and the coordinating report.
