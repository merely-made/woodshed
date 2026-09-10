// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! The Hocket sheet: the warm dim-practice-room palette + the one-screen
//! loop-recorder layout (pass-the-mic rail | loop table | transport),
//! ported from the 2026-07-08 UI concept. Flexbox throughout (genet's
//! exercised path); per-lane owner colour rides a `--voice` custom
//! property set inline. S5 replaces the waveform/meter stand-ins with
//! chisel leaves.

/// The full stylesheet. Static (no per-frame interpolation); the design
/// is a fixed warm-dark world for v1.
pub fn sheet() -> String {
    r#"
/* Palette lives on `.app`, the tree's root element — so `:root` and `.app`
   are the same element here and the vars cascade to every descendant, like the
   inline per-lane `--voice` does. */
.app { display: flex; flex-direction: column; width: 100%; height: 100%;
       box-sizing: border-box; font-family: sans-serif; font-size: 15px;
       --ground: #17130e; --surface: #201a12; --surface-2: #2b2318; --raised: #342a1c;
       --line: #3a3020; --line-soft: #2e2619;
       --text: #ece3d2; --text-dim: #a3937a; --text-faint: #6f6350;
       --voice-amber: #e0a64b; --voice-teal: #56b3a8; --voice-coral: #e0796a; --voice-sage: #a9b96b;
       --record: #e2493a; --record-glow: rgba(226,73,58,0.28);
       background-color: var(--ground); color: var(--text); }
.mono { font-family: monospace; }

/* top strip */
.top { display: flex; align-items: center; padding: 12px 22px; flex-shrink: 0;
       border-bottom: 1px solid var(--line-soft); }
.brand { font-size: 17px; color: var(--text); }
.brand-dot { color: var(--voice-amber); }
.session-name { color: var(--text-dim); font-size: 13px; padding-left: 14px; }
.project-status { color: var(--text-faint); font-size: 11px; padding-left: 10px; max-width: 220px;
                  overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
/* Update status. Sits beside the project status and must read as an aside,
   not a headline: without its own rule it inherits the default size and
   shouts over the session name it abuts. Amber because it is actionable
   information rather than a fault. Wide enough that the common
   "Version x.y.z available" fits whole; longer failure text still
   ellipsizes, and `--update-now` prints it in full. */
   `flex-shrink: 0` is what actually keeps it whole: in this flex row a
   default-shrink item gets squeezed and ellipsises mid-word regardless of
   max-width. The cap still bounds a long failure message. */
.update-status { color: var(--voice-amber); font-size: 11px; padding-left: 16px;
                 flex-shrink: 0; max-width: 330px; overflow: hidden;
                 white-space: nowrap; text-overflow: ellipsis; }
/* The same chip when it is also the button. Bordered so it reads as
   clickable rather than as a label that happens to respond. */
.update-action { border: 1px solid var(--voice-amber); border-radius: 7px;
                 padding: 5px 10px; margin-left: 10px; }
.update-action:hover { background-color: var(--raised); }
.top-spacer { flex-grow: 1; }
.chip { background-color: var(--surface); border: 1px solid var(--line-soft); color: var(--text-dim);
        font-size: 12px; padding: 5px 11px; border-radius: 14px; margin-right: 12px; }
.project-command { height: 30px; border-radius: 7px; border: 1px solid var(--line-soft);
                   background-color: var(--surface); color: var(--text-dim); font-size: 12px;
                   line-height: 30px; padding: 0 10px; margin-left: 7px; }
.project-command:hover { color: var(--text); border-color: var(--line); }
.project-save { color: var(--voice-teal); }
.export-length { display: flex; align-items: center; height: 30px; margin-left: 10px;
                 border: 1px solid var(--line-soft); border-radius: 7px; overflow: hidden; }
.export-mode-choice { color: var(--text-faint); font-size: 11px; line-height: 30px; padding: 0 8px;
                      border-right: 1px solid var(--line-soft); }
.export-mode-choice:hover { color: var(--text); }
.export-mode-choice-on { color: var(--voice-amber); background-color: var(--raised); }
.export-step { color: var(--text-dim); font-size: 15px; line-height: 30px; padding: 0 7px; }
.export-step:hover { color: var(--text); background-color: var(--raised); }
.export-bars { color: var(--text-dim); font-size: 10px; min-width: 38px; text-align: center; }

/* middle row: rail + table. `min-height: 0` overrides flexbox's default
   `min-height: auto` so `.body` can shrink below its lanes' min-content and
   `.table` scrolls internally instead of shoving `.transport` off-screen. */
.body { display: flex; flex-grow: 1; min-height: 0; }

/* pass-the-mic rail */
.rail { display: flex; flex-direction: column; width: 208px; flex-shrink: 0; padding: 16px 12px;
        border-right: 1px solid var(--line-soft); }
.eyebrow { color: var(--text-faint); font-size: 10px; text-transform: uppercase; letter-spacing: 2px;
           padding: 0 6px 10px; }
.peer { display: flex; align-items: center; padding: 9px 8px; border-radius: 11px; margin-bottom: 2px; }
.peer-turn { background-color: var(--surface); }
.av { width: 34px; height: 34px; border-radius: 17px; color: #17130e; text-align: center;
      line-height: 34px; font-size: 13px; margin-right: 11px; }
.who { display: flex; flex-direction: column; }
.who-name { color: var(--text); font-size: 14px; }
.who-sub { color: var(--text-faint); font-size: 11px; }
.peer-turn .who-sub { color: var(--voice-amber); }
.mic { flex-grow: 1; text-align: right; font-size: 15px; }
.rail-spacer { flex-grow: 1; }
.handoff { background-color: var(--voice-amber); color: #17130e; border-radius: 12px; padding: 12px;
           text-align: center; font-size: 14px; }
.handoff:hover { background-color: #edb45a; }
.handoff-note { text-align: center; color: var(--text-faint); font-size: 11px; padding-top: 8px; }

/* loop table */
.table { display: flex; flex-direction: column; flex-grow: 1; min-width: 0; min-height: 0;
         padding: 12px 20px 8px; overflow: scroll; }
.table-head { display: flex; padding: 0 2px 6px; }
.table-hint { flex-grow: 1; text-align: right; color: var(--text-faint); font-size: 12px; }

.lane { display: flex; align-items: stretch; background-color: var(--surface);
        border: 1px solid var(--line-soft); border-radius: 14px; padding: 11px 15px; margin-bottom: 9px; }
.lane-armed { border: 1px solid var(--voice); background-color: var(--surface-2); }
.lane-rec { border: 1px solid var(--record); background-color: var(--surface-2); }

.lane-id { display: flex; flex-direction: column; width: 168px; }
.lane-name { display: flex; align-items: center; }
.arm { width: 15px; height: 15px; border-radius: 8px; border: 2px solid var(--text-faint);
       margin-right: 9px; }
.lane-armed .arm { border: 2px solid var(--voice); background-color: var(--voice); }
.lane-rec .arm { border: 2px solid var(--record); background-color: var(--record); }
.lane-title { color: var(--text); font-size: 16px; }
.lane-meta { display: flex; padding-top: 6px; }
.tag { font-size: 11px; color: var(--text-dim); background-color: var(--raised);
       padding: 2px 7px; border-radius: 6px; margin-right: 8px; }
.tag-voice { color: var(--voice); }

.lane-wave { display: flex; flex-direction: column; flex-grow: 1; padding: 0 16px; }
.wave-summed { height: 40px; display: flex; align-items: center; flex-grow: 1; }
.layers { display: flex; flex-direction: column; padding-top: 4px; }
.layer { display: flex; align-items: center; height: 15px; }
.lnum { color: var(--text-faint); font-size: 9px; width: 18px; }
.layer-muted { opacity: 0.34; }
.wave-empty { color: var(--text-faint); font-size: 12px; align-self: center; }

.layer-wave { display: flex; align-items: center; flex-grow: 1; height: 11px; }
.wave-unavailable { color: var(--text-faint); font-size: 10px; align-self: center; }

.lane-ctl { display: flex; align-items: center; }
.lctl { width: 30px; height: 30px; border-radius: 8px; background-color: var(--raised);
        color: var(--text-dim); text-align: center; line-height: 30px; font-size: 12px; margin-left: 6px;
        border: 1px solid transparent; }
.lctl:hover { color: var(--text); border-color: var(--line); }
.lctl-on { color: var(--voice); }
.add-track { text-align: center; color: var(--text-faint); font-size: 13px; padding: 12px;
             border: 1px dashed var(--line); border-radius: 14px; }
.add-track:hover { color: var(--text-dim); border-color: var(--text-faint); }

/* transport */
.transport { display: flex; align-items: center; padding: 12px 24px 14px; flex-shrink: 0;
             border-top: 1px solid var(--line-soft); background-color: var(--surface); }
.t-left { display: flex; align-items: center; flex-grow: 1; }
.t-right { display: flex; align-items: center; flex-grow: 1; }
.t-right-inner { display: flex; align-items: center; margin-left: auto; }
.audio-devices { display: flex; flex-direction: column; align-items: flex-end; margin-right: 14px; }
.device-select { display: flex; align-items: center; margin-bottom: 3px; }
.device-label { color: var(--text-faint); font-size: 9px; width: 24px; text-transform: uppercase; }
.device-select .select { min-width: 154px; z-index: 2; }
.device-select .select-box { height: 20px; box-sizing: border-box; overflow: hidden; white-space: nowrap;
                             text-overflow: ellipsis; background-color: var(--raised); color: var(--text-dim);
                             border: 1px solid var(--line-soft); border-radius: 4px; font-size: 10px;
                             line-height: 18px; padding: 0 8px; }
.device-select .select-box:hover { color: var(--text); border-color: var(--line); }
.device-select .select-list { max-width: 240px; max-height: 170px; overflow: scroll; z-index: 3;
                              background-color: var(--surface-2); border: 1px solid var(--line);
                              box-sizing: border-box; }
.device-select .select-option { color: var(--text-dim); font-size: 11px; padding: 5px 8px; }
.device-select .select-option:hover { color: var(--text); background-color: var(--raised); }
.audio-status { color: var(--text-faint); font-size: 9px; max-width: 178px; overflow: hidden;
                white-space: nowrap; text-overflow: ellipsis; }
.readout { display: flex; flex-direction: column; margin-right: 18px; }
.readout-val { font-size: 22px; color: var(--text); }
.readout-val small { font-size: 12px; color: var(--text-dim); }
.stepper { display: flex; align-items: center; }
.step { width: 26px; height: 26px; border-radius: 7px; background-color: var(--raised);
        border: 1px solid var(--line-soft); color: var(--text-dim); text-align: center; line-height: 24px; }
.step:hover { color: var(--text); }
.step-val { font-size: 22px; color: var(--text); padding: 0 10px; }
.toggles { display: flex; flex-direction: column; }
.toggle { display: flex; align-items: center; background-color: var(--surface-2);
          border: 1px solid var(--line-soft); color: var(--text-dim); font-size: 13px;
          padding: 7px 12px; border-radius: 10px; margin-bottom: 6px; }
.toggle-on { color: var(--text); border-color: var(--line); }
.led { width: 8px; height: 8px; border-radius: 4px; background-color: var(--text-faint); margin-right: 8px; }
.toggle-on .led { background-color: var(--voice-teal); }

.record-wrap { display: flex; flex-direction: column; align-items: center; }
.record { width: 76px; height: 76px; border-radius: 38px; background-color: #2a2013;
          border: 2px solid var(--line); display: flex; align-items: center; justify-content: center; }
.record-armed { border: 2px solid var(--record); }
.record-core { width: 36px; height: 36px; border-radius: 18px; background-color: var(--record); }
.record:hover { border-color: var(--record); }
.record-label { color: var(--text-dim); font-size: 12px; padding-top: 7px; }
.record-label b { color: var(--text); }

.stop { width: 46px; height: 46px; border-radius: 12px; background-color: var(--surface-2);
        border: 1px solid var(--line); color: var(--text-dim); text-align: center; line-height: 44px;
        font-size: 16px; margin-right: 16px; }
.stop:hover { color: var(--text); }
.meter { display: flex; align-items: flex-end; }
.mbars { display: flex; align-items: flex-end; height: 46px; margin-right: 12px; }
.mcol { display: flex; flex-direction: column; align-items: center; margin-right: 4px; }
.mbar-stack { display: flex; flex-direction: column-reverse; width: 12px; height: 40px; }
.mlbl { color: var(--text-faint); font-size: 9px; padding-top: 3px; }
.mseg { height: 3px; margin-top: 2px; border-radius: 1px; }
.mpeak { color: var(--text-dim); font-size: 11px; }
"#
    .to_string()
}
