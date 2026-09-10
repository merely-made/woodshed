# Hand-Off UI Plan (host carrier, review, acceptance)

Follow-on to [2026-07-10_signed-handoff-envelope_plan.md](2026-07-10_signed-handoff-envelope_plan.md).
That plan landed the signed envelope protocol and left four host-side gaps in
its Progress log: recipient key exchange, a carrier, incoming staging and
review, and the user-facing acceptance action. This plan closes them.

## Goal

Make the pass-the-mic gesture usable from the Genet host. A musician can send a
session to a peer and accept one that arrives, over a file carrier, without
overstating confidentiality or pretending divergent edits are merged.

## Context (what already exists)

- **Engine.** `hocket-engine::handoff` builds and verifies the signed envelope:
  `HandoffEnvelope::{create, to_bytes, from_bytes, receive}` and
  `ReceivedHandoff::accept_branch`. Covered by focused tests. `receive` takes
  the recipient's public key by value and needs no private key, so it can run
  off the main thread.
- **Identity.** `hocket-genet::identity::LocalIdentity` (a personae sealed
  record, DPAPI-backed on Windows) implements `IdentityProvider`, exposing
  `master_public_key()` and a short six-byte `fingerprint()`.
- **Host.** `AppState` owns session, history, media store, the identity result,
  and an Armillary project worker that does zip I/O off the kernel thread
  (`project_io.rs`) with a typed command and update applied on the next frame.
  The circle rail (`view.rs::rail`) is the pass-the-mic surface. It shows the
  local performer and an identity line today; its comment already reserves the
  space for peers.
- **Personae keys.** `Ed25519PublicKey` has `to_bytes -> [u8; 32]` and
  `from_bytes(&[u8; 32])`. There is no string codec, so the contact token
  encoding is the host's responsibility.

## Design

Boundary rule from the repo guide: collaboration semantics stay in
`hocket-engine` (they already do). The host carries bytes, holds presentation
state, and wires gestures. `hocket-genet` stays thin.

### Carrier: a file, over the existing project worker

- Extend `ProjectCommand` with `WriteHandoff` and `ReadHandoff`, and
  `ProjectUpdate` with `HandoffWritten` and `HandoffReceived`, reusing the
  `Failed` variant. This reuses the actor and wake plumbing rather than spawning
  a second worker.
- **Send.** The main thread builds the envelope with `create()` (which needs the
  private identity), then hands the owned `HandoffEnvelope` to the worker. The
  worker serializes it (`to_bytes`, the heavy CBOR pass over the media) and
  writes the file.
- **Receive.** The worker reads the file, calls `from_bytes`, then
  `receive(own_master_public_key)`. Verification and media materialization run
  entirely in the worker; it returns an owned `ReceivedHandoff`. The private
  identity never leaves the main thread.
- The file is named `.hocket`, a single pass of the mic (the hocket technique
  is splitting a line between voices, which is the gesture). A `.hocket` is a
  transfer artifact, not an archive, and not structurally a `.hock`: a `.hock`
  is a zip container, while a `.hocket` is signed, addressed CBOR envelope
  bytes. The kinship of the two names is thematic. Once accepted, the session
  persists as an ordinary `.hock`.

### Recipient exchange: a contact token, replies auto-address

- Encode the 32-byte master public key as a copyable contact token. Lowercase
  hex for this cut, which adds no dependency. A checksummed, friendlier encoding
  is a follow-on.
- Show the token near the identity line in the rail, with a copy affordance.
- Sending needs the recipient's token. Two ways to supply it: paste one, or pick
  the sender of a staged or last-received hand-off. A reply therefore needs no
  paste, which keeps hands on the instrument after the first exchange.
- Parse with `Ed25519PublicKey::from_bytes`. A malformed token is refused before
  the worker runs.

### Staging and review: never clobber silently

- `AppState` gains `incoming: Option<ReceivedHandoff>`, host-local presentation
  state. A received hand-off stages here and does not touch the live session
  until accepted.
- A review surface (a card in the circle) shows the sender fingerprint, whether
  the hand-off continues the current session (same `session.id`) or opens a new
  one, the track, layer, and phrase counts, and how many media blobs are new.
  Its actions are Accept and Discard.

### Acceptance: same-session branch or new session

- **Same `session.id`.** Build a `ProjectBundle` from the live session and
  history, call `ReceivedHandoff::accept_branch`, and on success write session,
  history, and store back, then resync tempo and reconcile playback. This
  mirrors the `Opened` path. The engine retains the prior local branch.
- **Different `session.id`.** Adopt wholesale: stop playback, then replace
  session, history, and store, mirroring `apply_project_update(Opened)`.
- Either way: remember the sender for a one-tap reply, mark the project dirty
  against `saved_head`, clear `incoming`, and set an honest status line.

### Honesty (real feedback, not placebo)

- Addressing is not encryption. The file carrier is cleartext. A private session
  over a raw file needs an encryption policy first. State that at the point of
  send; do not imply confidentiality.
- The rail reflects real state only: the local identity, an incoming sender, and
  an outgoing "handed to" note. It does not fabricate a live peer roster, which
  belongs to the later sync layer.

## Tasks and done-conditions

Organized by feature target and validation, not by time.

1. **Carrier round-trip.** Worker write and read commands land.
   - Done: a `hocket-genet` test writes a self-addressed envelope to a temp
     file and reads it back to an equal `ReceivedHandoff`. A wrong-recipient
     file surfaces an error rather than panicking.
2. **Contact token and recipient entry.** Own token renders and copies; a pasted
   token parses or is refused.
   - Done: token encode and parse round-trip in a unit test; malformed input
     yields a refused-send status.
3. **Send gesture.** "Hand off" builds, writes, and reports status; recording
   blocks it, as saving does.
   - Done: a headed check writes a file; status shows the written path and the
     cleartext caveat.
4. **Receive, review, accept.** "Receive hand-off" stages; the card shows the
   facts; Accept applies (branch or adopt); Discard clears.
   - Done: a same-session accept integrates and checks out the incoming head; a
     new-session accept adopts; both reconcile playback; a headed check confirms
     audio follows the accepted head.
5. **Reply auto-address.** After accepting, or while staged, a reply
   pre-addresses to the sender.
   - Done: the reply path needs no pasted token and is addressed to the original
     sender key.

## Decisions (2026-07-18)

- **Hand-off file name.** `.hocket`, a single pass of the mic. Thematic kin to
  `.hock`, not structurally an instance of it (see the carrier section).
- **Contact token encoding.** Hex now. A checksummed, friendlier encoding is a
  follow-on.
- **Review surface shape.** A card in the circle for a single incoming. An
  overlay only if a queue ever appears.
- **Saved contacts.** Remember the last sender in this cut, which the reply
  auto-address design already provides. No stored address-book capability exists
  to reuse. The relevant prior art is the *resolution* half, not the storage
  half: mere's `gazetteer` crate (`crates/persona/gazetteer`, formerly `gazette`)
  resolves a handle or key to reachable, trust-stated endpoints (WebFinger
  today), and is incubating, mere-coupled, and unconsumed. The stored contact
  record is designed but unbuilt in mere's 2026-06-15 contact-identity brief,
  which roots contacts on the key with handle and endpoints hanging off it.
  Hocket's `.hocket` token already is that root key, so the MVP needs no
  resolver. A portable contacts capability, if wanted later, belongs on the
  persona tier beside `identity` and `gazetteer` and promoted to a standalone
  repo like the rest of the family, not inlined in `hocket-genet`. Out of scope
  here.

## Findings

- `receive()` needs only the recipient public key, so verification and
  materialization run entirely in the worker while the private identity stays on
  the main thread.
- `ReceivedHandoff.sender` is the durable sender identity, so replies are
  auto-addressable, which softens the one-time key-exchange friction.
- The rail already carries an identity line and a `handoff-note`, so the contact
  token and the incoming card have a home without a layout change.
- No stored address-book capability exists to reuse. The resolution half does:
  mere's `gazetteer` (formerly `gazette`) resolves handles or keys to endpoints,
  but it is incubating, mere-coupled, and unconsumed, and Hocket's key-rooted
  token needs no resolver. The stored contact record is designed but unbuilt
  (mere's 2026-06-15 contact-identity brief). Remember-last-sender is therefore
  the honest MVP, free from the reply auto-address path. The `mere-roster` crate
  is unrelated: it is a graph-object inspector panel.

## Progress

- 2026-07-18: Scoped. Read the envelope engine, host identity, `AppState`,
  `view.rs`, the project worker, and the personae key API. Plan drafted against
  the current code, not doc-to-doc. Nothing implemented yet.
- 2026-07-18: Decisions settled with Mark. File is `.hocket`; token is hex;
  review is a card in the circle; contacts is remember-last-sender, with a
  portable address book left as a separate future plan.
- 2026-07-18: Contacts prior-art check corrected. mere's `gazetteer` (formerly
  `gazette`) is the handle-resolution half and is incubating and mere-coupled;
  the stored contact record is designed but unbuilt (mere's 2026-06-15
  contact-identity brief, contacts key-rooted). Hocket's token is already the
  root key, so the MVP needs no resolver, and a shared contacts capability would
  live on the persona tier if built.
- 2026-07-18: Clipboard analysed as a cross-app capability, not a Hocket
  detail. genet already carries a text-only clipboard seam from its Servo
  lineage (`EmbedderMsg` Get/Set/ClearClipboardText) backed by arboard in the
  pelt-desktop port, but cambium app hosts like hocket-genet bypass the
  embedder and have no clipboard path. The robust answer is a shared genet
  clipboard service on the web `ClipboardItem` model, scoped in genet's
  `docs/2026-07-18_clipboard_capability_plan.md`. Interim for this plan:
  hocket-genet may call arboard for text directly (copy token, paste recipient)
  and migrate onto the shared service at its P0/P1. arboard is MIT/Apache and
  already in the family (genet), so this extends a blessed dependency.
- 2026-07-18: **Headed receipt via the genet-probe harness.** hocket-genet
  adopted genet-probe (Automatable + Driveable) and a turnstone-style self-drive
  scenario lane (`HOCKET_SCENARIO`, self-capture to PNG, `RESULT ok|fail`
  receipt, exits without saving). `scenarios/handoff_circle.scn` asserts the
  collaboration controls (Copy token, Paste recipient, Receive) are present in
  the circle and captures the frame; a green Windows run confirms the UI. This
  is hocket's first automatability consumer, reusable for future scenario tests.
  Follow-on: the circle's controls reuse `project-command` styling and want a
  visual pass; clicking gestures (Hand off / Receive / Paste recipient) are not
  scenario-driven yet because they open file dialogs or write the clipboard.
- 2026-07-18: **Tasks 3, 4, and 5 landed (send, receive, reply).** Send: paste a
  peer's contact token from the clipboard to set the recipient
  (`parse_contact_token`), then Hand off writes a signed `.hocket` via the save
  dialog. Receive: open a `.hocket`, the worker authenticates it off-thread and
  stages a `ReceivedHandoff`, a review card in the circle shows sender,
  continues-this-session vs new, and counts, with Accept and Discard. Accept
  integrates the branch (same session) or adopts wholesale (new), reconciles
  playback, and addresses the reply back to the sender (task 5, folded in). Both
  `dead_code` allows from the task-1 carrier are gone now that the gestures
  construct the commands. The whole loop is wired; `cargo test -p hocket-genet`
  green (14), clippy clean. Visual polish of the circle's collaboration controls
  (currently reusing `project-command` styling) is a follow-on.
- 2026-07-18: **Task 1 landed (carrier round-trip).** `ProjectCommand` gained
  `WriteHandoff`/`ReadHandoff` and `ProjectUpdate` gained
  `HandoffWritten`/`HandoffReceived`, served by the existing project worker: the
  main thread builds and signs the envelope, the worker serializes and writes it,
  and receive (file read, `from_bytes`, `receive`) runs entirely in the worker on
  the recipient public key alone. Two `hocket-genet` tests pass: a self-addressed
  envelope writes and reads back to an equal `ReceivedHandoff`, and a
  wrong-recipient file surfaces a `Failed` update without a panic. `AppState`
  shows honest placeholder status (handed off `<file>`, hand-off received from
  `<fingerprint>`); the send and receive gestures that construct the commands are
  tasks 3 and 4, so those two command variants carry a scoped `dead_code` allow
  until then. Full `hocket-genet` suite green (13 tests).
