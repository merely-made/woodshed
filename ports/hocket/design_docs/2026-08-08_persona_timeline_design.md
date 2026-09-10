# Persona Timeline — Design Sketch

**Date:** 2026-08-08
**Status:** design sketch from Mark (chat), no implementation planned yet.
Recorded so the shape is not lost; nothing here changes current scope.

## The concept, in Mark's terms

Click a participant and see their contributions: their tracks and loopline,
everything they added laid out linearly — their timeline — with breaks
indicating where the turn was passed. Beside it, a combined loopline: a master
track carrying everyone's turns in order. Split the session by person, or by
one person's inputs on individual tracks.

A persona is not just "who am I" at the join; it is a **lens over the session
data**. And not only Hocket's: the same faceting — who contributed what, when,
across a shared space — is a persona- and mere/moot-wide timeline concept.
Presented interactively: a timeline scene with node handles on individual
regions, swatches with subgraphs, a data-oriented arrangement that can be
presented beautifully.

## What the model records today (verified 2026-08-08)

The view needs per-turn attribution, and the model does not carry it yet:

- `Layer { phrase_id, gain, muted }` — no author.
- `HistoryNode { id, parent, edit, timestamp_ms }` — no author. The history
  DAG records what and when, never who.
- Authorship exists in exactly one place: the **hand-off envelope**
  (`hocket-engine/src/handoff.rs`), signed by a session-scoped key that the
  sender's durable identity attests. `accept_branch` verifies it, then
  integrates the nodes — and drops the who on the floor. `BranchAcceptance`
  reports counts, not provenance.

So the natural attribution unit is the **turn**, not the edit: everything
between receiving a session and handing it off is one person's. The envelope
already proves the boundary cryptographically; the model just never writes
down what was proved. The missing piece is small and shaped like: a turn mark
in the history (who held the session, from which node to which), recorded at
accept time from the verified envelope rather than trusted from content. Turn
breaks in Mark's description are exactly these marks.

## Where the pieces live

Per the propagate-capability-up-the-stack rule, named before building
app-local:

| Piece | Owning layer | Hocket's part |
|---|---|---|
| Timeline scene (time-ordered projection) | scenograph family (`sceno` / `scenomise` / `scenotime`, mere) | supply turns + layers as the projected data |
| Swatches, arrangement of the presented regions | platen / forme (mere), cambium views | musical reading: looplines, per-track lanes |
| Subgraph presentation, node handles on regions | sprigging `GraphCanvas`, cambium | region = a layer or a turn span |
| Per-author causal data | already present family-wide: stickleback / chartulary keep per-writer logs | hocket's history is not on that substrate; the turn mark above is its local equivalent |

The generalization Mark named — a mere/moot-wide contribution timeline — is
the same projection over per-writer logs the family already keeps. Hocket is
the first consumer with a musical reading, not the owner of the machinery.

## The doctrine line

The scope doctrine names the **arrange view as the scope-creep canary**. This
sits close to that line, so the line gets drawn explicitly:

- A persona timeline is a **provenance reading view**: who contributed what,
  when, laid out in time. Selection, navigation, attribution — yes.
- **Editing from the timeline** — dragging regions, trimming, re-arranging —
  is the arrange view, and stays out. If region handles ever grow move/trim
  affordances, that is the canary singing.

## Interaction with the shared-identity gate

The family-shared identity plan (mere,
`2026-08-08_family_shared_identity_plan.md`) leaves Hocket's vault wiring
gated on the contact-token rotation: pointing `LocalIdentity` at the shared
vault changes the master key that existing pasted tokens resolve to. The
timeline concept raises the stakes on doing that migration properly rather
than skipping it: personas become structural here — the axis the whole view
facets on — not a sealing detail. Same gate, more reason.
