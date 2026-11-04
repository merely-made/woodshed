use super::*;

impl StageState {
    /// The current lens's material as a rehearsal [`Card`] — the "+
    /// Rehearse" action (redesign R1: rehearse from any lens). Carries the
    /// tuning name and, for arpeggios, the touch; progression cards stamp
    /// their recipe provenance.
    pub fn card_from_lens(&self) -> Option<Card> {
        let root_pc = PitchClass::new(9 + self.root_idx as u8); // A = pc 9
        let setting = Setting {
            instrument: self.tuning().instrument.to_string(),
            tuning: Some(self.tuning().name.clone()),
            ..Setting::default()
        };
        let card = match self.lens {
            Lens::Scales => Card {
                id: CardId::UNASSIGNED,
                label: format!("{} {}", self.root_name(), self.scale().name),
                material: Material::Scale {
                    name: self.scale().name.to_string(),
                    root: root_pc,
                },
                setting,
                touch: Touch::Block,
                timing: Timing::default(),
                from: None,
            },
            Lens::Chords => Card {
                id: CardId::UNASSIGNED,
                label: self.material_name(),
                material: Material::Chord {
                    name: self.chord().name.to_string(),
                    root: root_pc,
                },
                setting,
                touch: Touch::Block,
                timing: Timing::default(),
                from: None,
            },
            Lens::Arpeggios => Card {
                id: CardId::UNASSIGNED,
                label: self.material_name(),
                material: Material::Chord {
                    name: self.arpeggio_chord().name.to_string(),
                    root: root_pc,
                },
                setting,
                touch: Touch::Arpeggiate {
                    direction: self.arpeggio_direction,
                    inversion: self.arpeggio_inversion,
                },
                timing: Timing::default(),
                from: None,
            },
            Lens::Progressions => {
                let board = self.progression_board()?;
                let prog = progression_catalog().get(self.progression_idx?)?;
                let major = scale_catalog().iter().find(|s| s.name == "Major")?;
                let chords = prog.apply_in_key(self.root(), major).ok()?;
                let expanded = self.progression_expanded.min(chords.len() - 1);
                let chord = &chords[expanded];
                Card {
                    id: CardId::UNASSIGNED,
                    label: board.expanded_label.clone(),
                    material: Material::Chord {
                        name: chord.formula.name.to_string(),
                        root: PitchClass::new(chord.root.pitch_class()),
                    },
                    setting,
                    touch: Touch::Block,
                    timing: Timing::default(),
                    from: Some(Recipe::Progression {
                        name: prog.name.to_string(),
                        key: root_pc,
                    }),
                }
            },
            Lens::Exercises => Card {
                id: CardId::UNASSIGNED,
                label: self.exercise().name.to_string(),
                material: Material::Riff {
                    name: self.exercise().name.to_string(),
                },
                setting,
                touch: Touch::Block,
                timing: Timing::default(),
                from: Some(Recipe::Exercise {
                    name: self.exercise().name.to_string(),
                }),
            },
        };
        Some(card)
    }

    /// The hand-drawn path as a rehearsal [`Card`] — Draw mode's Save. Unlike
    /// every other card, it names no catalog formula: the positions ride inline
    /// in a [`Material::Path`], with the root they were drawn over so degrees
    /// still resolve. This is the draw → save → practice loop's last leg.
    /// `None` when nothing is drawn.
    pub fn card_from_drawn_path(&self) -> Option<Card> {
        if self.authored_path.is_empty() {
            return None;
        }
        Some(Card {
            id: CardId::UNASSIGNED,
            label: format!(
                "{} path — {} notes",
                self.material_name(),
                self.authored_path.len()
            ),
            material: Material::Path {
                positions: self.authored_path.clone(),
                root: PitchClass::new(9 + self.root_idx as u8), // A = pc 9
            },
            setting: Setting {
                instrument: self.tuning().instrument.to_string(),
                tuning: Some(self.tuning().name.clone()),
                ..Setting::default()
            },
            // A drawn path carries its own order, so it walks — the honest touch
            // for it, where Block would sound the notes on top of each other.
            touch: Touch::Walk,
            timing: Timing::default(),
            from: None,
        })
    }

    pub fn dots_for_card(&self, card: &Card) -> Vec<FretDot> {
        if matches!(card.material, Material::ChordApproach { .. }) {
            return self
                .chord_approach_realization(card)
                .map(|approach| approach.dots)
                .unwrap_or_default();
        }
        if matches!(
            card.material,
            Material::Scale { .. } | Material::ScalePattern { .. }
        ) {
            return self
                .scale_card_realization(card)
                .map(|scale| scale.dots)
                .unwrap_or_default();
        }
        if matches!(card.material, Material::Chord { .. }) && card.setting.voicing_idx.is_some() {
            return self.selected_card_shape_dots(card).unwrap_or_default();
        }
        // Formula chord marks resolve on the Card's saved setup, like selected
        // shapes. Other material retains its existing domain-specific path.
        if let Material::Chord { name, root } = &card.material {
            let Ok(tuning) = self.resolve_card_tuning(card) else {
                return Vec::new();
            };
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
                return Vec::new();
            }
            let board = Fretboard::new(tuning, end);
            let root = Pitch::from_midi(48 + i32::from(root.value()), Spelling::Sharps);
            return chord_catalog()
                .iter()
                .find(|formula| formula.name == name)
                .and_then(|formula| board.positions_for_chord(formula, root).ok())
                .map(|positions| {
                    positions
                        .into_iter()
                        .filter(|position| position.fret >= start && position.fret <= end)
                        .map(FretDot::from_position)
                        .collect()
                })
                .unwrap_or_default();
        }
        let board = Fretboard::new(self.tuning(), self.fret_count);
        let root_of = |pc: &PitchClass| Pitch::from_midi(48 + pc.value() as i32, Spelling::Sharps);
        let positions = match &card.material {
            Material::Scale { name, root } | Material::ScalePattern { name, root, .. } => {
                scale_catalog()
                    .iter()
                    .find(|s| s.name == name.as_str())
                    .and_then(|s| board.positions_for_scale(s, root_of(root)).ok())
            },
            Material::Chord { name, root } | Material::ChordApproach { name, root, .. } => {
                chord_catalog()
                    .iter()
                    .find(|c| c.name == name.as_str())
                    .and_then(|c| board.positions_for_chord(c, root_of(root)).ok())
            },
            Material::Riff { name } => {
                // A riff card references an exercise; show its full
                // position set (the step-through runs on the Exercise lens).
                return exercise_catalog()
                    .iter()
                    .find(|e| e.name == name.as_str())
                    .map(|e| {
                        e.generate(&self.tuning(), &ExerciseParams::default())
                            .into_iter()
                            .map(|s| FretDot {
                                string_index: s.string_index,
                                fret: s.fret,
                                is_root: false,
                                label: if s.finger == 0 {
                                    String::new()
                                } else {
                                    s.finger.to_string()
                                },
                                // Exercise steps carry a finger, not a resolved
                                // pitch, so there is no note detail to show.
                                octave: 0,
                                degree: String::new(),
                                interval_name: String::new(),
                                frequency: 0.0,
                            })
                            .collect()
                    })
                    .unwrap_or_default();
            },
            Material::Path { root, .. } => {
                // Resolve the saved instruction on its retained setup. The
                // live fret window clips only this visualization.
                let root_pc = root.value() as i32;
                let Ok(notes) = self.path_contact_pitches(card) else {
                    return Vec::new();
                };
                return notes
                    .into_iter()
                    .filter(|((_, fret), _)| *fret >= self.fret_start && *fret <= self.fret_count)
                    .map(|((s, f), pitch)| {
                        let semis = (pitch.pitch_class() as i32 - root_pc).rem_euclid(12);
                        FretDot {
                            string_index: s,
                            fret: f,
                            is_root: semis == 0,
                            label: format!("{}{}", pitch.name, pitch.accidental),
                            octave: pitch.octave,
                            degree: degree_label(semis).to_string(),
                            interval_name: interval_name(semis).to_string(),
                            frequency: pitch.frequency() as f32,
                        }
                    })
                    .collect();
            },
        };
        // Setting fidelity: a pinned fret window filters the positions to
        // the hand position (capo + per-card tuning still deferred). The board's
        // neck window (fret_start..) applies on top, since the board is drawn to
        // that extent.
        let window = card.setting.fret_window;
        let neck_start = self.fret_start;
        let in_window = |fret: u8| {
            fret >= neck_start
                && window.is_none_or(|w| fret >= w.start && fret <= w.start.saturating_add(w.span))
        };
        positions
            .map(|ps| {
                ps.into_iter()
                    .filter(|p| in_window(p.fret))
                    .map(FretDot::from_position)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Resolve every authored Path contact on the Card's retained setup.
    /// Repeated contacts and their order are material, so this never sorts or
    /// deduplicates. The live viewport is intentionally not consulted.
    pub(crate) fn path_contact_pitches(
        &self,
        card: &Card,
    ) -> Result<Vec<((usize, u8), Pitch)>, String> {
        let Material::Path { positions, .. } = &card.material else {
            return Err("Card is not Path material".into());
        };
        let tuning = self
            .resolve_card_tuning(card)
            .map_err(|error| error.to_string())?;
        let fret_limit = tuning.instrument.standard_fret_count();
        let capo = card.setting.capo.unwrap_or(0);
        if positions.is_empty() {
            return Err("Path has no positions".into());
        }
        if let Some(&(string, fret)) = positions.iter().find(|(string, fret)| {
            *string >= tuning.string_count() || *fret > fret_limit || *fret < capo
        }) {
            return Err(format!(
                "Path contact ({string}, {fret}) is unavailable on its saved setup"
            ));
        }
        let board = Fretboard::new(tuning, fret_limit);
        Ok(positions
            .iter()
            .map(|&(string, fret)| ((string, fret), board.pitch_at(string, fret)))
            .collect())
    }

    /// Effective contacts used by generic sound, managed lowering, and
    /// retained recipe validation. Paths resolve the complete saved sequence;
    /// other material reuses its established dot realization.
    pub(crate) fn card_contact_pitches(
        &self,
        card: &Card,
    ) -> Result<Vec<((usize, u8), f32)>, String> {
        if matches!(card.material, Material::Path { .. }) {
            return self.path_contact_pitches(card).map(|notes| {
                notes
                    .into_iter()
                    .map(|(contact, pitch)| (contact, pitch.frequency() as f32))
                    .collect()
            });
        }
        Ok(self
            .dots_for_card(card)
            .into_iter()
            .map(|dot| ((dot.string_index, dot.fret), dot.frequency))
            .collect())
    }
}
