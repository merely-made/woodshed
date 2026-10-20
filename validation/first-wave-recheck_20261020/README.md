# A1–A5 and Q1 current-source recheck

Rechecked on October 20, 2026, from freshly fetched `origin/main`
`9f80bb5eabc4b513e057b22af6f80d83ecb8e9ac`. The isolated branch is
`codex/audit-first-wave-20261020`. The primary checkout and unrelated work were
preserved. Two concrete corrections are committed locally at `22c9900` (A1)
and `6a65a4d` (Q1). No push is authorized or performed in this wave.

The parallel lanes first traced current production callers rather than treating
the frozen audit as a current defect list. One Hocket owner covered A2, A3 and
Hocket's A4 model/project/handoff boundary, avoiding competing edits to its
shared state. A1 owned Woodshed lifecycle, A5 owned Redshank recovery, and the
coordinator owned Woodshed A4, Q1, tests, documentation and Git. P5a remains
separate with its existing owner and acceptance record.

| Package | Current result |
|---|---|
| A1 Woodshed transition | Existing preservation and clock retention remain. New admission guard refuses an incoming sealed persona different from the selected target before retiring outgoing practice. Explicit Retry can admit the correct persona. |
| A2 Hocket transition | Existing full-bundle/media revision dirtiness, Save/Discard/Cancel, active capture/I/O barriers, completion rechecks and missing-media refresh remain. No new source patch warranted. |
| A3 Hocket consent | Existing profile/key/losing-token binding and refusal-before-consent-write remain. No new source patch warranted. |
| A4 persisted models | Existing explicit history-wire discrimination, consumer Stemma guard, loaded Set identity admission and Hocket graph/project/handoff checks remain. The stale supplier comment now identifies locked Eidetic `738ef159`. A fallible supplier API remains a concrete follow-up. |
| A5 Redshank recovery | Existing failed-generation refusal, valid older fallback and latched nonwriting recovery remain, including edit/retry/close refusal. No new source patch warranted. |
| Q1 evidence/diagnostics | Existing process/link corrections pass current controls. New strict terminal result makes all `audio_ddx` modes require output progress and zero observed device events. Product runtime recovery policy is unchanged. |

The source records are [A1](sources/a1.md), [Hocket A2/A3/A4](sources/hocket.md),
[Woodshed A4 and Q1](sources/core-q1.md), and [A5](sources/a5.md).
They retain caller evidence and source hashes; their individual handoffs precede
the coordinator's executable results. The [aggregate receipt](receipt.json)
records exact source/lock hashes, commands, process exits, raw-log digests and
the [captured checks](checks/).

| Current software gate | Passed | Ignored |
|---|---:|---:|
| Woodshed core, woodshedding, Hocket model and project libraries | 603 | 0 |
| Woodshed host, including new A1 regression and production preservation/close paths | 281 | 7 |
| Hocket host lifecycle/identity/project admission | 94 | 2 |
| Redshank storage and three targeted desktop recovery/restoration cases | 18 | 0 |
| Process-evidence and historical-link controls | 21 | 0 |
| `audio_ddx` qualification outcomes | 3 | 0 |
| **Distinct parent/library/control cases** | **1,020** | **9** |

All nine counted commands exited zero. Five successful Woodshed child-process
invocations are recorded separately, without adding them again to the distinct
case total. The preliminary model, diagnostic and 26-case persona runs are also
excluded from that total. Existing fixture/custody/native helpers remain ignored.
Raw logs and original process receipts are copied byte-for-byte; their absolute
paths identify the external capture, while the aggregate indexes repository-local
copies by relative path and verifies their digests. No historical product
receipt was rewritten or requalified.

Independent read-only review of the A1/Q1 patch found no actionable blocker.
Source/document whitespace checks pass. The new regression exercises the live
transition hook with wrong identity, retained outgoing instructions/store/audio
epoch/export state, no automatic retry, then explicit admission and preserved
clock bindings. Diagnostic controls require no hardware.

The remaining design choices are D1 persona storage namespaces/migration, D2
future capture replacement and same-persona no-op behavior, and D3 failed-recovery
repair/reset/exit interaction. No ruling is inferred here. A checked Stemma
supplier reconstruction API is a separate concrete follow-up; the locked
supplier still exposes trusted indexing and the current consumer guard stays.

This receipt establishes source and device-free CPU results. There was no new
headed desktop run, real custody IPC, native fresh-process persona/project
reopen, physical audio/microphone/MIDI run, P5a acceptance or publication. The
test subprocesses are distinct from headed native acceptance. Existing native
receipts remain tied to their own source and suppliers.
