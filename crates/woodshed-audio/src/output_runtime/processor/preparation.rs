use super::*;

impl PreparedBacking {
    pub fn prepare(
        song: &Song,
        rate: u32,
        cache: &mut Vec<ChordCache>,
    ) -> Result<Self, OutputError> {
        if song.bars.is_empty() || song.bars.len() > MAX_BARS {
            return Err(err("backing needs 1..=256 segments"));
        }
        let mut resource_frames = 0usize;
        let mut render_work = 0u64;
        for bar in &song.bars {
            if !bar.bpm.is_finite() || bar.bpm <= 0.0 {
                return Err(err("backing tempo must be finite and positive"));
            }
            let duration = usize::try_from(bar.checked_duration_frames(rate).map_err(err)?)
                .map_err(|_| err("backing duration is too large"))?;
            if bar.chord_ref.is_some() && bar.backing_phrase.is_some() {
                return Err(err("backing chord and phrase are mutually exclusive"));
            }
            if let Some(phrase) = &bar.backing_phrase {
                let beats = u64::from(bar.time_signature.numerator.max(1))
                    .saturating_mul(u64::from(bar.length.max(1)));
                let layout = crate::backing_phrase::preflight(
                    phrase,
                    duration,
                    bar.beat_frames(rate),
                    beats,
                    rate,
                )
                .map_err(err)?;
                render_work = render_work
                    .checked_add(layout.work_frames)
                    .ok_or_else(|| err("aggregate phrase render work overflow"))?;
                if render_work > crate::backing_phrase::MAX_RENDER_WORK {
                    return Err(err("aggregate backing phrase exceeds render-work bound"));
                }
            }
            let chord_len = if bar.chord_ref.is_some() {
                bar.chord_measure_secs().max(0.5).clamp(0.3, 2.0) * rate as f32
            } else {
                0.0
            };
            resource_frames = resource_frames
                .checked_add(duration)
                .and_then(|n| n.checked_add(chord_len.ceil() as usize))
                .and_then(|n| {
                    n.checked_add(if bar.backing_phrase.is_some() {
                        duration
                    } else {
                        0
                    })
                })
                .ok_or_else(|| err("backing PCM exceeds resource bound"))?;
            if resource_frames > MAX_PCM_FRAMES {
                return Err(err("aggregate backing PCM exceeds 16777216 frames"));
            }
        }
        let mut next_cache = Vec::new();
        let mut total = 0usize;
        let mut bars = Vec::with_capacity(song.bars.len());
        for bar in &song.bars {
            if !bar.bpm.is_finite() || bar.bpm <= 0.0 {
                return Err(err("backing tempo must be finite and positive"));
            }
            let duration = bar.checked_duration_frames(rate).map_err(err)?;
            let beats = u64::from(bar.time_signature.numerator.max(1));
            let phrase_beats = beats.saturating_mul(u64::from(bar.length.max(1)));
            let beat = bar.beat_frames(rate);
            let measure = bar.measure_frames(rate);
            total = total
                .checked_add(
                    usize::try_from(duration).map_err(|_| err("backing duration is too large"))?,
                )
                .ok_or_else(|| err("backing duration is too large"))?;
            if total > MAX_PCM_FRAMES {
                return Err(err("backing exceeds 16777216 prepared frames"));
            }
            if let Some(pcm) = &bar.audio_buffer {
                if pcm.sample_rate_hz != rate {
                    return Err(err("backing PCM rate differs from output rate"));
                }
                if pcm.len() != duration as usize {
                    return Err(err("backing PCM must fit its segment duration"));
                }
                if !pcm.has_finite_samples() {
                    return Err(err("backing PCM contains nonfinite samples"));
                }
            }
            let chord = if let Some(chord) = &bar.chord_ref {
                if chord.pitches_hz.len() > 256 {
                    return Err(err("backing chord exceeds 256 pitches"));
                }
                let pitches = if chord.pitches_hz.is_empty() {
                    vec![chord.root_freq_hz]
                } else {
                    chord.pitches_hz.clone()
                };
                if pitches.iter().any(|f| !f.is_finite() || *f <= 0.0) {
                    return Err(err("backing chord pitches must be finite and positive"));
                }
                // The existing backing strikes once per measure, even for long blocks.
                let duration_seconds = bar.chord_measure_secs();
                let duration_seconds = duration_seconds.max(0.5).clamp(0.3, 2.0);
                if !crate::chord_audio::chord_frequencies_render_finite(
                    &pitches,
                    f64::from(duration_seconds),
                ) {
                    return Err(err("backing chord frequencies cannot render finite PCM"));
                }
                let bits: Vec<_> = pitches.iter().map(|f| f.to_bits()).collect();
                let pcm = if let Some(hit) = cache.iter().find(|c| {
                    c.pitches == bits && c.duration == duration_seconds.to_bits() && c.rate == rate
                }) {
                    hit.pcm.clone()
                } else {
                    ArcGc::new(render_chord(
                        &ChordRender::strum(pitches, duration_seconds),
                        rate,
                    ))
                };
                if !pcm.has_finite_samples() {
                    return Err(err("prepared chord contains nonfinite PCM"));
                }
                next_cache.push(ChordCache {
                    pitches: bits,
                    duration: duration_seconds.to_bits(),
                    rate,
                    pcm: pcm.clone(),
                });
                Some(pcm)
            } else {
                None
            };
            let phrase_pcm = if let Some(phrase) = &bar.backing_phrase {
                Some(
                    crate::backing_phrase::render(
                        phrase,
                        duration as usize,
                        beat,
                        phrase_beats,
                        rate,
                    )
                    .map_err(err)?,
                )
            } else {
                None
            };
            bars.push(PreparedBar {
                duration,
                beat: beat.max(1),
                beat_unit: if bar.legacy_clock {
                    4
                } else {
                    bar.time_signature.denominator
                },
                measure,
                lead: bar.count_in_frames(rate),
                beats,
                chord,
                phrase_pcm,
                loop_pcm: bar.audio_buffer.clone(),
            });
        }
        *cache = next_cache;
        Ok(Self {
            bars,
            one_shot: song.one_shot,
            click: song.click_enabled,
            replace: song.record_replace,
        })
    }
}
impl PreparedClick {
    pub fn prepare(pattern: &SequencerPattern, rate: u32) -> Result<Self, OutputError> {
        if !pattern.bpm.is_finite()
            || pattern.bpm <= 0.0
            || pattern.steps_per_bar() == 0
            || pattern.steps_per_bar() > 1024
            || pattern.tracks.len() > 64
        {
            return Err(err("click pattern exceeds supported bounds"));
        }
        let mut lengths = Vec::with_capacity(pattern.steps_per_bar());
        let mut total = 0usize;
        for idx in 0..pattern.steps_per_bar() {
            let mut longest = 0usize;
            for track in &pattern.tracks {
                if track.muted || !matches!(track.steps.get(idx), Some(crate::Step::Active { .. }))
                {
                    continue;
                }
                let length = match &track.sound {
                    Sound::Click {
                        duration_seconds, ..
                    } => {
                        if !duration_seconds.is_finite() || *duration_seconds < 0.0 {
                            return Err(err("invalid click duration"));
                        }
                        (*duration_seconds * rate as f32).ceil() as usize
                    },
                    Sound::Sample {
                        buffer,
                        pitch_semitones,
                        ..
                    } => {
                        if !pitch_semitones.is_finite() {
                            return Err(err("invalid sample pitch"));
                        }
                        (buffer.len() as f64 * rate as f64
                            / buffer.sample_rate_hz.max(1) as f64
                            / 2f64.powf(*pitch_semitones as f64 / 12.0))
                        .ceil() as usize
                    },
                }
                .checked_add(1)
                .ok_or_else(|| err("click PCM exceeds resource bound"))?;
                longest = longest.max(length);
            }
            total = total
                .checked_add(longest)
                .ok_or_else(|| err("click PCM exceeds resource bound"))?;
            if total > MAX_PCM_FRAMES {
                return Err(err("aggregate click PCM exceeds 16777216 frames"));
            }
            lengths.push(longest);
        }
        let mut steps = Vec::with_capacity(pattern.steps_per_bar());
        for (idx, &length) in lengths.iter().enumerate() {
            let mut data: Vec<f32> = vec![0.0; length];
            for track in &pattern.tracks {
                if track.muted {
                    continue;
                }
                let Some(step) = track.steps.get(idx) else {
                    continue;
                };
                let crate::Step::Active { accent } = step else {
                    continue;
                };
                let limit = match &track.sound {
                    Sound::Click {
                        duration_seconds, ..
                    } => (*duration_seconds * rate as f32).ceil() as usize + 1,
                    Sound::Sample {
                        buffer,
                        pitch_semitones,
                        ..
                    } => {
                        (buffer.len() as f64 * rate as f64
                            / buffer.sample_rate_hz.max(1) as f64
                            / 2f64.powf(*pitch_semitones as f64 / 12.0))
                        .ceil() as usize
                            + 1
                    },
                };
                if limit > MAX_PCM_FRAMES
                    || !track
                        .sound
                        .render_sample(0, rate as f32, *accent)
                        .unwrap_or(0.0)
                        .is_finite()
                {
                    return Err(err("click voice exceeds supported resource bounds"));
                }
                for (local, sample) in data.iter_mut().enumerate().take(limit) {
                    if let Some(value) =
                        track
                            .sound
                            .render_sample(local as u32, rate as f32, *accent)
                    {
                        *sample += value;
                    }
                }
            }
            steps.push((!data.is_empty()).then(|| ArcGc::new(SampleBuffer::new(data, rate))));
        }
        Ok(Self {
            steps,
            divisions: pattern.subdivision.divisions_per_beat.max(1) as u32,
            beat_unit: pattern.time_signature.denominator,
        })
    }
}
impl RecordingScope {
    pub fn includes(self, bar: usize) -> bool {
        match self {
            Self::Follow => true,
            Self::Selected(target) => bar == target,
        }
    }
    pub(super) fn selected(self) -> Option<usize> {
        match self {
            Self::Follow => None,
            Self::Selected(target) => Some(target),
        }
    }
}
impl Recording {
    pub fn prepare(song: &Song, rate: u32, generation: u64) -> Result<Self, OutputError> {
        let mut bars = Vec::with_capacity(song.bars.len());
        let mut total = 0usize;
        for bar in &song.bars {
            let len = usize::try_from(bar.checked_duration_frames(rate).map_err(err)?)
                .map_err(|_| err("capture duration is too large"))?;
            total = total
                .checked_add(len)
                .ok_or_else(|| err("capture storage exceeds bound"))?;
            if total > MAX_PCM_FRAMES {
                return Err(err("capture storage exceeds 16777216 frames"));
            }
            let data = if let Some(pcm) = &bar.audio_buffer {
                if pcm.sample_rate_hz != rate || pcm.len() != len {
                    return Err(err("captured backing clock differs from current segment"));
                }
                pcm.data.as_ref().clone()
            } else {
                vec![0.0; len]
            };
            bars.push(RecordBar {
                pcm: SampleBuffer::new(data, rate),
                touched: false,
                retired: None,
            });
        }
        Ok(Self {
            scope: RecordingScope::Follow,
            generation,
            bars,
            valid: true,
            overflow: 0,
        })
    }
}
