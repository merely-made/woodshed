# Windows and Linux MIDI diagnostic acceptance — 2026-10-21

Status: prepared; native replay awaits restoration of Djinn SSH identities. Linux app and synthetic fixture builds pass; the Windows build is still active. No Windows/Linux UI or dialog pass is claimed. This is a separate qualification packet; the macOS packet
remains frozen. Retained receipt budget: **100 MB total**. Source archives,
profiles and reusable native build targets stay outside this packet.

## Required observations

- Default-off recording and empty session history after fresh-process reopen of
  the same saved profile, with the configured bindings retained.
- Actual retained Record/Stop controls, appended restart history, latest-event
  readout, and unchanged captured history during an off-time request.
- Real native Cancel and Save, released busy state, retained cancellation history,
  and full JSON Status equality excluding only the request identifier.
- Native app close requests while a real save chooser is pending, followed by
  Cancel and Save in separate fresh processes; no replay or incidental MIDI.
- Focused journal/lifecycle/export-close checks on each native host.

The Linux lane reuses the prior control recipe and independent ALSA receiver.
The Windows lane records deliberate no-output dispatcher refusals to exercise
capture without installing a virtual MIDI driver. It does not qualify Windows
MIDI delivery. Both lanes use the existing Status projection, writer admission,
production retained controls, export worker, journal and persistence path.

## Initial portability finding

`woodshed-genet` used `rfd` unconditionally for diagnostic and pack preparation,
but declared the dependency only for macOS. The existing dependency now covers
all three desktop targets; Linux explicitly selects its existing GTK backend.
There is no second chooser implementation, recorder or writer. macOS compilation
passes after this manifest change. Native results are pending.

## Isolation and infrastructure

The source snapshot is transferred by SHA-256 manifest into owned detached
worktrees on O-PC and the ThinkPad. Their primary checkouts and normal profiles
are preserved. Existing stable target locations are reused. The prior encrypted
synthetic settings template and deterministic Djinn custody fixture supply a
saved profile with no normal user vault access.

`run_suite.py` composes controls, reopen, pending-close/cancel and
pending-close/save processes. Native chooser controls are answered through
Windows UI Automation or Linux X11 events. Close requests use native WM_CLOSE
or WM_DELETE_WINDOW; those are distinct from the earlier Mac host-command
qualification. All helpers are acceptance infrastructure only.

## Retained execution state

Owned source worktrees are `C:/Users/mark_/Code/worktrees/woodshed-midi-diagnostics`
and `/home/markik/Code/worktrees/woodshed-midi-diagnostics`. They retain the hashed
working source for this qualification task. Windows builds reuse
`C:/t/cargo-targets/woodshed` and `C:/t/cargo-targets/mere`; Linux uses the stable
Woodshed and Mere `target` paths. No isolated Cargo home was created. See
`status.json` for pending work and the exact SSH blocker. The Windows interactive
scheduled-task probe has not run because its SSH authentication failed before
any task was registered.
