use super::*;

impl Processor {
    pub(super) fn clear_voices(&mut self) {
        for v in &mut self.song_voices {
            *v = None;
        }
        for v in &mut self.preview_voices {
            *v = None;
        }
    }

    pub(super) fn start_playing(&mut self, playing: bool) {
        self.playing = playing;
        self.clear_voices();
        self.song_last_beat = None;
        self.song_last_measure = None;
        self.count_in_pos = 0;
        self.count_in = if playing {
            self.backing
                .as_ref()
                .filter(|b| b.click)
                .map_or(0, |b| b.bars[self.cursor.bar_idx].lead)
        } else {
            0
        };
    }

    pub(super) fn finish_take(&mut self) {
        if let Some(mut take) = self.take.take() {
            if self
                .feed
                .as_ref()
                .is_some_and(|f| f.overflow_count() != take.overflow)
            {
                take.valid = false;
                self.invalid = true;
            }
            if take.valid && take.generation == self.generation {
                if let Some(backing) = self.backing.as_mut() {
                    let scope = take.scope;
                    for (idx, bar) in take.bars.iter_mut().enumerate() {
                        if bar.touched && scope.includes(idx) {
                            bar.retired = backing.bars[idx].loop_pcm.replace(bar.pcm.clone());
                        }
                    }
                }
            }
            let stamp = CompletionStamp {
                generation: take.generation,
                valid: take.valid,
            };
            self.completed = Some(CompletedTake {
                storage: take,
                stamp,
            });
        }
    }

    pub(super) fn flush_return(&mut self) {
        if let Some(take) = self.completed.take() {
            if let Err(rtrb::PushError::Full(take)) = self.returns.push(take) {
                self.completed = Some(take);
            }
        }
    }

    pub(super) fn stop_capture(&mut self) {
        self.finish_take();
        for op in &mut self.pending {
            if matches!(op, Some(Boundary::Arm { .. })) {
                *op = None;
                self.shared.boundaries.fetch_sub(1, Ordering::Release);
            }
        }
        let mut write = 0;
        for read in 0..self.pending_len {
            if self.pending[read].is_some() {
                self.pending.swap(read, write);
                write += 1;
            }
        }
        self.pending_len = write;
    }

    pub(super) fn push_boundary(&mut self, op: Boundary) {
        if self.pending_len < COMMAND_BOUND {
            self.pending[self.pending_len] = Some(op);
            self.pending_len += 1;
        } else {
            self.invalid = true;
            self.shared.boundaries.fetch_sub(1, Ordering::Release);
        } // Retired resources use deferred GC.
    }

    /// Includes queued selected arms so later controls cannot redirect an
    /// accepted reservation while it waits for its first boundary/count-in.
    pub(super) fn selected_capture_bar(&self) -> Option<usize> {
        self.take
            .as_ref()
            .and_then(|take| take.scope.selected())
            .or_else(|| {
                self.pending[..self.pending_len]
                    .iter()
                    .find_map(|op| match op {
                        Some(Boundary::Arm { take, .. }) => take.scope.selected(),
                        _ => None,
                    })
            })
    }

    pub(super) fn boundary(&mut self) {
        self.song_last_beat = None;
        self.song_last_measure = None;
        let pending_len = self.pending_len;
        let mut retained = 0;
        for idx in 0..pending_len {
            if let Some(op) = self.pending[idx].take() {
                if matches!(&op, Boundary::Arm { seek: false, bar, .. } if self.cursor.bar_idx != *bar)
                {
                    self.pending[retained] = Some(op);
                    retained += 1;
                    continue;
                }
                self.shared.boundaries.fetch_sub(1, Ordering::Release);
                match op {
                    Boundary::Change(PendingChange::SeekTo { bar_idx }) => {
                        if self.selected_capture_bar().is_none()
                            && self
                                .backing
                                .as_ref()
                                .is_some_and(|b| bar_idx < b.bars.len())
                        {
                            self.cursor = SongCursor {
                                bar_idx,
                                sample_in_bar: 0,
                            };
                        }
                    },
                    Boundary::Change(PendingChange::StopRecording) => {
                        self.finish_take();
                        // A boundary stop cancels earlier queued arms as well
                        // as an active take. Later arms retain FIFO semantics.
                        for slot in &mut self.pending[..retained] {
                            if matches!(slot, Some(Boundary::Arm { .. })) {
                                *slot = None;
                                retained -= 1;
                                self.shared.boundaries.fetch_sub(1, Ordering::Release);
                            }
                        }
                    },
                    Boundary::Change(PendingChange::SetPlaying(playing)) => {
                        self.playing = playing;
                        if !playing {
                            self.clear_voices();
                            self.finish_take();
                        }
                    },
                    Boundary::Change(PendingChange::StartRecording { .. }) => {},
                    Boundary::Arm {
                        bar,
                        seek,
                        mut take,
                    } => {
                        if seek {
                            self.cursor = SongCursor {
                                bar_idx: bar,
                                sample_in_bar: 0,
                            };
                        }
                        if self.cursor.bar_idx == bar && self.completed.is_none() {
                            if let Some(feed) = self.feed.as_mut() {
                                take.overflow = feed.overflow_count();
                                self.invalid = false;
                                self.take = Some(take);
                            } else {
                                self.invalid = true;
                            }
                        }
                    },
                }
            }
        }
        self.pending_len = retained;
    }

    pub(super) fn click_trigger(&mut self, accent: bool, song: bool) {
        // Backing always uses the same pre-rendered quarter click as the
        // root metronome's default; calibration patterns affect only its lane.
        let pcm = self.backing_click[usize::from(!accent)].clone();
        if song {
            trigger(&mut self.song_voices, pcm);
        } else {
            trigger(&mut self.click_voices, pcm);
        }
    }

    pub(super) fn frame(&mut self) -> f32 {
        let program_running = self.program_frame_start();
        if self.take.is_none() {
            if let Some(feed) = self.feed.as_mut() {
                let _ = feed.pop_sample();
            }
        }
        if self.click_playing && self.program.is_none() {
            if let Some(click) = self.click.as_ref() {
                let beat_frames =
                    audio_primitives::frames_per_beat(self.rate, self.bpm, click.beat_unit).max(1);
                let absolute_step = (self.click_position / beat_frames as f64) as u64;
                let index = (absolute_step % click.steps.len() as u64) as usize;
                if self.last_click != Some(absolute_step) {
                    if let Some(pcm) = &click.steps[index] {
                        trigger(&mut self.click_voices, pcm.clone());
                    }
                    self.last_click = Some(absolute_step);
                }
                self.click_position += click.divisions as f64;
            }
        }
        let metro = if self.program.is_some() && self.program_status != ProgramStatus::Playing {
            0.0
        } else {
            mix(&mut self.click_voices).clamp(-1.0, 1.0)
        };
        let mut song_sample = 0.0;
        if self.playing && self.backing.is_some() {
            if self.count_in > 0 {
                let beat = self.count_in_pos
                    / self.backing.as_ref().unwrap().bars[self.cursor.bar_idx].beat;
                if self.song_last_beat != Some(beat) {
                    self.click_trigger(beat == 0, true);
                    self.song_last_beat = Some(beat);
                }
                self.count_in -= 1;
                self.count_in_pos += 1;
                if self.count_in == 0 {
                    self.song_last_beat = None;
                }
                song_sample += mix(&mut self.song_voices);
            } else {
                let bar = &self.backing.as_ref().unwrap().bars[self.cursor.bar_idx];
                let beat = self.cursor.sample_in_bar / bar.beat;
                let measure = self.cursor.sample_in_bar / bar.measure;
                if self.backing_rhythm && self.backing.as_ref().unwrap().click {
                    if let Some(rhythm) = &self.click {
                        // Position comes from the admitted backing bar, never
                        // the UI clock. Scale its notated beat to the kit unit.
                        let unit = u128::from(bar.beat)
                            .saturating_mul(u128::from(bar.beat_unit))
                            .max(1);
                        let step = (self.cursor.sample_in_bar as u128
                            * u128::from(rhythm.divisions)
                            * u128::from(rhythm.beat_unit)
                            / unit) as u64;
                        if self.song_last_beat != Some(step) {
                            if let Some(pcm) = &rhythm.steps[step as usize % rhythm.steps.len()] {
                                trigger(&mut self.song_voices, pcm.clone());
                            }
                            self.song_last_beat = Some(step);
                        }
                    }
                } else if self.backing.as_ref().unwrap().click && self.song_last_beat != Some(beat)
                {
                    let accent = beat.is_multiple_of(bar.beats);
                    self.click_trigger(accent, true);
                    self.song_last_beat = Some(beat);
                }
                let bar = &self.backing.as_ref().unwrap().bars[self.cursor.bar_idx];
                if self.song_last_measure != Some(measure) {
                    if let Some(pcm) = &bar.chord {
                        trigger(&mut self.song_voices, pcm.clone());
                    }
                    self.song_last_measure = Some(measure);
                }
                if let Some(pcm) = &bar.phrase_pcm {
                    song_sample += pcm.data[self.cursor.sample_in_bar as usize];
                }
                if let Some(take) = &self.take {
                    let target = take.scope.selected().unwrap_or(self.cursor.bar_idx);
                    let data = &take.bars[target].pcm.data;
                    song_sample += data[self.cursor.sample_in_bar as usize % data.len()];
                } else if let Some(pcm) = &bar.loop_pcm {
                    if !pcm.is_empty() {
                        song_sample += pcm.data[self.cursor.sample_in_bar as usize % pcm.len()];
                    }
                }
                if let Some(take) = self.take.as_mut() {
                    let sample = self.feed.as_mut().and_then(|feed| {
                        audio_primitives::capture::InputFeed::pop_sample(&mut **feed)
                    });
                    if let Some(value) = sample {
                        let target = take.scope.selected().unwrap_or(self.cursor.bar_idx);
                        let bar = &mut take.bars[target];
                        let data = Arc::get_mut(&mut bar.pcm.data)
                            .expect("active recording storage has exclusive ownership");
                        let idx = self.cursor.sample_in_bar as usize % data.len();
                        let mode = if self.replace {
                            audio_primitives::capture::WriteMode::IndexedReplace
                        } else {
                            audio_primitives::capture::WriteMode::IndexedOverdubClamped
                        };
                        let written = audio_primitives::capture::TakeAccumulator::default()
                            .write(data, mode, idx, value);
                        debug_assert!(written);
                        bar.touched = true;
                    } else {
                        take.valid = false;
                        self.invalid = true;
                    }
                    if self
                        .feed
                        .as_ref()
                        .is_some_and(|f| f.overflow_count() != take.overflow)
                    {
                        take.valid = false;
                        self.invalid = true;
                    }
                }
                song_sample += mix(&mut self.song_voices);
                self.cursor.sample_in_bar += 1;
                let backing = self.backing.as_ref().unwrap();
                if self.cursor.sample_in_bar >= backing.bars[self.cursor.bar_idx].duration {
                    self.cursor.sample_in_bar = 0;
                    if let Some(target) = self.selected_capture_bar() {
                        self.cursor.bar_idx = target;
                    } else if self.cursor.bar_idx + 1 == backing.bars.len() {
                        if backing.one_shot {
                            self.playing = false;
                            self.clear_voices();
                            self.invalid |= self.pending[..self.pending_len]
                                .iter()
                                .any(|op| matches!(op, Some(Boundary::Arm { .. })));
                            self.stop_capture();
                            for op in &mut self.pending {
                                if op.take().is_some() {
                                    self.shared.boundaries.fetch_sub(1, Ordering::Release);
                                }
                            }
                            self.pending_len = 0;
                        } else {
                            self.cursor.bar_idx = 0;
                        }
                    } else {
                        self.cursor.bar_idx += 1;
                    }
                    self.boundary();
                }
            }
        } else {
            song_sample += mix(&mut self.preview_voices);
        }
        let program_sample = if program_running {
            self.mix_program_voices()
        } else {
            0.0
        };
        self.program_frame_end();
        (metro + (song_sample * self.gain).clamp(-1.0, 1.0) + program_sample).clamp(-1.0, 1.0)
    }
}
