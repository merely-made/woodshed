# Native arrangement span editing — 2026-10-16

Qualified source: Woodshed `b3afe76`, shared Mere `d1e1d7aae`.
The canonical lockfile changes only the 45 Mere source entries. The existing
Genet, Netrender and Dramatis cohorts are retained.

Fourteen focused consumer tests and six shared surface tests pass; the locked
desktop build succeeds. The source/binary/lock identities are in `source.json`.
Native seed and fresh-reopen processes present 229 and 94 frames, respectively;
all five captures were reviewed. The isolated real Keeper uses the synthetic
identity and public receipt fixture recorded in `keeper-source.json`.

The native fixture moves Card 2, cancels that move with Escape, repeats and
commits it, moves by keyboard, resizes both edges, and moves an authored rest in
the compact layout. Every edit uses the ordinary captured pointer/key route.
Only the owning draft changes until explicit Save. The live and sealed Sets
then agree; a fresh process reopens the same entire Set and portable document.
`verify.py` checks exact data equality, intended field differences, successful
native results and receipt hashes without writing anything.

`initial/` retains the first failing native run and capture regression.
Removing the conditional notice shifted positional children and retired capture;
borrowing an unchanged text field during view rebuilding also cleared the
gesture. A stable notice slot and invalidation on actual draft content changes
resolve these. Selected edit controls sit immediately after the primary scene.

To reproduce, check out the source in `source.json`, run the focused Cargo
command in `receipt.json`, and build `cargo build -p woodshed-genet --locked`.
Use the repository's `scenarios/native_arrangement_spans.scn` followed by
`scenarios/native_arrangement_spans_reopen.scn` with one isolated profile and
an unlocked synthetic receipt Keeper. The recorded C launchers show the exact
environment; give a new run its own directory and endpoint.

This qualifies native alignment editing and persistence on this macOS host.
It does not qualify physical audio, browser deployment, Knot editing, or a
shared editing-authority contract. Metre is the next concrete editing consumer
before extracting domain-independent edit intents.
