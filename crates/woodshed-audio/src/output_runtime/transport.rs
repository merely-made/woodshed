use super::*;

impl OutputRuntime {
    pub fn song(&self) -> Song {
        let mut song = self.song.clone();
        let s = self.snapshot();
        song.cursor = s.cursor;
        if self.shared.commands.load(Ordering::Acquire) == 0 {
            song.playing = s.playing;
        }
        song.recording = s.recording;
        if !self.capture_busy() {
            song.pending.clear();
        }
        song
    }

    pub fn set_song(&mut self, mut song: Song) -> Result<(), OutputError> {
        if self.program_owns_output() {
            return Err(err("stop the prepared program before replacing backing"));
        }
        if self.capture_busy() {
            return Err(err(
                "cannot replace backing while capture is armed or active",
            ));
        }
        let prepared = PreparedBacking::prepare(&song, self.rate, &mut self.chord_cache)?;
        let generation = self
            .shared
            .generation
            .load(Ordering::Acquire)
            .wrapping_add(1);
        self.send(Command::Backing {
            data: Some(OwnedGc::new(prepared)),
            reset: true,
            clear: false,
            playing: song.playing,
            generation,
        })?;
        self.cancel_worker();
        self.shared.generation.store(generation, Ordering::Release);
        song.cursor = Default::default();
        song.recording = false;
        song.pending.clear();
        self.song = song;
        Ok(())
    }

    pub fn set_metronome(&mut self, bpm: f32, playing: bool) -> Result<(), OutputError> {
        if !bpm.is_finite() || bpm <= 0.0 {
            return Err(err("metronome tempo must be finite and positive"));
        }
        if self.metronome == (bpm, playing) {
            return Ok(());
        }
        let transition = playing != self.metronome.1;
        self.send(Command::Metronome { bpm, playing })?;
        if transition {
            self.cancel_worker();
        }
        self.metronome = (bpm, playing);
        Ok(())
    }

    pub fn set_click_pattern(&mut self, pattern: SequencerPattern) -> Result<(), OutputError> {
        let bpm = pattern.bpm;
        let data = PreparedClick::prepare(&pattern, self.rate)?;
        self.send(Command::ClickPattern {
            data: Some(OwnedGc::new(data)),
            bpm,
            playing: None,
            backing_rhythm: false,
        })?;
        self.metronome.0 = bpm;
        Ok(())
    }

    /// Prepare and admit the pattern and play state as one bounded operation.
    pub fn set_click_pattern_playing(
        &mut self,
        pattern: SequencerPattern,
        playing: bool,
    ) -> Result<(), OutputError> {
        self.set_rhythm_pattern(pattern, playing, false)
    }

    /// A pack uses the existing metronome and backing lanes; prepared sample
    /// steps are admitted together, without callback decoding or a second clock.
    pub fn set_pack_pattern_playing(
        &mut self,
        pattern: SequencerPattern,
        playing: bool,
    ) -> Result<(), OutputError> {
        self.set_rhythm_pattern(pattern, playing, true)
    }

    pub(super) fn set_rhythm_pattern(
        &mut self,
        pattern: SequencerPattern,
        playing: bool,
        backing_rhythm: bool,
    ) -> Result<(), OutputError> {
        let bpm = pattern.bpm;
        let data = PreparedClick::prepare(&pattern, self.rate)?;
        self.send(Command::ClickPattern {
            data: Some(OwnedGc::new(data)),
            bpm,
            playing: Some(playing),
            backing_rhythm,
        })?;
        self.metronome = (bpm, playing);
        Ok(())
    }

    pub fn start_click_pattern(&mut self, pattern: SequencerPattern) -> Result<(), OutputError> {
        self.set_click_pattern_playing(pattern, true)
    }

    pub fn play_click(&mut self) -> Result<(), OutputError> {
        self.send(Command::ClickPlaying(true))?;
        self.metronome.1 = true;
        Ok(())
    }

    pub fn stop_click(&mut self) -> Result<(), OutputError> {
        self.send(Command::ClickPlaying(false))?;
        self.metronome.1 = false;
        Ok(())
    }

    pub fn play(&mut self) -> Result<(), OutputError> {
        if self.program_owns_output() {
            return Err(err("stop the prepared program before starting backing"));
        }
        self.cancel_worker();
        self.send(Command::Playing(true))?;
        self.song.playing = true;
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), OutputError> {
        self.cancel_worker();
        self.send(Command::Playing(false))?;
        self.song.playing = false;
        Ok(())
    }

    pub fn rewind(&mut self) -> Result<(), OutputError> {
        if self.selected_capture_requested {
            return Err(err("stop selected capture before rewinding backing"));
        }
        self.cancel_worker();
        self.send(Command::Rewind)
    }

    pub fn set_output_gain(&mut self, gain: f32) -> Result<(), OutputError> {
        if !gain.is_finite() {
            return Err(err("output gain must be finite"));
        }
        self.send(Command::Gain(gain.clamp(0.0, 1.0)))
    }
}
