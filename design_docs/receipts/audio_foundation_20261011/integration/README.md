# Audio foundation integration — 2026-10-11

The integration starts at Woodshed `cd69da0`. Its application source through
`900094c` includes the corrected standalone Firewheel provider
`2b03846d2e0dcc6ded2a85367bb8c5071ae99cdc`, both migrated consumers, portable
Hocket project operations and the bounded Set-backed Looper. Later receipt
and documentation commits preserve this source. The validation script's final
test scopes and relevant source/lockfiles are hashed in `summary.json`.

The four independent workspace checks run from a temporary directory, outside
repository Cargo overrides, using Rust 1.97.1 and `--locked`:

| Workspace | Passed | Explicitly ignored |
| --- | ---: | ---: |
| Woodshed, including desktop application | 870 | 1 custody receipt |
| Hocket, including portable project and manual CLI | 166 | 1 custody receipt |
| Redshank | 261 | 7 device/fixture checks |
| Ringdown | 157 | 0 |
| Total | 1,454 | 9 |

All four locked metadata checks and the scoped desktop builds pass. Logs and
source hashes are retained here. The initial CI lane separately qualifies
actionlint, shell syntax and eight real PowerShell release-tag fixture cases
in [its receipt](../ci/README.md). The provider's exact-head GitHub Actions pass
Linux/macOS/Windows standalone streams, full tests, formatting and docs;
`firewheel-pr-status-final.json` retains that historical observation. The
maintainer subsequently removed GitHub workflows and the default PR review
step. Local validation is the continuing gate; the foundation is published
directly to both repositories' main branches. No additional Actions result is
required or inferred.

Native proof is source-specific:

- [Hocket](../../../../ports/hocket/design_docs/receipts/firewheel_20261011/README.md):
  five finite processes, nineteen reviewed nonblank captures, two appearance
  reopens, and explicit device-free input refusal at source `f5e0aed`.
- [Redshank](../redshank-consumer/README.md): three finite processes, twelve
  reviewed nonblank captures and an authored-appearance reopen at source
  `7d44ab1`. Playback and microphone were not requested in these empty profiles.
- [Earlier Redshank retention](../retention/README.md): three processes prove
  bounded publication, settings reopen and recovery after corrupting both latest
  generations, at the separately recorded pre-migration source.
- Woodshed: `woodshed-rejected-locked/` retains the first failed replay. Its Mac
  was locked; surface acquisition reported `Occluded`, with zero presented
  frames. No Set native success or reopen is claimed. CPU tests independently
  use two production FsBackend/session restore processes to check saved source
  and settings, without qualifying the live keeper path.

Physical devices, microphone permission, audible capture/replay, mixed-rate
input and latency calibration remain open. The bounded Set seam does not close
P7: exact legacy saved backing remains, while full authority migration,
companion PCM, WAV export, mixed material and one output clock without callback
mutexes or recording allocations still need implementation and acceptance.
Hocket native project/handoff custody, Redshank Turnstone and its ruled repo
extraction, Windows/Linux native presentation and release publication remain
separate gates. Direct main publication follows these recorded source checks;
release tagging is a separate action.
