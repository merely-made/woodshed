# Signed Hand-Off Envelope Plan

## Goal

Define the complete, authentic payload that one Hocket peer hands to another,
without tying it to a premature network carrier or pretending divergent edits
have already been reconciled.

## Design

- `hocket-engine::handoff` builds a versioned envelope containing a project
  bundle plus every media blob referenced by its phrases. It refuses incomplete
  snapshots and verifies each media hash on receipt.
- The sender derives a session-scoped signing key through `personae`; a
  master-signed `DerivedKeyAttestation` binds that key to the durable sender
  identity and session salt. The envelope addresses the snapshot to the
  intended recipient's public key.
- The signed bytes cover the format, session id, sender, recipient, manifest,
  and media. A recipient must match and the signature must verify before a
  `ReceivedHandoff` is materialized.
- The encoded bytes are carrier-neutral. A Murm attachment, Iroh transfer, or
  file exchange can move them without reinterpreting session or media state.
  Recipient addressing is neither encryption nor proof of private-key
  possession: Murm/Iroh supplies confidential authenticated transport, while
  raw file exchange needs a separate encryption policy before it is suitable
  for private sessions.
- Receipt produces a staged snapshot. `History` can retain and integrate a
  same-root branch, but incoming conflict reconciliation is not claimed here.
- `ReceivedHandoff::accept_branch` transactionally integrates the incoming
  graph, checks out its head, verifies its manifest projection, and imports
  missing referenced media. It preserves the previous local branch rather than
  fabricating a merged head.

## Done Conditions

- A complete project and all referenced media serialize into a signed envelope.
- A receiving host checks the expected recipient address and reconstructs the
  same bundle/media after sender and payload verification.
- Wrong-recipient, altered, malformed-signature, and missing-media envelopes
  fail before a host can accept them.
- The protocol does not add identity, device, or transport state to a project.
- Same-root acceptance either updates bundle/media together or leaves both
  untouched.

## Progress

- 2026-07-10: **PARTIAL.** The engine protocol and focused tests are landed.
  Core same-root branch acceptance is landed; branch merge still waits for a
  conflict-reconciliation policy.
- 2026-07-11: **PARTIAL.** The Genet host now loads or creates one durable
  local identity in a `personae` sealed record using its OS-protected startup
  root. Windows has the concrete DPAPI backend; unsupported platforms expose an
  unavailable state rather than minting an ephemeral identity. Recipient key
  exchange, a carrier, incoming staging/review, and the user-facing acceptance
  action remain open.
- 2026-07-11: **LANDED protocol hardening.** Envelope v2 carries a
  master-signed `personae::DerivedKeyAttestation`, so receipt identifies the
  durable sender that authorized the session key. Recipient binding is now
  described accurately as addressing; confidentiality belongs to the carrier.
- 2026-07-18: The remaining host-side work (recipient exchange, carrier, review
  UI, acceptance action) is carried forward into
  [2026-07-18_handoff-ui_plan.md](2026-07-18_handoff-ui_plan.md).
