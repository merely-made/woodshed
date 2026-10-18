# Woodshed source authority audit — frozen read and coordination closure

Status: frozen collective source read and finding reconciliation complete;
remediation open; repository record published through `8204688` ([publication record](publication.json)).
The canonical current frontier is the
[design plan](../../design_docs/2026-10-16_source_authority_audit_plan.md).

The audited source is `d12201bfe80cc94345c6308c46caaebf4a9fe42e`.
Eleven partitions cover 936 assigned text files / 231,127 lines, with a further
63-line W3C fixture read by the parent: **937 unique files / 231,190 lines**.
Program source/tests are 303 files / 149,331 lines; remaining categories include
configuration, fixtures, native scenarios, lockfiles and structurally reviewed
receipt metadata. Binary assets, every historical screenshot/log and third-party
implementations were not an exhaustive source-reading scope. This was a
collective read with exact file/range/hash attestations, not a claim that the
coordinator personally reread every partition.

The separately frozen P5a overlay has **33 files / 34,861 semantic lines**,
including all unchanged ranges. It overlaps the published snapshot and is not
added to its unique total. The original local overlay was uncommitted; its exact
external frozen source is required to reproduce that hash check. Its ledger is
not current-head qualification.

The original [audit synthesis](AUDIT.md), [eleven reports](audit-receipt.json),
[assignment inventory](inventory.json), per-lane coverage and
[original package catalog/crosswalk](REMEDIATION_PLAN.md) are copied byte-for-byte.
The original reports retain their historical absolute snapshot links, statements,
source locations and publication fields. Read those as dated evidence; the
canonical plan owns today's execution frontier. Frozen tracked source can be
recovered with `git show d12201bf:path/to/file`; no product snapshot was copied
into the repository for this closure.

The [new coverage result](current/coverage-verify.stdout) checks every published
input against Git blobs from that revision, exact line counts, assignment
uniqueness and complete inclusive ranges. It checks all original report hashes,
the additional fixture, and all overlay semantic ranges and input hashes.
Fifteen parent corroboration entries concern already assigned files (eleven
partial and four complete rereads); they do not add unique full-file reads.
The [process receipt](current/coverage-verify-process.json) records actual exit
zero and the verifier hash. Three [negative range controls](current/verifier-controls-process.json)
pass, and [optimized Python is refused](current/verifier-optimized-refusal.json).
The first new-helper attempt mistakenly treated partial parent corroboration as
additional unique reads and failed; that failed process/stderr is retained. It
was a helper bookkeeping error, not a missing partition read.

Current-source reconciliation uses local `a42477297b692feb7df4d8be74fba4cb83bcaf8c`.
Of the original 936 assigned paths, 841 are byte-identical and 95 changed; none
is missing. This is a metadata comparison plus targeted caller/finding review,
**not a new full semantic read of all changed or newly added files**. There is
no claim that the complete October 16 read establishes absence of current bugs.

All **93 original raw finding rows** have a disposition in the
[current ledger](current-finding-ledger.json), alongside O1 and separately named
LO-4, LO-5 and LO-K1. Raw entries include candidates, gaps and duplicates; they
are not a unique production-bug count. Ringdown X1 aliases theory T2; LO-5
aliases CORE-4. Ringdown R8 remains a P3 method-metadata duplication, with no
ordinary default guarded-path hardware bypass established.

| Current reconciliation | Principal result |
| --- | --- |
| [Safety](current/safety.md) | Ten scoped implementations present with historical receipt limits; Hocket H3 persona binding and H5 missing-media facts remain open. Woodshed has twelve historical bounded persona/close/quarantine/reopen captures; Hocket/Redshank retain CPU/build-only evidence. |
| [Runtime and views](current/runtime.md) | Thirty retained scoped findings and five old-overlay source replacements. Managed clock, unavailable-preview admission and fixture changes are credited without pretending generic Path/Walk Hear or retained setup is resolved. Current execution qualification remains unassessed. |
| [Ports, protocol and instrument](current/ports.md) | Forty scoped rows / thirty-nine canonical findings preserve their mechanisms; no reviewed matching receipt closes those triggers. Source continuity does not establish runtime/device failure or acceptance. |
| [Evidence and tooling](current/evidence-tooling-current.md) | Original false-pass helpers and nine obsolete scenarios remain; scorer and package gaps remain. macOS microphone wording is corrected, Linux/Windows wording is not. The recipe cohort aligned, with historical execution proof kept at its original source. |

The safety reviewer checked raw software/build logs, source bindings, seventy
native artifact-manifest entries and the prior excluded runs. Exact historical
qualified commits `cec2213` and `d955f5d` are reachable from fetched `origin/main`
`125be57d`. Source/supplier drift is explicit; neither a preserved helper body
nor those historical receipts establishes current-head integrated acceptance.
No new product suite, desktop build, native launch, fresh reopen, microphone,
physical audibility, protocol hardware or other-platform gate ran in this closure.

H3/H5 and Q1 were CPU qualified and published through `6e6498a`;
B1–B3 through `b1c9dfd`. B4 now has a separate local
[PCM/timing receipt](../pcm-timing_20261018/README.md): 308 distinct passes at
`5dd01ae`. These targeted implementations do not refresh the frozen full read
or establish native acceptance. The next runtime frontier starts at B5 shared
Recorder SelectOne admission, with subsequent product/controller, protocol and
compatibility work retaining their existing owners. Existing media, playback
clocks and native validation lanes remain protected.
The source audit is finished; the implementation backlog is not.

[Closure and artifact hashes](closure.json) bind these records. Raw original
sources and read logs remain under
`/Users/markik/Code/testing/woodshed/full-source-audit-20261016`.
To reproduce coverage with that separately retained overlay:

```sh
python3 validation/full-source-audit_20261016/verify_coverage.py \
  --repository /Users/markik/Code/repos/woodshed \
  --overlay-source /Users/markik/Code/testing/woodshed/full-source-audit-20261016/local-p5a-overlay
python3 -m unittest discover -s validation/full-source-audit_20261016 \
  -p test_verify_coverage.py -v
```

Missing Git objects or overlay files fail verification; they are never counted
as zero gaps. This verifier checks recorded reader attestations and identity,
not code semantics or product success.
