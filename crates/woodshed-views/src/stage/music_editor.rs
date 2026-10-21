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
    pub fn leave_music_owner(&mut self) {
        self.music_editor.practice.stop();
        self.music_editor.active_tone = None;
        self.music_editor.fret_gesture = None;
        self.music_editor.picking_preview = None;
        self.music_editor.choose_chord = false;
    }
    pub fn music_practice_requested(&self) -> bool {
        matches!(
            self.music_editor.practice.status(),
            practice::PracticeStatus::Playing | practice::PracticeStatus::Paused
        )
    }
    pub fn music_practice_score_ref(&self) -> Option<&Score> {
        let draft = self.music_draft()?;
        Some(
            self.music_editor
                .picking_preview
                .as_ref()
                .filter(|p| p.owner == self.working_sets.active_id && p.baseline == draft.score)
                .map(|p| &p.score)
                .unwrap_or(&draft.score),
        )
    }
    pub fn music_practice_score(&self) -> Option<Score> {
        self.music_practice_score_ref().cloned()
    }
    pub fn music_practice_tempo(&self) -> f64 {
        f64::from(self.transport.bpm)
    }
    pub fn set_music_practice_tempo(&mut self, bpm: f64) {
        if bpm.is_finite() && (1.0..=400.0).contains(&bpm) {
            self.set_bpm(bpm as f32);
        }
    }
    pub fn practice_music(&mut self) {
        let Some(score) = self.music_practice_score() else {
            return;
        };
        let result = self.music_editor.practice.start(
            self.working_sets.active_id,
            &score,
            self.music_practice_tempo(),
            self.audio_session.clone(),
        );
        match result {
            Ok(_) => {
                self.stop_rehearsal();
                self.stage.arpeggio_playing = false;
                self.stage.exercise_playing = false;
                self.stage.scale_run_playing = false;
                self.song_playing = false;
                self.audio_requests
                    .push(woodshed_core::audio::AudioRequest::CancelPreview);
                self.music_editor.notice = None;
            },
            Err(reason) => self.music_editor.notice = Some(reason),
        }
    }
    pub fn pause_music_practice(&mut self) {
        self.music_editor.practice.pause();
    }
    pub fn resume_music_practice(&mut self) {
        if let Err(reason) = self.music_editor.practice.resume() {
            self.music_editor.notice = Some(reason);
        }
    }
    pub fn stop_music_practice(&mut self) {
        self.music_editor.practice.stop();
    }
    pub fn match_music_tempo(&mut self) {
        if let Some(bpm) = self.music_draft().map(|d| d.score.tempo) {
            self.set_music_practice_tempo(f64::from(bpm));
        }
    }
    pub fn music_drag_active(&self) -> bool {
        self.music_editor.fret_gesture.is_some()
    }
    pub fn music_preview_active(&self) -> bool {
        self.music_editor.picking_preview.as_ref().is_some_and(|p| {
            p.owner == self.working_sets.active_id
                && self.music_draft().is_some_and(|d| d.score == p.baseline)
        })
    }
    pub fn music_presentation_key(&self) -> String {
        format!(
            "{:?}",
            (
                self.music_editor.picking_preview.as_ref().map(|p| &p.score),
                self.music_editor.practice.playhead().map(|p| (
                    p.bar_index,
                    p.event_index,
                    p.is_rest
                )),
                self.music_editor.practice.status(),
                self.music_editor.practice.pending_update(),
                self.music_editor.follow_practice
            )
        )
    }
    pub fn music_draft(&self) -> Option<&MusicDraft> {
        self.music_editor.draft(self.working_sets.active_id)
    }
    pub fn arrangement_drafts_dirty(&self) -> bool {
        self.arrangement_editor.dirty() || self.music_editor.dirty()
    }
    pub fn sync_music_editor(&mut self) {
        let owner = self.working_sets.active_id;
        if self.workspace.active_panel() != Some(WorkspacePanel::Arrangement)
            || self.music_editor.drafts.contains_key(&owner)
        {
            return;
        }
        if let Some(doc) = self.set.music.clone() {
            match doc.score() {
                Ok(score) => {
                    self.music_editor
                        .drafts
                        .insert(owner, MusicDraft::new(score, Some(doc)));
                },
                Err(e) => {
                    self.music_editor.notice =
                        Some(format!("MusicXML preserved; editing unavailable: {e}"))
                },
            }
        }
    }
    pub fn set_music_view(&mut self, view: NotationView) {
        if let Some(d) = self
            .music_editor
            .drafts
            .get_mut(&self.working_sets.active_id)
        {
            d.view = view;
        }
    }
    pub fn select_music(&mut self, bar: usize, note: usize) {
        self.music_editor.follow_practice = false;
        self.music_editor.active_tone = None;
        if let Some(d) = self
            .music_editor
            .drafts
            .get_mut(&self.working_sets.active_id)
        {
            d.history.break_run();
            d.selected = (bar, note);
            d.clamp();
        }
    }
    fn edit_music(
        &mut self,
        edit: impl FnOnce(&mut Score, (usize, usize)) -> Result<(), String>,
        notice: &str,
    ) {
        let result = self
            .music_editor
            .drafts
            .get_mut(&self.working_sets.active_id)
            .ok_or("Start music first".to_owned())
            .and_then(|d| d.change(edit));
        self.music_editor.notice = Some(match result {
            Ok(()) => notice.into(),
            Err(e) => e,
        });
    }
    pub fn music_undo(&mut self) {
        if let Some(d) = self
            .music_editor
            .drafts
            .get_mut(&self.working_sets.active_id)
        {
            if let Some(score) = d.history.undo(d.score.clone()) {
                d.score = score;
                d.clamp();
                self.music_editor.notice = Some("Musical edit undone.".into());
            }
        }
    }
    pub fn music_redo(&mut self) {
        if let Some(d) = self
            .music_editor
            .drafts
            .get_mut(&self.working_sets.active_id)
        {
            if let Some(score) = d.history.redo(d.score.clone()) {
                d.score = score;
                d.clamp();
                self.music_editor.notice = Some("Musical edit redone.".into());
            }
        }
    }
    pub fn save_music(&mut self) {
        let owner = self.working_sets.active_id;
        let result = (|| {
            let d = self
                .music_editor
                .drafts
                .get(&owner)
                .ok_or("Start music first")?;
            if self.set.music != d.base {
                return Err("Saved music changed. Reopen it before saving this draft.".to_owned());
            }
            // Preserve exact source bytes when the working reading is unchanged.
            if !d.dirty() {
                return Ok(d.base.clone());
            }
            MusicDocument::from_score(&d.score).map(Some)
        })();
        match result {
            Ok(doc) => {
                self.set.music = doc.clone();
                if let Some(d) = self.music_editor.drafts.get_mut(&owner) {
                    d.base = doc;
                    d.saved_score = Some(d.score.clone());
                    d.history.mark_saved();
                }
                self.music_editor.notice = Some("Music saved to this working Set.".into());
            },
            Err(e) => self.music_editor.notice = Some(e),
        }
    }
    pub fn export_music(&mut self) {
        let result = (|| {
            if let Some(d) = self.music_draft() {
                let doc = if !d.dirty() {
                    d.base.clone().ok_or("No source")?
                } else {
                    MusicDocument::from_score(&d.score)?
                };
                Ok((
                    format!(
                        "{}.musicxml",
                        self.working_sets.active_name.replace(['/', ':', '\\'], "-")
                    ),
                    doc.musicxml,
                ))
            } else {
                self.set
                    .music
                    .as_ref()
                    .map(|d| ("woodshed.musicxml".into(), d.musicxml.clone()))
                    .ok_or("No music to export".into())
            }
        })();
        match result {
            Ok(request) => self.music_editor.export_request = Some(request),
            Err(e) => self.music_editor.notice = Some(e),
        }
    }
    fn begin_music(&mut self, score: Score) {
        if self.arrangement_editor.dirty()
            || self
                .music_editor
                .drafts
                .get(&self.working_sets.active_id)
                .is_some_and(MusicDraft::dirty)
        {
            self.music_editor.notice =
                Some("Save the current arrangement draft before starting music.".into());
            return;
        }
        if self.set.music.is_some() {
            self.music_editor.notice = Some(
                "This Set already has music. Edit it here, or choose a new working Set.".into(),
            );
            return;
        }
        match score.validate() {
            Ok(()) => {
                self.music_editor
                    .drafts
                    .insert(self.working_sets.active_id, MusicDraft::new(score, None));
                self.music_editor.legacy = false;
                self.music_editor.notice = None;
            },
            Err(e) => self.music_editor.notice = Some(e),
        }
    }
    /// Explicit fixture for native qualification only; ordinary starts use Set material.
    pub fn music_example(&mut self, rhythm: bool) {
        self.begin_music(if rhythm {
            Score::rhythm_example()
        } else {
            Score::progression_example()
        });
    }
    pub fn new_music_exercise(&mut self, beats: u8, beat_type: u8) {
        let bpm = self.transport.bpm;
        if !bpm.is_finite() || bpm.fract() != 0.0 || !(1.0..=400.0).contains(&bpm) {
            self.music_editor.notice =
                Some("This notation profile needs a whole-number tempo from 1 to 400 BPM.".into());
            return;
        }
        let tuning = self.stage.tuning();
        let value = if beat_type == 8 {
            NoteValue::Eighth
        } else {
            NoteValue::Quarter
        };
        let score = Score {
            title: self.working_sets.active_name.clone(),
            instrument: tuning.instrument.to_string(),
            tuning: tuning
                .strings
                .iter()
                .rev()
                .map(|p| p.midi() as u8)
                .collect(),
            tempo: bpm as u16,
            measures: vec![Measure {
                meter: TimeSignature { beats, beat_type },
                harmony: None,
                events: (0..beats)
                    .map(|_| NoteEvent {
                        notes: vec![],
                        value,
                        rest: true,
                    })
                    .collect(),
            }],
        };
        self.begin_music(score);
    }
    pub fn write_set_music(&mut self) {
        let result = music_from_set(
            &self.stage,
            &self.set,
            &self.working_sets.active_name,
            self.transport.bpm,
        );
        match result {
            Ok(score) => self.begin_music(score),
            Err(e) => self.music_editor.notice = Some(e),
        }
    }
    pub fn music_rest(&mut self) {
        self.edit_music(
            |score, (b, n)| {
                let e = &mut score.measures[b].events[n];
                if e.rest && e.notes.is_empty() {
                    e.notes.push(FrettedNote { string: 1, fret: 0 });
                }
                e.rest = !e.rest;
                if e.rest {
                    e.notes.clear();
                }
                Ok(())
            },
            "Note/rest changed; the following music keeps its timing.",
        );
    }
    pub fn music_split(&mut self, triplet: bool) {
        self.edit_music(
            |score, (b, n)| {
                let e = score.measures[b].events[n].clone();
                if e.value != NoteValue::Quarter {
                    return Err("Select a quarter note or quarter rest to split.".into());
                }
                let count = if triplet { 3 } else { 2 };
                let mut split = e;
                split.value = if triplet {
                    NoteValue::TripletEighth
                } else {
                    NoteValue::Eighth
                };
                score.measures[b].events.splice(n..=n, vec![split; count]);
                Ok(())
            },
            if triplet {
                "Quarter replaced by three triplet eighths."
            } else {
                "Quarter replaced by two eighth notes."
            },
        );
    }
    pub fn music_fret(&mut self, string: u8, fret: u8) {
        self.music_editor.active_tone = Some(FrettedNote { string, fret });
        self.edit_music(|score,(b,n)|{let event=&mut score.measures[b].events[n];let position=FrettedNote{string,fret};
            if event.notes.contains(&position){return Ok(());}
            if event.notes.iter().any(|p|p.string==string){return Err("This string already sounds a note. Drag that note to move it, or remove it first.".into());}
            event.notes.push(position);event.rest=false;Ok(())},"Note added; written chord preserved.");
    }
    pub fn music_remove_tone(&mut self) {
        let Some(position) = self.music_editor.active_tone else {
            self.music_editor.notice = Some("Select a fretboard note first.".into());
            return;
        };
        self.edit_music(
            move |score, (b, n)| {
                let event = &mut score.measures[b].events[n];
                event.notes.retain(|p| *p != position);
                event.rest = event.notes.is_empty();
                Ok(())
            },
            "Selected note removed.",
        );
    }
    pub fn music_step_pitch(&mut self, delta: i8) {
        self.edit_music(
            |score, (b, n)| {
                let e = &mut score.measures[b].events[n];
                if e.rest || e.notes.len() != 1 {
                    return Err("Select one sounding note to change its pitch.".into());
                }
                let fret = i16::from(e.notes[0].fret) + i16::from(delta);
                if !(0..=36).contains(&fret) {
                    return Err("Pitch lies outside this fretboard.".into());
                }
                e.notes[0].fret = fret as u8;
                Ok(())
            },
            "Pitch changed; rhythm preserved.",
        );
    }
    pub fn music_symbol(&mut self, symbol: ChordSymbol) {
        self.edit_music(
            move |score, (b, _)| {
                score.measures[b].harmony = Some(symbol);
                Ok(())
            },
            "Written chord changed; notes preserved.",
        );
    }
    pub fn music_voicing(&mut self, direction: i8) {
        let result = (|| {
            let d = self.music_draft().ok_or("Start music first")?;
            let bar = &d.score.measures[d.selected.0];
            let symbol = bar.harmony.as_ref().ok_or("Choose a written chord first")?;
            let chord_name = match symbol.kind {
                ChordKind::Minor7 => "Minor 7",
                ChordKind::Dominant7 => "Dominant 7",
                ChordKind::Major7 => "Major 7",
                ChordKind::Minor => "Minor",
                ChordKind::Major => "Major",
                ChordKind::Diminished => "Diminished",
                ChordKind::Augmented => "Augmented",
                ChordKind::Sus2 => "Suspended 2nd",
                ChordKind::Sus4 => "Suspended 4th",
            };
            let tuning = woodshed_core::tunings()
                .iter()
                .find(|t| {
                    t.instrument.to_string() == d.score.instrument
                        && t.strings.iter().rev().map(|p| p.midi() as u8).eq(d
                            .score
                            .tuning
                            .iter()
                            .copied())
                })
                .ok_or("No catalog voicings for this custom tuning; use the fretboard")?;
            let mut card = woodshedding::rehearsal::Card {
                material: woodshedding::rehearsal::Material::Chord {
                    name: chord_name.into(),
                    root: woodshedding::pitch::PitchClass::new(symbol.root),
                },
                setting: woodshedding::rehearsal::Setting {
                    instrument: d.score.instrument.clone(),
                    tuning: Some(tuning.name.into()),
                    fret_window: Some(woodshedding::rehearsal::FretWindow { start: 0, span: 24 }),
                    ..Default::default()
                },
                id: Default::default(),
                label: symbol.label(),
                touch: Default::default(),
                timing: Default::default(),
                from: None,
            };
            let current = d.score.measures[d.selected.0].events[d.selected.1]
                .notes
                .clone();
            let mut variants = vec![];
            for i in 0..24 {
                if self.stage.select_card_shape_index(&mut card, i).is_err() {
                    break;
                }
                if let woodshed_core::CardShapeStatus::Available(shape) =
                    self.stage.card_shape_status(&card)
                {
                    variants.push(
                        shape
                            .physical_positions()
                            .iter()
                            .map(|(s, f)| FrettedNote {
                                string: (d.score.tuning.len() - s) as u8,
                                fret: *f,
                            })
                            .collect::<Vec<_>>(),
                    );
                }
            }
            if variants.is_empty() {
                return Err(
                    "No playable voicing in this instrument's range. Use the fretboard.".into(),
                );
            }
            let index = variants
                .iter()
                .position(|v| {
                    let mut a = v.clone();
                    let mut b = current.clone();
                    a.sort_by_key(|p| p.string);
                    b.sort_by_key(|p| p.string);
                    a == b
                })
                .unwrap_or(if direction > 0 { variants.len() - 1 } else { 0 });
            let index =
                (index as isize + direction as isize).rem_euclid(variants.len() as isize) as usize;
            Ok(variants[index].clone())
        })();
        match result {
            Ok(notes) => self.edit_music(
                move |score, (b, n)| {
                    score.measures[b].events[n].notes = notes;
                    score.measures[b].events[n].rest = false;
                    Ok(())
                },
                "Voicing chosen from this instrument's catalog.",
            ),
            Err(e) => self.music_editor.notice = Some(e),
        }
    }
    pub fn music_tempo(&mut self, delta: i16) {
        self.edit_music(
            move |score, _| {
                score.tempo = (score.tempo as i16 + delta).clamp(1, 400) as u16;
                Ok(())
            },
            "Tempo changed.",
        );
    }
    pub fn music_add_bar(&mut self) {
        self.edit_music(
            |score, (b, _)| {
                let meter = score.measures[b].meter;
                let value = if meter.beat_type == 8 {
                    NoteValue::Eighth
                } else {
                    NoteValue::Quarter
                };
                if ![4, 8].contains(&meter.beat_type) {
                    return Err("Add bar currently supports quarter/eighth-note meters.".into());
                }
                score.measures.push(Measure {
                    meter,
                    harmony: None,
                    events: (0..meter.beats)
                        .map(|_| NoteEvent {
                            notes: vec![],
                            value,
                            rest: true,
                        })
                        .collect(),
                });
                Ok(())
            },
            "Empty bar added.",
        );
    }
    pub fn music_listen(&mut self) {
        if let Some(d) = self.music_draft() {
            let e = &d.score.measures[d.selected.0].events[d.selected.1];
            if e.rest {
                self.music_editor.notice = Some("Selected rest is silent.".into());
                return;
            }
            let pitches = e
                .notes
                .iter()
                .filter_map(|p| d.score.pitch(*p).ok())
                .map(frequency)
                .collect();
            let duration_s = (e.value.quarter_beats() * 60.0 / f64::from(d.score.tempo)) as f32;
            self.audio_requests
                .push(woodshed_core::audio::AudioRequest::PreviewPitches {
                    pitches,
                    duration_s,
                    strum_s: 0.0,
                });
        }
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
pub fn screen(ui: &UiState) -> UiChild {
    if ui.music_editor.legacy && ui.set.music.is_none() && ui.music_draft().is_none() {
        return super::arrangement_editor::screen(ui);
    }
    let mut children: Vec<UiChild> = vec![Box::new(el(
        "h2",
        text(format!("Arrangement · {}", ui.working_sets.active_name)),
    ))];
    if let Some(notice) = &ui.music_editor.notice {
        children.push(Box::new(
            el("p", text(notice.clone()))
                .attr("role", "status")
                .attr("class", "music-notice"),
        ));
    }
    let Some(d) = ui.music_draft() else {
        if ui.set.music.is_some() {
            children.push(action(
                "Export preserved MusicXML",
                "music-export",
                UiState::export_music,
            ));
        } else {
            children.push(Box::new(el("p",text("Write music from this Set, or start a rhythmic exercise. Chart, tab and score follow the same music."))));
            children.push(action(
                if ui
                    .set
                    .progression_arrangement
                    .as_ref()
                    .is_some_and(|a| !a.alignment.time_signatures.is_empty())
                {
                    "Write saved arrangement as music"
                } else {
                    "Write each Card as a 4/4 bar"
                },
                "music-from-set",
                UiState::write_set_music,
            ));
            children.push(Box::new(el("h3", text("New rhythmic exercise"))));
            children.push(Box::new(
                el(
                    "div",
                    vec![
                        action("Start in 4/4", "music-new-four", |ui| {
                            ui.new_music_exercise(4, 4)
                        }),
                        action("Start in 3/4", "music-new-three", |ui| {
                            ui.new_music_exercise(3, 4)
                        }),
                        action("Start in 6/8", "music-new-six", |ui| {
                            ui.new_music_exercise(6, 8)
                        }),
                    ],
                )
                .attr("class", "music-commands"),
            ));
            if !ui.set.cards.is_empty()
                || ui.set.progression_arrangement.is_some()
                || !ui.arrangement_editor.drafts.is_empty()
            {
                children.push(action(
                    "Open existing Card alignment",
                    "music-legacy",
                    |ui| ui.music_editor.legacy = true,
                ));
            }
        }
        return Box::new(el("section", children).attr("class", "music-editor"));
    };
    let mut toolbar: Vec<UiChild> = vec![];
    for view in [NotationView::Chart, NotationView::Tab, NotationView::Score] {
        toolbar.push(Box::new(clickable(
            el("button", text(view_label(view)))
                .attr("class", "t-btn music-view")
                .attr("aria-pressed", (d.view == view).to_string())
                .attr("data-music-view", view_label(view)),
            move |ui: &mut UiState, _| ui.set_music_view(view),
        )));
    }
    match ui.music_editor.practice.status() {
        practice::PracticeStatus::Playing => {
            toolbar.push(action(
                "Pause",
                "music-practice-pause",
                UiState::pause_music_practice,
            ));
            toolbar.push(action(
                "Stop",
                "music-practice-stop",
                UiState::stop_music_practice,
            ));
        },
        practice::PracticeStatus::Paused => {
            toolbar.push(action(
                "Resume",
                "music-practice-resume",
                UiState::resume_music_practice,
            ));
            toolbar.push(action(
                "Stop",
                "music-practice-stop",
                UiState::stop_music_practice,
            ));
        },
        _ => toolbar.push(action(
            "Practice arrangement",
            "music-practice-play",
            UiState::practice_music,
        )),
    }
    toolbar.push(action("Undo", "music-undo", UiState::music_undo));
    toolbar.push(action("Redo", "music-redo", UiState::music_redo));
    toolbar.push(action("Save", "music-save", UiState::save_music));
    toolbar.push(action(
        "Export MusicXML…",
        "music-export",
        UiState::export_music,
    ));
    children.push(Box::new(el("div", toolbar).attr("class", "music-commands")));
    let mut practice_controls: Vec<UiChild> = vec![
        action("Slower", "music-practice-slower", |ui| {
            ui.set_music_practice_tempo((ui.music_practice_tempo() - 4.0).clamp(1.0, 400.0))
        }),
        Box::new(el(
            "span",
            text(format!(
                "Practice {} BPM · written {} BPM",
                ui.music_practice_tempo(),
                d.score.tempo
            )),
        )),
        action("Faster", "music-practice-faster", |ui| {
            ui.set_music_practice_tempo((ui.music_practice_tempo() + 4.0).clamp(1.0, 400.0))
        }),
        action(
            "Match written tempo",
            "music-practice-match-tempo",
            UiState::match_music_tempo,
        ),
    ];
    for (label, mode) in [
        ("Off", woodshedding::rehearsal::LoopMode::Off),
        ("Arrangement", woodshedding::rehearsal::LoopMode::All),
        ("Bar", woodshedding::rehearsal::LoopMode::Card),
    ] {
        practice_controls.push(Box::new(clickable(
            el("button", text(format!("Repeat: {label}")))
                .attr("class", format!("t-btn music-repeat music-repeat-{label}"))
                .attr("aria-pressed", (ui.set.loop_mode == mode).to_string()),
            move |ui: &mut UiState, _| ui.set.loop_mode = mode,
        )));
    }
    if ui.music_editor.practice.pending_update() {
        practice_controls.push(Box::new(
            el("span", text("Edits queued for next bar")).attr("role", "status"),
        ));
    }
    children.push(Box::new(
        el("div", practice_controls).attr("class", "music-commands"),
    ));
    let mut follow_controls = vec![action(
        if ui.music_editor.follow_practice {
            "Edit selection"
        } else {
            "Follow playback"
        },
        "music-follow-playback",
        |ui| ui.music_editor.follow_practice = !ui.music_editor.follow_practice,
    )];
    if ui.music_editor.follow_practice {
        follow_controls.push(action("Whole arrangement", "music-follow-all", |ui| {
            ui.music_editor.follow_range = false
        }));
        follow_controls.push(action("Selected bar", "music-follow-selected", |ui| {
            if let Some(bar) = ui.music_draft().map(|d| d.selected.0) {
                ui.music_editor.follow_first.selected = bar;
                ui.music_editor.follow_last.selected = bar;
                ui.music_editor.follow_range = true;
            }
        }));
        let options = (1..=d.score.measures.len())
            .map(|b| format!("Bar {b}"))
            .collect::<Vec<_>>();
        follow_controls.push(Box::new(el("span", text("From"))));
        follow_controls.push(Box::new(map_state(
            select(
                &ui.music_editor.follow_first,
                &options.iter().map(String::as_str).collect::<Vec<_>>(),
            ),
            |ui: &mut UiState| &mut ui.music_editor.follow_first,
        )));
        follow_controls.push(Box::new(el("span", text("through"))));
        follow_controls.push(Box::new(map_state(
            select(
                &ui.music_editor.follow_last,
                &options.iter().map(String::as_str).collect::<Vec<_>>(),
            ),
            |ui: &mut UiState| &mut ui.music_editor.follow_last,
        )));
        follow_controls.push(action("Use bars", "music-follow-range", |ui| {
            ui.music_editor.follow_range = true
        }));
    }
    children.push(Box::new(
        el("div", follow_controls).attr("class", "music-commands"),
    ));
    let (b, n) = d.selected;
    let measure = &d.score.measures[b];
    let event = &measure.events[n];
    children.push(Box::new(
        el(
            "div",
            vec![
                Box::new(el("h3", text(d.score.title.clone()))) as UiChild,
                Box::new(
                    el(
                        "span",
                        text(format!(
                            "{} · {} strings · {}/{} · {} BPM · {}",
                            d.score.instrument,
                            d.score.tuning.len(),
                            measure.meter.beats,
                            measure.meter.beat_type,
                            d.score.tempo,
                            if d.dirty() { "Draft" } else { "Saved" }
                        )),
                    )
                    .attr("class", "music-quiet"),
                ),
            ],
        )
        .attr("class", "music-heading"),
    ));
    if let Some(leaf) = notation_leaf(ui) {
        let size = leaf.intrinsic_size();
        let mut score: Vec<UiChild> = vec![Box::new(custom_leaf::<UiState, ()>(
            crate::notation_leaf::NOTATION_LEAF_KEY,
            size.width.ceil() as u32,
            size.height.ceil() as u32,
        ))];
        for label in leaf.labels() {
            score.push(Box::new(
                el("span", text(label.text))
                    .attr("class", "music-score-label")
                    .attr(
                        "style",
                        format!("position:absolute;left:{}px;top:{}px", label.x, label.y),
                    ),
            ));
        }
        for (bar, note, (x, y, w, h)) in leaf.event_rects() {
            if ui.music_preview_active() {
                continue;
            }
            let e = &d.score.measures[bar].events[note];
            let pitches = e
                .notes
                .iter()
                .filter_map(|p| d.score.pitch(*p).ok())
                .map(note_name)
                .collect::<Vec<_>>()
                .join(" ");
            let name = format!(
                "Bar {}, {}, {}",
                bar + 1,
                if e.rest { "rest" } else { &pitches },
                e.value.label()
            );
            score.push(Box::new(clickable(
                el("button", ())
                    .attr("class", "music-note")
                    .attr("aria-label", name)
                    .attr("aria-pressed", (d.selected == (bar, note)).to_string())
                    .attr("data-music-bar", bar.to_string())
                    .attr("data-music-note", note.to_string())
                    .attr(
                        "style",
                        format!("position:absolute;left:{x}px;top:{y}px;width:{w}px;height:{h}px"),
                    ),
                move |ui: &mut UiState, _| ui.select_music(bar, note),
            )));
        }
        children.push(Box::new(
            el("div", score).attr("class", "music-sheet").attr(
                "style",
                format!(
                    "position:relative;width:{}px;height:{}px",
                    size.width, size.height
                ),
            ),
        ));
    }
    let pitches = event
        .notes
        .iter()
        .filter_map(|p| d.score.pitch(*p).ok())
        .map(note_name)
        .collect::<Vec<_>>()
        .join(" · ");
    children.push(Box::new(el(
        "h3",
        text(format!(
            "Bar {} · {} · {}",
            b + 1,
            if event.rest { "Rest" } else { &pitches },
            event.value.label()
        )),
    )));
    let mut commands = vec![action(
        if event.rest { "Make note" } else { "Make rest" },
        "music-rest",
        UiState::music_rest,
    )];
    if event.value == NoteValue::Quarter {
        commands.push(action("Split into eighths", "music-eighths", |ui| {
            ui.music_split(false)
        }));
        commands.push(action("Make triplet", "music-triplet", |ui| {
            ui.music_split(true)
        }));
    }
    if event.notes.len() == 1 && !event.rest {
        commands.push(action("Pitch −", "music-pitch-down", |ui| {
            ui.music_step_pitch(-1)
        }));
        commands.push(action("Pitch +", "music-pitch-up", |ui| {
            ui.music_step_pitch(1)
        }));
    }
    commands.push(action(
        "Sustain for bar",
        "music-sustain",
        UiState::music_sustain,
    ));
    commands.push(action(
        "Listen to selection",
        "music-listen",
        UiState::music_listen,
    ));
    children.push(Box::new(
        el("div", commands).attr("class", "music-commands"),
    ));
    children.push(action(
        if measure.harmony.is_some() {
            "Change written chord…"
        } else {
            "Add chord symbol…"
        },
        "music-change-chord",
        UiState::open_music_chord,
    ));
    if ui.music_editor.choose_chord {
        let roots = [
            "C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B",
        ];
        children.push(Box::new(
            el(
                "div",
                vec![
                    Box::new(map_state(
                        select(&ui.music_editor.chord_root, &roots),
                        |ui: &mut UiState| &mut ui.music_editor.chord_root,
                    )) as UiChild,
                    Box::new(map_state(
                        select(&ui.music_editor.chord_kind, &QUALITY_LABELS),
                        |ui: &mut UiState| &mut ui.music_editor.chord_kind,
                    )),
                    action(
                        "Use written chord",
                        "music-use-chord",
                        UiState::apply_music_chord,
                    ),
                    action("Cancel", "music-cancel-chord", |ui| {
                        ui.music_editor.choose_chord = false
                    }),
                ],
            )
            .attr("class", "music-commands"),
        ));
    }
    if let Some(symbol) = measure.harmony.as_ref() {
        let mut chords = vec![
            Box::new(el(
                "span",
                text(format!("Written chord: {}", symbol.label())),
            )) as UiChild,
            action("Previous voicing", "music-voicing-prev", |ui| {
                ui.music_voicing(-1)
            }),
            action("Next voicing", "music-voicing-next", |ui| {
                ui.music_voicing(1)
            }),
            action("Quarter notes ↑", "music-pick-up", |ui| {
                ui.preview_music_pattern(pattern::PickingPattern::QuarterAscending)
            }),
            action("Quarter notes ↓", "music-pick-down", |ui| {
                ui.preview_music_pattern(pattern::PickingPattern::QuarterDescending)
            }),
            action("Eighth notes ↑↓", "music-pick-eighths", |ui| {
                ui.preview_music_pattern(pattern::PickingPattern::EighthUpDown)
            }),
        ];
        if d.score.chord_mismatch(b) {
            chords.push(Box::new(el("p",text("These notes differ from the written chord. Keep the symbol, or choose a suggestion below.")).attr("class","music-mismatch").attr("role","status")));
            for suggestion in d.score.chord_suggestions(b) {
                chords.push(action(
                    format!("Use {}", suggestion.label()),
                    "music-chord-suggestion",
                    move |ui| ui.music_symbol(suggestion.clone()),
                ));
            }
        }
        children.push(Box::new(el("div", chords).attr("class", "music-commands")));
    }
    if let Some(preview) = ui.music_editor.picking_preview.as_ref() {
        children.push(Box::new(
            el(
                "p",
                text({
                    let m = &preview.score.measures[preview.selected.0];
                    let picks = m
                        .events
                        .iter()
                        .flat_map(|e| e.notes.iter())
                        .filter_map(|n| preview.score.pitch(*n).ok())
                        .map(note_name)
                        .collect::<Vec<_>>()
                        .join(" → ");
                    let used = preview
                        .source_voicing
                        .iter()
                        .filter(|n| m.events.iter().any(|e| e.notes.contains(n)))
                        .count();
                    format!(
                        "{} · {used} of {} chord tones · {picks}",
                        preview.pattern.label(),
                        preview.source_voicing.len()
                    )
                }),
            )
            .attr("role", "status")
            .attr("class", "music-pattern-summary")
            .attr("aria-label", preview.summary()),
        ));
        children.push(Box::new(
            el(
                "div",
                vec![
                    action(
                        "Apply picking pattern",
                        "music-pick-apply",
                        UiState::apply_music_pattern,
                    ),
                    action(
                        "Cancel preview",
                        "music-pick-cancel",
                        UiState::cancel_music_pattern,
                    ),
                ],
            )
            .attr("class", "music-commands"),
        ));
    }
    children.push(action(
        "Remove selected note",
        "music-remove-tone",
        UiState::music_remove_tone,
    ));
    if ui.music_editor.follow_practice {
        let score = ui.music_editor.practice.applied_score().unwrap_or(&d.score);
        let end = score.measures.len().saturating_sub(1);
        let (first, last) = if ui.music_editor.follow_range {
            let first = ui.music_editor.follow_first.selected.min(end);
            (
                first,
                ui.music_editor.follow_last.selected.min(end).max(first),
            )
        } else {
            (0, end)
        };
        children.push(playback_board::board(
            ui,
            score,
            ui.music_editor
                .practice
                .playhead()
                .map(|p| (p.bar_index, p.event_index)),
            first..=last,
        ));
    } else {
        children.push(board(ui, d));
    }
    children.push(Box::new(
        el(
            "div",
            vec![
                action("Written tempo −", "music-slower", |ui| ui.music_tempo(-4)),
                action("Written tempo +", "music-faster", |ui| ui.music_tempo(4)),
                action("Add bar", "music-add-bar", UiState::music_add_bar),
            ],
        )
        .attr("class", "music-commands"),
    ));
    Box::new(on_key(
        el("section", children).attr("class", "music-editor"),
        UiState::music_key,
    ))
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
mod tests {
    use super::*;
    fn fixture(rhythm: bool) -> UiState {
        let mut ui = UiState::new();
        ui.activate_workspace_panel(WorkspacePanel::Arrangement);
        ui.music_example(rhythm);
        ui
    }
    #[test]
    fn musical_edits_share_one_history_across_views_save_and_reopen() {
        let mut ui = fixture(true);
        ui.save_music();
        let original = ui.set.music.clone();
        let reading = ui.music_draft().unwrap().score.clone();
        ui.set_music_view(NotationView::Tab);
        ui.select_music(1, 0);
        assert!(!ui.music_draft().unwrap().dirty());
        assert!(!ui.music_draft().unwrap().can_undo());
        assert_eq!(ui.set.music, original);
        ui.select_music(0, 0);
        ui.music_fret(2, 3);
        ui.music_split(true);
        ui.music_rest();
        assert!(ui.music_draft().unwrap().score.measures[0].events[0].rest);
        assert_eq!(
            ui.music_draft().unwrap().score.measures[0].events[0]
                .notes
                .len(),
            0
        );
        ui.music_undo();
        assert!(!ui.music_draft().unwrap().score.measures[0].events[0].rest);
        ui.music_redo();
        ui.save_music();
        let saved = ui.set.music.clone().unwrap();
        assert!(!ui.music_draft().unwrap().dirty());
        ui.music_undo();
        assert!(ui.music_draft().unwrap().dirty());
        ui.music_redo();
        assert!(!ui.music_draft().unwrap().dirty());
        assert_eq!(ui.set.music.as_ref(), Some(&saved));
        assert_ne!(saved.score().unwrap(), reading);
        let session = ui.to_persisted();
        let mut reopened = UiState::new();
        reopened.apply_persisted(&session, ui.app_settings.clone());
        reopened.activate_workspace_panel(WorkspacePanel::Arrangement);
        reopened.sync_music_editor();
        assert_eq!(reopened.set.music, Some(saved.clone()));
        assert_eq!(
            reopened.music_draft().unwrap().score,
            saved.score().unwrap()
        );
    }
    #[test]
    fn symbol_is_authored_and_suggestions_do_not_rewrite_it() {
        let mut ui = fixture(false);
        let written = ui.music_draft().unwrap().score.measures[0].harmony.clone();
        ui.music_fret(6, 2);
        assert_eq!(ui.music_draft().unwrap().score.measures[0].harmony, written);
        assert!(ui.music_draft().unwrap().score.chord_mismatch(0));
        let notes = ui.music_draft().unwrap().score.measures[0].events.clone();
        ui.open_music_chord();
        ui.music_editor.chord_root.selected = 7;
        ui.music_editor.chord_kind.selected = 2;
        ui.apply_music_chord();
        assert_eq!(
            ui.music_draft().unwrap().score.measures[0]
                .harmony
                .as_ref()
                .unwrap()
                .label(),
            "G7"
        );
        assert_eq!(ui.music_draft().unwrap().score.measures[0].events, notes);
    }
    #[test]
    fn each_set_has_its_own_draft_and_normal_session_storage() {
        let mut ui = fixture(false);
        let first = ui.working_sets.active_id;
        ui.save_music();
        let first_xml = ui.set.music.clone();
        ui.music_fret(1, 2);
        let first_reading = ui.music_draft().unwrap().score.clone();
        let second = ui.create_working_set();
        ui.activate_workspace_panel(WorkspacePanel::Arrangement);
        ui.music_example(true);
        ui.save_music();
        let second_xml = ui.set.music.clone();
        ui.activate_working_set(first);
        ui.activate_workspace_panel(WorkspacePanel::Arrangement);
        assert_eq!(ui.music_draft().unwrap().score, first_reading);
        assert_eq!(ui.set.music, first_xml);
        ui.save_music();
        let encoded = serde_json::to_string(&ui.to_persisted()).unwrap();
        let session = woodshed_core::storage::decode_session(&encoded)
            .unwrap()
            .session;
        assert_eq!(
            session
                .working_sets
                .get(second, &session.set)
                .unwrap()
                .music,
            second_xml
        );
    }
    #[test]
    fn reopening_a_session_clears_previous_owner_drafts() {
        let mut ui = fixture(true);
        ui.save_music();
        let session = ui.to_persisted();
        ui.music_fret(1, 5);
        assert!(ui.arrangement_drafts_dirty());
        ui.apply_persisted(&session, ui.app_settings.clone());
        ui.activate_workspace_panel(WorkspacePanel::Arrangement);
        ui.sync_music_editor();
        assert!(!ui.music_draft().unwrap().dirty());
        assert_eq!(
            ui.music_draft().unwrap().score,
            session.set.music.unwrap().score().unwrap()
        );
    }
    #[test]
    fn invalid_pitch_and_export_do_not_change_saved_source() {
        let mut ui = fixture(true);
        ui.save_music();
        let saved = ui.set.music.clone();
        let before = ui.music_draft().unwrap().score.clone();
        ui.music_fret(50, 0);
        assert_eq!(ui.music_draft().unwrap().score, before);
        assert!(!ui.music_draft().unwrap().can_undo());
        ui.music_fret(2, 3);
        ui.export_music();
        let xml = &ui.music_editor.export_request.as_ref().unwrap().1;
        assert_eq!(
            MusicDocument {
                musicxml: xml.clone()
            }
            .score()
            .unwrap(),
            ui.music_draft().unwrap().score
        );
        assert_eq!(ui.set.music, saved);
    }
    #[test]
    fn new_exercise_respects_explicit_meter_and_selected_tuning() {
        let mut ui = UiState::new();
        ui.activate_workspace_panel(WorkspacePanel::Arrangement);
        ui.transport.bpm = 120.0;
        ui.new_music_exercise(6, 8);
        let score = &ui.music_draft().unwrap().score;
        assert_eq!(score.tempo, 120);
        assert_eq!(
            score.measures[0].meter,
            TimeSignature {
                beats: 6,
                beat_type: 8
            }
        );
        assert_eq!(score.measures[0].events.len(), 6);
        assert!(
            score.measures[0]
                .events
                .iter()
                .all(|e| e.rest && e.notes.is_empty())
        );
        assert_eq!(
            score.tuning,
            ui.stage
                .tuning()
                .strings
                .iter()
                .rev()
                .map(|p| p.midi() as u8)
                .collect::<Vec<_>>()
        );
    }
    #[test]
    fn collected_and_duplicated_sets_keep_independent_music() {
        let mut ui = fixture(false);
        ui.save_music();
        let source = ui.set.clone();
        let mut copy = woodshedding::rehearsal::Set::default();
        copy.copy_instructions_from(&source).unwrap();
        assert_eq!(copy.music, source.music);
        copy.music.as_mut().unwrap().musicxml.push(' ');
        assert_ne!(copy.music, source.music);
    }
}
