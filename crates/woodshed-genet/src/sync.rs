//! The tail of every input dispatch: push the state through the seams it owns.
//!
//! Dropdown picks land in the core, the audio backend is told the new
//! transport, the MIDI ports are connected or dropped, window-chrome requests
//! are honoured, the session is persisted, and a theme or accessibility change
//! asks the host for a new sheet.

use cambium_genet_winit_host::AppCtx;
use woodshed_core::audio::{AudioBackend, AudioRequest, CalibrationStatus};
use woodshed_core::loop_plan::{LoopPlan, LooperSource};
use woodshed_core::midi::MidiBackend as _;
use woodshed_core::song::SongDoc;
use woodshed_views::stage::{UiChild, UiState};

use crate::shared::Shared;

/// The runner shape this application uses, spelled once.
/// A boxed closure rather than a `fn` pointer: the desktop root captures the
/// host's window-verb handle, so the caption buttons can invoke it without a
/// flag on the shared (browser-portable) `UiState`.
pub type Logic = Box<dyn FnMut(&UiState) -> UiChild>;
/// The host context a woodshed hook receives.
pub type Ctx<'a> = AppCtx<'a, UiState, Logic, UiChild>;

/// Resolve a MIDI port dropdown selection to a connect target: index 0 = "None"
/// (disconnect), else `ports[idx - 1]`.
fn midi_port_at(ports: &[String], selected: usize) -> Option<String> {
    if selected == 0 {
        None
    } else {
        ports.get(selected - 1).cloned()
    }
}

/// Everything woodshed does after an input dispatch.
pub fn after_dispatch(shared: &mut Shared, ctx: &mut Ctx<'_>) {
    // First, so that everything below sees the session the chosen persona's
    // key opened rather than the empty one that stood in for it.
    crate::persona::after_dispatch(shared, ctx);
    crate::appearance::after_dispatch(ctx);
    // Graph motion changes only transient projection state. Pointer Down and
    // Move leave this set; Up clears it before this hook and therefore runs the
    // ordinary tail once. Skipping here avoids three retained-root rebuilds,
    // two serializations, audio/MIDI polling, and two synchronous store writes
    // for every sampled position while preserving the existing commit path at
    // gesture release.
    if ctx.runner.state().set_graph_drag_active {
        shared.view_only_dispatches = shared.view_only_dispatches.saturating_add(1);
        return;
    }
    shared.full_dispatch_syncs = shared.full_dispatch_syncs.saturating_add(1);
    push_backend(shared, ctx);
    // Window verbs used to be drained here from four `UiState` flags. The
    // host owns them now: the caption buttons call `WindowCommands` directly
    // and the title bar drags from its `--app-region` declaration.
    midi_devices(shared, ctx);
}

/// Sync dropdown state into the core, push the audio state through the backend
/// seam, capture the skin settings, and persist.
fn push_backend(shared: &mut Shared, ctx: &mut Ctx<'_>) {
    let mut sheet = None;
    let mut persisted: Option<String> = None;
    let mut persisted_settings: Option<String> = None;

    // Set/Card ids are session-local. Every reset/open, including reopening
    // the same persona, releases PCM custody before admitting incoming ids.
    let audio_session = ctx.runner.state().audio_session.clone();
    if shared
        .last_audio_session
        .as_ref()
        .is_none_or(|last| !std::sync::Arc::ptr_eq(last, &audio_session))
    {
        if let Some(backend) = shared.backend.as_mut() {
            backend.clear_loop_session();
        }
        shared.last_song = None;
        shared.last_loop_plan = None;
        shared.last_audio_session = Some(audio_session);
    }
    if let Some(backend) = shared.backend.as_mut() {
        // Retire capture/preview work before any source admission or command.
        backend.service();
    }
    let backend = shared.backend.as_mut();
    let last_song = &mut shared.last_song;
    let last_loop_plan = &mut shared.last_loop_plan;
    let storage = shared.storage.as_ref();
    ctx.runner.update(|ui| {
        ui.sync();
        let appearance_attempted = woodshed_views::appearance::commit_selection(ui, |settings| {
            let store = storage.ok_or("Choose a persona before saving appearance.")?;
            let json = serde_json::to_string(settings).map_err(|error| error.to_string())?;
            store
                .try_save_settings(&json)
                .map_err(|error| error.to_string())
        });
        if let Some(backend) = backend {
            // Calibration owns the shared runtime's click lane during a run.
            if !ui.calib_active {
                backend.set_metronome(ui.transport);
            }
            backend.set_tuner_enabled(ui.tuner.enabled);
            admit_playback(ui, backend, last_song, last_loop_plan);
            backend.set_song_transport(ui.song_playing);
            // Every one-shot command the views queued this frame, realized in
            // the order the user asked for it. Draining here is the only place
            // a request is consumed, so none can be left set or cleared twice.
            let mut capture_changed = false;
            for request in std::mem::take(&mut ui.audio_requests) {
                match request {
                    AudioRequest::SongRewind => backend.song_rewind(),
                    AudioRequest::CancelPreview => backend.cancel_preview(),
                    AudioRequest::PreviewVoicing => {
                        let (pitches, dur, strum) = ui.preview_voicing();
                        if !pitches.is_empty() {
                            backend.preview_pitches(&pitches, dur, strum);
                        }
                    },
                    AudioRequest::PreviewPitches {
                        pitches,
                        duration_s,
                        strum_s,
                    } => {
                        if !pitches.is_empty() {
                            backend.preview_pitches(&pitches, duration_s, strum_s);
                        }
                    },
                    AudioRequest::PreviewNote(freq) => backend.preview_note(freq, 0.9),
                    AudioRequest::CalibrationStart => {
                        backend.calibration_start();
                        ui.calib_status = backend.calibration_poll();
                        ui.calib_active =
                            matches!(ui.calib_status, CalibrationStatus::Running { .. });
                    },
                    AudioRequest::CalibrationCancel => {
                        backend.calibration_cancel();
                        ui.calib_active = false;
                        ui.calib_status = CalibrationStatus::Idle;
                    },
                    AudioRequest::CalibrationAccept => {
                        if let CalibrationStatus::Success { latency_ms, .. } = ui.calib_status {
                            backend.set_latency_ms(Some(latency_ms));
                        }
                        ui.calib_status = CalibrationStatus::Idle;
                    },
                    AudioRequest::SongRecordToggle
                        if ui.looper.source == LooperSource::SavedSong
                            && last_loop_plan.is_none()
                            && last_song.is_some() =>
                    {
                        if ui.song_recording {
                            backend.loop_stop_record();
                        } else {
                            backend.song_arm_record(ui.song_edit_cursor);
                        }
                    },
                    AudioRequest::SongClearLoop
                        if ui.looper.source == LooperSource::SavedSong
                            && last_loop_plan.is_none()
                            && last_song.is_some() =>
                    {
                        backend.song_clear_loop(ui.song_edit_cursor)
                    },
                    AudioRequest::SongRecordToggle | AudioRequest::SongClearLoop => {
                        ui.loop_notice = Some(
                            "Load saved backing before recording or clearing a saved bar.".into(),
                        );
                    },
                    AudioRequest::LoopRecordToggle(id) => {
                        let result = if last_loop_plan
                            .as_ref()
                            .is_none_or(|plan| plan.index_of(id).is_none())
                        {
                            Err("That Card is not in the currently loaded backing.".into())
                        } else if backend.loop_capture_busy() {
                            backend.loop_stop_record();
                            capture_changed = true;
                            Ok(())
                        } else {
                            backend.loop_arm_record(id)
                        };
                        if let Err(reason) = result {
                            ui.loop_notice = Some(reason);
                        }
                    },
                    AudioRequest::LoopClear(id) => {
                        if last_loop_plan.is_none() {
                            ui.loop_notice =
                                Some("Choose Set backing before clearing a Card loop.".into());
                        } else {
                            match backend.loop_clear(id) {
                                Ok(()) => capture_changed = true,
                                Err(reason) => ui.loop_notice = Some(reason),
                            }
                        }
                    },
                }
            }
            if capture_changed {
                admit_playback(ui, backend, last_song, last_loop_plan);
                backend.set_song_transport(ui.song_playing);
            }
            ui.latency_ms = backend.latency_ms();
            backend.song_set_record_replace(ui.song_record_replace);
            // Source controls must include delayed legacy capture as well.
            ui.song_recording = backend.loop_capture_busy();
            ui.song_loop_bars = backend.song_loop_bars();
            ui.audio_error = backend.error().map(str::to_owned);
        }
        sheet = Some(crate::appearance::stylesheet(ui));
        persisted = serde_json::to_string(&ui.to_persisted()).ok();
        if !appearance_attempted {
            persisted_settings = serde_json::to_string(&ui.app_settings).ok();
        }
    });

    if let Some(sheet) = sheet.and_then(|sheet| shared.reskin_sheet(sheet)) {
        // The host takes it from here: a new sheet forces a full relayout.
        *ctx.set_sheet = Some(sheet);
    }
    // No store while the persona gate is up, and nothing worth saving either:
    // what would be written is the empty state standing in for a session that
    // has not been read.
    let Some(storage) = shared.storage.as_ref() else {
        return;
    };
    if let Some(json) = persisted {
        storage.save(&json);
    }
    if let Some(json) = persisted_settings {
        storage.save_settings(&json);
    }
}

/// Connect / disconnect per the dropdowns, and re-scan the port lists on
/// request.
fn midi_devices(shared: &mut Shared, ctx: &mut Ctx<'_>) {
    let midi = &mut shared.midi;
    ctx.runner.update(|ui| {
        if std::mem::take(&mut ui.midi.refresh_requested) {
            ui.midi.input_ports = midi.input_ports();
            ui.midi.output_ports = midi.output_ports();
        }
        let in_target = midi_port_at(&ui.midi.input_ports, ui.midi.input_dd.selected);
        if midi.connected_input() != in_target.as_deref() {
            midi.connect_input(in_target.as_deref());
        }
        let out_target = midi_port_at(&ui.midi.output_ports, ui.midi.output_dd.selected);
        if midi.connected_output() != out_target.as_deref() {
            midi.connect_output(out_target.as_deref());
        }
        ui.midi.connected_in = midi.connected_input().map(str::to_string);
        ui.midi.connected_out = midi.connected_output().map(str::to_string);
    });
}

/// Publish the admitted snapshot to the view. A refused edit must not make the
/// timeline or capture controls describe a plan the engine never loaded.
fn admit_playback(
    ui: &mut UiState,
    backend: &mut impl AudioBackend,
    last_song: &mut Option<SongDoc>,
    last_loop_plan: &mut Option<LoopPlan>,
) {
    let loaded_source = last_loop_plan
        .as_ref()
        .map_or(LooperSource::SavedSong, |plan| {
            LooperSource::Set(plan.owner)
        });
    if ui.looper.source != loaded_source && backend.loop_capture_busy() {
        ui.looper.source = loaded_source;
        ui.loop_admitted = last_loop_plan.clone();
        ui.loop_notice = Some("Stop recording / armed capture before changing backing source. The loaded backing remains active.".into());
        return;
    }
    match ui.looper.source {
        LooperSource::SavedSong => {
            if last_loop_plan.is_some() || last_song.as_ref() != Some(&ui.song) {
                match backend.try_set_song(&ui.song) {
                    Ok(()) => {
                        *last_song = Some(ui.song.clone());
                        *last_loop_plan = None;
                        ui.loop_admitted = None;
                        ui.loop_notice = None;
                    },
                    Err(reason) => {
                        ui.looper.source = loaded_source;
                        ui.loop_admitted = last_loop_plan.clone();
                        ui.loop_notice = Some(reason);
                        if last_song.is_none() && last_loop_plan.is_none() {
                            ui.song_playing = false;
                        }
                    },
                }
            }
        },
        LooperSource::Set(_) => {
            match ui.requested_loop_plan() {
                Ok(plan) => {
                    if last_loop_plan.as_ref() != Some(&plan) {
                        match backend.set_loop_plan(&plan) {
                            Ok(()) => {
                                *last_loop_plan = Some(plan);
                                *last_song = None;
                                ui.loop_notice = None;
                                ui.song_bar_live = 0;
                            },
                            Err(reason) => ui.loop_notice = Some(reason),
                        }
                    }
                },
                Err(_) => ui.loop_notice = Some("The selected Set is not ready. The loaded backing remains unchanged; open a Card below to resolve its readiness.".into()),
            }
            ui.loop_admitted = last_loop_plan.clone();
            if let Some(plan) = last_loop_plan.as_ref() {
                if ui
                    .loop_selected
                    .is_none_or(|id| plan.index_of(id).is_none())
                {
                    ui.loop_selected = plan.segments.first().map(|segment| segment.id);
                }
            }
            if last_loop_plan.is_none() && last_song.is_none() {
                ui.song_playing = false;
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use woodshed_core::audio::{TransportState, TunerReading};
    use woodshedding::rehearsal::{Hold, Set};

    #[derive(Default)]
    struct Backend {
        admitted: Option<LoopPlan>,
        songs: Vec<SongDoc>,
        reject: bool,
        busy: bool,
    }
    impl AudioBackend for Backend {
        fn set_metronome(&mut self, _: TransportState) {}
        fn set_tuner_enabled(&mut self, _: bool) {}
        fn tuner_reading(&self) -> Option<TunerReading> {
            None
        }
        fn set_song(&mut self, doc: &SongDoc) {
            self.songs.push(doc.clone());
            self.admitted = None;
        }
        fn try_set_song(&mut self, doc: &SongDoc) -> Result<(), String> {
            if self.busy {
                return Err(
                    "Stop recording / armed capture before replacing saved backing.".into(),
                );
            }
            if self.reject {
                return Err("Output is unavailable.".into());
            }
            self.set_song(doc);
            Ok(())
        }
        fn set_loop_plan(&mut self, plan: &LoopPlan) -> Result<(), String> {
            if self.reject {
                return Err("Stop armed recording".into());
            }
            self.admitted = Some(plan.clone());
            Ok(())
        }
        fn set_song_transport(&mut self, _: bool) {}
        fn loop_capture_busy(&self) -> bool {
            self.busy
        }
        fn song_rewind(&mut self) {}
        fn song_bar(&self) -> Option<usize> {
            None
        }
        fn error(&self) -> Option<&str> {
            None
        }
    }

    fn ready_ui() -> UiState {
        let mut ui = UiState::new();
        ui.song.bars.push(Default::default());
        ui.stage.set_lens(woodshed_core::Lens::Chords);
        let mut card = ui.stage.card_from_lens().unwrap();
        card.timing.hold = Hold::Bars(1);
        ui.set.push(card);
        ui.loop_working_set(ui.working_sets.active_id);
        ui
    }

    #[test]
    fn refused_edit_and_other_owner_keep_the_actual_admitted_timeline() {
        let mut ui = ready_ui();
        let mut backend = Backend::default();
        let (mut song, mut plan) = (None, None);
        admit_playback(&mut ui, &mut backend, &mut song, &mut plan);
        let loaded = plan.clone().unwrap();
        ui.song_playing = true;
        ui.set.cards[0].label = "pending edit".into();
        backend.reject = true;
        admit_playback(&mut ui, &mut backend, &mut song, &mut plan);
        assert_eq!(ui.loop_admitted, Some(loaded.clone()));
        assert_eq!(backend.admitted, Some(loaded.clone()));
        assert!(ui.loop_notice.as_deref().unwrap().contains("armed"));
        assert!(ui.song_playing);
        let mut other = Set::default();
        other.push(ui.set.cards[0].clone());
        let owner = ui.working_sets.create(&mut ui.set, "Other", other);
        ui.loop_working_set(owner);
        admit_playback(&mut ui, &mut backend, &mut song, &mut plan);
        assert_eq!(ui.loop_admitted, Some(loaded));
        assert_ne!(
            ui.requested_loop_plan().unwrap().owner,
            plan.as_ref().unwrap().owner
        );
        backend.reject = false;
        admit_playback(&mut ui, &mut backend, &mut song, &mut plan);
        assert_eq!(ui.loop_admitted.as_ref().unwrap().owner, owner);
        assert!(ui.loop_notice.is_none());
    }

    #[test]
    fn saved_bar_edit_refusal_preserves_accepted_payload_and_capture_state() {
        let mut ui = ready_ui();
        ui.looper.source = LooperSource::SavedSong;
        let mut backend = Backend::default();
        let (mut song, mut plan) = (None, None);
        admit_playback(&mut ui, &mut backend, &mut song, &mut plan);
        let accepted = song.clone();
        let submissions = backend.songs.clone();
        ui.song_playing = true;
        ui.song.bars[0].pitches_hz = vec![217.3, 281.6, 336.2];
        ui.song.bars[0].beats = 5;
        ui.song.bars[0].length = 3;
        backend.busy = true;
        admit_playback(&mut ui, &mut backend, &mut song, &mut plan);
        assert_eq!(song, accepted);
        assert_eq!(backend.songs, submissions);
        assert!(backend.busy && ui.song_playing);
        assert!(ui.loop_notice.as_deref().unwrap().contains("armed"));
        assert!(plan.is_none() && ui.loop_admitted.is_none());
        backend.busy = false;
        admit_playback(&mut ui, &mut backend, &mut song, &mut plan);
        assert_eq!(song, Some(ui.song.clone()));
        assert_eq!(backend.songs.last(), Some(&ui.song));
        assert!(ui.loop_notice.is_none());
    }

    #[test]
    fn unavailable_saved_replacement_keeps_loaded_set_and_an_unready_set_keeps_saved_backing() {
        let mut ui = ready_ui();
        let mut backend = Backend::default();
        let (mut song, mut plan) = (None, None);
        admit_playback(&mut ui, &mut backend, &mut song, &mut plan);
        let accepted = plan.clone();
        ui.song_playing = true;
        ui.looper.source = LooperSource::SavedSong;
        backend.reject = true;
        admit_playback(&mut ui, &mut backend, &mut song, &mut plan);
        assert_eq!(plan, accepted);
        assert_eq!(ui.loop_admitted, accepted);
        assert_eq!(backend.admitted, accepted);
        assert_eq!(ui.looper.source, LooperSource::Set(accepted.unwrap().owner));
        assert!(song.is_none() && backend.songs.is_empty() && ui.song_playing);
        assert!(ui.loop_notice.as_deref().unwrap().contains("unavailable"));

        backend.reject = false;
        ui.looper.source = LooperSource::SavedSong;
        admit_playback(&mut ui, &mut backend, &mut song, &mut plan);
        let saved = song.clone();
        ui.loop_working_set(ui.working_sets.active_id);
        ui.set.cards[0].timing.hold = Hold::Manual;
        admit_playback(&mut ui, &mut backend, &mut song, &mut plan);
        assert!(plan.is_none() && ui.loop_admitted.is_none());
        assert_eq!(song, saved);
        assert!(ui.song_playing);
        assert!(ui.loop_notice.as_deref().unwrap().contains("not ready"));
    }

    #[test]
    fn unready_initial_source_cannot_play_and_saved_song_restores_exact_data() {
        let mut ui = ready_ui();
        ui.set.cards[0].timing.hold = Hold::Manual;
        ui.song_playing = true;
        let mut backend = Backend::default();
        let (mut song, mut plan) = (None, None);
        admit_playback(&mut ui, &mut backend, &mut song, &mut plan);
        assert!(plan.is_none());
        assert!(!ui.song_playing);
        ui.looper.source = LooperSource::SavedSong;
        ui.song.bars[0].pitches_hz = vec![261.111, 399.777];
        ui.song.bars[0].beats = 5;
        ui.song.bars[0].length = 3;
        admit_playback(&mut ui, &mut backend, &mut song, &mut plan);
        assert_eq!(backend.songs, vec![ui.song.clone()]);
        assert!(ui.loop_admitted.is_none());
    }

    #[test]
    fn loop_source_reopens_in_a_fresh_process() {
        let dir = tempfile::tempdir().unwrap();
        for mode in ["seed", "verify"] {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "sync::tests::loop_source_receipt_worker",
                    "--nocapture",
                ])
                .env("WOODSHED_LOOP_WORKER", mode)
                .env("WOODSHED_STATE", dir.path().join("practice.json"))
                .env("WOODSHED_SETTINGS", dir.path().join("settings.json"))
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{mode}: {}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(String::from_utf8_lossy(&output.stdout).contains("loop-source PASS"));
        }
    }

    #[test]
    fn loop_source_receipt_worker() {
        let Ok(mode) = std::env::var("WOODSHED_LOOP_WORKER") else {
            return;
        };
        let backend: crate::storage::HostBackend = Box::<crate::storage::FsBackend>::default();
        let store = woodshed_core::storage::SessionStore::new(backend);
        let mut expected = ready_ui();
        expected.looper.beats = 3;
        expected.looper.inherited_bpm = 98.75;
        expected.song.bars[0].pitches_hz = vec![267.125, 409.875];
        expected.song.bars[0].beats = 7;
        expected.song.bars[0].length = 2;
        let expected_plan = expected.requested_loop_plan().unwrap();
        if mode == "seed" {
            store.save(&serde_json::to_string(&expected.to_persisted()).unwrap());
            store.save_settings(&serde_json::to_string(&expected.app_settings).unwrap());
            assert!(store.load().is_some());
        } else {
            assert_eq!(mode, "verify");
            let mut reopened = UiState::new();
            let generation = reopened.audio_session.clone();
            crate::session::restore(&store, &mut reopened);
            assert!(!std::sync::Arc::ptr_eq(
                &generation,
                &reopened.audio_session
            ));
            assert_eq!(reopened.looper, expected.looper);
            assert_eq!(reopened.requested_loop_plan().unwrap(), expected_plan);
            assert_eq!(reopened.song, expected.song);
            assert!(!reopened.song_playing);
            assert!(reopened.loop_admitted.is_none());
            let mut backend = Backend::default();
            let (mut song, mut plan) = (None, None);
            admit_playback(&mut reopened, &mut backend, &mut song, &mut plan);
            assert_eq!(backend.admitted, Some(expected_plan));
            assert!(backend.songs.is_empty());
        }
        println!("loop-source PASS {mode}");
    }

    #[test]
    fn looper_controls_show_readiness_route_owned_capture_and_retain_saved_backing() {
        use cambium_genet_winit_host::Harness;
        use taproot::Selector;
        use woodshed_views::stage::stage_root;
        for scenario in [
            include_str!("../../../scenarios/set_loop_plan.scn"),
            include_str!("../../../scenarios/set_loop_plan_reopen.scn"),
        ] {
            taproot::Scenario::parse(scenario).expect("Set backing native scenario grammar");
        }
        let mut ui = ready_ui();
        ui.looper.source = LooperSource::SavedSong;
        ui.song.bars[0].pitches_hz = vec![258.123, 417.875];
        let saved = ui.song.clone();
        ui.set.cards[0].timing.hold = Hold::Manual;
        ui.select_app_section(woodshed_core::storage::AppSection::Looper);
        ui.set_viewport_width(1100.0);
        ui.set_viewport_height(900.0);
        let logic: Logic = Box::new(stage_root);
        let mut h = Harness::new(woodshed_views::theme::slate_stage_css(), ui, logic);
        h.layout_at(1100.0, 900.0);
        assert!(h.click_on(&Selector::class("t-btn").containing("Loop this Set")));
        let mut backend = Backend::default();
        let (mut song, mut plan) = (None, None);
        h.update(|ui| admit_playback(ui, &mut backend, &mut song, &mut plan));
        {
            let dom = h.runner().dom();
            let dom = dom.borrow();
            assert!(
                !taproot::matching(
                    &dom,
                    &Selector::class("t-readout").containing("Choose a positive Bars hold")
                )
                .is_empty()
            );
        }
        assert!(h.click_on(&Selector::class("t-btn").containing("Play")));
        assert!(!h.state().song_playing);
        h.update(|ui| {
            ui.set.cards[0].timing.hold = Hold::Bars(1);
            admit_playback(ui, &mut backend, &mut song, &mut plan);
        });
        let first = plan.as_ref().unwrap().segments[0].id;
        assert!(h.click_on(&Selector::class("t-btn").containing("Clock: 4/4")));
        assert_eq!(h.state().looper.beats, 5);
        assert!(h.click_on(&Selector::class("t-btn").containing("Tempo +")));
        assert_eq!(h.state().looper.inherited_bpm, 125.0);
        let repeat = Selector::role("button").with_attr("data-action", "toggle-set-one-shot");
        assert!(h.click_on(&repeat));
        assert!(h.state().looper.one_shot);
        h.update(|ui| admit_playback(ui, &mut backend, &mut song, &mut plan));
        assert!(plan.as_ref().unwrap().one_shot);
        h.update(|ui| {
            let mut other = ui.set.cards[0].clone();
            other.label = "other Card".into();
            ui.set.push(other);
            ui.set.cards.reverse();
            admit_playback(ui, &mut backend, &mut song, &mut plan);
        });
        assert_eq!(h.state().loop_selected, Some(first));
        assert_eq!(plan.as_ref().unwrap().index_of(first), Some(1));
        assert!(h.click_on(&Selector::class("t-btn").containing("Rec Card")));
        assert_eq!(
            h.state().audio_requests,
            vec![AudioRequest::LoopRecordToggle(first)]
        );
        assert!(h.click_on(&Selector::class("t-btn").containing("Saved backing")));
        h.update(|ui| admit_playback(ui, &mut backend, &mut song, &mut plan));
        assert_eq!(h.state().looper.source, LooperSource::SavedSong);
        assert_eq!(h.state().song, saved);
        assert!(h.state().loop_admitted.is_none());
    }

    #[test]
    fn armed_source_switches_are_refused_in_both_directions_without_replacing_backing() {
        let mut ui = ready_ui();
        let mut backend = Backend::default();
        let (mut song, mut plan) = (None, None);
        admit_playback(&mut ui, &mut backend, &mut song, &mut plan);
        let loaded = plan.clone();
        ui.song_playing = true;
        backend.busy = true;
        ui.looper.source = LooperSource::SavedSong;
        admit_playback(&mut ui, &mut backend, &mut song, &mut plan);
        assert_eq!(
            ui.looper.source,
            LooperSource::Set(loaded.as_ref().unwrap().owner)
        );
        assert_eq!(ui.loop_admitted, loaded);
        assert_eq!(backend.admitted, loaded);
        assert!(backend.songs.is_empty());
        assert!(backend.busy && ui.song_playing);
        assert!(ui.loop_notice.as_deref().unwrap().contains("armed"));
        backend.busy = false;
        ui.looper.source = LooperSource::SavedSong;
        admit_playback(&mut ui, &mut backend, &mut song, &mut plan);
        let saved = backend.songs.clone();
        let last_saved = song.clone();
        backend.busy = true;
        ui.looper.source = LooperSource::Set(ui.working_sets.active_id);
        admit_playback(&mut ui, &mut backend, &mut song, &mut plan);
        assert_eq!(ui.looper.source, LooperSource::SavedSong);
        assert!(plan.is_none() && backend.admitted.is_none());
        assert_eq!(song, last_saved);
        assert_eq!(backend.songs, saved);
        assert!(backend.busy && ui.song_playing);
        assert!(ui.loop_notice.as_deref().unwrap().contains("armed"));
    }

    #[test]
    fn source_controls_refuse_known_armed_capture_in_both_directions() {
        use cambium_genet_winit_host::Harness;
        use taproot::Selector;
        use woodshed_views::stage::stage_root;
        let mut ui = ready_ui();
        ui.song_recording = true;
        ui.song_playing = true;
        let original_source = ui.looper.source;
        ui.select_app_section(woodshed_core::storage::AppSection::Looper);
        ui.set_viewport_width(1100.0);
        ui.set_viewport_height(900.0);
        let logic: Logic = Box::new(stage_root);
        let mut h = Harness::new(woodshed_views::theme::slate_stage_css(), ui, logic);
        h.layout_at(1100.0, 900.0);
        assert!(h.click_on(&Selector::class("t-btn").containing("Saved backing")));
        assert_eq!(h.state().looper.source, original_source);
        assert!(h.state().song_recording && h.state().song_playing);
        h.update(|ui| ui.looper.source = LooperSource::SavedSong);
        assert!(h.click_on(&Selector::class("t-btn").containing("Loop this Set")));
        assert_eq!(h.state().looper.source, LooperSource::SavedSong);
        assert!(h.state().song_recording && h.state().song_playing);
        assert!(h.state().loop_notice.as_deref().unwrap().contains("armed"));
    }
}
