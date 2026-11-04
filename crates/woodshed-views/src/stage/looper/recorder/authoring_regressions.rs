use super::*;
#[test]
fn no_backend_keeps_set_and_saved_bar_clock_authoring_without_runtime_admission() {
    let mut ui = super::super::UiState::new();
    ui.looper.source = LooperSource::Set(ui.working_sets.active_id);
    let key = projection(&ui).transport.tempo.unwrap().target;
    dispatch(
        &mut ui,
        HostAction::NudgeTempo {
            target: key,
            delta: 5.0,
        },
    );
    assert_eq!(ui.looper.inherited_bpm, 125.0);
    assert!(ui.audio_requests.is_empty());
    assert!(!ui.song_playing);
    ui.looper.source = LooperSource::SavedSong;
    ui.song.bars.push(Default::default());
    ui.song.bars.push(Default::default());
    let key = projection(&ui).transport.tempo.unwrap().target;
    dispatch(
        &mut ui,
        HostAction::NudgeTempo {
            target: key,
            delta: 5.0,
        },
    );
    assert_eq!(ui.song.bars[0].bpm, 125.0);
    assert_eq!(ui.song.bars[1].bpm, 120.0);
    assert!(ui.audio_requests.is_empty());
}
#[test]
fn old_transport_controls_cannot_be_redirected_by_valid_pending_backing_edits() {
    let mut ui = super::super::UiState::new();
    ui.song.bars.push(Default::default());
    ui.recorder_observation.source = Some(LooperSource::SavedSong);
    ui.recorder_observation.saved_song = Some(ui.song.clone());
    ui.recorder_observation.output_rate = Some(48000);
    let context = projection(&ui).transport.context;
    ui.song.bars[0].cycle_root();
    for action in [
        HostAction::Play(context.clone()),
        HostAction::Rewind(context.clone()),
        HostAction::ToggleClick(context.clone()),
        HostAction::SetReplace {
            target: context.clone(),
            replace: true,
        },
    ] {
        dispatch(&mut ui, action);
        assert!(!ui.song_playing);
        assert!(ui.audio_requests.is_empty());
        assert!(ui.loop_notice.is_some());
    }
    dispatch(&mut ui, HostAction::Stop(context));
    assert!(ui.loop_notice.is_none());
}
