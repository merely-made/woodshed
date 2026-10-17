# Preservation and input admission — 2026-10-16

This receipt covers the first audit-remediation slice in an isolated Woodshed
worktree based on `21d0c2e697165770d9f5787dadc91f526c8e2b02`.

[Implementation plan](../../design_docs/2026-10-16_preservation_input_admission_plan.md)
names the disjoint agent ownership, contracts, review corrections and open gates.
`receipt.json` records actual test-process exits and per-source SHA-256 values;
`*-final.log` contains the corresponding command output. Repeated suite runs and
child-process cases are accounted separately rather than added to unique tests.

The source changes protect outgoing Woodshed practice before persona retirement,
Hocket projects before close/replacement, and Redshank data after failed recovery.
Input guards reject malformed history graphs and ambiguous Card identities before
trusted restoration. Valid legacy migration, retained branches, partial graph
union and older-generation recovery remain supported.

Four implementation agents owned disjoint files. Peer source review found and
corrected incoming-custody refusal, partial-union compatibility, vanished-newest
fallback and exhausted Card mint handling. The history scrub keeps its original
16 ms assertion; checkout validates bounded execution paths while complete graph
validation remains at durable admission and integration boundaries.

No native apps, physical devices or real persona profiles were used in this lane.
The test suites include their pre-existing isolated subprocess reopening fixtures;
those are software tests, not a new native receipt. New UI/OS interaction and the
full codec-wake-to-persona-install path remain native qualification work. Existing
native receipts retain their original source and claims. No supplier revision,
clock policy, old PCM or primary-checkout source was changed by this lane.

The implementation is committed locally only. Publication and integration into
the concurrently owned primary checkout remain separate actions.
