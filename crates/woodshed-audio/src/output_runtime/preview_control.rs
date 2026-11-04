use super::*;

impl OutputRuntime {
    pub(super) fn cancel_worker(&mut self) {
        let revision = self.preview.cancel();
        self.shared
            .preview_revision
            .store(revision, Ordering::Release);
    }

    pub fn cancel_preview(&mut self) -> Result<(), OutputError> {
        self.cancel_worker();
        self.send(Command::CancelPreview)
    }

    /// Admit an exact resolved Card duration, without generic audition minima.
    /// Refusal happens before cancellation or worker publication.
    pub fn preview_card_pitches(
        &mut self,
        pitches: &[f32],
        duration: f32,
        strum_ms: f32,
    ) -> Result<(), OutputError> {
        if self.program_owns_output() {
            return Err(err(
                "stop the prepared program before starting a one-shot preview",
            ));
        }
        if !duration.is_finite() || duration <= 0.0 || !strum_ms.is_finite() || strum_ms < 0.0 {
            return Err(err(
                "Card preview requires finite positive duration and finite nonnegative spacing",
            ));
        }
        if self.song.playing || self.snapshot().playing {
            return Err(err("Stop backing playback before auditioning this Card"));
        }
        if (duration * self.rate as f32).round() < 1.0 {
            return Err(err(
                "Card preview duration is shorter than one output frame",
            ));
        }
        if pitches.len() > 1 && strum_ms > 0.0 && strum_ms / 1000.0 * (self.rate as f32) < 1.0 {
            return Err(err("Ordered Card spacing is shorter than one output frame"));
        }
        self.preview_pitches_inner(pitches, duration, strum_ms, true)
    }

    pub fn preview_note(&mut self, frequency: f32, duration: f32) -> Result<(), OutputError> {
        if self.program_owns_output() {
            return Err(err(
                "stop the prepared program before starting a one-shot preview",
            ));
        }
        if !duration.is_finite() {
            return Err(err("preview requires a finite duration"));
        }
        self.preview_pitches_inner(&[frequency], duration.max(0.05), 0.0, false)
    }

    pub fn preview_pitches(
        &mut self,
        pitches: &[f32],
        duration: f32,
        strum: f32,
    ) -> Result<(), OutputError> {
        if self.program_owns_output() {
            return Err(err(
                "stop the prepared program before starting a one-shot preview",
            ));
        }
        // f32::max treats NaN as the other operand. Validate the authored
        // request before applying the existing finite convenience minima.
        if !duration.is_finite() || !strum.is_finite() {
            return Err(err("preview requires finite duration and strum values"));
        }
        self.preview_pitches_inner(pitches, duration.max(0.1), strum.max(0.0), true)
    }

    pub(super) fn preview_pitches_inner(
        &mut self,
        pitches: &[f32],
        duration: f32,
        strum: f32,
        replace: bool,
    ) -> Result<(), OutputError> {
        if self.snapshot().playing {
            return Ok(());
        }
        if !duration.is_finite()
            || !strum.is_finite()
            || duration * self.rate as f32 > MAX_PCM_FRAMES as f32
            || pitches.len() > 256
        {
            return Err(err("preview exceeds supported resource bounds"));
        }
        let pitches: Vec<_> = pitches
            .iter()
            .copied()
            .filter(|f| f.is_finite() && *f > 0.0)
            .collect();
        if pitches.is_empty() {
            return Ok(());
        }
        if replace {
            self.send(Command::CancelPreview)?;
        }
        let revision = self.preview.request(
            ChordRender {
                pitches_hz: pitches,
                duration_seconds: duration,
                strum_offset_ms: strum,
                ..Default::default()
            },
            self.rate,
        );
        self.shared
            .preview_revision
            .store(revision, Ordering::Release);
        Ok(())
    }

    /// Queue two independently voiced chord groups as one ordered preview.
    /// The worker renders both groups and the pause into one bounded buffer,
    /// so the output processor plays before then after without a later request
    /// cancelling the first chord.
    pub fn preview_phrase(
        &mut self,
        before_hz: &[f32],
        after_hz: &[f32],
        chord_duration_secs: f32,
        gap_secs: f32,
        strum_ms: f32,
    ) -> Result<(), OutputError> {
        if self.program_owns_output() {
            return Err(err(
                "stop the prepared program before starting a one-shot preview",
            ));
        }
        if self.snapshot().playing {
            return Ok(());
        }
        if !chord_duration_secs.is_finite()
            || chord_duration_secs <= 0.0
            || !gap_secs.is_finite()
            || gap_secs < 0.0
            || !strum_ms.is_finite()
            || before_hz.len() > 256
            || after_hz.len() > 256
            || (2.0 * chord_duration_secs + gap_secs) * self.rate as f32 > MAX_PCM_FRAMES as f32
        {
            return Err(err("ordered preview exceeds supported resource bounds"));
        }
        let valid = |pitches: &[f32]| {
            pitches
                .iter()
                .copied()
                .filter(|frequency| frequency.is_finite() && *frequency > 0.0)
                .collect::<Vec<_>>()
        };
        let before = valid(before_hz);
        let after = valid(after_hz);
        if before.is_empty() || after.is_empty() {
            return Ok(());
        }
        let group = |pitches_hz| ChordRender {
            pitches_hz,
            duration_seconds: chord_duration_secs,
            strum_offset_ms: strum_ms.max(0.0),
            ..Default::default()
        };
        // Cancel any currently audible preview before replacing its worker
        // request. Cancelling the worker alone cannot retire a voice already
        // handed to the processor.
        self.send(Command::CancelPreview)?;
        let revision =
            self.preview
                .request_phrase(vec![group(before), group(after)], gap_secs, self.rate);
        self.shared
            .preview_revision
            .store(revision, Ordering::Release);
        Ok(())
    }
}
