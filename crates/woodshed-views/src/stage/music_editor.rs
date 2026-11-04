//! Set-owned MusicXML editing. The score is a working reading of the saved XML;
//! musical views and selection never create another persistent document.
use super::{UiChild, UiState};
use crate::notation_leaf::{NotationLeaf, NotationView};
use crate::workspace::WorkspacePanel;
mod gestures;
mod pattern;
mod playback_board;
pub mod practice;
use cambium::{
    History, Key, KeyEvent, NamedKey, SelectState, clickable, custom_leaf, el, map_state, on_key,
    on_pointer, select, text,
};
use std::collections::BTreeMap;
use woodshed_core::working_sets::WorkingSetId;
use woodshedding::notation::{
    ChordKind, ChordSymbol, FrettedNote, Measure, MusicDocument, NoteEvent, NoteValue, Score,
    TimeSignature,
};

pub struct MusicDraft {
    pub score: Score,
    base: Option<MusicDocument>,
    saved_score: Option<Score>,
    history: History<Score, ()>,
    pub selected: (usize, usize),
    pub view: NotationView,
}
impl MusicDraft {
    fn new(score: Score, base: Option<MusicDocument>) -> Self {
        Self {
            saved_score: base.as_ref().map(|_| score.clone()),
            score,
            base,
            history: History::new(),
            selected: (0, 0),
            view: NotationView::Chart,
        }
    }
    pub fn dirty(&self) -> bool {
        self.saved_score.as_ref() != Some(&self.score)
    }
    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }
    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }
    fn clamp(&mut self) {
        self.selected.0 = self
            .selected
            .0
            .min(self.score.measures.len().saturating_sub(1));
        self.selected.1 = self.selected.1.min(
            self.score.measures[self.selected.0]
                .events
                .len()
                .saturating_sub(1),
        );
    }
    fn change(
        &mut self,
        edit: impl FnOnce(&mut Score, (usize, usize)) -> Result<(), String>,
    ) -> Result<(), String> {
        let before = self.score.clone();
        let mut next = before.clone();
        edit(&mut next, self.selected)?;
        next.validate()?;
        if next != before {
            self.history.break_run();
            self.history.record(before, None, 0);
            self.score = next;
            self.clamp();
        }
        Ok(())
    }
}
pub struct MusicEditor {
    pub practice: practice::MusicPractice,
    pub follow_practice: bool,
    pub follow_range: bool,
    pub follow_first: SelectState,
    pub follow_last: SelectState,
    pub drafts: BTreeMap<WorkingSetId, MusicDraft>,
    pub notice: Option<String>,
    pub export_request: Option<(String, String)>,
    pub legacy: bool,
    pub choose_chord: bool,
    pub active_tone: Option<FrettedNote>,
    fret_gesture: Option<gestures::FretGesture>,
    picking_preview: Option<pattern::PickingPreview>,
    pub chord_root: SelectState,
    pub chord_kind: SelectState,
}
impl Default for MusicEditor {
    fn default() -> Self {
        Self {
            practice: Default::default(),
            follow_practice: false,
            follow_range: false,
            follow_first: SelectState::new(0).with_label("First bar"),
            follow_last: SelectState::new(0).with_label("Last bar"),
            drafts: Default::default(),
            notice: None,
            export_request: None,
            legacy: false,
            choose_chord: false,
            active_tone: None,
            fret_gesture: None,
            picking_preview: None,
            chord_root: SelectState::new(0).with_label("Chord root"),
            chord_kind: SelectState::new(0).with_label("Chord quality"),
        }
    }
}
const CHORD_KINDS: [ChordKind; 9] = [
    ChordKind::Major,
    ChordKind::Minor,
    ChordKind::Dominant7,
    ChordKind::Minor7,
    ChordKind::Major7,
    ChordKind::Diminished,
    ChordKind::Augmented,
    ChordKind::Sus2,
    ChordKind::Sus4,
];
const QUALITY_LABELS: [&str; 9] = [
    "Major",
    "Minor",
    "Dominant 7",
    "Minor 7",
    "Major 7",
    "Diminished",
    "Augmented",
    "Sus2",
    "Sus4",
];
impl MusicEditor {
    pub fn dirty(&self) -> bool {
        self.drafts.values().any(MusicDraft::dirty)
    }
    pub fn draft(&self, owner: WorkingSetId) -> Option<&MusicDraft> {
        self.drafts.get(&owner)
    }
}
fn note_name(midi: u8) -> String {
    format!(
        "{}{}",
        [
            "C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"
        ][usize::from(midi % 12)],
        i16::from(midi) / 12 - 1
    )
}
fn frequency(midi: u8) -> f32 {
    440.0 * 2.0_f32.powf((f32::from(midi) - 69.0) / 12.0)
}
fn view_label(view: NotationView) -> &'static str {
    match view {
        NotationView::Chart => "Chart",
        NotationView::Tab => "Tab",
        NotationView::Score => "Score",
    }
}

impl UiState {
    pub fn open_music_chord(&mut self) {
        let symbol = self
            .music_draft()
            .and_then(|d| d.score.measures[d.selected.0].harmony.clone());
        if let Some(symbol) = symbol {
            self.music_editor.chord_root.selected = usize::from(symbol.root);
            self.music_editor.chord_kind.selected = CHORD_KINDS
                .iter()
                .position(|kind| *kind == symbol.kind)
                .unwrap_or(0);
        }
        self.music_editor.choose_chord = true;
    }
    pub fn apply_music_chord(&mut self) {
        let symbol = ChordSymbol {
            root: self.music_editor.chord_root.selected.min(11) as u8,
            kind: CHORD_KINDS[self.music_editor.chord_kind.selected.min(8)],
        };
        self.music_symbol(symbol);
        self.music_editor.choose_chord = false;
    }
    pub fn music_sustain(&mut self) {
        self.edit_music(|score,(b,n)|{let m=&mut score.measures[b];let units=u32::from(m.meter.beats)*24/u32::from(m.meter.beat_type);let value=match units{24=>NoteValue::Whole,12=>NoteValue::Half,6=>NoteValue::Quarter,3=>NoteValue::Eighth,_=>return Err("This bar needs tied or dotted notation before it can hold one sustained event.".into())};let mut event=m.events[n].clone();event.value=value;m.events=vec![event];Ok(())},"Selection sustained for the bar.");
    }
    pub fn music_key(&mut self, event: KeyEvent) {
        if matches!(event.key, Key::Named(NamedKey::Escape)) {
            self.cancel_music_drag();
            if self.music_editor.picking_preview.is_some() {
                self.cancel_music_pattern();
            }
            event.prevent_default();
            return;
        }
        let command = event.mods.ctrl || event.mods.meta;
        if let Key::Character(c) = &event.key {
            if command && c.eq_ignore_ascii_case("z") {
                if event.mods.shift {
                    self.music_redo();
                } else {
                    self.music_undo();
                }
                event.prevent_default();
                event.stop_propagation();
            }
        }
    }
}

fn music_from_set(
    stage: &woodshed_core::StageState,
    set: &woodshedding::rehearsal::Set,
    title: &str,
    fallback_bpm: f32,
) -> Result<Score, String> {
    woodshed_core::music::score_from_set(stage, set, title, fallback_bpm)
}
fn action(label: impl Into<String>, class: &str, f: impl Fn(&mut UiState) + 'static) -> UiChild {
    Box::new(clickable(
        el("button", text(label.into())).attr("class", format!("t-btn {class}")),
        move |ui: &mut UiState, _| f(ui),
    ))
}
fn colors(ui: &UiState) -> (sprigging::ColorF, sprigging::ColorF) {
    let p = ui.appearance.resolve(&ui.app_settings.appearance).palette;
    let convert = |c: tinct::Srgb| sprigging::ColorF {
        r: c.r as f32 / 255.0,
        g: c.g as f32 / 255.0,
        b: c.b as f32 / 255.0,
        a: 1.0,
    };
    (convert(p.text), convert(p.primary))
}
pub fn notation_leaf(ui: &UiState) -> Option<NotationLeaf> {
    let d = ui.music_draft()?;
    let width = music_width(ui);
    let shown = ui
        .music_editor
        .picking_preview
        .as_ref()
        .filter(|p| p.owner == ui.working_sets.active_id && p.baseline == d.score)
        .map(|p| &p.score)
        .unwrap_or(&d.score);
    let (ink, accent) = colors(ui);
    Some(
        NotationLeaf::new(shown.clone(), d.view, d.selected, width, ink, accent).with_playhead(
            ui.music_editor
                .practice
                .playhead()
                .map(|p| (p.bar_index, p.event_index)),
        ),
    )
}
fn music_width(ui: &UiState) -> f32 {
    if ui.viewport_width > 0.0 {
        (ui.viewport_width - 72.0).clamp(240.0, 1160.0)
    } else {
        800.0
    }
}
fn board(ui: &UiState, d: &MusicDraft) -> UiChild {
    let (b, n) = d.selected;
    let e = &d.score.measures[b].events[n];
    let selected_fret = e.notes.iter().map(|p| p.fret).max().unwrap_or(0);
    let first = if selected_fret > 8 {
        selected_fret - 7
    } else {
        0
    };
    let width = music_width(ui).min(760.0);
    let label_w = 24.0;
    let cell = (width - label_w) / 9.0;
    let count = d.score.tuning.len();
    let height = count as f32 * 27.0 + 26.0;
    let palette = ui.appearance.resolve(&ui.app_settings.appearance).palette;
    let css = |c: tinct::Srgb| format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b);
    let mut children: Vec<UiChild> = vec![];
    for f in 0..9 {
        children.push(Box::new(el("span",text((first+f).to_string())).attr("style",format!("position:absolute;left:{}px;top:{}px;width:{cell}px;text-align:center;font-size:12px",label_w+f as f32*cell,count as f32*27.0+4.0))));
    }
    for (row, open) in d.score.tuning.iter().enumerate() {
        let top = row as f32 * 27.0;
        children.push(Box::new(el("span", text(note_name(*open))).attr(
            "style",
            format!("position:absolute;left:0;top:{top}px;font-size:11px;width:24px"),
        )));
        children.push(Box::new(el("div",()).attr("style",format!("position:absolute;left:{label_w}px;top:{}px;width:{}px;border-top:1px solid #75806c",top+13.0,width-label_w))));
        for f in first..first + 9 {
            let string = (row + 1) as u8;
            let proposed = ui.music_drag_position() == Some(((row + 1) as u8, f));
            let active = !e.rest && e.notes.iter().any(|p| p.string == string && p.fret == f);
            let name = note_name(open.saturating_add(f));
            children.push(Box::new(on_key(
                clickable(
                    el(
                        "button",
                        el(
                            "span",
                            text(if active || proposed {
                                name.clone()
                            } else {
                                "\u{a0}".into()
                            }),
                        )
                        .attr("class", "music-fret-dot")
                        .attr("style", if active {
                            format!("background:{};color:{}", css(palette.primary), css(palette.on_primary))
                        } else {
                            format!("color:{}", css(palette.text))
                        }),
                    )
                    .attr(
                        "class",
                        format!(
                            "t-btn music-fret{}{}",
                            if active { " music-fret-selected" } else { "" },
                            if proposed { " music-fret-proposed" } else { "" }
                        ),
                    )
                    .attr("aria-label", format!("String {string}, fret {f}, {name}"))
                    .attr("aria-pressed", active.to_string())
                    .attr("data-music-position", format!("s{string}/f{f}/"))
                    .attr("data-music-string", string.to_string())
                    .attr("data-music-fret", f.to_string())
                    .attr(
                        "style",
                        format!(
                            "position:absolute;left:{}px;top:{top}px;width:{cell}px;height:27px;color:{}",
                            label_w + f32::from(f - first) * cell, css(palette.text)
                        ),
                    ),
                    |_ui: &mut UiState, _| {},
                ),
                move |ui: &mut UiState, event: KeyEvent| {
                    if matches!(event.key, Key::Named(NamedKey::Enter | NamedKey::Space))
                        || matches!(&event.key,Key::Character(c) if c==" ")
                    {
                        ui.music_fret(string, f);
                        event.prevent_default();
                        event.stop_propagation();
                    } else {
                        ui.music_key(event);
                    }
                },
            )));
        }
    }
    Box::new(
        el(
            "section",
            vec![
                Box::new(el("h3", text("On the fretboard"))) as UiChild,
                Box::new(
                    el(
                        "p",
                        text(format!(
                            "{} strings · frets {first}–{} · highest string at top",
                            count,
                            first + 8
                        )),
                    )
                    .attr("class", "music-quiet"),
                ),
                Box::new(on_pointer(
                    el("div", children).attr("class", "music-fretboard").attr(
                        "style",
                        format!("position:relative;width:{width}px;height:{height}px"),
                    ),
                    UiState::music_board_pointer,
                )),
            ],
        )
        .attr("class", "music-board"),
    )
}
pub const CSS:&str="
.music-editor{display:flex;flex-direction:column;gap:12px;padding:16px;min-width:0;}
.music-editor>.t-btn{align-self:flex-start;}
.music-editor h2,.music-editor h3,.music-editor p{margin:0;}
.music-editor h3{font-size:17px;font-weight:500;}
.music-commands{display:flex;flex-wrap:wrap;align-items:center;gap:8px;}
.music-heading{display:flex;flex-direction:column;gap:6px;}
.music-quiet,.music-notice{font-size:13px;opacity:0.8;}
.music-note{background:transparent;border:0;padding:0;border-radius:3px;}
.music-note[aria-pressed=true],.music-note:hover,.music-note:focus{outline:1px solid #9eb68e;background:transparent;}
.music-sheet{flex:none;}
.music-score-label{font-size:13px;pointer-events:none;}
.music-board{display:flex;flex-direction:column;gap:8px;padding-top:12px;border-top:1px solid #606878;}
.music-fretboard{flex:none;}
.music-fret{border-radius:0;display:flex;align-items:center;justify-content:center;padding:0;border:0;border-right:1px solid #75806c;background:transparent;font-size:11px;}
.music-fret-proposed{outline:2px solid #adc997;}
.music-fret-dot{display:flex;align-items:center;justify-content:center;min-width:22px;height:22px;border-radius:11px;}

.music-fret-proposed .music-fret-dot{outline:2px solid #adc997;}
.music-mismatch{width:100%;}
.music-view[aria-pressed=true],.music-repeat[aria-pressed=true]{border-bottom:2px solid #9eb68e;}
";

#[cfg(test)]
#[path = "music_editor/tests.rs"]
mod tests;

#[path = "music_editor/editing.rs"]
mod editing;

#[path = "music_editor/render.rs"]
mod render;

pub use render::screen;
