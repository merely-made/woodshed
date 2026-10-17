# Preservation/input integration onto P5 — 2026-10-17

This is the current-source CPU integration receipt. The October 16 receipt is
historical and remains unchanged. Source baseline is
`b88cb26836cffb4c4b091d15c53bb1014d575cdc`; isolated changes are from `07c170b`
and `860c5c1`. `integration.json` records source hashes and actual command exits.
Native qualification and publication belong to the coordinating chat after this
lane hands off its local commit. No native applications are launched here.

The two additive conflicts preserve both documentation entries and both
`practice_clock_managed` and `event_clock` across persona reset. The combined
reset regression also checks that transient run position/program is cleared.
Other automatically merged product files retain their P5 changes.

## Native handoff hooks

- Woodshed: `desktop_root` in `crates/woodshed-genet/src/main.rs` exposes
  `.practice-close-retry`, `.practice-close-cancel`, `.practice-close-discard`;
  persona decisions expose `.persona-save-retry`, `.persona-save-cancel`, and
  `.persona-save-discard`. Native close uses `prepare_close`; a pending switch
  blocks other close paths. Settings opens the existing real persona picker.
- `WOODSHED_STATE` and `WOODSHED_SETTINGS` isolate owned fixture files. Invalid
  retained history can exercise the real quarantine/close refusal; an owned
  failing destination can exercise filesystem save refusal. Existing scenario
  observations include `practice-saved` and `practice-save-problem`.
- Persona completion runs from the production `after_wake` hook after codec
  poll/publication. Controlled encoder blocking and injected Pending custody
  openers exist only in CPU tests. Shipping scenarios have no deterministic
  worker-pause or failed-opener action; a native claim needs actual queued-take
  evidence or a separately reviewed bounded fixture seam. Screenshots alone do
  not prove the exact codec-wake-to-install sequence.
- Hocket: preservation panel buttons have `data-action` values
  `preservation-save`, `preservation-discard`, `preservation-cancel`, and
  `preservation-retry`. Retained loaded-project controls use
  `retry-loaded-project` / `dismiss-loaded-project`. Dirty project close can be
  exercised with a tempo edit and actual close, without opening a microphone.
  Canceled/failed Save must retain the project. Active capture replacement
  rejection needs an admitted capture or separately bounded fixture.
- Redshank: `REDSHANK_DATA_DIR` isolates immutable `state-N.json` generations.
  A sole corrupt generation yields a latched “Not saved” recovery notice, no
  storage worker and close refusal. Older valid fallback is a separate case.
  This change adds no repair/reset/Retry/Discard recovery UI.

Use independent fixture directories and hash original evidence before/after
native runs. Existing native receipts retain their historical source; this CPU
receipt makes no headed, microphone, playback or physical-audio claim.

Final local gate: all three test processes and desktop builds exited zero; 1,355
passing cases plus five capture child invocations counted separately, nine ignored
tests. Original supplier lock bytes remain unchanged. Executable hashes and exact
paths are in `integration.json`; native acceptance and publication are pending in
the coordinating chat.


Bounded Woodshed native follow-through is now recorded in
[`native/README.md`](native/README.md). Its historical persona run uses `cec2213`;
the P6 candidate `d955f5d` adds and qualifies the shared frame/dispatch close
guard. Hocket/Redshank native UI and test-only failure cases retain their limits.
The CPU receipt above remains unchanged and does not acquire native claims.
