# Woodshed P5 shared clock qualification — 2026-10-17

Qualified source is `6f65f60f457337e2943346e8d537d5e0f21deac0`, over the normal merge of the concurrent native arrangement span work. The [receipt](receipt.json) binds the exact binary, supplier identities, CPU checks and isolated native processes.

The existing OutputRuntime processor now schedules Card dwell, musical events and click, and publishes the event position used by the Fretboard. Pause freezes phase and voice position; Resume continues. Tempo changes preserve beat position, while Seconds holds retain active duration. Manual plays once, waits, and completes only on Next. Durable Sets and history stay product-owned.

The combined gate passes **881 tests**, plus **five separate capture child invocations**, with **seven existing ignores**: 174 audio, 276 core, five CLI, one portability, 195 desktop and 230 views. The final scenario-only corrections repeat the nine desktop clock cases and desktop build. Product clock source is unchanged after the combined gate. Logs are in [checks](checks/integrated-gate.log).

Native v3 uses the real 48 kHz output, a generated isolated Keeper and a temporary sealed profile. The seed and fresh reopen both pass: 442 and 103 presented frames; four and one captures. All five captures were reviewed at 1180×900 logical size (2360×1800 pixels). Pause retains the same highlighted F and exact device-reported position through a one-second hold and a visible BPM edit; Resume keeps the epoch. The Seconds Card measures exactly **2,000 ms**. Manual holds for two seconds before visible Next. The three Cards, owner and six Started/Completed history rows reopen exactly; the transient clock starts stopped.

[Native comparison](comparison.json) records process exits, exact reopening, 207 unchanged source hashes, unchanged seed artifacts after exit and four unchanged original generated Keeper fixture files. App and Keeper processes exited. Credentials, vaults, sealed profile blobs and binaries are excluded from this publication.

Live Stage sequences, chord groups and repeated physical contacts have core/audio/desktop offline coverage. The native flow specifically qualifies Rehearsal. These results do not establish whole-product or physical-device qualification.

Native v1 is excluded because of scenario BPM-minimum and boundary-wait assumptions. V2 behavior assertions pass but its final validator used the wrong history order. Their [exclusion records](excluded/v1/reason.json) and receipts remain separate from the qualified v3 evidence.

Run `python3 validation/stage-p5_20261017/verify-native.py` from any directory to verify artifact hashes, historical Git source hashes and exact captured payload comparisons. The verifier requires no running app, profile or Keeper.

External guarded launchers and complete working artifacts remain under `/Users/markik/Code/testing/woodshed/stage-p5-20261016/`. Preservation/input audit commits `07c170b` and `860c5c1` remain isolated and unintegrated.

Raw logs preserve original bytes. Frame traces contain trailing spaces and Cargo logs contain EOF blank lines, so the unrestricted staged whitespace check exits 2. Source/docs and all non-log evidence pass; this exception does not modify or invalidate the raw process receipts.
