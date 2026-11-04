use super::*;

impl Processor {
    pub(super) fn trigger_program_events(&mut self) {
        let Some(program) = self.program.as_ref() else {
            return;
        };
        let Some(card) = program.cards.get(self.program_card_index) else {
            return;
        };
        while let Some(event) = card.events.get(self.program_event_index) {
            let onset_reached = match event.onset {
                ProgramPosition::BeatTicks(ticks) => {
                    (self.program_tick_phase_q32 >> 32) as u64 >= ticks
                },
                ProgramPosition::DeviceFrames(frames) => self.program_card_frames >= frames,
            };
            if !onset_reached {
                break;
            }
            let group_base = match self.program_event_group_base {
                Some((onset, base)) if onset == event.onset => base,
                _ => {
                    let base = self.program_card_frames;
                    self.program_event_group_base = Some((event.onset, base));
                    base
                },
            };
            let due_frame = group_base.saturating_add(event.onset_offset_frames);
            self.program_event_due_frame = Some((self.program_event_index, due_frame));
            if self.program_card_frames < due_frame {
                break;
            }
            let tone = program.tones[event.tone].clone();
            let gate_end = match (event.onset, event.gate) {
                (ProgramPosition::BeatTicks(at), ProgramDuration::BeatTicks(len)) => {
                    at.saturating_add(len)
                },
                (ProgramPosition::DeviceFrames(at), ProgramDuration::DeviceFrames(len)) => {
                    at.saturating_add(len)
                },
                (_, ProgramDuration::UntilCardEnd) => u64::MAX,
                _ => u64::MAX,
            };
            let slot_index = self
                .program_voices
                .iter()
                .position(Option::is_none)
                .or_else(|| {
                    // A previous card may still be inside its bounded release
                    // tail. Reclaim its oldest released voice before sacrificing
                    // an onset from the current card.
                    self.program_voices
                        .iter()
                        .enumerate()
                        .filter_map(|(index, voice)| {
                            voice
                                .as_ref()
                                .filter(|voice| voice.released)
                                .map(|voice| (index, voice.release_age))
                        })
                        .max_by_key(|(_, age)| *age)
                        .map(|(index, _)| index)
                });
            let Some(slot_index) = slot_index else {
                // Validation bounds same-card overlap. If a malformed or
                // otherwise unexpected schedule exhausts the fixed pool,
                // leave the event pending for a later frame instead of
                // silently consuming it.
                break;
            };
            self.program_voices[slot_index] = Some(ProgramVoice {
                tone,
                position: 0,
                phase_q64: 0,
                age: 0,
                gate: event.gate,
                gate_end: match event.gate {
                    ProgramDuration::BeatTicks(length) => {
                        ((self.program_tick_phase_q32 >> 32) as u64).saturating_add(length)
                    },
                    ProgramDuration::DeviceFrames(length) => {
                        self.program_card_frames.saturating_add(length)
                    },
                    ProgramDuration::UntilCardEnd => gate_end,
                },
                amplitude: event.amplitude,
                released: false,
                release_age: 0,
                card_index: self.program_card_index,
            });
            self.program_event_token = Some(event.token);
            self.program_event_index += 1;
            self.program_event_due_frame = None;
        }
    }

    pub(super) fn program_frame_start(&mut self) -> bool {
        if !self.flush_program_transition() {
            return false;
        }
        if self.program_advance {
            self.program_advance = false;
            let was_paused = self.program_status == ProgramStatus::Paused;
            if self.program_card_started {
                let _ = self.program_transition(
                    ProgramTransitionKind::Ended(ProgramEndReason::Completed),
                    ProgramPendingAction::AdvanceCard,
                );
                if self.program_status == ProgramStatus::WaitingObserver {
                    return false;
                }
            } else {
                self.advance_program_card();
            }
            if was_paused
                && !matches!(
                    self.program_status,
                    ProgramStatus::Stopped | ProgramStatus::Complete
                )
            {
                self.program_resume_after_pause = self.program_status;
                self.program_status = ProgramStatus::Paused;
            }
        }
        if self.program_status != ProgramStatus::Playing || self.program.is_none() {
            return false;
        }
        if !self.program_card_started {
            self.enter_program_card();
            if !self.program_card_started {
                return false;
            }
        }
        self.trigger_program_events();
        if self.click_playing {
            if let Some(click) = self.click.as_ref() {
                let step = if self.backing_rhythm {
                    // Program ticks count quarter notes. Keep the kit's meter
                    // and subdivisions rational, including non-divisors of 960.
                    (self.program_tick_phase_q32
                        * u128::from(click.beat_unit)
                        * u128::from(click.divisions)
                        / (u128::from(super::super::clock::PROGRAM_TICKS_PER_BEAT) * 4 << 32))
                        as u64
                } else {
                    let ticks_per_step = (super::super::clock::PROGRAM_TICKS_PER_BEAT
                        / click.divisions.max(1) as u64)
                        .max(1);
                    ((self.program_tick_phase_q32 >> 32) as u64) / ticks_per_step
                };
                if self.program_click_last_step != Some(step) {
                    let index = (step % click.steps.len() as u64) as usize;
                    if let Some(pcm) = &click.steps[index] {
                        trigger(&mut self.click_voices, pcm.clone());
                    }
                    self.program_click_last_step = Some(step);
                }
            }
        }
        true
    }

    pub(super) fn mix_program_voices(&mut self) -> f32 {
        if self.program_status != ProgramStatus::Playing {
            return 0.0;
        }
        let beat_ticks = (self.program_tick_phase_q32 >> 32) as u64;
        let attack = (self.rate as f64 * 0.015).round().max(1.0) as u64;
        let release = (self.rate as f64 * 0.250).round().max(1.0) as u64;
        let current_card = self
            .program
            .as_ref()
            .and_then(|program| program.cards.get(self.program_card_index));
        let current_card_end = current_card.map(|card| card.end);
        let current_card_bpm = current_card.map_or(120.0, |card| card.bpm);
        let mut sum = 0.0;
        for slot in &mut self.program_voices {
            let Some(voice) = slot else {
                continue;
            };
            let expired = voice.card_index != self.program_card_index
                || match voice.gate {
                    ProgramDuration::BeatTicks(_) => beat_ticks >= voice.gate_end,
                    ProgramDuration::DeviceFrames(_) => self.program_card_frames >= voice.gate_end,
                    ProgramDuration::UntilCardEnd => match current_card_end {
                        Some(ProgramEnd::BeatTicks(end)) => {
                            let release_ticks = (current_card_bpm
                                * 0.250
                                * super::super::clock::PROGRAM_TICKS_PER_BEAT as f64
                                / 60.0)
                                .ceil() as u64;
                            beat_ticks >= end.saturating_sub(release_ticks)
                        },
                        Some(ProgramEnd::DeviceFrames(end)) => {
                            self.program_card_frames >= end.saturating_sub(release)
                        },
                        Some(ProgramEnd::Manual { tail_frames, .. }) => {
                            self.program_tail_started
                                && self.program_tail_remaining <= release.min(tail_frames)
                        },
                        _ => false,
                    },
                };
            if expired && !voice.released {
                voice.released = true;
                voice.release_age = 0;
            }
            if voice.released && voice.release_age >= release {
                *slot = None;
                continue;
            }
            let sample = if let Some(step) = voice.tone.phase_step_q64 {
                const PHASE_FRACTION_MASK: u64 = (1u64 << FALLBACK_PHASE_FRACTION_BITS) - 1;
                let index = (voice.phase_q64 >> FALLBACK_PHASE_FRACTION_BITS) as usize;
                let next = (index + 1) % voice.tone.pcm.len();
                let fraction = (voice.phase_q64 & PHASE_FRACTION_MASK) as f32
                    / (1u64 << FALLBACK_PHASE_FRACTION_BITS) as f32;
                let first = voice.tone.pcm.data[index];
                let second = voice.tone.pcm.data[next];
                voice.phase_q64 = voice.phase_q64.wrapping_add(step);
                first + (second - first) * fraction
            } else {
                let sample = voice.tone.pcm.data[voice.position];
                voice.position += 1;
                if voice.position == voice.tone.pcm.len() {
                    voice.position = 0;
                }
                sample
            };
            let attack_gain = (voice.age as f32 / attack as f32).min(1.0);
            let release_gain = if voice.released {
                (1.0 - voice.release_age as f32 / release as f32).max(0.0)
            } else {
                1.0
            };
            sum += sample * voice.amplitude * attack_gain * release_gain;
            voice.age = voice.age.saturating_add(1);
            if voice.released {
                voice.release_age = voice.release_age.saturating_add(1);
            }
        }
        sum
    }

    pub(super) fn program_frame_end(&mut self) {
        if self.program_status == ProgramStatus::WaitingManual {
            self.program_active_frames = self.program_active_frames.saturating_add(1);
            return;
        }
        if self.program_status != ProgramStatus::Playing || !self.program_card_started {
            return;
        }
        let Some(end) = self
            .program
            .as_ref()
            .and_then(|program| program.cards.get(self.program_card_index))
            .map(|card| card.end)
        else {
            return;
        };
        self.program_active_frames = self.program_active_frames.saturating_add(1);
        self.program_card_frames = self.program_card_frames.saturating_add(1);
        self.program_tick_phase_q32 = self
            .program_tick_phase_q32
            .saturating_add(u128::from(self.program_tick_step_q32));
        let completed = match end {
            ProgramEnd::BeatTicks(ticks) => {
                (self.program_tick_phase_q32 >> 32) >= u128::from(ticks)
            },
            ProgramEnd::DeviceFrames(frames) => self.program_card_frames >= frames,
            ProgramEnd::Manual {
                preview_ticks,
                tail_frames,
            } => {
                if !self.program_tail_started
                    && (self.program_tick_phase_q32 >> 32) >= u128::from(preview_ticks)
                {
                    self.program_tail_started = true;
                    self.program_tail_remaining = tail_frames;
                    false
                } else if self.program_tail_started {
                    self.program_tail_remaining = self.program_tail_remaining.saturating_sub(1);
                    self.program_tail_remaining == 0
                } else {
                    false
                }
            },
            ProgramEnd::Refusal(_) => false,
        };
        if completed {
            for voice in self.program_voices.iter_mut().flatten() {
                if !voice.released {
                    voice.released = true;
                    voice.release_age = 0;
                }
            }
            if matches!(end, ProgramEnd::Manual { .. }) {
                for voice in &mut self.program_voices {
                    *voice = None;
                }
                self.program_manual_body_done = true;
                self.program_status = ProgramStatus::WaitingManual;
            } else {
                let _ = self.program_transition(
                    ProgramTransitionKind::Ended(ProgramEndReason::Completed),
                    ProgramPendingAction::NextCard,
                );
            }
        }
    }

    pub(super) fn program_snapshot(&self) -> ProgramSnapshot {
        let current_card = self
            .program
            .as_ref()
            .and_then(|program| program.cards.get(self.program_card_index));
        let card_token = current_card.map(|card| card.token);
        let click_subdivision_ticks = self.click.as_ref().map_or(0, |click| {
            (super::super::clock::PROGRAM_TICKS_PER_BEAT * 4
                / (if self.backing_rhythm {
                    u64::from(click.beat_unit)
                } else {
                    4
                } * u64::from(click.divisions.max(1))))
            .max(1)
        });
        ProgramSnapshot {
            revision: self.program_revision,
            card_count: self
                .program
                .as_ref()
                .map_or(0, |program| program.cards.len()),
            epoch: self.program_epoch,
            device_frame: self.current_frame,
            card_index: self.program_card_index,
            card_token,
            event_index: self.program_event_index,
            event_token: self.program_event_token,
            beat_ticks: (self.program_tick_phase_q32 >> 32).min(u128::from(u64::MAX)) as u64,
            bpm_bits: current_card.map_or(0, |card| card.bpm.to_bits()),
            click_subdivision_ticks,
            card_frames: self.program_card_frames,
            active_frames: self.program_active_frames,
            status: self.program_status,
        }
    }
}
