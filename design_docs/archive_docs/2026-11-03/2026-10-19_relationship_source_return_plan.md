# Relationship source return

**Date:** 2026-10-19. **Status:** implemented and qualified; source-return publication authorized.

## Scope and ownership

Consolidate direct and exact-link-member source return over one operation-local
reading and resolved source. Preserve selected occurrence/relationship updates,
exact refusal wording and ordering, owner-qualified Card navigation, full source
reconstruction, legacy projection versions and background-runner custody. This
is an application-owned synchronous operation; no retained cache or authority
handle is introduced. Source changes are confined to comparison projection,
relationship return/installation and focused tests. Dependency, saved-format,
unrelated MusicXML/recorder work stay outside this pass.

## Phases and done-conditions

- R0: freeze exact route results and relevant before/after state before changing
  behavior. Cover versions 0/1/2/3, direct and member paths, active/inactive and
  ambiguous owners, currency, forged member/dataset inputs, invalid retained
  bytes and early/late failures. Record raw reading bytes, selections, label,
  working/runner ownership and Card state. Temporary capture code stays in
  testing and is removed from final product tests.
- R1: prepare typed destination and installation once. Direct return has no
  installation. Member return checks its target and initial owner/currency,
  prepares installation with the original error priority, computes a pure
  destination Result against the resolved Set, installs the selected reading,
  and only then propagates late refusal or navigates. This deliberately preserves
  selection installation on late failure. Cheap action availability remains
  distinct from full navigation admission.
- R2: reuse selected material for v3 reconstruction. A local core entry point
  preserves the previous comparison-then-passage error ordering and explicit
  stored-pair validation, then builds passage JSON from the resolved facts.
  Existing public passage semantics and legacy versions stay unchanged.
- R3: require frozen state/refusal equivalence, independent diff review,
  meaningful regression and construction-count tests, feature-disabled core
  checking and relevant receiving suites. No timing or native GUI claims follow
  from model/route tests alone.

## Findings

2026-10-19: member return decodes/compiles the reading, validates exact member,
resolves owner/currency, changes selected reading, installs it, then re-enters
public direct return. That repeats decode/compile and owner/revision work.
Direct return reconstructs v3 by comparison disclosure followed by passage
projection, repeating complete selection capture. Matching hashes alone do not
validate retained disclosures: exact dataset reconstruction remains mandatory.

The member installation boundary is observable. Early target/currency refusals
retain prior reading bytes; later source-refusal paths may retain the installed
selection and normalized JSON even while refusing navigation. The refactor
must preserve both behaviors, including priority of installation errors over
subsequent source errors. The first source lookup remains borrowed only through
pure computation, with an owned destination/error crossing installation.

## Progress

- 2026-10-19: fetched upstream and created an isolated checkout at `4fab895`.
  The original development checkout and local recorder history remain untouched.
  Frozen source hashes and provenance are in
  `Code/testing/woodshed-source-return-20261019/baseline.json`. Delegated route
  baseline/contract coverage and independent order/design review. Product
  behavior was unchanged while the baseline was captured.

- 2026-10-19: froze 312 observations (39 scenarios across four retained-reading
  versions and both routes) against the unchanged controller. Observations retain
  exact results, raw reading bytes, selection/label, complete active/inactive Sets,
  panels and background-runner state. The matrix and golden remain in testing.
- 2026-10-19: introduced private prepared-reading installation and destination
  preparation. Member installation still precedes propagation of late refusal;
  direct return does not install. Source matching still requires full dataset
  reconstruction and equality. The cheap current-source hint is unchanged.
- 2026-10-19: factored the passage builder and added a local core entry point that
  validates the stored comparison pair before the passage bound and reuses its
  resolved occurrences. Existing public passage validation order is unchanged.
- 2026-10-19: independent actual-diff review found no actionable correctness
  regression. It checked installation/error precedence, typed serialization,
  inactive-owner transitions, complete reconstruction and operation-local borrowing.
  Final executed qualification is recorded below.

## Construction accounting and limits

Test-only event counters cover successful active-owner v3 returns. Direct return
performs one decode, one compile, one owner lookup, one material-revision hash,
one source reconstruction and one navigation, with no reading installation.
Member return performs one decode, two compiles (entry and prepared installation),
one owner lookup, one revision hash, one reconstruction, one installation and one
navigation. Its prior route re-entry performed two decodes, three compiles, two
owner lookups and two hashes. These are method-entry counts, not allocation or
elapsed-time measurements.

At selected widths 2/3/64, the core composition comparison checks exact output and
selection/dataset/adjacent-pair construction counts. The combined path changes
complete selection resolution from two to one; one occurrence dataset and n−1
rich adjacent-pair projections remain. Stored-pair validation is still performed;
its pitch comparison is separate from the adjacent-pair projection counter.

The implementation introduces no durable validation token, source cache, shared
supplier API, dependency update or saved-format change. Source validity is reused
only inside this synchronous route, while the Set remains immutably borrowed
through pure destination preparation. Native GUI and device acceptance and CPU or
memory speedups are not established by these route tests.

## Qualification

The frozen matrix comparison passed all 312 observations with the same Cargo
command, unchanged matrix and unchanged golden. The temporary machine-local
capture hook was then removed; the permanent route and counter regressions remain.
Core qualification passed 23 focused comparison tests, all 313 default-feature
library tests, all 306 receiving view library tests (including the five new route
and accounting regressions), and a library check with `--no-default-features`.
Rust 1.99.1 and the unchanged locked dependency graph were used throughout. Scoped core/view
library Clippy completed successfully with warnings confined to unchanged files;
format and whitespace checks passed.

Detailed commands, source hashes and logs are retained in
`Code/testing/woodshed-source-return-20261019/qualification.json`, with
`REPORT.md`, `compare-receipt.json`, `compare-addendum.json` and `review.md`.
The frozen matrix qualifies the exercised cases; installation-failure priority
against a simultaneous later source failure was reviewed in control flow rather
than executed as a separate matrix case. Publication uses only the source-return patch and its plan/index; the baseline,
matrix, logs and receipts remain in testing.

- 2026-10-19: publication authorized. Fetched `origin/main` at `3c03353`;
  intervening Redshank and research-doc changes do not overlap source-return code
  or dependencies. Publish the scoped patch from its isolated checkout, preserving
  those upstream changes and leaving the original development checkout alone.
