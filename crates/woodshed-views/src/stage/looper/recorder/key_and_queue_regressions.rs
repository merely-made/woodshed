use super::*;
#[test]
fn signed_zero_saved_guards_use_consistent_bit_qualified_key_equality_and_hash() {
    let mut ui = super::super::UiState::new();
    ui.song.bars.push(Default::default());
    ui.recorder_observation.source = Some(LooperSource::SavedSong);
    ui.recorder_observation.saved_song = Some(ui.song.clone());
    let plus = context(&ui);
    ui.recorder_observation.saved_song.as_mut().unwrap().bars[0].root_freq_hz = -0.0;
    let minus = context(&ui);
    let hash = |key: &RecorderKey| {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut h);
        h.finish()
    };
    assert_ne!(plus, minus);
    assert_ne!(hash(&plus), hash(&minus));
    let rebuilt = context(&ui);
    assert_eq!(minus, rebuilt);
    assert_eq!(hash(&minus), hash(&rebuilt));
}
#[test]
fn two_queued_saved_bar_controls_keep_distinct_frozen_targets() {
    let mut ui = super::super::UiState::new();
    ui.song.bars = vec![Default::default(), Default::default()];
    ui.recorder_observation.source = Some(LooperSource::SavedSong);
    ui.recorder_observation.saved_song = Some(ui.song.clone());
    ui.recorder_observation.output_rate = Some(48000);
    ui.recorder_observation.buffers = vec![
        Some(RecorderBuffer {
            samples: Arc::new(vec![0.1]),
            sample_rate: 48000
        });
        2
    ];
    let keys: Vec<_> = projection(&ui)
        .lanes
        .iter()
        .map(|lane| lane.key.clone())
        .collect();
    dispatch(&mut ui, HostAction::Clear(keys[0].clone()));
    dispatch(&mut ui, HostAction::Clear(keys[1].clone()));
    assert!(
        matches!(&ui.audio_requests[..],[AudioRequest::Recorder(a),AudioRequest::Recorder(b)] if a.command==woodshed_core::audio::RecorderCommand::ClearSavedBar(0) && b.command==woodshed_core::audio::RecorderCommand::ClearSavedBar(1))
    );
}
