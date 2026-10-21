# Hocket admission and evidence-gate corrections

Status: implemented and CPU qualified at `0effba1`; published through `6e6498a` on 2026-10-18 ([publication](publication.json)). This is the
first implementation batch after the frozen source audit closeout. It addresses
Hocket H3/H5 and evidence E1/E2 through their existing owners.

The preceding label adoption and frozen audit closeout were published through
Woodshed `b123bf7` and Mere `e6250fd1c`; this batch has separate source and check
records. Historical receipts remain tied to their original inputs. No native
application, resident custody service, microphone or physical-device run is part
of this batch.

Three cheaper Luna workers own disjoint source areas. The coordinator reviews
custody admission, checks operational entrypoints and integrates the results.
The link helper's regression controls cover historical heading/file deletion,
existing debt, binary targets and current-symlink isolation. Its historical Git
lookup does not follow symlinks stored in that historical tree; cross-repository
links remain outside its revision scope.


The final checks pass **93 Hocket cases**, **14 process-evidence controls** and
**seven link-checker controls**: **114 distinct passes**, with two existing Hocket
ignores. The focused media case is included in that Hocket total. Both
qualification entrypoints refuse optimized Python with observed exit 1. The real
final link scan checks 121 baseline links and 122 current links with zero new findings. Exact commands,
source hashes, exits, log hashes and excluded draft runs are in
[receipt.json](receipt.json) and [checks](checks/).

Hocket confirmation binds the offered target profile/key and losing contact
token to the observed custody identity before writing consent or switching.
Changed offers and failed consent writes preserve the prior identity and consent.
A matching decision records the exact accepted profile/key; the confirmation
shows that identity. The CPU tests exercise production admission and temporary
consent files, without claiming real custody IPC or fresh-process reopening.

Same-session acceptance recomputes missing references from the accepted phrase
pool and actual imported store. The new regression restores one locally missing
phrase, retains a missing local-only phrase and branch, and checks imported and
pre-existing PCM. The original unchanged-head dirty-state regression is unchanged.

The evidence helpers require process receipts bound to run, command, timestamps,
log and digest. Suite parsing rejects failures and unclosed runs, retaining
nested-child handling. Historical context is labeled separately from observed
execution. The capture CLI records the real child status and refuses overwriting
an existing log or receipt. To capture new evidence, use a new run/output path:

```sh
python3 validation/capture_process.py --receipt /new/run.json --log /new/run.log \
  --run-id unique-run-name -- cargo test --locked -p package
```

The historical CI writer now requires explicit `--repo`, `--artifact`,
`--process-evidence` and new `--output` paths. The font verifier also requires
`--process-evidence`. These interfaces deliberately reject the prior bare
invocations. Existing historical receipts are preserved, and missing original
process evidence is never manufactured. This batch does not requalify those old
product executions or infer that an accepted historical run failed.

Integration review corrected a binary-target regression in the first link patch,
removed incidental formatting outside the Hocket patch, corrected draft persona
home binding, and replaced an invalid H5 fixture while restoring its original
regression. Failed compile/fixture runs remain excluded. Final source scope is
four Hocket host files and the named evidence helpers/tests; no lock, shared DSP,
PCM fixture, native scenario or supplier source changed.
