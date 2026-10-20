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

/// Recorder layout and the retained warm default. Shared roles override colors
/// only after an explicit application appearance selection.
pub fn sheet() -> String {
    format!(
        "{}\n{}",
        r#"
/* Recorder colors live on `.app`; the desktop wrapper receives shared root
   tokens. Semantic voice/record colors and inline lane `--voice` values stay
   with the recorder. */
.hocket-desktop { --ground: #17130e; --surface: #201a12; --text: #ece3d2; --voice-amber: #e0a64b; }
.app { display: flex; flex-direction: column; width: 100%; height: 100%;
       box-sizing: border-box; font-family: sans-serif; font-size: 15px;
       --ground: #17130e; --surface: #201a12; --surface-2: #2b2318; --raised: #342a1c;
       --line: #3a3020; --line-soft: #2e2619;
       --text: #ece3d2; --text-dim: #a3937a; --text-faint: #6f6350;
       --voice-amber: #e0a64b; --voice-teal: #56b3a8; --voice-coral: #e0796a; --voice-sage: #a9b96b;
       --record: #e2493a; --record-glow: rgba(226,73,58,0.28);
       background-color: var(--ground); color: var(--text); }
.mono { font-family: monospace; }
.project-preservation { display:flex; flex-wrap:wrap; align-items:center; gap:10px;
    padding:12px 18px; flex-shrink:0; background:var(--surface); color:var(--text); }
.project-preservation p { width:100%; margin:0; }
.project-preservation button { display:block; padding:8px 12px;
    border:1px solid currentColor; color:inherit; background:transparent; }
.hocket-desktop > .hocket-editor { flex:1; height:auto; min-height:0; }

/* top strip */
.top { display: flex; align-items: center; flex-wrap: wrap; row-gap: 8px;
       padding: 12px 22px; flex-shrink: 0;
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
   ellipsizes, and `--update-now` prints it in full.
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
/* Keep command labels inside their hit bounds when a long project status
   fills the compact toolbar. Wrap whole controls onto another row. */
.top .project-command, .top .chip, .top .export-length { flex-shrink: 0; white-space: nowrap; }
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
"#,
        hocket_views::theme::sheet()
    )
}
/// Portable built-in copy retains the recorder's exact palette in dark mode.
pub fn builtin() -> tabard::Theme {
    let mut theme = tabard::Theme::new(
        crate::appearance::PRODUCT_DEFAULT,
        "Hocket warm",
        tinct::Seeds {
            primary: tinct::Srgb::rgb(0xe0, 0xa6, 0x4b),
            secondary: tinct::Srgb::rgb(0x56, 0xb3, 0xa8),
            tertiary: tinct::Srgb::rgb(0xe0, 0x79, 0x6a),
            neutral: tinct::Srgb::rgb(0x20, 0x1a, 0x12),
            text_header: None,
            text_body: None,
            success: tinct::Srgb::rgb(0xa9, 0xb9, 0x6b),
            danger: tinct::Srgb::rgb(0xe2, 0x49, 0x3a),
            dark: true,
        },
    );
    theme.source = tabard::theme::registry::ThemeSource::BuiltIn;
    theme.mode_sheets.insert(
        "dark".into(),
        vec![format!(
            "{}\n.hocket-desktop, .app, .hocket-recorder {{ --line:#3a3020; --line-soft:#2e2619; }}",
            DEFAULT_TOKENS
        )],
    );
    theme
}

const DEFAULT_TOKENS: &str = ":root { --tabard-color-bg:#17130e; --tabard-color-surface:#201a12; --tabard-color-surface-2:#2b2318; --tabard-color-surface-hover:#342a1c; --tabard-color-text:#ece3d2; --tabard-color-text-dim:#a3937a; --tabard-color-text-disabled:#6f6350; --tabard-color-text-header:#ece3d2; --tabard-color-primary:#e0a64b; --tabard-color-on-primary:#17130e; --tabard-syntax-link:#56b3a8; --tabard-syntax-emphasis:#ece3d2; --tabard-syntax-keyword:#e0a64b; --tabard-syntax-string:#a9b96b; --tabard-syntax-number:#e0796a; }";

/// Project authorship/track colors and recording red remain semantic colors.
pub fn role_css() -> &'static str {
    ".hocket-desktop, .app, .hocket-recorder { --ground:var(--tabard-color-bg); --surface:var(--tabard-color-surface); --surface-2:var(--tabard-color-surface-2); --raised:var(--tabard-color-surface-hover); --line:var(--tabard-color-surface-hover); --line-soft:var(--tabard-color-surface-hover); --text:var(--tabard-color-text); --text-dim:var(--tabard-color-text-dim); --text-faint:var(--tabard-color-text-disabled); }"
}

/// Sparse authored sheets can use shared tokens or recorder selectors. Omitted
/// token roles retain the warm palette rather than becoming unset CSS values.
pub fn default_tokens() -> &'static str {
    DEFAULT_TOKENS
}
