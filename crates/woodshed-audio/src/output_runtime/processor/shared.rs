use super::*;

impl Shared {
    pub(super) fn publish(&self, snapshot: OutputSnapshot) {
        self.sequence.fetch_add(1, Ordering::AcqRel);
        self.frames.store(snapshot.device_frames, Ordering::Relaxed);
        self.blocks
            .store(snapshot.processing_blocks, Ordering::Relaxed);
        self.bar.store(snapshot.cursor.bar_idx, Ordering::Relaxed);
        self.in_bar
            .store(snapshot.cursor.sample_in_bar, Ordering::Relaxed);
        self.flags.store(
            usize::from(snapshot.playing)
                | usize::from(snapshot.recording) << 1
                | usize::from(snapshot.capture_armed) << 2
                | usize::from(snapshot.capture_invalid) << 3
                | usize::from(snapshot.click_playing) << 4,
            Ordering::Relaxed,
        );
        self.previews
            .store(snapshot.preview_voices, Ordering::Relaxed);
        self.preview_starts
            .store(snapshot.preview_starts, Ordering::Relaxed);
        self.click_step
            .store(snapshot.click_step, Ordering::Relaxed);
        self.program_epoch
            .store(snapshot.program.epoch.0, Ordering::Relaxed);
        self.program_revision
            .store(snapshot.program.revision, Ordering::Relaxed);
        self.program_card_count
            .store(snapshot.program.card_count, Ordering::Relaxed);
        self.program_card
            .store(snapshot.program.card_index, Ordering::Relaxed);
        self.program_token.store(
            snapshot.program.card_token.unwrap_or(u64::MAX),
            Ordering::Relaxed,
        );
        self.program_event
            .store(snapshot.program.event_index, Ordering::Relaxed);
        self.program_event_token.store(
            snapshot.program.event_token.unwrap_or(u64::MAX),
            Ordering::Relaxed,
        );
        self.program_ticks
            .store(snapshot.program.beat_ticks, Ordering::Relaxed);
        self.program_bpm_bits
            .store(snapshot.program.bpm_bits, Ordering::Relaxed);
        self.program_click_subdivision_ticks
            .store(snapshot.program.click_subdivision_ticks, Ordering::Relaxed);
        self.program_frames
            .store(snapshot.program.card_frames, Ordering::Relaxed);
        self.program_active_frames
            .store(snapshot.program.active_frames, Ordering::Relaxed);
        self.program_status
            .store(snapshot.program.status as usize, Ordering::Relaxed);
        self.sequence.fetch_add(1, Ordering::Release);
    }
    pub fn snapshot(&self) -> OutputSnapshot {
        loop {
            let before = self.sequence.load(Ordering::Acquire);
            if before & 1 != 0 {
                std::hint::spin_loop();
                continue;
            }
            let flags = self.flags.load(Ordering::Relaxed);
            let s = OutputSnapshot {
                device_frames: self.frames.load(Ordering::Relaxed),
                processing_blocks: self.blocks.load(Ordering::Relaxed),
                cursor: SongCursor {
                    bar_idx: self.bar.load(Ordering::Relaxed),
                    sample_in_bar: self.in_bar.load(Ordering::Relaxed),
                },
                playing: flags & 1 != 0,
                recording: flags & 2 != 0,
                capture_armed: flags & 4 != 0,
                capture_invalid: flags & 8 != 0,
                click_playing: flags & 16 != 0,
                preview_voices: self.previews.load(Ordering::Relaxed),
                preview_starts: self.preview_starts.load(Ordering::Relaxed),
                click_step: self.click_step.load(Ordering::Relaxed),
                program: ProgramSnapshot {
                    revision: self.program_revision.load(Ordering::Relaxed),
                    card_count: self.program_card_count.load(Ordering::Relaxed),
                    epoch: ProgramEpoch(self.program_epoch.load(Ordering::Relaxed)),
                    device_frame: self.frames.load(Ordering::Relaxed),
                    card_index: self.program_card.load(Ordering::Relaxed),
                    card_token: match self.program_token.load(Ordering::Relaxed) {
                        u64::MAX => None,
                        token => Some(token),
                    },
                    event_index: self.program_event.load(Ordering::Relaxed),
                    event_token: match self.program_event_token.load(Ordering::Relaxed) {
                        u64::MAX => None,
                        token => Some(token),
                    },
                    beat_ticks: self.program_ticks.load(Ordering::Relaxed),
                    bpm_bits: self.program_bpm_bits.load(Ordering::Relaxed),
                    click_subdivision_ticks: self
                        .program_click_subdivision_ticks
                        .load(Ordering::Relaxed),
                    card_frames: self.program_frames.load(Ordering::Relaxed),
                    active_frames: self.program_active_frames.load(Ordering::Relaxed),
                    status: match self.program_status.load(Ordering::Relaxed) {
                        1 => ProgramStatus::Playing,
                        2 => ProgramStatus::Paused,
                        3 => ProgramStatus::WaitingManual,
                        4 => ProgramStatus::Refused,
                        5 => ProgramStatus::Complete,
                        6 => ProgramStatus::WaitingObserver,
                        _ => ProgramStatus::Stopped,
                    },
                },
            };
            std::sync::atomic::fence(Ordering::Acquire);
            if before == self.sequence.load(Ordering::Relaxed) {
                return s;
            }
        }
    }
}
