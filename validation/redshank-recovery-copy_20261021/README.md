# Redshank D3 recovery exit and independent copy

Date: 2026-10-21. Qualified source: `421ea8e5bd865175a9b54a0d59e886cb857bb431`,
from published `01c4b3af61d17af69779619d2bd7fd77bf06abdc`.
The maintainer accepted D3 and authorized direct main publication. Publication
is observed through `f4904a27`: see the [publication record](publication.json) for the remote ref and source/evidence verification.

Failed original restoration retains a permanently nonwriting generation store.
An unchanged recovery session can close without attempting a save. Changed
sessions offer Save Recovery Copy, Keep Working and Exit Without Saving.
Keep Working preserves the editor and active capture. Explicit discard cancels
capture after the choice; pending listener jobs and appearance drafts retain
their own close barriers.

Save Recovery Copy selects a **new directory name** with an existing parent.
Existing destinations and aliases inside the original root are refused. The
worker copies local library audio, cached audio and desktop voice blobs, then
publishes the complete snapshot model last. Original URLs, representation
receipts, voice identities, inline transcripts and timed-target facts remain
intact. Point/span editor drafts are materialized only into that snapshot.
Neither the live editor nor the original store acquires new durability from
copy success. Newer revisions or draft text prevent automatic exit.

The existing `REDSHANK_DATA_DIR` startup override selects a copy on a subsequent
launch. Its Local/Cached paths are absolute to the chosen copy, so moving the
directory afterwards requires a separate relocation feature. Missing or
unsupported media refuses publication. A failed copy may leave an incomplete
new directory with no model generation; choose another new name to retry.

| Gate | Passing cases | Existing ignores |
| --- | ---: | ---: |
| `cargo test --locked -j 4 -p redshank-storage -p redshank-surfaces` | 22 storage + 107 surfaces | 0 |
| `cargo test --locked -j 4 -p redshank-desktop` | 92 | 1 |
| `cargo check --locked -j 4 -p redshank-desktop -p redshank-web` | Compilation passed | — |
| `cargo check --locked -j 4 -p redshank-web --target wasm32-unknown-unknown` | Actual Wasm code passed | — |

Total: **221 distinct passing tests**, one existing desktop ignore. The desktop
gate includes both native/application close hooks and a real Keep Working click
through the GPU-free host harness, the final dirty-appearance close barrier,
unchanged/dirty close, cancellation/failure, draft/revision races, ignored normal
save replies and unadmitted media deletion. Storage tests reopen a copied model
with all supported media after original files become unavailable and verify
the original generation/media inventory remains unchanged.

[receipt.json](receipt.json) lists accepted commands and retained superseded
attempts. Early compile errors and two host-fixture issues (missing relayout,
then an unavailable default appearance library) are corrected and excluded
from the final totals. The final desktop suite passes all added cases.
[source.json](source.json) records 74 source/config/lock hashes. Raw process
receipts retain their original absolute log paths; packaged logs are exact byte
copies, with hashes in those receipts. The capture wrapper records actual exits;
success was checked against process status and complete suite outcomes.

Parallel ownership was exclusive: storage snapshot, shared presentation and
desktop integration; a separate reader reviewed the combined production diff.
The review corrections preserve span drafts, exclude never-admitted recovery
removals from idle waits and bound failed-generation wording. Eleven completed
related audit chats were archived after their final reports were checked; see
the [archive outcomes](thread-archive.json). The active coordination/product
chats and P5a implementation ownership remain in place.

This is CPU, file/model reopen, source-review and compilation evidence. Native
window/chooser interaction, a fresh application process reopening the copy,
physical audio and device acceptance were not exercised. P5a, recording-capacity
policy, persona-store migration and supplier checked-construction intake remain
separate work.
