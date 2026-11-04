use super::*;

impl StageState {
    pub fn arpeggio_chord(&self) -> &'static ChordFormula {
        &chord_catalog()[self.arpeggio_idx.min(chord_catalog().len() - 1)]
    }

    pub fn select_arpeggio(&mut self, idx: usize) {
        if idx < chord_catalog().len() {
            self.arpeggio_idx = idx;
            self.arpeggio_position_idx = 0;
            self.arpeggio_step_idx = 0;
        }
    }

    pub fn arpeggio_select_position(&mut self, idx: usize) {
        self.arpeggio_position_idx = idx;
        self.arpeggio_step_idx = 0;
    }

    pub fn arpeggio_cycle_direction(&mut self) {
        self.arpeggio_direction = self.arpeggio_direction.next();
        self.arpeggio_step_idx = 0;
    }

    pub fn arpeggio_cycle_inversion(&mut self) {
        let tones = self.arpeggio_chord().intervals.len().max(1) as u8;
        self.arpeggio_inversion = (self.arpeggio_inversion + 1) % tones;
        self.arpeggio_position_idx = 0;
        self.arpeggio_step_idx = 0;
    }

    pub fn arpeggio_advance(&mut self) {
        self.arpeggio_step_idx = self.arpeggio_step_idx.wrapping_add(1);
    }

    /// Show or hide the touch's path trail on the board.
    pub fn toggle_path(&mut self) {
        self.path_shown = !self.path_shown;
    }

    /// Enter or leave draw mode (clicking markers draws the path by hand). The
    /// drawn path is kept when leaving, so the run keeps following it.
    pub fn toggle_draw_mode(&mut self) {
        self.draw_mode = !self.draw_mode;
    }

    /// Append a marker to the drawn path (the next step). Duplicates are allowed
    /// so a path can revisit a note.
    pub fn append_to_path(&mut self, string_index: usize, fret: u8) {
        self.authored_path.push((string_index, fret));
    }

    /// Undo the last drawn step.
    pub fn undo_path(&mut self) {
        self.authored_path.pop();
    }

    /// Clear the drawn path (the run reverts to the derived pitch-order).
    pub fn clear_path(&mut self) {
        self.authored_path.clear();
        self.scale_run_step = 0;
    }

    /// Reverse the drawn path (retrograde).
    pub fn reverse_path(&mut self) {
        self.authored_path.reverse();
        self.scale_run_step = 0;
    }

    /// Rotate the drawn path's start forward by one step.
    pub fn rotate_path(&mut self) {
        if !self.authored_path.is_empty() {
            self.authored_path.rotate_left(1);
            self.scale_run_step = 0;
        }
    }

    /// The run's visit order as `(string, fret)` — the path trail the leaf draws.
    pub fn run_positions(&self) -> Vec<(usize, u8)> {
        self.effective_run_path()
            .into_iter()
            .map(|(s, f, _)| (s, f))
            .collect()
    }

    /// The path the run actually walks: the hand-drawn `authored_path` when one
    /// exists (frequencies looked up from the current board), else the derived
    /// scale-run order. This is the seam where a drawn touch overrides the preset.
    pub fn effective_run_path(&self) -> Vec<(usize, u8, f32)> {
        if self.authored_path.is_empty() {
            return self.scale_run_path();
        }
        // Resolve each step's pitch from the neck itself rather than looking it
        // up among the current dots: a drawn note is a real position whatever
        // the lens shows, and a missed lookup would silently step in silence.
        let board = Fretboard::new(self.tuning(), self.fret_count);
        let strings = self.string_count();
        self.authored_path
            .iter()
            .filter(|(s, f)| *s < strings && *f <= self.fret_count)
            .map(|&(s, f)| (s, f, board.pitch_at(s, f).frequency() as f32))
            .collect()
    }

    /// Shift the whole drawn path along the neck by `delta` frets — move the
    /// shape, keep its intervals. A no-op if any note would leave the neck, so
    /// the shape never distorts by clamping. An octave (±12) is the shift that
    /// keeps every note's pitch class, so the path stays on the material's tones
    /// and its degrees hold — which is exactly the octave spider's generator.
    pub fn shift_path(&mut self, delta: i32) {
        if !self.can_shift_path(delta) {
            return;
        }
        for (_, f) in self.authored_path.iter_mut() {
            *f = (*f as i32 + delta) as u8;
        }
        self.scale_run_step = 0;
    }

    /// Whether [`Self::shift_path`] would fit on the neck — every note lands in
    /// `0..=fret_count`. The view gates the shift controls on this rather than
    /// offering a button that silently does nothing: on the default 12-fret neck
    /// an octave shift rarely fits at all.
    pub fn can_shift_path(&self, delta: i32) -> bool {
        !self.authored_path.is_empty()
            && self.authored_path.iter().all(|&(_, f)| {
                let shifted = f as i32 + delta;
                shifted >= 0 && shifted <= self.fret_count as i32
            })
    }

    /// The scale-run path: the current board dots sorted ascending by pitch, each
    /// as `(string, fret, frequency)`. A run climbs the neck through them.
    pub fn scale_run_path(&self) -> Vec<(usize, u8, f32)> {
        let mut dots: Vec<(usize, u8, f32)> = self
            .dots()
            .into_iter()
            .map(|d| (d.string_index, d.fret, d.frequency))
            .collect();
        dots.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
        dots
    }

    /// Advance the run one step: mark the note now sounding (for the board
    /// highlight), return its frequency to play, and move the cursor on. Wraps.
    pub fn scale_run_tick(&mut self) -> Option<f32> {
        let path = self.effective_run_path();
        if path.is_empty() {
            self.scale_run_active = None;
            return None;
        }
        let (s, f, freq) = path[self.scale_run_step % path.len()];
        self.scale_run_active = Some((s, f));
        self.scale_run_step = self.scale_run_step.wrapping_add(1);
        Some(freq)
    }

    pub fn toggle_scale_run(&mut self) {
        self.scale_run_playing = !self.scale_run_playing;
        self.scale_run_step = 0;
        self.scale_run_active = None;
    }

    /// The inversion's bass tone (the run's starting chord tone).
    pub(super) fn arpeggio_bass(&self) -> Interval {
        let formula = self.arpeggio_chord();
        let inv = (self.arpeggio_inversion as usize).min(formula.intervals.len().saturating_sub(1));
        formula
            .intervals
            .get(inv)
            .copied()
            .unwrap_or(Interval::PERFECT_UNISON)
    }

    /// Resolve the Arpeggio lens: the active shape's dots with the
    /// transport highlight, plus everything the deck controls display.
    pub fn arpeggio_board(&self) -> ArpeggioBoard {
        let formula = self.arpeggio_chord();
        let bass = self.arpeggio_bass();
        let board = Fretboard::new(self.tuning(), self.fret_count);
        let shapes = generate_shapes(&board, formula, self.root(), bass);
        let shape_count = shapes.len();
        let position_idx = self
            .arpeggio_position_idx
            .min(shape_count.saturating_sub(1));
        let shape = &shapes[position_idx];
        let run = ArpeggioRun::new(&shape.positions, bass, self.arpeggio_direction);
        let current = run.position_at(self.arpeggio_step_idx);
        let dots = shape
            .positions
            .iter()
            .enumerate()
            .map(|(i, p)| ArpDot {
                string_index: p.string_index,
                fret: p.fret,
                is_root: p.interval_from_root.is_some_and(|iv| iv.semitones() == 0),
                is_current: current == Some(i),
                label: format!("{}{}", p.pitch.name, p.pitch.accidental),
            })
            .collect();
        let inv = (self.arpeggio_inversion as usize).min(formula.intervals.len().saturating_sub(1));
        let inversion_label = match inv {
            0 => "Inv: Root".to_string(),
            1 => "Inv: 1st".to_string(),
            2 => "Inv: 2nd".to_string(),
            3 => "Inv: 3rd".to_string(),
            k => format!("Inv: {k}th"),
        };
        ArpeggioBoard {
            dots,
            shape_count,
            position_idx,
            start_fret: shape.start_fret,
            walk_len: run.walk_len(),
            step: self.arpeggio_step_idx % run.walk_len(),
            direction: self.arpeggio_direction,
            inversion_label,
        }
    }

    /// Frequency (Hz) of the arpeggio step-through's current tone — the
    /// step-sonification source. `None` if the walk is empty.
    pub fn arpeggio_current_pitch_hz(&self) -> Option<f32> {
        let formula = self.arpeggio_chord();
        let bass = self.arpeggio_bass();
        let board = Fretboard::new(self.tuning(), self.fret_count);
        let shapes = generate_shapes(&board, formula, self.root(), bass);
        if shapes.is_empty() {
            return None;
        }
        let position_idx = self.arpeggio_position_idx.min(shapes.len() - 1);
        let shape = &shapes[position_idx];
        let run = ArpeggioRun::new(&shape.positions, bass, self.arpeggio_direction);
        let current = run.position_at(self.arpeggio_step_idx)?;
        shape
            .positions
            .get(current)
            .map(|p| p.pitch.frequency() as f32)
    }
}
