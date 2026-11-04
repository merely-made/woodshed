use super::*;

impl StageState {
    /// The active lens's material name for captions ("A Minor Pentatonic",
    /// "A m7").
    pub fn material_name(&self) -> String {
        match self.lens {
            Lens::Scales => format!("{} {}", self.root_name(), self.scale().name),
            Lens::Chords => {
                let c = self.chord();
                if c.symbol.is_empty() {
                    format!("{} {}", self.root_name(), c.name)
                } else {
                    format!("{}{} ({})", self.root_name(), c.symbol, c.name)
                }
            },
            Lens::Arpeggios => {
                let c = self.arpeggio_chord();
                if c.symbol.is_empty() {
                    format!("{} {} arpeggio", self.root_name(), c.name)
                } else {
                    format!("{}{} arpeggio", self.root_name(), c.symbol)
                }
            },
            Lens::Progressions => match self.progression_idx {
                Some(i) => format!(
                    "{} in {}",
                    progression_catalog()[i.min(progression_catalog().len() - 1)].name,
                    self.root_name(),
                ),
                None => "Progression".to_string(),
            },
            Lens::Exercises => self.exercise().name.to_string(),
        }
    }

    pub fn exercises(&self) -> &'static [Exercise] {
        exercise_catalog()
    }

    pub fn exercise(&self) -> &'static Exercise {
        &exercise_catalog()[self.exercise_idx.min(exercise_catalog().len() - 1)]
    }

    pub fn select_exercise(&mut self, idx: usize) {
        if idx < exercise_catalog().len() {
            self.exercise_idx = idx;
            self.exercise_step_idx = 0;
        }
    }

    pub fn exercise_advance(&mut self) {
        self.exercise_step_idx = self.exercise_step_idx.wrapping_add(1);
    }

    /// Shift the four-fret hand position, clamped to the board.
    pub fn exercise_nudge_fret(&mut self, delta: i32) {
        let max = self.fret_count.saturating_sub(3);
        let next = (self.exercise_starting_fret as i32 + delta).clamp(0, max as i32);
        if next as u8 != self.exercise_starting_fret {
            self.exercise_starting_fret = next as u8;
            self.exercise_step_idx = 0;
        }
    }

    /// Resolve the Exercise lens: the current step plus its fading trail
    /// (the sequence-aware presentation — motion order, not a static
    /// rectangle of positions).
    pub fn exercise_board(&self) -> ExerciseBoard {
        let ex = self.exercise();
        let params = ExerciseParams {
            starting_fret: self.exercise_starting_fret,
            ..ExerciseParams::default()
        };
        let steps = ex.generate(&self.tuning(), &params);
        let total = steps.len().max(1);
        let step = self.exercise_step_idx % total;
        let mut dots = Vec::new();
        for recency in 0..=EXERCISE_TRAIL.min(step) {
            let s = steps[step - recency];
            // A trail entry under a newer dot on the same position would
            // repaint over it; keep the newest only.
            if dots
                .iter()
                .any(|d: &ExerciseDot| d.string_index == s.string_index && d.fret == s.fret)
            {
                continue;
            }
            dots.push(ExerciseDot {
                string_index: s.string_index,
                fret: s.fret,
                recency,
                label: if s.finger == 0 {
                    String::new()
                } else {
                    s.finger.to_string()
                },
            });
        }
        ExerciseBoard {
            dots,
            step,
            total,
            starting_fret: self.exercise_starting_fret,
            name: ex.name,
            description: ex.description,
        }
    }

    pub fn progressions(&self) -> &'static [Progression] {
        progression_catalog()
    }

    pub fn select_progression(&mut self, idx: usize) {
        if idx < progression_catalog().len() {
            self.progression_idx = Some(idx);
            self.progression_expanded = 0;
        }
    }

    pub fn progression_expand(&mut self, idx: usize) {
        self.progression_expanded = idx;
    }

    /// Materialize the selected progression in the current key (major
    /// scale of the shared root, matching woodshed-xilem) and resolve the
    /// expanded chord's tones. `None` until a progression is picked.
    pub fn progression_board(&self) -> Option<ProgressionBoard> {
        let idx = self.progression_idx?;
        let prog = progression_catalog().get(idx)?;
        let major = scale_catalog().iter().find(|s| s.name == "Major")?;
        let chords = prog.apply_in_key(self.root(), major).ok()?;
        if chords.is_empty() {
            return None;
        }
        let expanded = self.progression_expanded.min(chords.len() - 1);
        let cards: Vec<ProgressionCard> = chords
            .iter()
            .enumerate()
            .map(|(i, c)| ProgressionCard {
                numeral: format_role(&c.role),
                chord_label: format!("{}{}{}", c.root.name, c.root.accidental, c.formula.symbol),
                is_expanded: i == expanded,
            })
            .collect();
        let chord = &chords[expanded];
        let board = Fretboard::new(self.tuning(), self.fret_count);
        let dots = board
            .positions_for_chord(chord.formula, chord.root)
            .map(|ps| ps.into_iter().map(FretDot::from_position).collect())
            .unwrap_or_default();
        let expanded_label = format!(
            "{} ({})",
            cards[expanded].chord_label, cards[expanded].numeral
        );
        Some(ProgressionBoard {
            name: prog.name,
            description: prog.description,
            cards,
            dots,
            expanded_label,
        })
    }

    /// Resolve the active lens to fretboard dots. Empty for the
    /// not-yet-migrated lenses (and on a theoretically impossible
    /// transposition failure, rather than panicking in a render path).
    pub fn dots(&self) -> Vec<FretDot> {
        let board = Fretboard::new(self.tuning(), self.fret_count);
        let positions = match self.lens {
            Lens::Scales => board.positions_for_scale(self.scale(), self.root()),
            Lens::Chords => board.positions_for_chord(self.chord(), self.root()),
            _ => return Vec::new(),
        };
        let start = self.fret_start;
        positions
            .map(|ps| {
                ps.into_iter()
                    .filter(|p| p.fret >= start)
                    .map(FretDot::from_position)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The current transport lens's board as uniform painted-leaf markers plus
    /// the transport's current position (the leaf's bright "active" step). The
    /// host feeds the leaf from these and the view draws the overlay from them, so
    /// Arpeggio / Exercise / Progression render on the same painted neck (with
    /// orientation and the scroll viewport) as Scale / Chord. Scale / Chord return
    /// empty here and keep their richer [`Self::dots`] path (pins, draw, cards).
    pub fn lens_markers(&self) -> (Vec<LensMarker>, Option<(usize, u8)>) {
        match self.lens {
            Lens::Arpeggios => {
                let b = self.arpeggio_board();
                let active = b
                    .dots
                    .iter()
                    .find(|d| d.is_current)
                    .map(|d| (d.string_index, d.fret));
                let markers = b
                    .dots
                    .iter()
                    .map(|d| LensMarker {
                        string_index: d.string_index,
                        fret: d.fret,
                        is_root: d.is_root,
                        is_current: d.is_current,
                        is_trail: false,
                        label: d.label.clone(),
                    })
                    .collect();
                (markers, active)
            },
            Lens::Exercises => {
                let b = self.exercise_board();
                // recency 0 is the current step; the trail (recency > 0) is not
                // yet drawn on the leaf (a later step).
                let active = b
                    .dots
                    .iter()
                    .find(|d| d.recency == 0)
                    .map(|d| (d.string_index, d.fret));
                let markers = b
                    .dots
                    .iter()
                    .map(|d| LensMarker {
                        string_index: d.string_index,
                        fret: d.fret,
                        is_root: false,
                        is_current: d.recency == 0,
                        is_trail: d.recency > 0,
                        label: d.label.clone(),
                    })
                    .collect();
                (markers, active)
            },
            Lens::Progressions => {
                let markers = self
                    .progression_board()
                    .map(|b| {
                        b.dots
                            .iter()
                            .map(|d| LensMarker {
                                string_index: d.string_index,
                                fret: d.fret,
                                is_root: d.is_root,
                                is_current: false,
                                is_trail: false,
                                label: d.label.clone(),
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                (markers, None)
            },
            Lens::Scales | Lens::Chords => (Vec::new(), None),
        }
    }
}
