// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Scoped recorder layout and warm defaults. Hosts can append their authored
//! semantic role mapping without changing project or track colours.

pub fn sheet() -> &'static str {
    r#"/* This palette is local to a recorder mount; surrounding host styles do not
   receive generic table, transport, lane or mono rules. */
.hocket-recorder {
  box-sizing: border-box; font-family: sans-serif; font-size: 15px;
  --ground:#17130e; --surface:#201a12; --surface-2:#2b2318; --raised:#342a1c;
  --line:#3a3020; --line-soft:#2e2619; --text:#ece3d2; --text-dim:#a3937a; --text-faint:#6f6350;
  --voice-amber:#e0a64b; --voice-teal:#56b3a8; --voice-coral:#e0796a; --voice-sage:#a9b96b;
  --record:#e2493a; background-color:var(--ground); color:var(--text);
}
.hocket-recorder-surface { display:flex; flex-direction:column; width:100%; height:100%; min-height:0; min-width:0; }
.hocket-recorder-mono { font-family:monospace; }
.hocket-recorder-eyebrow { color:var(--text-faint); font-size:10px; text-transform:uppercase; letter-spacing:2px; padding:0 6px 10px; }
/* loop table */
.hocket-recorder-table { display: flex; flex-direction: column; flex-grow: 1; min-width: 0; min-height: 0;
         padding: 12px 20px 8px; overflow: scroll; }
.hocket-recorder-table-head { display: flex; padding: 0 2px 6px; }
.hocket-recorder-table-hint { flex-grow: 1; text-align: right; color: var(--text-faint); font-size: 12px; }

.hocket-recorder-lane { display: flex; align-items: stretch; background-color: var(--surface);
        flex-shrink: 0; border: 1px solid var(--line-soft); border-radius: 14px; padding: 11px 15px; margin-bottom: 9px; }
.hocket-recorder-lane-armed { border: 1px solid var(--voice); background-color: var(--surface-2); }
.hocket-recorder-lane-rec { border: 1px solid var(--record); background-color: var(--surface-2); }

.hocket-recorder-lane-id { display: flex; flex-direction: column; width: 168px; }
.hocket-recorder-lane-name { display: flex; align-items: center; }
.hocket-recorder-arm { width: 15px; height: 15px; border-radius: 8px; border: 2px solid var(--text-faint);
       margin-right: 9px; }
.hocket-recorder-lane-armed .hocket-recorder-arm { border: 2px solid var(--voice); background-color: var(--voice); }
.hocket-recorder-lane-rec .hocket-recorder-arm { border: 2px solid var(--record); background-color: var(--record); }
.hocket-recorder-lane-title { color: var(--text); font-size: 16px; }
.hocket-recorder-lane-meta { display: flex; padding-top: 6px; }
.hocket-recorder-tag { font-size: 11px; color: var(--text-dim); background-color: var(--raised);
       padding: 2px 7px; border-radius: 6px; margin-right: 8px; }
.hocket-recorder-tag-voice { color: var(--voice); }

.hocket-recorder-lane-wave { display: flex; flex-direction: column; flex-grow: 1; padding: 0 16px; }
.hocket-recorder-wave-summed { height: 40px; display: flex; align-items: center; flex-grow: 1; }
.hocket-recorder-layers { display: flex; flex-direction: column; padding-top: 4px; }
.hocket-recorder-layer { display: flex; align-items: center; height: 15px; }
.hocket-recorder-lnum { color: var(--text-faint); font-size: 9px; width: 18px; }
.hocket-recorder-layer-muted { opacity: 0.34; }
.hocket-recorder-wave-empty { color: var(--text-faint); font-size: 12px; align-self: center; }

.hocket-recorder-layer-wave { display: flex; align-items: center; flex-grow: 1; height: 11px; }
.hocket-recorder-wave-unavailable { color: var(--text-faint); font-size: 10px; align-self: center; }

.hocket-recorder-lane-ctl { display: flex; align-items: center; }
.hocket-recorder-lctl { width: 30px; height: 30px; border-radius: 8px; background-color: var(--raised);
        color: var(--text-dim); text-align: center; line-height: 30px; font-size: 12px; margin-left: 6px;
        border: 1px solid transparent; }
.hocket-recorder-lctl:hover { color: var(--text); border-color: var(--line); }
.hocket-recorder-lctl-on { color: var(--voice); }
.hocket-recorder-add-track { text-align: center; color: var(--text-faint); font-size: 13px; padding: 12px;
             border: 1px dashed var(--line); border-radius: 14px; }
.hocket-recorder-add-track:hover { color: var(--text-dim); border-color: var(--text-faint); }

/* transport */
.hocket-recorder-transport { display: flex; align-items: center; padding: 12px 24px 14px; flex-shrink: 0;
             border-top: 1px solid var(--line-soft); background-color: var(--surface); }
.hocket-recorder-t-left { display: flex; align-items: center; flex-grow: 1; }
.hocket-recorder-t-right { display: flex; align-items: center; flex-grow: 1; }
.hocket-recorder-t-right-inner { display: flex; align-items: center; margin-left: auto; }
.hocket-recorder-transport-wrap { flex-shrink:0; }
.hocket-recorder-status { color:var(--text-dim); font-size:11px; padding:4px 12px; }
.hocket-recorder-clear { width:auto; padding:0 8px; }
.hocket-recorder-stop-capture { width:auto; min-width:46px; padding:0 12px; }
.hocket-recorder-playing { color:var(--voice-teal); border-color:var(--voice-teal); }
.hocket-recorder-readout { display: flex; flex-direction: column; margin-right: 18px; }
.hocket-recorder-readout-val { font-size: 22px; color: var(--text); }
.hocket-recorder-readout-val small { font-size: 12px; color: var(--text-dim); }
.hocket-recorder-stepper { display: flex; align-items: center; }
.hocket-recorder-step { width: 26px; height: 26px; border-radius: 7px; background-color: var(--raised);
        border: 1px solid var(--line-soft); color: var(--text-dim); text-align: center; line-height: 24px; }
.hocket-recorder-step:hover { color: var(--text); }
.hocket-recorder-step-val { font-size: 22px; color: var(--text); padding: 0 10px; }
.hocket-recorder-toggles { display: flex; flex-direction: column; }
.hocket-recorder-toggle { display: flex; align-items: center; background-color: var(--surface-2);
          border: 1px solid var(--line-soft); color: var(--text-dim); font-size: 13px;
          padding: 7px 12px; border-radius: 10px; margin-bottom: 6px; }
.hocket-recorder-toggle-on { color: var(--text); border-color: var(--line); }
.hocket-recorder-led { width: 8px; height: 8px; border-radius: 4px; background-color: var(--text-faint); margin-right: 8px; }
.hocket-recorder-toggle-on .hocket-recorder-led { background-color: var(--voice-teal); }

.hocket-recorder-record-wrap { display: flex; flex-direction: column; align-items: center; }
.hocket-recorder-record { width: 76px; height: 76px; border-radius: 38px; background-color: #2a2013;
          border: 2px solid var(--line); display: flex; align-items: center; justify-content: center; }
.hocket-recorder-record-armed { border: 2px solid var(--record); }
.hocket-recorder-record-core { width: 36px; height: 36px; border-radius: 18px; background-color: var(--record); }
.hocket-recorder-record:hover { border-color: var(--record); }
.hocket-recorder-record-label { color: var(--text-dim); font-size: 12px; padding-top: 7px; }
.hocket-recorder-record-label b { color: var(--text); }

.hocket-recorder-stop { width: 46px; height: 46px; border-radius: 12px; background-color: var(--surface-2);
        border: 1px solid var(--line); color: var(--text-dim); text-align: center; line-height: 44px;
        font-size: 16px; margin-right: 16px; }
.hocket-recorder-stop:hover { color: var(--text); }
.hocket-recorder-meter { display: flex; align-items: flex-end; }
.hocket-recorder-mbars { display: flex; align-items: flex-end; height: 46px; margin-right: 12px; }
.hocket-recorder-mcol { display: flex; flex-direction: column; align-items: center; margin-right: 4px; }
.hocket-recorder-mbar-stack { display: flex; flex-direction: column-reverse; width: 12px; height: 40px; }
.hocket-recorder-mlbl { color: var(--text-faint); font-size: 9px; padding-top: 3px; }
.hocket-recorder-mseg { height: 3px; margin-top: 2px; border-radius: 1px; }
.hocket-recorder-mpeak { color: var(--text-dim); font-size: 11px; }

/* Compact changes arrangement only; all recorder actions and real leaves stay. */
/* One compact scroll surface keeps the transport after every lane, even when
   its wrapped controls are taller than the available recorder viewport. */
.hocket-recorder-compact.hocket-recorder-surface { overflow:auto; }
.hocket-recorder-compact.hocket-recorder-table { flex:0 0 auto; overflow:visible; padding:8px; }
.hocket-recorder-compact .hocket-recorder-lane { flex-direction:column; padding:10px; }
.hocket-recorder-compact .hocket-recorder-lane-id { width:auto; }
.hocket-recorder-compact .hocket-recorder-lane-wave { padding:8px 0; }
.hocket-recorder-compact .hocket-recorder-lane-ctl { justify-content:flex-end; }
.hocket-recorder-compact.hocket-recorder-transport { flex-wrap:wrap; gap:8px; padding:10px; }
.hocket-recorder-compact .hocket-recorder-record { width:54px; height:54px; }
.hocket-recorder-compact .hocket-recorder-record-core { width:28px; height:28px; }
.hocket-recorder-compact .hocket-recorder-t-right-inner { flex-wrap:wrap; gap:4px; }
"#
}
