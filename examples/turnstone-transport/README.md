# Woodshed transport in Turnstone

This Windows desktop regression starts a looping backing in an isolated Woodshed practice. The Turnstone Workbench pane controls the active backing with **Play/Pause**, **Previous bar**, **Next bar**, and **Gain**. Attach minimizes Woodshed's window. Detach restores the same window. Woodshed owns the song and audio output throughout.

The pane uses Turnstone's **Genet (Bole)** viewer. It reads the playing state and bar position from Woodshed. Commands go through a session-specific loopback address to the desktop event loop. This proof exercises a producer-owned web view and native window handoff. A shared WIT interface and general app picker are separate work.

Use Python 3.11 or later, a Windows desktop with an audio output, a Woodshed desktop build, and a Turnstone build with the `scripted-webgl` feature. Supply a fresh receipt directory each time.

```powershell
python examples/turnstone-transport/receive.py --woodshed C:/t/cargo-targets/woodshed/debug/woodshed-genet.exe --turnstone C:/t/cargo-targets/turnstone/debug/turnstone.exe --turnstone-root C:/Users/mark_/Code/repos/turnstone --receipt C:/Users/mark_/Code/testing/woodshed/receipts/transport-handoff
```

Add `--leave-playing` to leave the returned Woodshed window open. Closing Woodshed ends the receiver and releases its temporary profiles. The default regression closes both test applications.

Look in `result.json` for the Woodshed PID, native window handle, stream ID, callback revision, output gain and song cursor at each checkpoint. The regression checks uninterrupted playback on attach and detach. Between those handoffs it pauses, steps forward and back to bar boundaries, changes gain, and resumes. While paused, the device keeps rendering and the song cursor stays still. The `pane` directory contains the Workbench captures and the scenario outcome.
