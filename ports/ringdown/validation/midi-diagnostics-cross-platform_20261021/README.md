# Desktop diagnostic controls — Windows and Linux, 2026-10-21

**Status: Windows and Linux native acceptance pass; final Windows preservation
metadata collection is pending.**
The tested product source is merged main
`71ac17e15b6ae86ac58f2a14d3a3e62e2a4887a7`. Each host admitted a 1,240-file
SHA-256 source manifest after checking its earlier 1,137-file working snapshot.
Remote primary checkouts and historical qualification packets are preserved.

## Linux accepted cohort

`native/linux-r5/` is the first complete native cohort on Fedora Linux,
GNOME Wayland session with the application and GTK chooser on Xwayland/X11.
It does not qualify a native Wayland window path.

- Four fresh app processes exit normally: controls, same-profile reopen,
  pending-close/Cancel and pending-close/Save.
- Recording starts off. Record, trigger, Stop retains ten events. A real send
  while stopped reaches an independent ALSA receiver without changing history.
  Restarting recording appends to twenty events, including all four start/stop
  boundaries, with no overwrite or truncation. Latest-event display has 12 rows.
- Native GTK Cancel retains history and releases busy state. Native Save creates
  a new JSON whose whole Status matches the independent scripting Status,
  excluding its request id. The exported snapshot includes bindings and output
  facts as well as the full journal.
- Exit while recording is on, then reopen the same encrypted saved profile:
  recording is off, history is absent, bindings survive, and saved settings do
  not contain diagnostics preferences or history.
- Native X11 WM_DELETE_WINDOW reaches the actual host close guard during the
  chooser. The app remains open with the pending-export notice. Native Cancel
  or Save then permits normal exit. Close/Save writes the exact frozen Status;
  neither close process attempts MIDI. This is an injected native close event,
  not a physical mouse click.
- The independent receiver captures exactly three CC67 press/release gestures:
  `[176,67,127]`, `[176,67,0]`, repeated three times. No guitar is involved.

All four process receipts identify the same binary:
`ba12cdb4f448d6b2cf37ad7a33a3ba1f4ec8c1811916d5b37df5d74cd209016a`.
`independent-recheck.json` is produced by the separate read-only `verify.py`.
The host also reports 43 passing desktop MIDI/diagnostic cases and one passing
isolated journal lifecycle case. Their full logs are retained in `checks/linux-current-midi-tests.log` and
`checks/linux-current-journal-tests.log`. The later complete `native/linux-r7/` cohort adds reviewed scrolled readout
captures, including export notice, full latest-event history and the fresh
zero-history/off state. Its independent recheck also passes.

## Windows accepted cohort

`native/windows-r3/` is the complete native cohort on Windows 11, in the logged-in
user's interactive session. Four fresh app processes exit normally: controls,
same-profile reopen, pending-close/Cancel and pending-close/Save.

- Recording starts off. Start/trigger/Stop and restart/trigger/Stop retain eight
  events, including four recording boundaries and two explicit no-output
  dispatcher refusals. A request while stopped leaves history unchanged. The
  latest-event readout contains all eight retained events, with no overwrite or
  truncation.
- Actual native Win32 Cancel and Save controls exercise the production export
  worker. Cancel retains history and releases busy state. Save creates a new
  JSON whose complete Status matches the independent scripting Status, excluding
  its request id. A separate local read-only recheck passes.
- Exiting while recording is on and reopening the same encrypted saved profile
  starts recording off, with no history. Bindings survive; diagnostics recording
  preferences and history are absent from saved settings.
- Native WM_CLOSE reaches the actual main-window guard while a chooser is open.
  The pending-export notice keeps the app visible; native Cancel or Save then
  permits normal exit. Close/Save writes the exact frozen Status. These are
  injected native close events, not physical mouse clicks.
- All phases attempt and accept zero MIDI messages. This lane qualifies
  dispatcher-refusal diagnostics and desktop lifecycle behavior; Windows
  MIDI/BLE delivery remains unqualified. No virtual MIDI driver is installed.

All four process receipts identify the same binary:
`99c4468965e32da45e709a6675eb24fcc2b25b95e4079dc58712219162de5515`.
The host also passes the same 43 desktop MIDI/diagnostic cases and one isolated
journal lifecycle case as Linux. The temporary interactive scheduled task was
removed after exit with LastTaskResult 0 and no remaining task. The installed
Python 3.12 interpreter runs the acceptance driver; the broken Scoop launcher is
not used. `native_windows.ps1` targets only the owned app and dialog.

## Portability fix and driver findings

The product already used `rfd` in diagnostic export and pack preparation on all
desktops, but declared it only on macOS. The dependency is now common to desktop
hosts and enables its existing GTK backend on Linux. No chooser, writer or
journal implementation was duplicated. The integration packet records current
Woodshed/Hocket compilation and the shared-writer merge.

The first Linux Escape attempt left its native chooser open. Subsequent
superseded drivers exposed Python GI accessor and role-name mismatches. The
complete cohort uses the actual owned GTK dialog's AT-SPI filename field and
Cancel/Save actions. Failed attempts are retained on the host and are distinct
from product behavior; no product retry or protocol change was made.

The first Windows driver could not find Cancel through UI Automation. Its
successor reached the actual controls, but WM_SETTEXT alone did not commit the
shell dialog's folder/filename state for the final close/Save. The accepted
driver enters characters through the native Edit control, verifies its text,
and invokes the real Save/Cancel button. The superseded failures are retained
separately; neither establishes a product defect. Scrolled captures of retained
history and the fresh default-off reopen have been reviewed on both platforms.

The local native recipes use the retained scenario controls, the existing
Status command and file gates. New runs use fresh synthetic profile/keeper
roots, never normal user vaults. Rebuildable per-run GPU/font caches are removed
after owned processes exit; captures, saved profiles, transcripts and failures
are retained. No guitar is required. Retained source worktrees, stable build
targets and the 100 MB receipt budget are recorded in `status.json`.
