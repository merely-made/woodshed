use super::*;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use woodshed_core::audio::{TransportState, TunerReading};
use woodshed_core::personal_relations::{PersonalRelationSource, PersonalRelationSourceId};
use woodshed_core::storage::SessionStore;
use woodshedding::rehearsal::{Hold, Set};

#[derive(Default)]
struct RoutingBackend {
    accepted: Option<String>,
    completion: Option<String>,
    requested: Option<String>,
    error: Option<String>,
}
impl woodshed_core::midi::MidiBackend for RoutingBackend {
    fn poll(&mut self) {
        if let Some(name) = self.completion.take() {
            self.accepted = Some(name);
        }
    }
    fn input_ports(&self) -> Vec<String> {
        vec![]
    }
    fn output_ports(&self) -> Vec<String> {
        vec!["B".into(), "A".into()]
    }
    fn connect_input(&mut self, _: Option<&str>) {}
    fn connect_output(&mut self, port: Option<&str>) {
        self.requested = port.map(str::to_string);
    }
    fn connected_input(&self) -> Option<&str> {
        None
    }
    fn connected_output(&self) -> Option<&str> {
        self.accepted.as_deref()
    }
    fn clock_bpm(&self) -> Option<f32> {
        None
    }
    fn recent_events(&self) -> Vec<String> {
        vec![]
    }
    fn set_clock_out(&mut self, _: bool, _: bool, _: f32) {}
    fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

struct RejectingSessionBackend(Arc<Mutex<BTreeMap<String, Vec<u8>>>>);

#[async_trait::async_trait]
impl muniment::Backend for RejectingSessionBackend {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, muniment::StoreError> {
        Ok(self.0.lock().unwrap().get(key).cloned())
    }
    async fn put(&self, _key: &str, _bytes: &[u8]) -> Result<(), muniment::StoreError> {
        Err(muniment::StoreError::Backend(
            "write refused for test".into(),
        ))
    }
    async fn delete(&self, key: &str) -> Result<(), muniment::StoreError> {
        self.0.lock().unwrap().remove(key);
        Ok(())
    }
    async fn list(&self, prefix: &str) -> Result<Vec<String>, muniment::StoreError> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .keys()
            .filter(|key| key.starts_with(prefix))
            .cloned()
            .collect())
    }
    async fn scan(&self, start: &str, end: &str) -> Result<Vec<String>, muniment::StoreError> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .range(start.to_owned()..end.to_owned())
            .map(|(key, _)| key.clone())
            .collect())
    }
    async fn apply(&self, _ops: &[muniment::backend::WriteOp]) -> Result<(), muniment::StoreError> {
        Err(muniment::StoreError::Backend(
            "write refused for test".into(),
        ))
    }
}

struct WriteThenReadFailureBackend {
    bytes: Arc<Mutex<BTreeMap<String, Vec<u8>>>>,
    writes: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl muniment::Backend for WriteThenReadFailureBackend {
    async fn get(&self, _key: &str) -> Result<Option<Vec<u8>>, muniment::StoreError> {
        Err(muniment::StoreError::Backend(
            "readback unavailable after write".into(),
        ))
    }
    async fn put(&self, key: &str, bytes: &[u8]) -> Result<(), muniment::StoreError> {
        self.bytes
            .lock()
            .unwrap()
            .insert(key.to_owned(), bytes.to_vec());
        self.writes.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    async fn delete(&self, key: &str) -> Result<(), muniment::StoreError> {
        self.bytes.lock().unwrap().remove(key);
        Ok(())
    }
    async fn list(&self, prefix: &str) -> Result<Vec<String>, muniment::StoreError> {
        Ok(self
            .bytes
            .lock()
            .unwrap()
            .keys()
            .filter(|key| key.starts_with(prefix))
            .cloned()
            .collect())
    }
    async fn scan(&self, start: &str, end: &str) -> Result<Vec<String>, muniment::StoreError> {
        Ok(self
            .bytes
            .lock()
            .unwrap()
            .range(start.to_owned()..end.to_owned())
            .map(|(key, _)| key.clone())
            .collect())
    }
    async fn apply(&self, _ops: &[muniment::backend::WriteOp]) -> Result<(), muniment::StoreError> {
        Err(muniment::StoreError::Backend(
            "batch writes are unavailable in this test".into(),
        ))
    }
}

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
            return Err("Stop recording / armed capture before replacing saved backing.".into());
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
            .env("WOODSHED_DATA_DIR", dir.path())
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
    let directory = std::path::PathBuf::from(
        std::env::var_os("WOODSHED_DATA_DIR").expect("isolated application data root required"),
    )
    .join("fixed-test-owner");
    let backend: crate::storage::HostBackend = Box::new(crate::storage::FsBackend::at(&directory));
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
        include_str!("../../../../scenarios/set_loop_plan.scn"),
        include_str!("../../../../scenarios/set_loop_plan_reopen.scn"),
        include_str!("../../../../scenarios/claims_seed.scn"),
        include_str!("../../../../scenarios/claims_reopen.scn"),
    ] {
        taproot::Scenario::parse(scenario).expect("native session scenario grammar");
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
            !taproot::matching_classes(
                &dom,
                &Selector::class("t-readout").containing("Choose positive Bars or finite Seconds")
            )
            .is_empty()
        );
    }
    assert!(h.click_on(&Selector::role("button").with_attr("aria-label", "Play backing")));
    assert!(!h.state().song_playing);
    h.update(|ui| {
        ui.set.cards[0].timing.hold = Hold::Bars(1);
        admit_playback(ui, &mut backend, &mut song, &mut plan);
    });
    let first = plan.as_ref().unwrap().segments[0].id;
    assert!(h.click_on(&Selector::class("t-btn").containing("Clock: 4/4")));
    assert_eq!(h.state().looper.beats, 5);
    assert!(h.click_on(&Selector::role("button").with_attr("aria-label", "Increase tempo")));
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
    h.update(|ui| {
        ui.recorder_observation = woodshed_views::stage::RecorderObservation {
            source: plan.as_ref().map(|plan| LooperSource::Set(plan.owner)),
            plan: plan.clone(),
            output_rate: Some(48000),
            ..Default::default()
        };
    });
    assert!(h.click_on(&Selector::role("switch").with_attr("aria-label", "Record")));
    assert!(
        matches!(&h.state().audio_requests[..],[AudioRequest::Recorder(request)] if request.command==woodshed_core::audio::RecorderCommand::RecordCard(first))
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

#[path = "tests/midi.rs"]
mod midi;

#[path = "tests/relations.rs"]
mod relations;
