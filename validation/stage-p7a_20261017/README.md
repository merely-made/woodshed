# P7a mixed Bars backing — 2026-10-17

Qualified bounded macOS slice on committed source
`0fafd20a625ef915a5933678e21e43b9de48611e`, with implementation `4304a068`
and baseline `8aba700`. Supplier locks remain unchanged.

The Looper now admits ordered Scale, Scale Pattern, Path and Chord articulation
Cards with positive Bars holds. P5's Card realization supplies ordered/repeated
pitches and strum spacing; its four-quarter Bars positions scale to the explicit
Looper meter. Existing block-Chord backing, including deliberate muted silence
and once-per-measure strikes, stays on its existing rendering path. Saved
backing keeps its exact payload and compatibility path.

Phrase PCM is prepared before the callback, uses integer frame onsets, and
mixes with the existing click and captured PCM. Live playback and WAV export
use the same production renderer. Preflight checks finite pitches, event/group
counts, frame-representable order/strum, aggregate PCM and whole-snapshot render
work before admission or writer use. Capture clock metadata and Set/Card IDs
remain unchanged; captures survive reordering and source edits, and clock edits
require explicit Clear.

The final candidate gate exits zero with **925 distinct cases**, five separately
counted capture child invocations and seven existing full-gate ignores. The
breakdown is 185 audio, 288 core unit, one Card-loop integration, five CLI, one
portability, 207 desktop and 238 view cases. The desktop build passes. A separate
invocation of the normally ignored sealed-profile WAV reference helper passes;
it is counted separately. Earlier fixture failures and repeated checks are
retained without inflating the total.

| Qualified native phase | Presented frames | Reviewed captures |
| --- | ---: | ---: |
| Mixed backing seed | 228 | 6 |
| Fresh sealed reopen | 124 | 2 |

The real native output run covers mixed 3/4 backing with distinct Card tempos,
actual device-observed playback, a deliberately synthetic take on Scale Card 2,
reordering that Card to row zero, readiness refusal retaining loaded backing,
Bars recovery, export, exact saved-backing compatibility, and wide/compact views.
Input stays unopened. Checkpoints compare the backend's actual admitted plan and
output Song fields/events to the UI, then compare live Set/owner/Looper/saved
SongDoc/manifest/decoded take bits against actual sealed storage.

The three Cards and full retained payload reopen exactly, with playback stopped.
The 96,000-frame Scale take retains Set 1 / Card 2 and exact decoded bits. Actual
seed and reopen WAV worker files are **byte-identical**, and both equal a
separate offline render loaded from the sealed profile. The reference probe
leaves all profile file hashes unchanged. Each WAV is mono float32, 48 kHz,
160,800 frames, **3.350 seconds**, SHA-256:

`c802d565f9e4bda9a96bdb4d8cca6cb5eb2e02c2fdab1f47845d930ca74e26d1`

All eight qualified PNGs received independent visual review; the coordinator
reviewed source, tests, runtime/byte evidence and critical captures separately.
The compact fixed recorder dock covers part of the lower scrollable Card list;
core controls/status remain visible. Full responsive/touch/reduced-motion
acceptance stays P8. Preparation is synchronous bounded control work; no latency
qualification is claimed.

This slice keeps Manual, Seconds, Reps and eventless exercise Cards as explicit
readiness failures. Full saved-backing migration/removal and saved-song capture
persistence remain P7 work. No physical audibility, microphone/drift, other-OS
qualification or new native dialog-matrix claim follows from these fixtures.

`source.json` and `source-hashes.json` bind 334 Rust/manifest/lock files to the
historical Git revision. `receipt.json`, checkpoints, raw checks/native traces,
PNGs, worker WAVs and the independent reference record the outcome. `sha256.json`
binds all published artifacts except itself. Excluded native-v1/v2 failed
fixture attempts remain explicit failure evidence. Their apps and Keepers exited;
all qualified apps and the generated Keeper also exited, with its four original
fixture files unchanged. Keys, vaults, sealed profile blobs and executables remain
outside the repository under
`/Users/markik/Code/testing/woodshed/stage-p7a-20261017`.

```sh
python3 validation/stage-p7a_20261017/verify-native.py
```

The read-only verifier checks artifact hashes, historical source, actual command
exits, native results/captures, exact reopening and WAV bytes/frame format.
It does not recreate a native run. Raw Cargo/native logs retain original emitted
whitespace; non-log source/docs/structured artifacts pass whitespace checks.
Git refs establish publication separately from these qualification facts.
