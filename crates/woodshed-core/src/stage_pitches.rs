use super::*;

impl StageState {
    /// The chord / scale tones of the active lens as frequencies (Hz),
    /// lowest first — the raw material for the "♪ Hear" preview. Empty
    /// for the Exercise lens (a fingering pattern, not one voiceable
    /// chord) and before a progression is picked.
    pub fn voicing_pitches(&self) -> Vec<f32> {
        fn to_hz(ps: Vec<Pitch>) -> Vec<f32> {
            ps.iter().map(|p| p.frequency() as f32).collect()
        }
        match self.lens {
            Lens::Scales => self
                .scale()
                .apply_to(self.root())
                .map(to_hz)
                .unwrap_or_default(),
            Lens::Chords => self
                .chord()
                .apply_to(self.root())
                .map(to_hz)
                .unwrap_or_default(),
            Lens::Arpeggios => self
                .arpeggio_chord()
                .apply_to(self.root())
                .map(to_hz)
                .unwrap_or_default(),
            Lens::Progressions => self.progression_expanded_pitches().unwrap_or_default(),
            Lens::Exercises => Vec::new(),
        }
    }

    /// The active lens's material as an on-demand preview: `(pitches Hz,
    /// duration secs, strum offset ms)`. Empty pitches = nothing to voice.
    pub fn voicing_preview(&self) -> (Vec<f32>, f32, f32) {
        let pitches = self.voicing_pitches();
        if pitches.is_empty() {
            return (pitches, 0.0, 0.0);
        }
        let (dur, strum) = voicing_shape(pitches.len(), self.lens == Lens::Scales);
        (pitches, dur, strum)
    }

    /// Tones of the expanded progression chord (Hz) — the Progression
    /// arm of [`Self::voicing_pitches`].
    pub(super) fn progression_expanded_pitches(&self) -> Option<Vec<f32>> {
        let idx = self.progression_idx?;
        let prog = progression_catalog().get(idx)?;
        let major = scale_catalog().iter().find(|s| s.name == "Major")?;
        let chords = prog.apply_in_key(self.root(), major).ok()?;
        if chords.is_empty() {
            return None;
        }
        let expanded = self.progression_expanded.min(chords.len() - 1);
        let chord = &chords[expanded];
        let ps = chord.formula.apply_to(chord.root).ok()?;
        Some(ps.iter().map(|p| p.frequency() as f32).collect())
    }

    /// A rehearsal card's material as a preview `(pitches Hz, duration
    /// secs, strum offset ms)` — the Rehearsal-tab counterpart to
    /// [`Self::voicing_preview`]. Riff cards don't voice (empty).
    pub fn card_voicing(&self, card: &Card) -> CardPreviewResult {
        resolve_card_timing(&card.timing, 120.0)?;
        if matches!(card.material, Material::ChordApproach { .. }) {
            return self.chord_approach_preview(card, false);
        }
        if matches!(
            card.material,
            Material::Scale { .. } | Material::ScalePattern { .. }
        ) {
            return self.scale_card_preview(card, false);
        }
        fn to_hz(ps: Vec<Pitch>) -> Vec<f32> {
            ps.iter().map(|p| p.frequency() as f32).collect()
        }
        let root_of = |pc: &PitchClass| Pitch::from_midi(48 + pc.value() as i32, Spelling::Sharps);
        let (pitches, scale_like) = match &card.material {
            Material::Scale { name, root } | Material::ScalePattern { name, root, .. } => (
                scale_catalog()
                    .iter()
                    .find(|s| s.name == name.as_str())
                    .and_then(|s| s.apply_to(root_of(root)).ok())
                    .map(to_hz)
                    .unwrap_or_default(),
                true,
            ),
            Material::Chord { name, root } | Material::ChordApproach { name, root, .. } => {
                if card.setting.voicing_idx.is_some() {
                    let pitches = match self.selected_card_shape(card) {
                        CardShapeStatus::Available(shape) => to_hz(shape.concert_pitches()),
                        CardShapeStatus::Unavailable(_)
                        | CardShapeStatus::NotChord
                        | CardShapeStatus::Unselected => Vec::new(),
                    };
                    (pitches, false)
                } else {
                    (
                        chord_catalog()
                            .iter()
                            .find(|c| c.name == name.as_str())
                            .and_then(|c| c.apply_to(root_of(root)).ok())
                            .map(to_hz)
                            .unwrap_or_default(),
                        false,
                    )
                }
            },
            Material::Riff { .. } => (Vec::new(), false),
            Material::Path { .. } => {
                // A drawn path is inherently sequential: sound it as an
                // ascending-style cascade *in drawn order* (its order is the
                // material), not as a block.
                let Ok(notes) = self.path_contact_pitches(card) else {
                    return Ok((Vec::new(), 0.0, 0.0));
                };
                let hz: Vec<f32> = notes
                    .into_iter()
                    .map(|(_, pitch)| pitch.frequency() as f32)
                    .collect();
                (hz, true)
            },
        };
        if pitches.is_empty() {
            return Ok((pitches, 0.0, 0.0));
        }
        // Walk is a behaviour, not a label: it visits the notes one at a time
        // (the cascade shape) rather than sounding them together.
        let scale_like = scale_like || matches!(card.touch, Touch::Walk);
        let (dur, strum) = voicing_shape(pitches.len(), scale_like);
        Ok((pitches, dur, strum))
    }

    /// The card's *effective* sound, after its mark mode is applied — the
    /// audible half of mark + solo/mute. Off (or nothing marked) is the plain
    /// voicing; Solo plays only the marked positions' pitches; Mute plays the
    /// voicing minus the marked notes' pitch classes. Same shape tuple as
    /// [`Self::card_voicing`]. This is what the "hear it" paths resolve to.
    /// Resolve inherited rehearsal tempo before articulation and dwell are
    /// calculated, so a Card without a BPM uses the same clock as its runner.
    pub fn card_sounding_pitches_at_tempo(
        &self,
        card: &Card,
        fallback_bpm: f32,
    ) -> CardPreviewResult {
        if card.timing.bpm.is_some() {
            return self.card_sounding_pitches(card);
        }
        let mut effective = card.clone();
        effective.timing.bpm = Some(fallback_bpm);
        self.card_sounding_pitches(&effective)
    }

    pub fn card_sounding_pitches(&self, card: &Card) -> CardPreviewResult {
        resolve_card_timing(&card.timing, 120.0)?;
        if matches!(card.material, Material::ChordApproach { .. }) {
            return self.chord_approach_preview(card, true);
        }
        if matches!(
            card.material,
            Material::Scale { .. } | Material::ScalePattern { .. }
        ) {
            return self.scale_card_preview(card, true);
        }
        connected_catalog::arpeggiate_preview(card, self.card_sounding_pitches_unarticulated(card)?)
    }

    pub(super) fn card_sounding_pitches_unarticulated(&self, card: &Card) -> CardPreviewResult {
        let marked = &card.setting.marked;
        if marked.is_empty() || card.setting.mark_mode == MarkMode::Off {
            return self.card_voicing(card);
        }
        match card.setting.mark_mode {
            MarkMode::Off => self.card_voicing(card),
            MarkMode::Solo => {
                // Play only the marked positions, as their actual fretboard
                // pitches — isolate the shape you selected.
                let set: std::collections::HashSet<(usize, u8)> = marked.iter().copied().collect();
                let contacts = match self.card_contact_pitches(card) {
                    Ok(contacts) => contacts,
                    Err(_) => return Ok((Vec::new(), 0.0, 0.0)),
                };
                let mut hz: Vec<f32> = contacts
                    .into_iter()
                    .filter(|(contact, pitch)| set.contains(contact) && *pitch > 0.0)
                    .map(|(_, pitch)| pitch)
                    .collect();
                if matches!(card.material, Material::Path { .. }) {
                    if hz.is_empty() {
                        return Ok((Vec::new(), 0.0, 0.0));
                    }
                    let (dur, strum) = voicing_shape(hz.len(), true);
                    return Ok((hz, dur, strum));
                }
                hz.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                hz.dedup();
                if hz.is_empty() {
                    return Ok((Vec::new(), 0.0, 0.0));
                }
                let (dur, strum) = voicing_shape(hz.len(), false);
                Ok((hz, dur, strum))
            },
            MarkMode::Mute => {
                // Drop the marked notes' pitch classes from the clean voicing,
                // so the chord keeps voicing nicely minus the muted note(s).
                let contacts = self.card_contact_pitches(card).unwrap_or_default();
                let muted_pcs: std::collections::HashSet<u8> = marked
                    .iter()
                    .filter_map(|pos| {
                        contacts
                            .iter()
                            .find(|(contact, _)| contact == pos)
                            .map(|(_, frequency)| pc_from_hz(*frequency))
                    })
                    .collect();
                let (pitches, _, _) = self.card_voicing(card)?;
                let kept: Vec<f32> = pitches
                    .into_iter()
                    .filter(|&f| !muted_pcs.contains(&pc_from_hz(f)))
                    .collect();
                if kept.is_empty() {
                    return Ok((Vec::new(), 0.0, 0.0));
                }
                let (dur, strum) =
                    voicing_shape(kept.len(), matches!(card.material, Material::Path { .. }));
                Ok((kept, dur, strum))
            },
        }
    }

    /// Frequency (Hz) of the exercise step-through's current note.
    /// `None` if the exercise generates no steps.
    pub fn exercise_current_pitch_hz(&self) -> Option<f32> {
        let ex = self.exercise();
        let params = ExerciseParams {
            starting_fret: self.exercise_starting_fret,
            ..ExerciseParams::default()
        };
        let steps = ex.generate(&self.tuning(), &params);
        if steps.is_empty() {
            return None;
        }
        let step = self.exercise_step_idx % steps.len();
        let s = &steps[step];
        let board = Fretboard::new(self.tuning(), self.fret_count);
        Some(board.pitch_at(s.string_index, s.fret).frequency() as f32)
    }
}
