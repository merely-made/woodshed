# Current-persona selection

Qualified local D2 candidate on October 21, 2026. Source is `6ca442f6`, based on
freshly fetched `71ac17e1`. The primary checkout is preserved. No push is part of
this candidate.

Selecting a profile now only dismisses the switch picker when its roster
already marks that canonical profile chosen and an installed store is sealed
to it. The selection returns before transition-owned preservation, custody
opening or reset. Dirty authored work, save notices, the audio-session identity,
clocks, export tokens and close state remain installed. Ordinary dispatch
autosave remains outside this selection seam.

The roster check matters because this picker also changes the family's chosen
persona. If its roster shows a different choice, selecting the locally installed
persona still needs the normal custody transaction. A missing store, pending
seal, absent seal or different canonical identity also retains the existing
preservation/admission path. Equal display names cannot make a selection a no-op.

The pre-change negative control runs the new production-hook regression against
unchanged `71ac17e1` persona code. It exits 101 at the expected assertion:
`pending_persona_switch` is still created and a refused settings write is
observed. The fake store refuses writes before any real custody open can occur.
That failed control is retained and excluded from successful qualification.

The candidate's complete `persona::` gate passes **28 cases, zero failures and
zero ignores**, including two new tests. The second new test covers five
negative scenarios without inflating the case count. Exact process receipts,
raw-log digests, the baseline patch, tested source hashes and committed-source
equivalence are in [receipt.json](receipt.json) and [checks](checks/).

The local source commit has exactly the tested code and lock bytes. The process
receipt truthfully records the base HEAD while those source edits were dirty;
the retained hashes bind that execution to the committed candidate. The lock and
supplier cohort are unchanged from the recorded base.

The current phase and acceptance boundary live in the existing
[preservation plan](../../design_docs/2026-10-16_preservation_input_admission_plan.md#p4-current-persona-selection).
This is device-free host qualification. There is no new native GUI, real custody
IPC, physical capture or P5a acceptance. A Reload action, persona-store migration
and Redshank recovery UX retain their separate decisions and implementation scope.
