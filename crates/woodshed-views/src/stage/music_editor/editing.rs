use super::*;

impl UiState {
    pub fn leave_music_owner(&mut self) {
        self.music_editor.practice.stop();
        self.leave_music_owner_view();
    }
    /// Leave owner-local editing gestures while a navigation route selects a
    /// different Set. The captured practice run remains bound to its owner.
    pub(in super::super) fn leave_music_owner_view(&mut self) {
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
    pub(super) fn edit_music(
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
    pub(super) fn begin_music(&mut self, score: Score) {
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
