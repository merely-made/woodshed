use super::*;

/// Resolve a raw frequency (Hz) to a `pitch_detector::note::NoteDetectionResult`-
/// shaped tuple — the rest of the analyzer expects that shape.
/// Mirrors pitch-detector's note-resolution math: nearest 12-TET
/// note at A=440, cents offset signed in [-50, +50], `in_tune` if
/// |cents| < [`IN_TUNE_CENTS_TOLERANCE`].
pub(super) fn freq_to_note_detection_result(
    freq_hz: f64,
) -> pitch_detector::note::NoteDetectionResult {
    // Semitones above A4 (440 Hz). MIDI note number = round(s) + 69.
    let semis = 12.0 * (freq_hz / 440.0).log2();
    let midi = (semis.round() as i32) + 69;
    // Reference frequency of the nearest 12-TET note.
    let nearest_hz = 440.0 * 2.0_f64.powf((midi as f64 - 69.0) / 12.0);
    // Cents offset of the actual freq from the nearest note.
    let cents = 1200.0 * (freq_hz / nearest_hz).log2();
    // MIDI → (name, octave). C4 = MIDI 60, so pitch-class = midi % 12,
    // octave = midi / 12 - 1.
    let pc = midi.rem_euclid(12) as usize;
    let octave = (midi / 12) - 1;
    let note_name = match pc {
        0 => PdNoteName::C,
        1 => PdNoteName::CSharp,
        2 => PdNoteName::D,
        3 => PdNoteName::DSharp,
        4 => PdNoteName::E,
        5 => PdNoteName::F,
        6 => PdNoteName::FSharp,
        7 => PdNoteName::G,
        8 => PdNoteName::GSharp,
        9 => PdNoteName::A,
        10 => PdNoteName::ASharp,
        _ => PdNoteName::B,
    };
    // Adjacent note names — used by pitch-detector's hint logic; we
    // fill them in by stepping pc ±1 mod 12.
    let prev_pc = (pc + 11) % 12;
    let next_pc = (pc + 1) % 12;
    let to_pd = |pc: usize| match pc {
        0 => PdNoteName::C,
        1 => PdNoteName::CSharp,
        2 => PdNoteName::D,
        3 => PdNoteName::DSharp,
        4 => PdNoteName::E,
        5 => PdNoteName::F,
        6 => PdNoteName::FSharp,
        7 => PdNoteName::G,
        8 => PdNoteName::GSharp,
        9 => PdNoteName::A,
        10 => PdNoteName::ASharp,
        _ => PdNoteName::B,
    };
    pitch_detector::note::NoteDetectionResult {
        actual_freq: freq_hz,
        note_name,
        octave: octave as i32,
        cents_offset: cents,
        note_freq: nearest_hz,
        in_tune: cents.abs() < IN_TUNE_CENTS_TOLERANCE,
        previous_note_name: to_pd(prev_pc),
        next_note_name: to_pd(next_pc),
    }
}

impl TunerHandle {
    pub fn snapshot(&self) -> TunerSnapshot {
        self.inner.lock().unwrap().snapshot.clone()
    }

    /// Set the RMS amplitude threshold below which detection is
    /// suppressed.
    pub fn set_threshold(&self, threshold_rms: f64) {
        self.inner.lock().unwrap().threshold_rms = threshold_rms;
    }

    pub fn threshold(&self) -> f64 {
        self.inner.lock().unwrap().threshold_rms
    }

    pub fn set_detector_kind(&self, kind: DetectorKind) {
        self.inner.lock().unwrap().detector_kind = kind;
    }

    pub fn detector_kind(&self) -> DetectorKind {
        self.inner.lock().unwrap().detector_kind
    }

    pub fn set_target_hint(&self, hint: Option<DetectedNoteName>) {
        self.inner.lock().unwrap().target_hint = hint;
    }

    pub fn target_hint(&self) -> Option<DetectedNoteName> {
        self.inner.lock().unwrap().target_hint.clone()
    }

    /// Enable or disable pitch detection. When disabled the FFT
    /// pipeline is skipped entirely — useful when the user is on a
    /// non-tuner tab.
    pub fn set_enabled(&self, enabled: bool) {
        self.inner.lock().unwrap().enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.inner.lock().unwrap().enabled
    }
}

impl Default for PitchAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl PitchAnalyzer {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(PitchInternals {
                snapshot: TunerSnapshot::default(),
                threshold_rms: DEFAULT_SILENCE_RMS_THRESHOLD,
                detector_kind: DetectorKind::Fft,
                target_hint: None,
                // Enabled by default so a tuner-only app "just works"
                // without an explicit handshake. UIs that want to gate
                // by tab visibility should call `handle.set_enabled(false)`
                // after construction.
                enabled: true,
            })),
            buffer: Vec::with_capacity(ANALYSIS_WINDOW * 2),
            fft_detector: HannedFftDetector::default(),
            cepstrum_detector: PowerCepstrum::default(),
            history: VecDeque::with_capacity(HISTORY_LEN),
            harmonic_suppress_streak: 0,
        }
    }

    /// Clone-able handle into this analyzer's shared state.
    pub fn handle(&self) -> TunerHandle {
        TunerHandle {
            inner: Arc::clone(&self.state),
        }
    }
}

impl Analyzer for PitchAnalyzer {
    fn process(&mut self, samples: &[f32], sample_rate_hz: f32, _at: Instant) {
        // Read the live config and the enable flag in one lock.
        let (enabled, threshold, kind, hint) = {
            let s = self.state.lock().unwrap();
            (
                s.enabled,
                s.threshold_rms,
                s.detector_kind,
                s.target_hint.clone(),
            )
        };
        if !enabled {
            // Keep the buffer drained-ish so when we re-enable we
            // don't analyze stale audio.
            if self.buffer.len() > ANALYSIS_WINDOW {
                let to_drop = self.buffer.len() - ANALYSIS_WINDOW;
                self.buffer.drain(..to_drop);
            }
            return;
        }

        for &s in samples {
            self.buffer.push(s as f64);
        }
        if self.buffer.len() < ANALYSIS_WINDOW {
            return;
        }
        let start = self.buffer.len() - ANALYSIS_WINDOW;
        let signal = &self.buffer[start..];
        let rms = compute_rms(signal);
        let sample_rate = sample_rate_hz as f64;

        let raw = if rms >= threshold {
            let range = MIN_DETECT_FREQ..MAX_DETECT_FREQ;
            // pitch-detector v0.3 only implements HintedNoteDetector for
            // HannedFftDetector, so when a target hint is set we route
            // through FFT regardless of selected kind.
            let result = match (kind, hint) {
                (_, Some(name)) => self.fft_detector.detect_note_with_hint_and_range(
                    name,
                    signal,
                    sample_rate,
                    Some(range),
                ),
                (DetectorKind::Fft, None) => {
                    detect_note_in_range(signal, &mut self.fft_detector, sample_rate, range)
                },
                (DetectorKind::Cepstrum, None) => {
                    detect_note_in_range(signal, &mut self.cepstrum_detector, sample_rate, range)
                },
                // McLeod returns a raw frequency + clarity; we
                // resolve to note name / cents locally since
                // pitch-detector's NoteDetectionResult builder
                // isn't on this code path. `freq_to_note` mirrors
                // pitch-detector's internal math: nearest 12-TET
                // note at A=440, cents offset signed (-50..+50),
                // `in_tune` if |cents| < tolerance.
                (DetectorKind::McLeod, None) => MCLEOD.with(|cell| {
                    cell.borrow_mut()
                        .get_pitch(
                            signal,
                            sample_rate as usize,
                            MCLEOD_POWER_THRESHOLD,
                            MCLEOD_CLARITY_THRESHOLD,
                        )
                        .and_then(|p| {
                            let f = p.frequency;
                            if f < MIN_DETECT_FREQ || f > MAX_DETECT_FREQ {
                                None
                            } else {
                                Some(freq_to_note_detection_result(f))
                            }
                        })
                }),
            };
            result.map(|n| DetectedNote {
                name: n.note_name,
                octave: n.octave,
                note_freq_hz: n.note_freq,
                actual_freq_hz: n.actual_freq,
                cents_offset: n.cents_offset,
                in_tune: n.in_tune,
            })
        } else {
            None
        };

        // Octave-error correction: if the new raw detection is a
        // small-integer multiple/fraction of the prior stable freq,
        // suspect harmonic confusion and suppress unless it persists.
        let prior_stable = compute_stable(&self.history);
        let suspect = matches!(
            (&raw, &prior_stable),
            (Some(note), Some(stable))
                if is_harmonic_of(note.actual_freq_hz, stable.actual_freq_hz)
        );

        if suspect {
            self.harmonic_suppress_streak += 1;
            if self.harmonic_suppress_streak >= HARMONIC_SUPPRESS_GRACE {
                self.history.clear();
                if let Some(note) = raw {
                    self.history.push_back(note);
                }
                self.harmonic_suppress_streak = 0;
            }
        } else {
            self.harmonic_suppress_streak = 0;
            match raw {
                Some(note) => {
                    if self.history.len() >= HISTORY_LEN {
                        self.history.pop_front();
                    }
                    self.history.push_back(note);
                },
                None => self.history.clear(),
            }
        }

        let stable = compute_stable(&self.history);

        let mut s = self.state.lock().unwrap();
        s.snapshot.input_level = rms as f32;
        s.snapshot.note = stable;
        drop(s);

        // Drain by HOP_SIZE so successive analyses overlap.
        let to_drop = HOP_SIZE.min(self.buffer.len());
        self.buffer.drain(..to_drop);
    }
}

pub(super) fn compute_rms(signal: &[f64]) -> f64 {
    if signal.is_empty() {
        return 0.0;
    }
    let sum_sq: f64 = signal.iter().map(|s| s * s).sum();
    (sum_sq / signal.len() as f64).sqrt()
}

/// True if `raw` looks like a harmonic-confusion artifact of `reference`:
/// approximately a 2×, 3×, or 4× multiple, or a 1/2, 1/3, 1/4 fraction.
pub(super) fn is_harmonic_of(raw_freq: f64, reference_freq: f64) -> bool {
    if reference_freq < 1.0 || raw_freq < 1.0 {
        return false;
    }
    let ratio = raw_freq / reference_freq;
    let near = |target: f64| (ratio - target).abs() / target < 0.03;
    near(2.0) || near(3.0) || near(4.0) || near(0.5) || near(1.0 / 3.0) || near(0.25)
}

/// Majority-stable detection across recent history, with promotion of
/// a sub-multiple fundamental when the consensus is its harmonic.
pub(super) fn compute_stable(history: &VecDeque<DetectedNote>) -> Option<DetectedNote> {
    if history.len() < 2 {
        return None;
    }
    let mut best_count = 0;
    let mut best: Option<&DetectedNote> = None;
    for candidate in history {
        let count = history
            .iter()
            .filter(|n| n.name == candidate.name && n.octave == candidate.octave)
            .count();
        if count > best_count {
            best_count = count;
            best = Some(candidate);
        }
    }
    let required = history.len().div_ceil(2);
    if best_count < required {
        return None;
    }
    let consensus = best?;
    let consensus_freq = consensus.actual_freq_hz;

    // Fundamental-bias promotion.
    if consensus_freq > 1.0 {
        for entry in history {
            let f = entry.actual_freq_hz;
            if f < 1.0 || f >= consensus_freq * 0.95 {
                continue;
            }
            if is_harmonic_of(consensus_freq, f) {
                return Some(entry.clone());
            }
        }
    }

    history
        .iter()
        .rev()
        .find(|n| n.name == consensus.name && n.octave == consensus.octave)
        .cloned()
}
