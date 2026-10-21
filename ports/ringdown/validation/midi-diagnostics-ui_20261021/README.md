# Desktop MIDI diagnostic controls — 2026-10-21

**Implemented and compile-qualified. No automated tests or native UI/save-dialog
trial were run in this pass.** The historical input and guitar diagnostic
packets remain frozen and are not relabeled as qualification of this UI.

In Settings → Audio and MIDI, the existing pedal surface now provides:

- Record/Stop MIDI diagnostics: session-only, recording off on a normal launch.
  The existing explicit environment startup opt-in remains supported.
- Show/Hide latest events: up to 12 entries with timestamps, stage, request,
  connection identity, bytes and details.
- Recording/retained/overwritten/truncated readouts.
- Export MIDI diagnostics: the full retained history plus output status,
  bindings and last pedal report in a version-1 JSON envelope.

One shared instrument journal retains at most 512 events. Stop retains history;
start/stop boundary events make capture gaps visible. Nothing is saved in app
settings or projects. Off avoids detail construction/history locking on event
recording. UI polling uses a revision-keyed projection cache.

The existing scripting Status command and desktop export call one
`Dispatcher::status` function. Snapshot creation is read-only. A new-file save
dialog and background file job follow the established desktop rfd/wake/close
pattern; no musical action or connection is introduced. Export freezes bytes
on the click. Cancel/failure keeps history. Existing paths are refused, and a
pending export holds the window open until its dialog/job finishes or cancels.

Build checks: Mac desktop application, portable core, Windows GNU and Linux GNU
shared instrument with BLE enabled all pass, locked/offline. Logs live under
`checks/`. No new dependency, platform driver, MIDI writer or retry path was
added. Cross-target instrument compilation does not qualify native platform UI
or file-dialog behavior. Source/preservation manifests and the before-state
relative patch retain the exact scope of this pass.
