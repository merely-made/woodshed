use super::*;
use woodshed_core::Lens;
use woodshedding::rehearsal::Hold;
fn ready() -> super::super::UiState {
    let mut ui = super::super::UiState::new();
    ui.stage.set_lens(Lens::Chords);
    for (bpm, bars) in [(90.0, 1), (150.0, 2)] {
        let mut card = ui.stage.card_from_lens().unwrap();
        card.timing.bpm = Some(bpm);
        card.timing.hold = Hold::Bars(bars);
        ui.set.push(card);
    }
    ui.looper.beats = 7;
    ui.loop_working_set(ui.working_sets.active_id);
    let plan = ui.requested_loop_plan().unwrap();
    ui.loop_selected = Some(plan.segments[0].id);
    ui.loop_admitted = Some(plan.clone());
    ui.recorder_observation = RecorderObservation {
        source: Some(LooperSource::Set(plan.owner)),
        plan: Some(plan),
        buffers: vec![
            Some(RecorderBuffer {
                samples: Arc::new(vec![0.25, -0.5, 0.75]),
                sample_rate: 48000,
            }),
            None,
        ],
        output_rate: Some(48000),
        status: "input opens on Record".into(),
        ..Default::default()
    };
    ui
}
#[test]
fn seconds_lane_discloses_absolute_duration_and_stale_duration_command_refuses() {
    let mut ui = ready();
    ui.set.cards[0].timing.hold = Hold::Seconds(1.125);
    let plan = ui.requested_loop_plan().unwrap();
    ui.recorder_observation.plan = Some(plan.clone());
    ui.loop_admitted = Some(plan);
    let projection = projection(&ui);
    assert!(projection.lanes[0].clock.contains("1.125 s"));
    assert!(!projection.lanes[0].clock.contains("× 0"));
    assert_eq!(projection.transport.meter_heading, "Selected timing");
    let stale = projection.transport.play.as_ref().unwrap().target.clone();
    drop(projection);
    ui.set.cards[0].timing.hold = Hold::Seconds(2.0);
    let changed = ui.requested_loop_plan().unwrap();
    ui.recorder_observation.plan = Some(changed.clone());
    ui.loop_admitted = Some(changed);
    assert_ne!(stale, context(&ui));
    let playing = ui.song_playing;
    ui.audio_requests.clear();
    dispatch(&mut ui, HostAction::Play(stale));
    assert_eq!(ui.song_playing, playing);
    assert!(ui.audio_requests.is_empty());
    assert!(ui.loop_notice.as_deref().unwrap().contains("changed"));
}

#[test]
fn heterogeneous_projection_uses_actual_pcm_and_render_target_without_authoring_a_second_session() {
    let mut ui = ready();
    let display = projection(&ui);
    assert_eq!(display.lanes[0].clock, "90 bpm · 7/4 × 1");
    assert_eq!(display.lanes[1].clock, "150 bpm · 7/4 × 2");
    assert!(display.meters.is_none());
    assert!(display.transport.add.is_none());
    assert!(display.lanes[0].controls.mute.is_none());
    let target = display.lanes[0].key.clone();
    let other = display.lanes[1].key.clone();
    drop(display);
    let session = ui.to_persisted();
    dispatch(&mut ui, HostAction::Select(other));
    dispatch(&mut ui, HostAction::Record(target.clone()));
    assert!(
        matches!(&ui.audio_requests[..],[AudioRequest::Recorder(request)] if request.command==woodshed_core::audio::RecorderCommand::RecordCard(match target.target{RecorderTarget::Card(id)=>id,_=>unreachable!()}))
    );
    assert!(!ui.song_recording);
    assert_eq!(
        serde_json::to_value(&ui.to_persisted().set).unwrap(),
        serde_json::to_value(&session.set).unwrap()
    );
}
#[test]
fn stale_session_source_reorder_and_saved_bar_keys_refuse_queued_actions() {
    let mut ui = ready();
    let old = projection(&ui).lanes[0].key.clone();
    ui.audio_session = Arc::new(());
    dispatch(&mut ui, HostAction::Record(old));
    assert!(ui.audio_requests.is_empty());
    let old = projection(&ui).lanes[0].key.clone();
    ui.recorder_observation
        .plan
        .as_mut()
        .unwrap()
        .segments
        .reverse();
    dispatch(&mut ui, HostAction::Clear(old));
    assert!(ui.audio_requests.is_empty());
    let mut ui = super::super::UiState::new();
    ui.song.bars.push(Default::default());
    ui.recorder_observation.source = Some(LooperSource::SavedSong);
    ui.recorder_observation.saved_song = Some(ui.song.clone());
    ui.recorder_observation.output_rate = Some(48000);
    let old = projection(&ui).lanes[0].key.clone();
    ui.song.bars[0].cycle_root();
    dispatch(&mut ui, HostAction::Record(old));
    assert!(ui.audio_requests.is_empty());
}
#[test]
fn scoped_capture_focus_survives_selection_and_returns_to_selection_when_idle() {
    let mut ui = ready();
    ui.recorder_observation.capture_busy = true;
    ui.recorder_observation.capture_target = Some(0);
    ui.recorder_observation.capture_epoch = Some(1);
    ui.song_recording = true;
    let other = projection(&ui).lanes[1].key.clone();
    dispatch(&mut ui, HostAction::Select(other));
    let selected = ui.loop_selected;
    let armed = projection(&ui);
    assert_eq!(armed.transport.record_note, "Armed Card 1");
    drop(armed);
    ui.recorder_observation.input_permission_pending = true;
    let consent = projection(&ui);
    assert!(
        consent
            .transport
            .status
            .contains("Allow microphone access in macOS")
    );
    assert!(
        consent
            .transport
            .input_status
            .contains("Stop cancels pending Record")
    );
    drop(consent);
    ui.recorder_observation.input_permission_pending = false;
    let armed = projection(&ui);
    assert!(armed.transport.status.contains("Opening microphone"));
    assert!(
        armed
            .transport
            .input_status
            .contains("recording has not started")
    );
    assert_eq!(armed.transport.meter_heading, "Capture timing");
    assert_eq!(
        armed.transport.meter_label.as_deref(),
        Some("90 bpm · 7/4 × 1")
    );
    assert!(armed.lanes[1].selected && !armed.lanes[0].selected);
    assert_eq!(armed.lanes[0].recording, RecordingState::Armed);
    drop(armed);
    ui.recorder_observation.recording_bar = Some(0);
    ui.recorder_observation.input_rate = Some(48_000);
    let display = projection(&ui);
    assert_eq!(display.lanes[0].recording, RecordingState::Recording);
    assert_eq!(display.lanes[1].recording, RecordingState::Idle);
    assert_eq!(display.transport.record_note, "Recording Card 1");
    assert_eq!(
        display.transport.meter_label.as_deref(),
        Some("90 bpm · 7/4 × 1")
    );
    let record = display.transport.record.as_ref().unwrap();
    assert!(!record.enabled);
    assert_eq!(record.target.capture_epoch, Some(1));
    let stop = display
        .transport
        .stop_capture
        .as_ref()
        .unwrap()
        .target
        .clone();
    drop(display);
    dispatch(&mut ui, HostAction::StopCapture(stop.clone()));
    assert!(
        matches!(&ui.audio_requests[..],[AudioRequest::Recorder(request)] if request.command==woodshed_core::audio::RecorderCommand::StopCapture{start_bar:0,capture_epoch:1})
    );
    assert!(ui.song_recording);
    let key = projection(&ui).lanes[0].key.clone();
    dispatch(&mut ui, HostAction::Clear(key));
    assert_eq!(ui.audio_requests.len(), 1);
    assert!(
        ui.loop_notice
            .as_deref()
            .unwrap()
            .contains("Stop recording")
    );
    ui.recorder_observation.capture_busy = false;
    ui.recorder_observation.capture_target = None;
    ui.recorder_observation.capture_epoch = None;
    ui.recorder_observation.recording_bar = None;
    ui.song_recording = false;
    let idle = projection(&ui);
    assert_eq!(ui.loop_selected, selected);
    assert_eq!(idle.transport.record_note, "Selected Card 2");
    assert_eq!(idle.transport.meter_heading, "Selected timing");
    assert_eq!(
        idle.transport.meter_label.as_deref(),
        Some("150 bpm · 7/4 × 2")
    );
    assert!(idle.transport.record.as_ref().unwrap().enabled);
    assert!(idle.transport.stop_capture.is_none());
}
#[test]
fn busy_set_capture_without_accepted_mapping_never_falls_back_to_selection() {
    for (target, epoch) in [(None, Some(1)), (Some(99), Some(1)), (Some(0), None)] {
        let mut ui = ready();
        ui.loop_selected = Some(ui.recorder_observation.plan.as_ref().unwrap().segments[1].id);
        ui.recorder_observation.capture_busy = true;
        ui.recorder_observation.capture_target = target;
        ui.recorder_observation.capture_epoch = epoch;
        let display = projection(&ui);
        assert_eq!(display.transport.record_note, "Capture target unavailable");
        assert!(
            display
                .transport
                .status
                .contains("Capture target unavailable")
        );
        assert_eq!(display.transport.meter_heading, "Capture timing");
        assert!(display.transport.meter_label.is_none());
        let record = display.transport.record.as_ref().unwrap();
        assert!(!record.enabled);
        assert_eq!(record.target.target, RecorderTarget::Context);
        assert!(display.transport.stop_capture.is_none());
        assert!(display.transport.stop.as_ref().unwrap().enabled);
        assert!(display.lanes[1].selected);
    }
}
#[test]
fn recording_lane_reports_runtime_destination_even_if_it_disagrees_with_set_target() {
    let mut ui = ready();
    ui.recorder_observation.capture_busy = true;
    ui.recorder_observation.capture_target = Some(0);
    ui.recorder_observation.capture_epoch = Some(1);
    ui.recorder_observation.recording_bar = Some(1);
    let display = projection(&ui);
    assert_eq!(display.lanes[0].recording, RecordingState::Idle);
    assert_eq!(display.lanes[1].recording, RecordingState::Recording);
    assert_eq!(display.transport.record_note, "Recording Card 1");
}
#[test]
fn saved_mute_changes_the_admitted_control_scope_and_failed_capture_is_not_armed() {
    let mut ui = super::super::UiState::new();
    ui.song.bars = vec![Default::default()];
    ui.looper.source = LooperSource::SavedSong;
    ui.recorder_observation = RecorderObservation {
        source: Some(LooperSource::SavedSong),
        saved_song: Some(ui.song.clone()),
        output_rate: Some(48000),
        ..Default::default()
    };
    let old = context(&ui);
    ui.song.bars[0].toggle_silent();
    ui.recorder_observation.saved_song = Some(ui.song.clone());
    assert_ne!(
        context(&ui),
        old,
        "a queued control cannot silently adopt changed audibility"
    );
    ui.recorder_observation.capture_invalid = true;
    let display = projection(&ui);
    assert!(display.lanes[0].name.contains("silent"));
    assert_eq!(display.transport.recording, RecordingState::Idle);
    assert!(display.transport.status.contains("no new take saved"));
    drop(display);
    ui.recorder_observation.capture_busy = true;
    let display = projection(&ui);
    assert!(display.transport.status.contains("take will be discarded"));
}

#[test]
fn saved_song_capture_keeps_moving_destination_and_selection_contract() {
    let mut ui = super::super::UiState::new();
    ui.song.bars = vec![Default::default(), Default::default()];
    ui.looper.source = LooperSource::SavedSong;
    ui.song_edit_cursor = 1;
    ui.recorder_observation = RecorderObservation {
        source: Some(LooperSource::SavedSong),
        saved_song: Some(ui.song.clone()),
        capture_busy: true,
        capture_target: Some(0),
        capture_epoch: Some(1),
        recording_bar: Some(1),
        output_rate: Some(48000),
        ..Default::default()
    };
    let display = projection(&ui);
    assert_eq!(display.lanes[0].recording, RecordingState::Idle);
    assert_eq!(display.lanes[1].recording, RecordingState::Recording);
    assert!(display.lanes[1].selected);
    assert_eq!(display.transport.record_note, "Selected bar 2");
    assert_eq!(display.transport.meter_heading, "Selected timing");
    assert_eq!(
        display.transport.meter_label.as_ref(),
        Some(&display.lanes[1].clock)
    );
    let stop = &display.transport.stop_capture.as_ref().unwrap().target;
    assert_eq!(stop.target, RecorderTarget::SavedBar(0));
    assert_eq!(stop.capture_epoch, Some(1));
}
#[test]
fn mounted_component_routes_real_select_record_and_clear_controls() {
    use cambium_genet_winit_host::Harness;
    use taproot::Selector;
    let ui = ready();
    let target = ui.recorder_observation.plan.as_ref().unwrap().segments[0].id;
    let logic: Box<dyn FnMut(&super::super::UiState) -> super::super::UiChild> = Box::new(surface);
    let mut h = Harness::new(crate::theme::slate_stage_css(), ui, logic);
    h.layout_at(1100.0, 600.0);
    assert!(h.click_on(&Selector::class("hocket-recorder-arm")));
    assert_eq!(h.state().loop_selected, Some(target));
    assert!(h.click_on(&Selector::class("hocket-recorder-record")));
    assert!(
        matches!(&h.state().audio_requests[..],[AudioRequest::Recorder(request)] if request.command==woodshed_core::audio::RecorderCommand::RecordCard(target))
    );
    assert!(h.click_on(&Selector::class("hocket-recorder-clear")));
    assert!(
        matches!(h.state().audio_requests.last(),Some(AudioRequest::Recorder(request)) if request.command==woodshed_core::audio::RecorderCommand::ClearCard(target))
    );
}
