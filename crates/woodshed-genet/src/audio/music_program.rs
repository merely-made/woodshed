//! Disposable score schedule → the existing generic output program.
use woodshed_audio::output_runtime::{
    BEAT_TICKS, ProgramCard, ProgramDuration, ProgramEnd, ProgramEvent, ProgramLoop, ProgramPlan,
    ProgramPosition,
};
use woodshedding::timing::{ArrangementSchedule, TICKS_PER_QUARTER};

// OutputRuntime accepts at most 256 segments and 4096 sounding events. Also
// bound logical events (including rests) in this adapter before allocating an
// output program. These are admission limits, not musical score constraints.
const MAX_PLAYBACK_MEASURES: usize = 256;
const MAX_PLAYBACK_NOTES: usize = 4096;
const MAX_PLAYBACK_EVENTS: usize = 4096;

fn ticks(value: u64) -> Result<u64, String> {
    let scaled = value
        .checked_mul(BEAT_TICKS)
        .ok_or("Arrangement clock conversion overflow.")?;
    if TICKS_PER_QUARTER == 0 || scaled % TICKS_PER_QUARTER != 0 {
        return Err("Arrangement timing is not exact on the output clock.".into());
    }
    Ok(scaled / TICKS_PER_QUARTER)
}

pub(super) fn lower_program(
    schedule: &ArrangementSchedule,
    loop_mode: ProgramLoop,
) -> Result<ProgramPlan, String> {
    if !schedule.tempo_bpm.is_finite() || !(1.0..=400.0).contains(&schedule.tempo_bpm) {
        return Err("Arrangement practice tempo must be finite and within 1..=400 BPM.".into());
    }
    if schedule.measures.is_empty() || schedule.measures.len() > MAX_PLAYBACK_MEASURES {
        return Err("Arrangement output requires 1..=256 measures.".into());
    }
    let mut logical_count = 0usize;
    let mut sounding_count = 0usize;
    for measure in &schedule.measures {
        logical_count = logical_count
            .checked_add(measure.events.len())
            .ok_or("Arrangement event count overflow.")?;
        if logical_count > MAX_PLAYBACK_EVENTS {
            return Err("Arrangement exceeds 4096 logical events.".into());
        }
        for event in &measure.events {
            sounding_count = sounding_count
                .checked_add(event.notes.len())
                .ok_or("Arrangement sounding count overflow.")?;
            if sounding_count > MAX_PLAYBACK_NOTES {
                return Err("Arrangement exceeds 4096 sounding contacts.".into());
            }
        }
    }
    let mut cards = Vec::with_capacity(schedule.measures.len());
    for (bar, measure) in schedule.measures.iter().enumerate() {
        if measure.measure_index != bar || measure.duration_ticks == 0 {
            return Err("Arrangement measure identity or duration is invalid.".into());
        }
        let mut events = Vec::new();
        let mut expected_onset = 0u64;
        for (index, event) in measure.events.iter().enumerate() {
            if event.event_index != index
                || event.onset_ticks != expected_onset
                || event.gate_ticks == 0
            {
                return Err("Arrangement event identity, onset or gate is invalid.".into());
            }
            expected_onset = event
                .onset_ticks
                .checked_add(event.gate_ticks)
                .filter(|end| *end <= measure.duration_ticks)
                .ok_or("Arrangement event gate exceeds its measure.")?;
            let token = u64::try_from(index)
                .ok()
                .and_then(|n| n.checked_add(1))
                .ok_or("Arrangement event token overflow.")?;
            // The engine supplies its established synthesis/release envelope.
            // This adapter only controls the exact authored gate and onset.
            let amplitude = 0.45 / (event.notes.len().max(1) as f32).sqrt();
            for note in &event.notes {
                if note.midi > 127 || !note.frequency_hz.is_finite() || note.frequency_hz <= 0.0 {
                    return Err("Arrangement contains an invalid sounding pitch.".into());
                }
                events.push(ProgramEvent {
                    token,
                    onset: ProgramPosition::BeatTicks(ticks(event.onset_ticks)?),
                    onset_offset_frames: 0,
                    gate: ProgramDuration::BeatTicks(ticks(event.gate_ticks)?),
                    frequency_hz: note.frequency_hz,
                    amplitude,
                });
            }
        }
        if expected_onset != measure.duration_ticks {
            return Err("Arrangement events must fill the whole measure, including rests.".into());
        }
        cards.push(ProgramCard {
            token: u64::try_from(bar)
                .ok()
                .and_then(|n| n.checked_add(1))
                .ok_or("Arrangement measure token overflow.")?,
            bpm: schedule.tempo_bpm,
            end: ProgramEnd::BeatTicks(ticks(measure.duration_ticks)?),
            events,
        });
    }
    Ok(ProgramPlan {
        cards,
        start_index: 0,
        loop_mode,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use woodshed_audio::output_runtime::{OutputRuntime, ProgramStatus};
    use woodshedding::notation::{
        FrettedNote, Measure, NoteEvent, NoteValue, Score, TimeSignature,
    };
    fn render(runtime: &mut OutputRuntime, frames: usize) -> Vec<f32> {
        runtime.service().unwrap();
        let mut pcm = vec![0.0; frames * 2];
        for chunk in pcm.chunks_mut(8192 * 2) {
            runtime.render_offline(chunk).unwrap();
        }
        pcm
    }
    fn start(score: &Score, rate: u32, mode: ProgramLoop) -> OutputRuntime {
        let schedule = ArrangementSchedule::from_score(score, 120.0).unwrap();
        let mut runtime = OutputRuntime::offline(rate).unwrap();
        render(&mut runtime, 1);
        let prepared = runtime
            .prepare_program(lower_program(&schedule, mode).unwrap())
            .unwrap();
        runtime.start_program(prepared).unwrap();
        runtime
    }
    #[test]
    fn triplets_and_rest_keep_exact_gates_and_engine_release_becomes_silent() {
        let score = Score::rhythm_example();
        let p = lower_program(
            &ArrangementSchedule::from_score(&score, 120.0).unwrap(),
            ProgramLoop::Once,
        )
        .unwrap();
        assert_eq!(p.cards[0].events.len(), 4);
        assert_eq!(p.cards[0].end, ProgramEnd::BeatTicks(3840));
        for (i, event) in p.cards[1].events.iter().take(3).enumerate() {
            assert_eq!(event.onset, ProgramPosition::BeatTicks(i as u64 * 320));
            assert_eq!(event.gate, ProgramDuration::BeatTicks(320));
        }
        let mut runtime = start(&score, 9600, ProgramLoop::Once);
        let pcm = render(&mut runtime, 19000);
        assert!(pcm[..28000].iter().any(|x| x.abs() > 0.0001));
        // Authored rest starts at frame14400. The established engine releases
        // voices after the gate; allow its250ms tail before asserting silence.
        assert!(pcm[35000..].iter().all(|x| x.abs() < 0.00001));
        assert_eq!(runtime.snapshot().program.card_index, 0);
    }
    #[test]
    fn chord_contacts_are_simultaneous_without_strum_and_rest_only_segments_survive() {
        let mut s = Score::progression_example();
        let schedule = ArrangementSchedule::from_score(&s, 120.0).unwrap();
        let p = lower_program(&schedule, ProgramLoop::Once).unwrap();
        for e in &p.cards[0].events {
            assert_eq!(e.token, 1);
            assert_eq!(e.onset, ProgramPosition::BeatTicks(0));
            assert_eq!(e.onset_offset_frames, 0);
            assert_eq!(e.gate, ProgramDuration::BeatTicks(3840));
        }
        for e in &mut s.measures[0].events {
            e.notes.clear();
            e.rest = true;
        }
        let p = lower_program(
            &ArrangementSchedule::from_score(&s, 120.0).unwrap(),
            ProgramLoop::Once,
        )
        .unwrap();
        assert!(p.cards[0].events.is_empty());
        assert_eq!(p.cards[0].end, ProgramEnd::BeatTicks(3840));
        let mut runtime = start(&s, 9600, ProgramLoop::Once);
        assert!(render(&mut runtime, 18000).iter().all(|x| *x == 0.0));
    }
    fn mixed() -> Score {
        let mut s = Score::rhythm_example();
        let e = NoteEvent {
            notes: vec![FrettedNote { string: 1, fret: 0 }],
            rest: false,
            value: NoteValue::Quarter,
        };
        s.measures = vec![
            Measure {
                meter: TimeSignature {
                    beats: 3,
                    beat_type: 4,
                },
                harmony: None,
                events: vec![e.clone(); 3],
            },
            Measure {
                meter: TimeSignature {
                    beats: 6,
                    beat_type: 8,
                },
                harmony: None,
                events: vec![
                    NoteEvent {
                        value: NoteValue::Eighth,
                        ..e
                    };
                    6
                ],
            },
        ];
        s
    }
    #[test]
    fn mixed_meter_boundaries_loop_modes_and_once_completion_use_existing_clock() {
        for mode in [ProgramLoop::Once, ProgramLoop::Set, ProgramLoop::Card] {
            let mut runtime = start(&mixed(), 9600, mode);
            render(&mut runtime, 14410);
            let snap = runtime.snapshot().program;
            assert_eq!(
                snap.card_index,
                if mode == ProgramLoop::Card { 0 } else { 1 }
            );
            assert_eq!(snap.status, ProgramStatus::Playing);
            render(&mut runtime, 14410);
            let snap = runtime.snapshot().program;
            assert_eq!(
                snap.status,
                if mode == ProgramLoop::Once {
                    ProgramStatus::Complete
                } else {
                    ProgramStatus::Playing
                }
            );
            if mode != ProgramLoop::Once {
                assert_eq!(snap.card_index, 0);
            }
        }
    }
    fn partitioned(rate: u32, chunk: usize) -> (Vec<f32>, u64, usize) {
        let mut runtime = start(&mixed(), rate, ProgramLoop::Once);
        let total = rate as usize * 2;
        let mut pcm = Vec::with_capacity(total * 2);
        while pcm.len() < total * 2 {
            let frames = chunk.min(total - pcm.len() / 2);
            pcm.extend(render(&mut runtime, frames));
        }
        let s = runtime.snapshot().program;
        (pcm, s.beat_ticks, s.card_index)
    }
    #[test]
    fn output_is_buffer_partition_independent_and_rate_tracks_score_time() {
        for rate in [9600, 19200] {
            let a = partitioned(rate, 127);
            let b = partitioned(rate, 1024);
            assert_eq!(a, b);
            assert_eq!(a.2, 1);
            assert!((a.1 as i64 - 960).abs() <= 2);
        }
    }
    #[test]
    fn malformed_or_excessive_schedule_is_refused_without_replacing_active_program() {
        let score = mixed();
        let schedule = ArrangementSchedule::from_score(&score, 120.0).unwrap();
        let mut runtime = start(&score, 9600, ProgramLoop::Once);
        render(&mut runtime, 100);
        let epoch = runtime.snapshot().program.epoch;
        let mut bad = schedule.clone();
        bad.measures[1].events[0].gate_ticks = u64::MAX;
        assert!(lower_program(&bad, ProgramLoop::Once).is_err());
        bad = schedule.clone();
        bad.measures[0].events[0].notes[0].frequency_hz = f64::NAN;
        assert!(lower_program(&bad, ProgramLoop::Once).is_err());
        bad = schedule.clone();
        bad.measures = vec![bad.measures[0].clone(); 257];
        assert!(lower_program(&bad, ProgramLoop::Once).is_err());
        bad = schedule.clone();
        bad.measures[0].events[0].notes = vec![bad.measures[0].events[0].notes[0].clone(); 4097];
        assert!(
            lower_program(&bad, ProgramLoop::Once)
                .unwrap_err()
                .contains("4096")
        );
        let mut excessive = lower_program(&schedule, ProgramLoop::Once).unwrap();
        excessive.cards[0].events = vec![excessive.cards[0].events[0].clone(); 4097];
        assert!(runtime.prepare_program(excessive).is_err());
        let mut plan = lower_program(&schedule, ProgramLoop::Once).unwrap();
        plan.cards[0].events[0].gate = ProgramDuration::BeatTicks(u64::MAX);
        assert!(runtime.prepare_program(plan).is_err());
        render(&mut runtime, 100);
        assert_eq!(runtime.snapshot().program.epoch, epoch);
        assert_eq!(runtime.snapshot().program.status, ProgramStatus::Playing);
    }

    #[test]
    fn valid_music_exceeding_engine_capacity_is_refused_only_by_adapter() {
        let mut score = Score::progression_example();
        let measure = score.measures[0].clone();
        score.measures = vec![measure.clone(); MAX_PLAYBACK_MEASURES];
        let timing = ArrangementSchedule::from_score(&score, 120.0).unwrap();
        assert_eq!(
            lower_program(&timing, ProgramLoop::Once)
                .unwrap()
                .cards
                .len(),
            256
        );
        score.measures.push(measure.clone());
        let timing = ArrangementSchedule::from_score(&score, 120.0).unwrap();
        assert!(
            lower_program(&timing, ProgramLoop::Once)
                .unwrap_err()
                .contains("256")
        );

        let mut chord_bar = measure;
        assert_eq!(chord_bar.events[0].notes.len(), 4);
        chord_bar.events = vec![
            NoteEvent {
                value: NoteValue::Eighth,
                ..chord_bar.events[0].clone()
            };
            8
        ];
        score.measures = vec![chord_bar.clone(); 128];
        let timing = ArrangementSchedule::from_score(&score, 120.0).unwrap();
        let program = lower_program(&timing, ProgramLoop::Once).unwrap();
        assert_eq!(
            program.cards.iter().map(|c| c.events.len()).sum::<usize>(),
            4096
        );
        score.measures.push(chord_bar.clone());
        let timing = ArrangementSchedule::from_score(&score, 120.0).unwrap();
        assert!(
            lower_program(&timing, ProgramLoop::Once)
                .unwrap_err()
                .contains("sounding contacts")
        );

        let mut rest_bar = chord_bar;
        rest_bar.meter.beats = 16;
        rest_bar.events = vec![
            NoteEvent {
                notes: Vec::new(),
                rest: true,
                value: NoteValue::Eighth,
            };
            32
        ];
        score.measures = vec![rest_bar.clone(); 128];
        let timing = ArrangementSchedule::from_score(&score, 120.0).unwrap();
        assert!(
            lower_program(&timing, ProgramLoop::Once)
                .unwrap()
                .cards
                .iter()
                .all(|c| c.events.is_empty())
        );
        score.measures.push(rest_bar);
        let timing = ArrangementSchedule::from_score(&score, 120.0).unwrap();
        assert!(
            lower_program(&timing, ProgramLoop::Once)
                .unwrap_err()
                .contains("logical events")
        );
    }
}
