# Native project and handoff acceptance

Status (2026-10-16): bounded native acceptance complete at fd4663a, after coordinated lock intake.

## Feature target and done conditions

Exercise actual native project Open/Save and WAV Export dialogs and self-addressed
handoff write/receive dialogs, staged review and same-session acceptance. Reopen
the saved project in a fresh native process. Use a real isolated Keeper identity,
private settings/profile paths, deterministic PCM and the existing project worker.
No microphone, user vault or external recipient is involved.

Record scoped tests/build, exact source and lock hashes, native captures, project/
media/history identity, signed artifact verification, WAV bytes and fresh-process
reopening separately. A fixture generator may prepare a .hock and reference WAV;
scenario-only observations may inspect state, but no hook bypasses file dialogs,
worker admission or acceptance. Native audio/listening/latency are separate gates.

## Findings

2026-10-16: the old handoff_circle scenario only captures controls. Current host
commands open rfd native dialogs; the worker now correlates all five operations.
Main advanced to e4ddb67 with a coordinated Hocket lock intake; this acceptance
uses those committed pins without changing manifests or locks. A real Keeper can
provide a durable self-addressed identity; the .hocket carrier remains cleartext.

## Progress

2026-10-16: preparing deterministic file fixtures, metadata-only checkpoints,
finite native seed/reopen scenarios and isolated launchers. Retain failed probes
separately and count only the final committed-source matrix.

2026-10-16 probe findings: native Save/Open/edit/save/export completed, but the
appearance receipt mode deliberately disabled the clipboard, so recipient setup
and handoff did not execute. Added HOCKET_RECEIPT_CLIPBOARD=1 as an explicit
opt-in to the real OS clipboard while retaining device-free audio. Default
appearance receipts remain clipboard-isolated. This failed probe is excluded
from acceptance counts. Native filename edits require leaving the field before
Save; the first path-edit probe was also retained separately.

2026-10-16 final result: two committed-source native processes pass with seven
reviewed captures at 1180×980 and 960×980. Actual rfd dialogs perform initial
Save, Open, WAV export, signed handoff write and authenticated receive. Same-
session acceptance retains a divergent local history branch; the fresh process
reviews/discards/adopts a new session, saves it, then opens the original saved
project with exact bundle, history, media identity and all 48,000 PCM float bits.
Both native stereo WAV exports are byte-identical to the portable reference.
Compressed archive media and original fixtures are unchanged.

Capture review found compact toolbar label overflow during long statuses. Whole
controls now wrap rather than their labels; the complete pair was rerun after
that correction. Status labels still truncate by design. Receipt-directory
launcher and clipboard probes are excluded from final counts and retained.

Locked offline Hocket workspace tests pass 116 distinct cases, with two opt-in
tests ignored; the explicit deterministic fixture generator separately passes.
All-target checking and desktop build pass. The real isolated synthetic Keeper
was stopped after the runs. No pins, manifests, lockfiles or formats changed.
Evidence: `../../../validation/hocket-native-project_20261016/receipt.json`, with
retained external captures and reproducible metadata/file verification. This
qualifies self-addressed local project/handoff workflow only. Two-peer/network
transfer, signed per-turn operations and physical audio remain outside scope.
