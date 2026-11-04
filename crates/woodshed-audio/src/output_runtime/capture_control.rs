use super::*;

impl OutputRuntime {
    pub fn attach_capture_handle(
        &mut self,
        handle: &crate::LooperCaptureHandle,
    ) -> Result<(), OutputError> {
        if self.program_owns_output() {
            return Err(err("stop the prepared program before attaching capture"));
        }
        if self.capture_busy() {
            return Err(err("cannot replace a busy input feed"));
        }
        if handle.source_rate() == 0 || handle.source_rate() != self.rate {
            return Err(err("capture requires equal nonzero input/output rates"));
        }
        if self.native && self.stream.is_none() {
            return Err(err("output stream is inactive"));
        }
        self.shared
            .commands
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                (n < COMMAND_BOUND).then_some(n + 1)
            })
            .map_err(|_| err("output command queue is full"))?;
        let Some(feed) = handle.take_consumer() else {
            self.shared.commands.fetch_sub(1, Ordering::Release);
            return Err(err("input consumer is already attached"));
        };
        self.cx.queue_event_for(
            self.node,
            NodeEventType::custom(Command::Capture(Some(OwnedGc::new(feed)))),
        );
        self.capture_attached = true;
        Ok(())
    }

    /// Read one processor publication plus the control-side reservation. A UI
    /// projection must not combine different blocks' capture phase and cursor.
    pub fn capture_observation(&self) -> (OutputSnapshot, bool) {
        let snapshot = self.snapshot();
        let busy = self.capture_requested || snapshot.recording || snapshot.capture_armed;
        (snapshot, busy)
    }

    pub fn capture_busy(&self) -> bool {
        self.capture_observation().1
    }

    pub fn clear_recording(&mut self, bar_idx: usize) -> Result<(), OutputError> {
        if self.capture_busy() {
            return Err(err("cannot clear a busy capture"));
        }
        let mut song = self.song.clone();
        song.bars
            .get_mut(bar_idx)
            .ok_or_else(|| err("recording bar out of range"))?
            .audio_buffer = None;
        song.bars[bar_idx].legacy_clock = false;
        let data = PreparedBacking::prepare(&song, self.rate, &mut self.chord_cache)?;
        self.send(Command::Backing {
            data: Some(OwnedGc::new(data)),
            reset: false,
            clear: false,
            playing: self.song().playing,
            generation: self.shared.generation.load(Ordering::Acquire),
        })?;
        self.song = song;
        Ok(())
    }

    pub fn set_record_replace(&mut self, replace: bool) -> Result<(), OutputError> {
        self.send(Command::RecordReplace(replace))?;
        self.song.record_replace = replace;
        Ok(())
    }

    /// Existing song capture follows backing traversal after its initial arm.
    pub fn arm_capture(&mut self, bar_idx: usize, seek: bool) -> Result<(), OutputError> {
        self.arm_capture_scoped(bar_idx, seek, RecordingScope::Follow)
    }

    /// Seek at the admitted boundary, then repeat and record only this segment
    /// until stopped. The saved-song follow API and its seek flag are unchanged.
    pub fn arm_selected_capture(&mut self, bar_idx: usize) -> Result<(), OutputError> {
        self.arm_capture_scoped(bar_idx, true, RecordingScope::Selected(bar_idx))
    }

    pub(super) fn arm_capture_scoped(
        &mut self,
        bar_idx: usize,
        seek: bool,
        scope: RecordingScope,
    ) -> Result<(), OutputError> {
        if self.program_owns_output() {
            return Err(err("stop the prepared program before starting capture"));
        }
        if !self.capture_attached {
            return Err(err("recording input is not attached"));
        }
        if self.capture_busy() {
            return Err(err("capture already armed or active"));
        }
        if bar_idx >= self.song.bars.len() {
            return Err(err("recording bar out of range"));
        }
        if self.song.bars.iter().any(|bar| bar.legacy_clock) {
            return Err(err(
                "Clear the legacy-clock loops before recording. Existing takes keep their original playback clocks.",
            ));
        }
        let mut take = Recording::prepare(
            &self.song,
            self.rate,
            self.shared.generation.load(Ordering::Acquire),
        )?;
        take.scope = scope;
        self.send_boundary(Command::Arm {
            bar_idx,
            seek,
            take: Some(OwnedGc::new(take)),
        })?;
        self.capture_requested = true;
        self.selected_capture_requested = matches!(scope, RecordingScope::Selected(_));
        self.song
            .pending
            .push(PendingChange::StartRecording { bar_idx });
        Ok(())
    }

    pub fn stop_capture_now(&mut self) -> Result<(), OutputError> {
        self.send(Command::StopCapture)
    }
}
