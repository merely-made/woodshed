use super::*;

impl AudioNode for OutputNode {
    type Configuration = EmptyConfig;
    fn info(&self, _: &EmptyConfig) -> Result<AudioNodeInfo, NodeError> {
        Ok(AudioNodeInfo::new()
            .debug_name("woodshed_output")
            .channel_config(ChannelConfig {
                num_inputs: ChannelCount::ZERO,
                num_outputs: ChannelCount::MONO,
            }))
    }
    fn construct_processor(
        &self,
        _: &EmptyConfig,
        cx: ConstructProcessorContext,
    ) -> Result<impl AudioNodeProcessor, NodeError> {
        Ok(Processor {
            shared: self.shared.clone(),
            returns: self
                .returns
                .borrow_mut()
                .take()
                .expect("output processor constructed once"),
            program_transitions: self
                .program_transitions
                .borrow_mut()
                .take()
                .expect("program transition processor constructed once"),
            backing: None,
            click: None,
            backing_rhythm: false,
            feed: None,
            take: None,
            completed: None,
            generation: 0,
            cursor: Default::default(),
            playing: false,
            count_in: 0,
            count_in_pos: 0,
            backing_click: default_click(cx.stream_info.sample_rate.get()),
            gain: 0.8,
            replace: false,
            bpm: 120.0,
            click_playing: false,
            click_position: 0.0,
            last_click: None,
            song_last_beat: None,
            song_last_measure: None,
            click_voices: std::array::from_fn(|_| None),
            song_voices: std::array::from_fn(|_| None),
            preview_voices: std::array::from_fn(|_| None),
            preview_starts: 0,
            pending: std::array::from_fn(|_| None),
            pending_len: 0,
            rate: cx.stream_info.sample_rate.get(),
            blocks: 0,
            handled_commands: 0,
            invalid: false,
            program: None,
            program_replacement: None,
            program_revision: 0,
            program_requested_revision: 0,
            program_status: ProgramStatus::Stopped,
            program_epoch: ProgramEpoch::default(),
            program_card_index: 0,
            program_event_index: 0,
            program_tick_phase_q32: 0,
            program_card_frames: 0,
            program_tail_remaining: 0,
            program_advance: false,
            program_transition_pending: None,
            program_stop_pending: false,
            program_resume_after_observer: ProgramStatus::Playing,
            program_active_frames: 0,
            program_click_last_beat: None,
            program_voices: std::array::from_fn(|_| None),
            program_event_token: None,
            program_card_started: false,
            program_pending_action: ProgramPendingAction::None,
            program_deferred_controls: std::array::from_fn(|_| None),
            program_deferred_len: 0,
            current_frame: 0,
            program_tail_started: false,
            program_click_last_step: None,
            program_tick_step_q32: 0,
            program_event_due_frame: None,
            program_event_group_base: None,
            program_manual_body_done: false,
            program_resume_after_pause: ProgramStatus::Playing,
            program_loop_mode: ProgramLoop::Once,
        })
    }
}

impl AudioNodeProcessor for Processor {
    fn events(&mut self, _: &ProcInfo, events: &mut ProcEvents, _: &mut ProcExtra) {
        for mut event in events.drain() {
            if let Some(command) = event.downcast_mut::<Command>() {
                match command {
                    Command::Backing {
                        data,
                        reset,
                        clear,
                        playing,
                        generation,
                    } => {
                        if *clear {
                            self.program_replacement = None;
                            if self.program.is_some() {
                                // Session replacement silences the old
                                // program at this callback boundary. If an
                                // earlier transition awaits observation,
                                // retain a dedicated stop request so it
                                // cannot be displaced by ordinary controls.
                                for voice in &mut self.program_voices {
                                    *voice = None;
                                }
                                for voice in &mut self.click_voices {
                                    *voice = None;
                                }
                                if self.program_transition_pending.is_some() {
                                    self.program_stop_pending = true;
                                } else {
                                    self.program_deferred_controls = std::array::from_fn(|_| None);
                                    self.program_deferred_len = 0;
                                    self.apply_program_control(DeferredProgramControl::Stop);
                                }
                            }
                            if let Some(take) = self.take.as_mut() {
                                take.valid = false;
                            }
                            self.stop_capture();
                            for op in &mut self.pending {
                                if op.take().is_some() {
                                    self.shared.boundaries.fetch_sub(1, Ordering::Release);
                                }
                            }
                            self.pending_len = 0;
                            self.invalid = false;
                        }
                        std::mem::swap(data, &mut self.backing);
                        self.generation = *generation;
                        self.replace = self.backing.as_ref().is_some_and(|b| b.replace);
                        if *reset {
                            self.cursor = Default::default();
                            self.start_playing(*playing);
                        }
                    },
                    Command::ClickPattern {
                        data,
                        bpm,
                        playing,
                        backing_rhythm,
                    } => {
                        self.backing_rhythm = *backing_rhythm;
                        self.song_last_beat = None;
                        self.program_click_last_step = None;
                        std::mem::swap(data, &mut self.click);
                        self.bpm = *bpm;
                        if let Some(playing) = playing {
                            self.click_playing = *playing;
                        }
                        self.click_position = 0.0;
                        self.last_click = None;
                        for v in &mut self.click_voices {
                            *v = None;
                        }
                    },
                    Command::Metronome { bpm, playing } => {
                        let unit = self.click.as_ref().map_or(4, |click| click.beat_unit);
                        let before =
                            audio_primitives::frames_per_beat(self.rate, self.bpm, unit).max(1);
                        let after = audio_primitives::frames_per_beat(self.rate, *bpm, unit).max(1);
                        self.click_position = self.click_position / before as f64 * after as f64;
                        self.bpm = *bpm;
                        if *playing != self.click_playing {
                            for voice in &mut self.preview_voices {
                                *voice = None;
                            }
                            self.click_playing = *playing;
                            self.click_position = 0.0;
                            self.last_click = None;
                            if !*playing {
                                for v in &mut self.click_voices {
                                    *v = None;
                                }
                            }
                        }
                    },
                    Command::ClickPlaying(playing) => {
                        self.click_playing = *playing;
                        self.click_position = 0.0;
                        self.last_click = None;
                        if !*playing {
                            for v in &mut self.click_voices {
                                *v = None;
                            }
                        }
                    },
                    Command::Playing(playing) => {
                        self.start_playing(*playing);
                        if !*playing {
                            self.finish_take();
                        }
                    },
                    Command::Rewind => {
                        // A stale queued rewind cannot redirect a selected take.
                        if self.selected_capture_bar().is_some() {
                            self.handled_commands += 1;
                            continue;
                        }
                        self.cursor = Default::default();
                        self.count_in = 0;
                        self.clear_voices();
                        self.song_last_beat = None;
                        self.song_last_measure = None;
                    },
                    Command::Gain(gain) => self.gain = *gain,
                    Command::RecordReplace(replace) => self.replace = *replace,
                    Command::Preview { revision, buffer } => {
                        if !self.playing
                            && *revision == self.shared.preview_revision.load(Ordering::Acquire)
                        {
                            if let Some(pcm) = buffer.take() {
                                trigger(&mut self.preview_voices, pcm);
                                self.preview_starts = self.preview_starts.saturating_add(1);
                            }
                        }
                    },
                    Command::CancelPreview => {
                        for v in &mut self.preview_voices {
                            *v = None;
                        }
                    },
                    Command::Capture(feed) => std::mem::swap(feed, &mut self.feed),
                    Command::Arm {
                        bar_idx,
                        seek,
                        take,
                    } => {
                        if let Some(take) = take.take() {
                            // This is a new accepted attempt, not the prior
                            // invalidated take's waiting phase. Queue refusal
                            // below still publishes its own invalid result.
                            self.invalid = false;
                            self.push_boundary(Boundary::Arm {
                                bar: *bar_idx,
                                seek: *seek,
                                take,
                            });
                        }
                    },
                    Command::Boundary(change) => {
                        if matches!(
                            change,
                            PendingChange::SeekTo { .. } | PendingChange::SetPlaying(_)
                        ) {
                            for voice in &mut self.preview_voices {
                                *voice = None;
                            }
                        }
                        self.push_boundary(Boundary::Change(change.clone()));
                    },
                    Command::StopCapture => self.stop_capture(),
                    Command::ProgramStart { data, epoch } => {
                        if !self.playing && self.take.is_none() {
                            self.program_replacement = None;
                            self.program_revision = 0;
                            self.program_requested_revision = 0;
                            self.program = data.take();
                            self.program_epoch = *epoch;
                            self.program_loop_mode = self
                                .program
                                .as_ref()
                                .map_or(ProgramLoop::Once, |program| program.loop_mode);
                            self.program_card_index = self
                                .program
                                .as_ref()
                                .map_or(0, |program| program.start_index);
                            self.program_event_index = 0;
                            self.program_tick_phase_q32 = 0;
                            self.program_card_frames = 0;
                            self.program_tail_remaining = 0;
                            self.program_active_frames = 0;
                            self.program_status = ProgramStatus::Playing;
                            self.program_card_started = false;
                            self.program_advance = false;
                            self.program_transition_pending = None;
                            self.program_stop_pending = false;
                            self.program_pending_action = ProgramPendingAction::None;
                            self.program_deferred_controls = std::array::from_fn(|_| None);
                            self.program_deferred_len = 0;
                            self.program_event_token = None;
                            self.program_tail_started = false;
                            self.program_click_last_step = None;
                            self.program_event_due_frame = None;
                            self.program_click_last_beat = None;
                        }
                    },
                    Command::ProgramReplace {
                        epoch,
                        revision,
                        data,
                    } if *epoch == self.program_epoch
                        && *revision > self.program_requested_revision
                        && self.program.is_some()
                        && !matches!(
                            self.program_status,
                            ProgramStatus::Stopped | ProgramStatus::Complete
                        )
                        && !self.program_stop_pending =>
                    {
                        if let Some(replacement) = data.take() {
                            self.program_requested_revision = *revision;
                            self.program_replacement = Some((*revision, replacement));
                        }
                    },
                    Command::ProgramReplace { .. } => {},
                    Command::ProgramPause { epoch, paused } if *epoch == self.program_epoch => {
                        if self.program_status == ProgramStatus::WaitingObserver {
                            if *paused {
                                if self.program_resume_after_observer != ProgramStatus::Paused {
                                    self.program_resume_after_pause =
                                        self.program_resume_after_observer;
                                }
                                self.program_resume_after_observer = ProgramStatus::Paused;
                            } else if self.program_resume_after_observer == ProgramStatus::Paused {
                                self.program_resume_after_observer =
                                    self.program_resume_after_pause;
                            }
                        } else if *paused
                            && self.program_status != ProgramStatus::Paused
                            && self.program_status != ProgramStatus::Stopped
                            && self.program_status != ProgramStatus::Complete
                        {
                            self.program_resume_after_pause = self.program_status;
                            self.program_status = ProgramStatus::Paused;
                        } else if !*paused && self.program_status == ProgramStatus::Paused {
                            self.program_status = self.program_resume_after_pause;
                        }
                    },
                    Command::ProgramPause { .. } => {},
                    Command::ProgramTempo { epoch, card, bpm } if *epoch == self.program_epoch => {
                        if let Some(program) = self.program.as_mut() {
                            if let Some(target) = program.cards.get_mut(*card) {
                                let step = *bpm
                                    * super::super::clock::PROGRAM_TICKS_PER_BEAT as f64
                                    * (u32::MAX as f64 + 1.0)
                                    / (60.0 * self.rate as f64);
                                if step.is_finite() && step >= 1.0 && step <= u64::MAX as f64 {
                                    target.bpm = *bpm;
                                    if *card == self.program_card_index {
                                        self.program_tick_step_q32 = step.round() as u64;
                                    }
                                }
                            }
                        }
                        if let Some((_, pending)) = self.program_replacement.as_mut() {
                            if let Some(target) = pending.cards.get_mut(*card) {
                                target.bpm = *bpm;
                            }
                        }
                    },
                    Command::ProgramTempo { .. } => {},
                    Command::ProgramTempoAll { epoch, bpm } if *epoch == self.program_epoch => {
                        let step = *bpm
                            * super::super::clock::PROGRAM_TICKS_PER_BEAT as f64
                            * (u32::MAX as f64 + 1.0)
                            / (60.0 * self.rate as f64);
                        if step.is_finite() && step >= 1.0 && step <= u64::MAX as f64 {
                            if let Some(program) = self.program.as_mut() {
                                for card in &mut program.cards {
                                    card.bpm = *bpm;
                                }
                                self.program_tick_step_q32 = step.round() as u64;
                            }
                            if let Some((_, pending)) = self.program_replacement.as_mut() {
                                for card in &mut pending.cards {
                                    card.bpm = *bpm;
                                }
                            }
                        }
                    },
                    Command::ProgramTempoAll { .. } => {},
                    Command::ProgramLoopMode { epoch, mode } if *epoch == self.program_epoch => {
                        self.program_loop_mode = *mode;
                    },
                    Command::ProgramLoopMode { .. } => {},
                    Command::ProgramAdvance { epoch } if *epoch == self.program_epoch => {
                        self.program_replacement = None;
                        if matches!(
                            self.program_status,
                            ProgramStatus::Playing
                                | ProgramStatus::Paused
                                | ProgramStatus::WaitingManual
                                | ProgramStatus::Refused
                        ) {
                            self.program_advance = true;
                        }
                    },
                    Command::ProgramAdvance { .. } => {},
                    Command::ProgramSeek { epoch, card } if *epoch == self.program_epoch => {
                        self.defer_program_control(DeferredProgramControl::Seek(*card));
                    },
                    Command::ProgramSeek { .. } => {},
                    Command::ProgramStop { epoch } if *epoch == self.program_epoch => {
                        self.defer_program_control(DeferredProgramControl::Stop);
                    },
                    Command::ProgramStop { .. } => {},
                }
                self.handled_commands += 1;
            }
        }
    }
    fn process(
        &mut self,
        info: &ProcInfo,
        buffers: ProcBuffers,
        _: &mut ProcExtra,
    ) -> ProcessStatus {
        self.flush_return();
        // Waiting/count-in input is not part of a take. It must not fill the
        // bounded feed and poison the take that begins at a future boundary.
        if self.take.is_none() && !self.playing {
            if let Some(feed) = self.feed.as_mut() {
                feed.discard_pending();
            }
        }
        let first_frame = info.clock_samples.0.max(0) as u64;
        for (offset, sample) in buffers.outputs[0][..info.frames].iter_mut().enumerate() {
            self.current_frame = first_frame.saturating_add(offset as u64);
            *sample = self.frame();
        }
        self.current_frame = first_frame.saturating_add(info.frames as u64);
        self.flush_return();
        self.blocks += 1;
        let program = self.program_snapshot();
        let click_step = if self.program.is_some()
            && matches!(
                self.program_status,
                ProgramStatus::Playing
                    | ProgramStatus::Paused
                    | ProgramStatus::WaitingManual
                    | ProgramStatus::WaitingObserver
                    | ProgramStatus::Refused
            ) {
            self.click.as_ref().map_or(0, |click| {
                let step = if self.backing_rhythm {
                    (self.program_tick_phase_q32
                        * u128::from(click.beat_unit)
                        * u128::from(click.divisions)
                        / (u128::from(super::super::clock::PROGRAM_TICKS_PER_BEAT) * 4 << 32))
                        as u64
                } else {
                    program.beat_ticks
                        / (super::super::clock::PROGRAM_TICKS_PER_BEAT
                            / u64::from(click.divisions.max(1)))
                        .max(1)
                };
                (step % click.steps.len().max(1) as u64) as usize
            })
        } else {
            self.click.as_ref().map_or(0, |c| {
                (self.last_click.unwrap_or(0) % c.steps.len() as u64) as usize
            })
        };
        self.shared.publish(OutputSnapshot {
            device_frames: info.clock_samples.0.max(0) as u64 + info.frames as u64,
            processing_blocks: self.blocks,
            cursor: self.cursor,
            playing: self.playing,
            recording: self.take.is_some(),
            capture_armed: self
                .pending
                .iter()
                .any(|op| matches!(op, Some(Boundary::Arm { .. }))),
            capture_invalid: self.invalid,
            preview_voices: self.preview_voices.iter().filter(|v| v.is_some()).count(),
            preview_starts: self.preview_starts,
            click_playing: self.click_playing,
            click_step,
            program,
        });
        if self.handled_commands != 0 {
            self.shared
                .commands
                .fetch_sub(self.handled_commands, Ordering::Release);
            self.handled_commands = 0;
        }
        ProcessStatus::OutputsModified
    }
}

impl OutputNode {
    pub fn new(
        shared: Arc<Shared>,
        returns: rtrb::Producer<CompletedRecording>,
        program_transitions: rtrb::Producer<ProgramTransition>,
    ) -> Self {
        Self {
            shared,
            returns: RefCell::new(Some(returns)),
            program_transitions: RefCell::new(Some(program_transitions)),
        }
    }
}
