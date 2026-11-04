use super::*;

impl StageState {
    /// Lower all Cards in Set order. Each card resolves using its retained
    /// setup and authored timing, with `fallback_bpm` used only when the Card
    /// has no BPM of its own.
    pub fn practice_program(&self, set: &Set, fallback_bpm: f32) -> PracticeProgram {
        PracticeProgram {
            cards: set
                .cards
                .iter()
                .map(|card| self.lower_card(card, fallback_bpm))
                .collect(),
        }
    }

    /// Lower one transient Stage clock to its ordered contacts. `bpm` is
    /// supplied by the host because tempo is not part of StageState.
    pub fn live_clock_card(&self, kind: LiveSequenceKind, bpm: f32) -> PracticeCard {
        let card = self.live_card_template(kind);
        let events = self.live_events(kind);
        let unavailable = if !bpm.is_finite() || bpm <= 0.0 {
            Some("Live sequence requires a finite positive tempo".to_string())
        } else if events.is_empty() {
            Some("Live sequence has no playable contacts".to_string())
        } else {
            None
        };
        PracticeCard {
            card,
            bpm,
            hold: PracticeHold::Manual {
                phrase_beats: events.len().saturating_sub(1) as f64,
                tail_seconds: 0.0,
            },
            events: if unavailable.is_some() {
                Vec::new()
            } else {
                events
            },
            unavailable,
        }
    }

    pub(super) fn lower_card(&self, card: &Card, fallback_bpm: f32) -> PracticeCard {
        let bpm = card.timing.bpm.unwrap_or(fallback_bpm);
        let failed = |reason: String| PracticeCard {
            card: card.clone(),
            bpm,
            hold: PracticeHold::Manual {
                phrase_beats: 0.0,
                tail_seconds: 0.0,
            },
            events: Vec::new(),
            unavailable: Some(reason),
        };

        if let Err(error) = crate::resolve_card_timing(&card.timing, fallback_bpm) {
            return failed(error.to_string());
        }
        // Riff Cards intentionally remain silent in the existing preview
        // contract. The Exercise lens has its own live sequence lowering.
        if matches!(card.material, Material::Riff { .. }) {
            if let Material::Riff { name } = &card.material {
                if !woodshedding::exercise::catalog()
                    .iter()
                    .any(|exercise| exercise.name == name)
                {
                    return failed(format!("Unknown exercise: {name}"));
                }
            }
            return PracticeCard {
                card: card.clone(),
                bpm,
                hold: authored_hold(card),
                events: Vec::new(),
                unavailable: None,
            };
        }

        let mut visits = match self.card_visits(card) {
            Ok(visits) => visits,
            Err(error) => return failed(error),
        };
        if visits.is_empty() {
            return failed("Card realization produced no playable notes".to_string());
        }
        self.apply_card_marks(card, &mut visits);
        self.apply_card_touch(card, &mut visits);
        if visits.is_empty() {
            return failed("Card marks or touch produced no playable notes".to_string());
        }

        let (expected_pitches, duration, strum_ms) =
            match self.card_sounding_pitches_at_tempo(card, fallback_bpm) {
                Ok(preview) => preview,
                Err(error) => return failed(error.to_string()),
            };
        let sequential = matches!(card.material, Material::Path { .. })
            || matches!(card.touch, Touch::Walk | Touch::Arpeggiate { .. });
        let realized_pitches = visits.iter().map(|visit| visit.pitch).collect::<Vec<_>>();
        if !same_pitches(&realized_pitches, &expected_pitches) {
            return failed(
                "Contact realization does not match the existing Card preview pitch order".into(),
            );
        }
        let groups = if sequential {
            visits.iter().map(|visit| vec![*visit]).collect::<Vec<_>>()
        } else {
            vec![visits]
        };

        let mut events = Vec::with_capacity(groups.len());
        for (ordinal, group) in groups.into_iter().enumerate() {
            events.push(PracticeEvent {
                ordinal: ordinal as u32,
                at: 0.0,
                strum_seconds: if sequential {
                    0.0
                } else {
                    f64::from(strum_ms) / 1000.0
                },
                pitches: group.iter().map(|visit| visit.pitch).collect(),
                contacts: group.iter().map(|visit| visit.contact).collect(),
            });
        }

        let hold = match authored_hold(card) {
            PracticeHold::Beats(total) => {
                set_even_onsets(&mut events, total);
                PracticeHold::Beats(total)
            },
            PracticeHold::Seconds(total) => {
                set_even_onsets(&mut events, total);
                PracticeHold::Seconds(total)
            },
            PracticeHold::Manual { .. } if sequential => {
                for event in &mut events {
                    event.at = f64::from(event.ordinal);
                }
                let phrase_beats = events.len().saturating_sub(1) as f64;
                PracticeHold::Manual {
                    phrase_beats,
                    // ordered_preview's sequential visit model reserves a
                    // fixed 400ms final tail. Keep that authored articulation
                    // invariant across tempo changes.
                    tail_seconds: 0.4,
                }
            },
            PracticeHold::Manual { .. } => PracticeHold::Manual {
                phrase_beats: 0.0,
                tail_seconds: f64::from(duration),
            },
        };

        PracticeCard {
            card: card.clone(),
            bpm,
            hold,
            events,
            unavailable: None,
        }
    }

    pub(super) fn card_visits(&self, card: &Card) -> Result<Vec<NoteVisit>, String> {
        match &card.material {
            Material::Scale { .. } | Material::ScalePattern { .. } => {
                let scale = self
                    .scale_card_realization(card)
                    .map_err(|error| error.to_string())?;
                Ok(scale
                    .notes
                    .into_iter()
                    .map(|note| NoteVisit {
                        pitch: note.pitch.frequency() as f32,
                        contact: (note.string_index, note.physical_fret),
                    })
                    .collect())
            },
            Material::ChordApproach { .. } => {
                let approach = self
                    .chord_approach_realization(card)
                    .map_err(|error| error.to_string())?;
                Ok(approach
                    .notes
                    .into_iter()
                    .map(|note| NoteVisit {
                        pitch: note.pitch.frequency() as f32,
                        contact: (note.string_index, note.physical_fret),
                    })
                    .collect())
            },
            Material::Chord { name, root } => self.chord_visits(card, name, *root),
            Material::Riff { .. } => Ok(Vec::new()),
            Material::Path { .. } => Ok(self
                .path_contact_pitches(card)?
                .into_iter()
                .map(|(contact, pitch)| NoteVisit {
                    pitch: pitch.frequency() as f32,
                    contact,
                })
                .collect()),
        }
    }

    pub(super) fn chord_visits(
        &self,
        card: &Card,
        name: &str,
        root: PitchClass,
    ) -> Result<Vec<NoteVisit>, String> {
        let formula = woodshedding::chord::catalog()
            .iter()
            .find(|formula| formula.name == name)
            .ok_or_else(|| format!("Unknown chord: {name}"))?;
        if card.setting.voicing_idx.is_some() {
            let shape = match self.card_shape_status(card) {
                CardShapeStatus::Available(shape) => shape,
                CardShapeStatus::Unavailable(reason) => return Err(reason.to_string()),
                CardShapeStatus::NotChord => return Err("Card is not chord material".into()),
                CardShapeStatus::Unselected => return Err("Card shape is not selected".into()),
            };
            return Ok(shape
                .physical_positions()
                .into_iter()
                .zip(shape.concert_pitches())
                .map(|(contact, pitch)| NoteVisit {
                    pitch: pitch.frequency() as f32,
                    contact,
                })
                .collect());
        }

        // An unselected formula chord is the existing abstract voicing. Give
        // each formula tone one deterministic exact physical realization in
        // the Card's saved window; never choose a nearest pitch/contact.
        let tuning = self
            .resolve_card_tuning(card)
            .map_err(|error| error.to_string())?;
        let end_limit = tuning.instrument.standard_fret_count();
        let (start, end) = card.setting.fret_window.map_or(
            (self.fret_start, self.fret_count.min(end_limit)),
            |window| {
                (
                    window.start,
                    window.start.saturating_add(window.span).min(end_limit),
                )
            },
        );
        let start = start.max(card.setting.capo.unwrap_or(0));
        if start > end {
            return Err("Chord setup has no physical contacts in its fret window".into());
        }
        let board = Fretboard::new(tuning, end);
        let root_pitch = woodshedding::pitch::Pitch::from_midi(
            48 + i32::from(root.value()),
            woodshedding::pitch::Spelling::Sharps,
        );
        let expected = formula
            .apply_to(root_pitch)
            .map_err(|error| error.to_string())?;
        let positions = board
            .positions_for_chord(formula, root_pitch)
            .map_err(|error| error.to_string())?;
        let mut visits = Vec::with_capacity(expected.len());
        for (index, pitch) in expected.into_iter().enumerate() {
            let interval = formula.intervals.get(index).copied();
            let contact = positions
                .iter()
                .filter(|position| {
                    position.fret >= start
                        && position.fret <= end
                        && position.interval_from_root == interval
                        && position.pitch.midi() == pitch.midi()
                })
                .min_by_key(|position| (position.fret, position.string_index))
                .map(|position| (position.string_index, position.fret))
                .ok_or_else(|| {
                    format!("Chord tone {} has no exact fretboard contact", pitch.midi())
                })?;
            visits.push(NoteVisit {
                pitch: pitch.frequency() as f32,
                contact,
            });
        }
        Ok(visits)
    }

    pub(super) fn apply_card_marks(&self, card: &Card, visits: &mut Vec<NoteVisit>) {
        if card.setting.marked.is_empty() || card.setting.mark_mode == MarkMode::Off {
            return;
        }
        let marked = card
            .setting
            .marked
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        let dots = self.card_contact_pitches(card).unwrap_or_default();
        match card.setting.mark_mode {
            MarkMode::Off => {},
            MarkMode::Mute => {
                let muted = dots
                    .iter()
                    .filter(|(contact, _)| marked.contains(contact))
                    .map(|(_, frequency)| crate::pc_from_hz(*frequency))
                    .collect::<std::collections::BTreeSet<_>>();
                visits.retain(|visit| !muted.contains(&crate::pc_from_hz(visit.pitch)));
            },
            MarkMode::Solo
                if matches!(&card.material, Material::Chord { .. } if card.setting.voicing_idx.is_none())
                    || matches!(&card.material, Material::Scale { .. }) =>
            {
                // These legacy previews define Solo as the selected fretboard
                // contacts, sorted/deduplicated by pitch. The contact remains
                // the authored selected physical position.
                let mut selected = dots
                    .into_iter()
                    .filter(|(contact, _)| marked.contains(contact))
                    .map(|(contact, frequency)| NoteVisit {
                        pitch: frequency,
                        contact,
                    })
                    .collect::<Vec<_>>();
                selected.sort_by(|a, b| a.pitch.total_cmp(&b.pitch));
                selected.dedup_by(|a, b| (a.pitch - b.pitch).abs() < 0.02);
                *visits = selected;
            },
            MarkMode::Solo => {
                let selected = dots
                    .into_iter()
                    .filter(|(contact, _)| marked.contains(contact))
                    .map(|(contact, frequency)| NoteVisit {
                        pitch: frequency,
                        contact,
                    })
                    .collect::<Vec<_>>();
                let is_pattern = matches!(&card.material, Material::ScalePattern { .. });
                visits.retain_mut(|visit| {
                    // Pattern preview filters repeated recipe visits by the
                    // pitches found at marked contacts. It does not replace a
                    // recipe's physical contact with another equal-pitch dot.
                    if is_pattern {
                        selected
                            .iter()
                            .any(|marked| same_pitch(marked.pitch, visit.pitch))
                    } else {
                        marked.contains(&visit.contact)
                    }
                });
            },
        }
    }

    pub(super) fn apply_card_touch(&self, card: &Card, visits: &mut Vec<NoteVisit>) {
        let Touch::Arpeggiate {
            direction,
            inversion,
        } = card.touch
        else {
            return;
        };
        visits.retain(|visit| visit.pitch.is_finite() && visit.pitch > 0.0);
        visits.sort_by(|a, b| a.pitch.total_cmp(&b.pitch));
        if let Material::Chord { name, root } = &card.material {
            if let Some(formula) = woodshedding::chord::catalog()
                .iter()
                .find(|formula| formula.name == name)
            {
                let index = usize::from(inversion).min(formula.intervals.len().saturating_sub(1));
                if let Some(interval) = formula.intervals.get(index) {
                    let capo = if card.setting.voicing_idx.is_some() {
                        i32::from(card.setting.capo.unwrap_or(0))
                    } else {
                        0
                    };
                    let bass = (i32::from(root.value()) + capo + interval.semitones())
                        .rem_euclid(12) as u8;
                    if let Some(start) = visits
                        .iter()
                        .position(|visit| crate::pc_from_hz(visit.pitch) == bass)
                    {
                        visits.drain(..start);
                    }
                }
            }
        }
        match direction {
            crate::ArpeggioDirection::Up => {},
            crate::ArpeggioDirection::Down => visits.reverse(),
            crate::ArpeggioDirection::UpDown => {
                let back = visits
                    .iter()
                    .skip(1)
                    .take(visits.len().saturating_sub(2))
                    .rev()
                    .copied()
                    .collect::<Vec<_>>();
                visits.extend(back);
            },
        }
    }

    pub(super) fn live_card_template(&self, kind: LiveSequenceKind) -> Card {
        let mut card = self.card_from_lens().unwrap_or_else(|| Card {
            id: CardId::UNASSIGNED,
            label: "Live sequence".into(),
            material: Material::Scale {
                name: self.scale().name.into(),
                root: PitchClass::new((9 + self.root_idx as u8) % 12),
            },
            setting: live_setting(self),
            touch: Touch::Block,
            timing: Timing::default(),
            from: None,
        });
        match kind {
            LiveSequenceKind::Arpeggio => {
                card.label = self.material_name();
                card.material = Material::Chord {
                    name: self.arpeggio_chord().name.into(),
                    root: PitchClass::new((9 + self.root_idx as u8) % 12),
                };
                card.setting = live_setting(self);
                card.touch = Touch::Arpeggiate {
                    direction: self.arpeggio_direction,
                    inversion: self.arpeggio_inversion,
                };
            },
            LiveSequenceKind::Exercise => {
                card.label = self.exercise().name.into();
                card.material = Material::Riff {
                    name: self.exercise().name.into(),
                };
                card.setting = live_setting(self);
                card.touch = Touch::Walk;
            },
            LiveSequenceKind::ScaleRun => {},
        }
        card
    }

    pub(super) fn live_events(&self, kind: LiveSequenceKind) -> Vec<PracticeEvent> {
        let contacts = match kind {
            LiveSequenceKind::Arpeggio => {
                let board = Fretboard::new(self.tuning(), self.fret_count);
                let shapes = generate_shapes(
                    &board,
                    self.arpeggio_chord(),
                    self.root(),
                    self.arpeggio_bass(),
                );
                let Some(shape) = shapes.get(
                    self.arpeggio_position_idx
                        .min(shapes.len().saturating_sub(1)),
                ) else {
                    return Vec::new();
                };
                let run = ArpeggioRun::new(
                    &shape.positions,
                    self.arpeggio_bass(),
                    self.arpeggio_direction,
                );
                (0..run.walk_len())
                    .filter_map(|ordinal| {
                        let index = run.position_at(ordinal)?;
                        let position = shape.positions.get(index)?;
                        Some((
                            position.string_index,
                            position.fret,
                            position.pitch.frequency() as f32,
                        ))
                    })
                    .collect::<Vec<_>>()
            },
            LiveSequenceKind::Exercise => {
                let exercise = self.exercise();
                let params = ExerciseParams {
                    starting_fret: self.exercise_starting_fret,
                    ..ExerciseParams::default()
                };
                let board = Fretboard::new(self.tuning(), self.fret_count);
                exercise
                    .generate(&self.tuning(), &params)
                    .into_iter()
                    .map(|step| {
                        (
                            step.string_index,
                            step.fret,
                            board.pitch_at(step.string_index, step.fret).frequency() as f32,
                        )
                    })
                    .collect()
            },
            LiveSequenceKind::ScaleRun => self.effective_run_path(),
        };
        contacts
            .into_iter()
            .enumerate()
            .map(|(ordinal, (string, fret, pitch))| PracticeEvent {
                ordinal: ordinal as u32,
                at: ordinal as f64,
                strum_seconds: 0.0,
                pitches: vec![pitch],
                contacts: vec![(string, fret)],
            })
            .collect()
    }
}
