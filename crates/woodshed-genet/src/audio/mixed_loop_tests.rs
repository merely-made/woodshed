use super::*;
use woodshed_core::{Lens, StageState, loop_plan::LooperConfig, working_sets::WorkingSetId};
use woodshedding::rehearsal::{Hold, Set, Timing, Touch};

fn mixed() -> LoopPlan {
    let mut stage = StageState::new();
    let mut set = Set::default();
    for (lens, bpm) in [
        (Lens::Chords, 240.0),
        (Lens::Scales, 180.0),
        (Lens::Arpeggios, 300.0),
    ] {
        stage.set_lens(lens);
        let mut card = stage.card_from_lens().unwrap();
        card.timing = Timing {
            bpm: Some(bpm),
            hold: Hold::Bars(1),
        };
        if lens == Lens::Scales {
            card.touch = Touch::Walk;
        }
        set.push(card);
    }
    LoopPlan::lower(
        WorkingSetId(1),
        "Mixed",
        &set,
        &stage,
        &LooperConfig {
            beats: 3,
            click: false,
            ..Default::default()
        },
    )
    .unwrap()
}

#[test]
fn admitted_phrase_changes_keep_capture_identity_and_clock_until_explicit_clear() {
    let mut backend = CpalBackend::offline_for_test();
    let original = mixed();
    let selected = &original.segments[1];
    let rate = backend.output_sample_rate().unwrap();
    let bar = Bar {
        duration_seconds: selected.seconds,
        bpm: selected.bpm,
        time_signature: TimeSignature::new(selected.beats, 4),
        length: selected.bars,
        ..Default::default()
    };
    let buffer = woodshed_audio::SampleBuffer::new(
        vec![0.125; bar.duration_samples(rate as f32) as usize],
        rate,
    );
    backend
        .restore_loop_captures(vec![CaptureSnapshot {
            seconds: selected.seconds,
            id: selected.id,
            bpm: selected.bpm,
            beats: selected.beats,
            bars: selected.bars,
            buffer: buffer.clone(),
            legacy_clock: false,
        }])
        .unwrap();
    backend.set_loop_plan(&original).unwrap();
    let (loaded, _) = backend.export_backing().unwrap();
    assert!(loaded.bars[0].backing_phrase.is_none());
    assert!(
        loaded.bars[1..]
            .iter()
            .all(|bar| bar.backing_phrase.is_some() && bar.chord_ref.is_none())
    );
    assert!(Arc::ptr_eq(
        &loaded.bars[1].audio_buffer.as_ref().unwrap().data,
        &buffer.data
    ));
    let mut edited = original.clone();
    edited.segments.swap(0, 1);
    edited.segments[0].phrase.as_mut().unwrap().events[0].pitches_hz[0] *= 1.01;
    backend.set_loop_plan(&edited).unwrap();
    let (admitted, _) = backend.export_backing().unwrap();
    assert!(Arc::ptr_eq(
        &admitted.bars[0].audio_buffer.as_ref().unwrap().data,
        &buffer.data
    ));
    assert!(admitted.bars[1].audio_buffer.is_none());
    assert_eq!(backend.capture_snapshot()[0].id, selected.id);
    assert_eq!(backend.captures.plan, Some(edited.clone()));
    let mut other = edited.clone();
    other.segments[0].beats = 7;
    assert!(
        backend
            .set_loop_plan(&other)
            .unwrap_err()
            .contains("Clear the captured loop")
    );
    assert_eq!(backend.captures.plan, Some(edited));
    assert!(Arc::ptr_eq(
        &backend.capture_snapshot()[0].buffer.data,
        &buffer.data
    ));
    backend.clear_stored_capture(selected.id).unwrap();
    backend.process_for_test(1);
    backend.set_loop_plan(&other).unwrap();
    assert!(
        backend.export_backing().unwrap().0.bars[0]
            .audio_buffer
            .is_none()
    );
    assert!(backend.capture_source_rate().is_none());
}

#[test]
fn rejected_device_phrase_keeps_the_admitted_plan_and_saved_backing_remains_exact() {
    let mut backend = CpalBackend::offline_for_test();
    let accepted = mixed();
    backend.set_loop_plan(&accepted).unwrap();
    let before = backend.export_backing().unwrap().0;
    let mut invalid = accepted.clone();
    let events = &mut invalid.segments[1].phrase.as_mut().unwrap().events;
    events[1].time = woodshed_core::loop_plan::EventTime::Beats(f64::MIN_POSITIVE);
    assert!(backend.set_loop_plan(&invalid).is_err());
    assert_eq!(backend.captures.plan, Some(accepted));
    let after = backend.export_backing().unwrap().0;
    assert_eq!(before.bars[1].backing_phrase, after.bars[1].backing_phrase);
    let mut saved = SongDoc::default();
    saved.add_bar_after(0);
    saved.bars[0].bpm = 137.0;
    saved.bars[0].beats = 7;
    saved.bars[0].length = 2;
    saved.bars[0].root_freq_hz = 253.125;
    saved.bars[0].pitches_hz = vec![253.125, 381.777];
    let payload = serde_json::to_value(&saved).unwrap();
    backend.try_set_song(&saved).unwrap();
    let (actual, _) = backend.export_backing().unwrap();
    assert!(backend.captures.plan.is_none());
    assert!(actual.bars[0].backing_phrase.is_none());
    assert_eq!(
        actual.bars[0].chord_ref.as_ref().unwrap().pitches_hz,
        saved.bars[0].pitches_hz
    );
    assert_eq!(actual.bars[0].bpm, 137.0);
    assert_eq!(actual.bars[0].time_signature.numerator, 7);
    assert_eq!(actual.bars[0].length, 2);
    assert_eq!(serde_json::to_value(saved).unwrap(), payload);
    assert!(backend.capture_source_rate().is_none());
}


fn seconds_mixed() -> LoopPlan {
    let mut stage = StageState::new();
    let mut set = Set::default();
    for (lens, hold, bpm) in [
        (Lens::Chords, Hold::Bars(1), 240.0),
        (Lens::Scales, Hold::Seconds(1.125), 180.0),
        (Lens::Arpeggios, Hold::Seconds(0.875), 300.0),
    ] {
        stage.set_lens(lens);
        let mut card = stage.card_from_lens().unwrap();
        card.timing = Timing { bpm: Some(bpm), hold };
        if lens == Lens::Scales { card.touch = Touch::Walk; }
        set.push(card);
    }
    LoopPlan::lower(WorkingSetId(1), "Seconds mixed", &set, &stage,
        &LooperConfig { beats: 3, click: false, ..Default::default() }).unwrap()
}

#[test]
fn seconds_device_projection_preserves_exact_duration_and_capture_owner_until_clear() {
    let mut backend = CpalBackend::offline_for_test();
    let original = seconds_mixed();
    let rate = backend.output_sample_rate().unwrap();
    backend.set_loop_plan(&original).unwrap();
    let (song, _) = backend.export_backing().unwrap();
    assert_eq!(song.bars[1].duration_samples(rate as f32), (1.125 * f64::from(rate)).round() as u64);
    assert_eq!(song.bars[2].duration_samples(rate as f32), (0.875 * f64::from(rate)).round() as u64);
    let selected = &original.segments[1];
    backend.clear_loop_session();
    let data = Arc::new((0..song.bars[1].duration_samples(rate as f32))
        .map(|i| [0.125, -0.25, -0.0, 0.0][i as usize % 4]).collect::<Vec<_>>());
    backend.restore_loop_captures(vec![CaptureSnapshot {
        id: selected.id, bpm: selected.bpm, beats: selected.beats, bars: selected.bars,
        seconds: selected.seconds, legacy_clock: false,
        buffer: woodshed_audio::SampleBuffer::new((*data).clone(), rate),
    }]).unwrap();
    backend.set_loop_plan(&original).unwrap();
    let prior = backend.export_backing().unwrap().0.bars[1].audio_buffer.clone().unwrap();
    let mut reordered = original.clone();
    reordered.segments.swap(0, 1);
    backend.set_loop_plan(&reordered).unwrap();
    let retained = backend.export_backing().unwrap().0.bars[0].audio_buffer.clone().unwrap();
    assert!(Arc::ptr_eq(&prior.data, &retained.data));
    assert!(prior.data.iter().zip(data.iter()).all(|(a,b)| a.to_bits()==b.to_bits()));
    let mut edited = reordered.clone();
    edited.segments[0].seconds = Some(1.25);
    assert!(backend.set_loop_plan(&edited).unwrap_err().contains("Clear"));
    let refused = backend.export_backing().unwrap().0;
    assert_eq!(backend.captures.plan, Some(reordered));
    assert!(Arc::ptr_eq(&prior.data, &refused.bars[0].audio_buffer.as_ref().unwrap().data));
    assert!(refused.bars[0].audio_buffer.as_ref().unwrap().data.iter().zip(data.iter()).all(|(a,b)|a.to_bits()==b.to_bits()));
    assert_eq!(refused.bars[0].duration_samples(rate as f32), prior.len() as u64);
    backend.clear_stored_capture(selected.id).unwrap();
    backend.process_for_test(1);
    backend.set_loop_plan(&edited).unwrap();
    assert!(backend.export_backing().unwrap().0.bars[0].audio_buffer.is_none());
    assert_eq!(backend.export_backing().unwrap().0.bars[0].duration_samples(rate as f32), (1.25 * f64::from(rate)).round() as u64);
    assert!(backend.capture_source_rate().is_none());
}
