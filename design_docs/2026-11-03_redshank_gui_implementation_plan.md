# Redshank GUI

Historical source: [Redshank GUI implementation plan](archive_docs/2026-11-03/2026-09-13_redshank_gui_implementation_plan.md).

Status (2026-11-03): **in progress; successor approved.**

## Scope
Implement the endorsed Redshank listening surface under the listening plan's product and data authority. Existing standalone/native appearance evidence is bounded to its actual platform and input; Turnstone owns its receiving surface.

## Work and done conditions
Recheck S2 Listen/Library/Notes/Settings and S3 Overview-tab/feed projections against current code. Redshank's tab, formerly `Mere`, is labelled Overview (`SurfaceTab::Overview`, scenario token `tab:overview`; Mark, 2026-11-05, extending Woodshed's 2026-11-04 rename). Preserve ordered actions, privacy, visible failure and reversible selection. Finish design completions and native export chooser interaction; qualify keyboard, human accessibility and a headed Turnstone receipt including microphone where authorized (original lines 164–313, 355–410, 460–520). Done when actual host interactions, save/reopen, stale/cancel playback, exact frame correlation and appropriate human assistive-technology checks are separately demonstrated.

## Deferred and limits
Speech at 1.5x needs human listening confirmation; external audio/playback fixtures, physical microphones and whole-Woodshed acceptance are distinct. Do not translate synthetic or macOS-only coverage into a cross-platform device claim.


## Findings and progress

The narrow Listen note row now keeps its already-stacked heading and actions in one flex column. A full-stylesheet layout test checks a long note at 511px; the pinned Genet 01d3f528 layout is covered, while native Turnstone receipt and other gates remain open. Recheck the remaining work against current source.

## Adopted surface design constraints

Original design rules, lines 100–128: one fixed-height dock swaps recording/buffering/completed/unavailable state in place; ember identifies playhead/frozen capture anchor; recording is a word plus danger role, never color alone; selection uses ink weight and active-row ring. Wide, narrow and phone widths share dock behavior; tablet landscape uses a left transport rail. Feed/episode/note nodes are projections of one structure, and capture actions remain beside transport independent of navigation. Validate any proposed change to these design rules with the owner rather than silently substituting a new design.

## Coverage question for review

Which S2/S3 and pass-2 design lane items are still open in current consumer code, including responsive phone/tablet and feed projections? Preserve each outstanding target in the appropriate owner.
