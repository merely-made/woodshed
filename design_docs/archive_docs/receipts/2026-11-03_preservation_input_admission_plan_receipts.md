# Preservation and admission receipts

Plan: [Preservation and admission](../../2026-11-03_preservation_input_admission_plan.md).

## 2026-11-04: custody client receiving qualification

The unchanged native Woodshed baseline check passed before editing. Final
locked/offline all-target checks pass for Woodshed, Hocket and the repaired
recipe fixture. The repository's full all-target Clippy gate and Hocket's
native all-target Clippy gate pass with existing warnings; the full-tree
line-ceiling and diff whitespace checks pass.

- App declaration tests: six pass against the real Ostiary authority checks;
  with empty declarations, the earlier negative run failed all six as expected.
- Native Woodshed: 19 storage tests pass and 49 tests selected by `persona`
  pass. These selections overlap. Each leaves one explicit custody receipt
  test ignored. Two earlier 600-second compile/link attempts expired before
  tests; a third bounded run passed with the warmed cache.
- Hocket's actual identity module, included in an isolated consumer: ten tests
  pass. Two parent tests explicitly execute its otherwise ignored receipt test
  in child processes, for absent and synthetic locked residents. They check
  nonzero child execution, public roster access, declarations on all three
  connections, pending identity and unchanged bytes. This is not live-custodian
  qualification.
- Recipe fixture: eight tests pass, covering current owner-generated disclosure,
  relationship validation, isolated filesystem round-trip, existing-destination
  refusal and path resolution. Separate CLI seed and reopen processes also pass,
  retaining edits, selection, musical evidence and the source Set. Native
  storage tests run in the real application;
  the fixture deliberately skips its included `host_storage` test copies.
- Both production declarations compile for `wasm32-unknown-unknown` with only
  portable Ostiary enabled. This does not qualify a browser custody client.
- Native normal/build dependency traversal rooted at Ostiary/Castellan-client
  has 124 package names in Woodshed and 126 in Hocket (124 in the recipe).
  These cones exclude Djinn, Graphshell, Chirograph/scenes, Hest, Pandect/kernel
  and Castellan. The GUI applications retain their own rendering dependencies.

All heavy gates use one Cargo job and test thread, a machine-wide lock,
3 GiB memory, zero swap, 150% CPU, 256 tasks and a 600-second timeout. Failed
and superseded logs remain with the final receipts. The lock-edge audit also
restored an incidental tempfile/getrandom rewrite in Hocket and the recipe;
no unrelated dependency selection is accepted. Headed, physical audio,
Windows and live-user custody behavior remain unqualified by this cut.
