# Bounded Woodshed preservation native acceptance — 2026-10-17

This extends the [CPU/build integration receipt](../integration.json). It
qualifies Woodshed preservation under actual corrupt saved-input quarantine,
without broadening Hocket, Redshank or physical-audio acceptance.

| Native source | Phase | Presented frames | Reviewed captures |
| --- | --- | ---: | ---: |
| `cec2213` | Persona Retry/Cancel/explicit discard | 191 | 6 |
| `cec2213` | Fresh quarantine reopen | 98 | 1 |
| `d955f5d` | Close Retry/Cancel/explicit discard | 139 | 4 |
| `d955f5d` | Fresh quarantine reopen | 97 | 1 |

The persona picker is the real sealed default persona in an isolated generated
Keeper fixture. Retry and Cancel retain three authored Cards; explicit Discard
and switch resets the live Set. The original corrupt saved file remains byte
identical, and a separate process reopens its quarantine with zero discarded
live Cards. `source.json` binds 328 historical source paths to `cec2213`.

The current candidate adds `guard_requested_close` for both dispatch and
frame-originated exits. A production-hook CPU regression covers retained work
and original bytes. The shipping scenario's close-admission action invokes
`prepare_close`; Retry and Cancel are actual view controls. Scenario completion
passes through the actual after-frame guard and leaves the decision window open.
After Cancel, the coordinator clicked the actual macOS close button, observed the
warning, tried Retry (unchanged), and explicitly chose Discard. The owned process
then exited and its original quarantine bytes remained unchanged. A fresh process
asserts zero Cards plus the decode quarantine and also retains its window after
scenario completion until explicit Discard. See `close/native-close-actions.json`
and `quarantine-reopen/native-close-actions.json`. Screenshots precede the final
discard; process/byte evidence establishes the final disposition.
`close-source.json` binds 331 historical paths to `d955f5d`.

All twelve qualified PNGs were visually reviewed; dialogs, retained Cards and
reopen notices are legible. Every qualified app process and both generated Keeper
processes exited. Their four original fixture files and the original quarantined
practice files retain their before hashes. Vaults, keys, state blobs and frozen
executables are not published. Raw evidence remains under
`/Users/markik/Code/testing/woodshed/preservation-20261017` and
`/Users/markik/Code/testing/woodshed/stage-p6-20261017`.

Excluded attempts remain explicit in `receipt.json` and `excluded/`: an occluded
zero-frame timeout, a picker selector/focus assumption, a persona wording
assertion, and two added reopen scripts with invalid assertion grammar. Their
receipts are failure evidence only. Corrected independent runs pass all assertions.

Hocket and Redshank have CPU/build evidence only. Deliberately held codec workers,
incoming-custody opener failure and capture replacement rejection remain
CPU-test-only coverage. No microphone, physical audibility, latency or other-OS
qualification is claimed. The broader source audit remains separately scoped.

```sh
python3 validation/preservation-input_20261017/native/verify-native.py
```

The verifier checks artifact hashes, historical source bindings, native scenario
results, image counts/dimensions, launch bindings and recorded final dispositions.
It does not recreate a live native run or infer its observations from source.
Raw frame logs retain exact original whitespace. Git refs establish publication
separately from these immutable qualification facts.
