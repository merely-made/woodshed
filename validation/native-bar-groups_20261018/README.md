# Native authored bar grouping — 2026-10-18

The native Woodshed grouping workflow passes with the local Mere prototype.
The explicit synthetic 4/4 Set is the same owner-exported progression used by
Graphshell. Ordinary arrangements still receive no implicit time signature.

Native controls select consecutive bars, edit bar counts, preview/cancel on the
shared scene, resolve grouping conflicts with Keep local / Use update, and apply
in the ordinary arrangement draft. Applied grouping shares the chronological
arrangement history with fields and span gestures. Save retains that history and
updates its saved marker. Proposal text and selection are transient.

Twenty-two Graphshell editing cases, 18 Woodshed draft cases and all three
native arrangement checks pass; the native build succeeds.

Two native processes pass with 306 presented frames and seven reviewed captures
at 1180×1100 and 420×1100 logical pixels. The seed run explicitly types `1 + 3`,
checks conflict choices, undoes back to saved `2 + 2`, and returns through the
workspace panel. The new process reopens the sealed Set without resetting it.
`verify.py` compares the entire Sets and portable scene documents: preview leaves
source intact; only `bar_groups` changes; Cards, musical spans, voicings, rests,
identity, tuning, time-signature map and playback instructions remain exact.

The final binary is frozen outside the repository at
`/Users/markik/Code/output/graphshell-meter-native-20261018/native-run-v3/frozen/woodshed-genet`.
`source/source.json` records its hash and both worktree HEADs; the 920 compiler-
listed inputs are hashed in `source/source-ledger.json`. Tracked input patches
and new source inputs are included. The separate build manifest, lock and cache
config use actual Woodshed sources with the active local Mere dependency cohort;
canonical manifests and lockfiles were not repinned. The real isolated Keeper
uses the existing fixed synthetic receipt identity, never a personal profile.

Initial candidates are excluded. They exposed clicks aimed at the label rather
than the actual count textbox, a duplicate outer horizontal scroller intercepting
narrow span-button hits, and an auto-height inner viewport clipping the scene.
The final control has its own accessible textbox target, one horizontal scene
scroller, and the full declared 300px viewport height. `initial/` preserves the
failed results and a clipping capture; full intermediate files remain in the
external output directory.

This is macOS native grouping and persistence qualification for these dirty
prototype sources. Dedicated time-signature controls remain unfinished pending
explicit decisions about meter changes, group remapping, anchors and proposal
undo. Full suites, physical audio, arrangement-driven playback, canonical family
integration and publication are not claimed.
