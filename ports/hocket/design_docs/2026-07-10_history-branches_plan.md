# History Branch Retention Plan

## Goal

Keep divergent local and incoming histories intact so a future peer hand-off
can offer a real branch rather than forcing one side's work to disappear.

## Design

- A commit after checkout creates a new child and retains every existing child.
- Checkout moves from one branch to another by inverting edits up to the lowest
  common ancestor and applying the target path forward.
- `History::integrate` unions validated same-root node sets without changing the
  local head or session projection. A caller can inspect or checkout the remote
  branch later.
- Redo remains deterministic by choosing the first child in `NodeId` order;
  any future branch chooser should call `checkout` with its explicit target.
- This is branch retention and transport intake support, not a CRDT or an edit
  merge rule. Concurrent edits still need deliberate product semantics.

## Done Conditions

- Checkout preserves and crosses sibling branches.
- Same-root incoming histories integrate without replacing the local head.
- Mismatched roots, duplicate-node conflicts, and missing parents fail clearly.
- Existing undo/redo and persistence tests remain valid.

## Progress

- 2026-07-10: **LANDED.** `hocket-model` retains branches, checks out through
  common ancestors, and validates graph integration. Focused branch/integration
  tests and the existing model validation suite pass.
