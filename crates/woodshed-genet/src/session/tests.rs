use super::*;
use muniment::{Backend, MemoryBackend, StoreError, WriteOp};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use woodshed_core::loop_capture::{
    CAPTURE_VERSION, LoopCaptureCodec, LoopCaptureRef, PER_BEAT_CLOCK,
};
use woodshed_core::loop_plan::LoopSegmentId;
use woodshed_core::working_sets::WorkingSetId;
use woodshed_views::workspace::WorkspacePanel;
use woodshedding::rehearsal::CardId;

const NAMESPACE: &str = "0123456789abcdef0123456789abcdef";

fn filesystem_fixture_store() -> SessionStore<HostBackend> {
    let root = std::path::PathBuf::from(
        std::env::var_os("WOODSHED_DATA_DIR").expect("isolated session fixture root"),
    );
    let backend: HostBackend = Box::new(crate::storage::FsBackend::at(
        root.join("personas/v1").join("1".repeat(64)),
    ));
    SessionStore::new(backend)
}

struct ReadOnlyBackend {
    inner: MemoryBackend,
    capture_reads: Arc<AtomicUsize>,
    damage_capture: bool,
}

#[async_trait::async_trait]
impl Backend for ReadOnlyBackend {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StoreError> {
        let mut bytes = self.inner.get(key).await?;
        if key.starts_with("loop-pcm/") {
            self.capture_reads.fetch_add(1, Ordering::Relaxed);
            if self.damage_capture {
                if let Some(bytes) = bytes.as_mut() {
                    *bytes.last_mut().unwrap() ^= 1;
                }
            }
        }
        Ok(bytes)
    }

    async fn put(&self, _: &str, _: &[u8]) -> Result<(), StoreError> {
        panic!("incoming admission must not write")
    }

    async fn delete(&self, _: &str) -> Result<(), StoreError> {
        panic!("incoming admission must not delete")
    }

    async fn list(&self, prefix: &str) -> Result<Vec<String>, StoreError> {
        self.inner.list(prefix).await
    }

    async fn scan(&self, start: &str, end: &str) -> Result<Vec<String>, StoreError> {
        self.inner.scan(start, end).await
    }

    async fn apply(&self, _: &[WriteOp]) -> Result<(), StoreError> {
        panic!("incoming admission must not apply writes")
    }
}

fn read_only_store(
    inner: MemoryBackend,
    damage_capture: bool,
) -> (SessionStore<HostBackend>, Arc<AtomicUsize>) {
    let capture_reads = Arc::new(AtomicUsize::new(0));
    let backend: HostBackend = Box::new(ReadOnlyBackend {
        inner,
        capture_reads: capture_reads.clone(),
        damage_capture,
    });
    (SessionStore::new(backend), capture_reads)
}

fn capture_fixture() -> (MemoryBackend, UiState) {
    let inner = MemoryBackend::default();
    let seed = SessionStore::new(inner.clone());
    let samples = vec![0.25; 2_000];
    let (digest, frames) = seed.try_write_capture(NAMESPACE, 1_000, &samples).unwrap();
    let mut ui = UiState::new();
    ui.stage_current(None);
    ui.loop_captures.version = CAPTURE_VERSION;
    ui.loop_captures.namespace = Some(NAMESPACE.into());
    ui.loop_captures.takes.push(LoopCaptureRef {
        id: LoopSegmentId {
            set: ui.working_sets.active_id,
            card: ui.set.cards[0].id,
        },
        bpm: 120.0,
        beats: 4,
        bars: 1,
        sample_rate: 1_000,
        frames,
        digest,
        codec: LoopCaptureCodec::RawF32V1,
        clock: Some(PER_BEAT_CLOCK.into()),
        seconds: None,
    });
    seed.try_save(&serde_json::to_string(&ui.to_persisted()).unwrap())
        .unwrap();
    (inner, ui)
}

#[test]
fn pending_custody_reports_known_nonwriting_instead_of_uncertain_durability() {
    let storage = SessionStore::new(Box::new(MemoryBackend::default()) as HostBackend);
    storage.session_unconfirmed("a write was refused");
    let mut ui = UiState::new();
    ui.seal = Some(woodshed_views::persona::PracticeSeal::Pending {
        reason: "the vault is locked".into(),
    });
    refresh_save_status(&storage, &mut ui);
    let notice = ui.practice_save_notice.as_deref().unwrap();
    assert!(notice.contains("not being saved") && notice.contains("vault is locked"));
    assert!(!notice.contains("unconfirmed") && !notice.contains("may already"));
    assert!(!ui.practice_saved);
    ui.seal = Some(woodshed_views::persona::PracticeSeal::Sealed {
        persona: "default".into(),
        protection: "test".into(),
    });
    refresh_save_status(&storage, &mut ui);
    assert!(
        ui.practice_save_notice
            .as_deref()
            .unwrap()
            .contains("unconfirmed")
    );
}

#[test]
fn incoming_admission_prepares_complete_state_without_writes_or_source_minting() {
    let (inner, mut expected) = capture_fixture();
    expected.app_settings.appearance.theme = "Ember".into();
    let seed = SessionStore::new(inner.clone());
    seed.try_save_settings(&serde_json::to_string(&expected.app_settings).unwrap())
        .unwrap();
    let original = seed.load().unwrap();
    let keys = inner.len();
    let (incoming, reads) = read_only_store(inner.clone(), false);
    let admitted = admit(&incoming).unwrap();
    assert_eq!(admitted.app_settings, expected.app_settings);
    assert_eq!(
        serde_json::to_value(&admitted.set).unwrap(),
        serde_json::to_value(&expected.set).unwrap()
    );
    assert_eq!(admitted.loop_captures, expected.loop_captures);
    assert!(admitted.personal_relations.is_none());
    assert!(!admitted.rehearsal_running && !admitted.song_playing && !admitted.tuner.enabled);
    assert_eq!(reads.load(Ordering::Relaxed), 1);
    assert_eq!(seed.load().unwrap(), original);
    assert_eq!(inner.len(), keys);

    let (empty, _) = read_only_store(MemoryBackend::default(), false);
    let admitted = admit(&empty).unwrap();
    assert!(admitted.personal_relations.is_none());
    assert!(admitted.loop_captures.is_empty());
}

#[test]
fn incoming_admission_refuses_corrupt_slots_and_unknown_workspace() {
    for fault in ["settings", "session", "workspace"] {
        let inner = MemoryBackend::default();
        let seed = SessionStore::new(inner.clone());
        match fault {
            "settings" => seed.try_save_settings("not settings").unwrap(),
            "session" => seed.try_save("not a session").unwrap(),
            "workspace" => {
                let mut saved = UiState::new().to_persisted();
                saved.workspace_json = Some(r#"{"version":999,"future":"retained"}"#.into());
                seed.try_save(&serde_json::to_string(&saved).unwrap())
                    .unwrap();
            },
            _ => unreachable!(),
        }
        let original = (seed.load(), seed.load_settings());
        let (incoming, _) = read_only_store(inner, false);
        assert!(admit(&incoming).is_err(), "{fault}");
        assert!(incoming.write_refusal().is_some(), "{fault}");
        assert_eq!((seed.load(), seed.load_settings()), original);
    }
}

#[test]
fn incoming_admission_refuses_an_already_unconfirmed_store() {
    let (incoming, _) = read_only_store(MemoryBackend::default(), false);
    incoming.session_unconfirmed("earlier save could not be verified");
    assert!(admit(&incoming).err().unwrap().contains("earlier save"));
}

#[test]
fn incoming_admission_refuses_invalid_capture_manifest() {
    let (inner, mut ui) = capture_fixture();
    ui.loop_captures.version = 999;
    let seed = SessionStore::new(inner.clone());
    seed.try_save(&serde_json::to_string(&ui.to_persisted()).unwrap())
        .unwrap();
    let (incoming, reads) = read_only_store(inner, false);
    assert!(admit(&incoming).err().unwrap().contains("manifest version"));
    assert_eq!(reads.load(Ordering::Relaxed), 0);
}

#[test]
fn incoming_admission_reads_later_references_and_refuses_missing_pcm() {
    let (inner, mut ui) = capture_fixture();
    let mut missing = ui.loop_captures.takes[0].clone();
    missing.id = LoopSegmentId {
        set: WorkingSetId(2),
        card: CardId(7),
    };
    missing.digest = "0".repeat(64);
    ui.loop_captures.takes.push(missing);
    let seed = SessionStore::new(inner.clone());
    seed.try_save(&serde_json::to_string(&ui.to_persisted()).unwrap())
        .unwrap();
    let (incoming, reads) = read_only_store(inner, false);
    assert!(admit(&incoming).err().unwrap().contains("missing"));
    assert_eq!(reads.load(Ordering::Relaxed), 2);
}

#[test]
fn incoming_admission_refuses_damaged_pcm_digest() {
    let (inner, _) = capture_fixture();
    let (incoming, reads) = read_only_store(inner, true);
    assert!(admit(&incoming).err().unwrap().contains("integrity"));
    assert_eq!(reads.load(Ordering::Relaxed), 1);
}

#[test]
fn incoming_admission_refuses_pcm_that_disagrees_with_its_clock() {
    let (inner, mut ui) = capture_fixture();
    ui.loop_captures.takes[0].bpm = 60.0;
    let seed = SessionStore::new(inner.clone());
    seed.try_save(&serde_json::to_string(&ui.to_persisted()).unwrap())
        .unwrap();
    let (incoming, reads) = read_only_store(inner, false);
    assert!(admit(&incoming).err().unwrap().contains("recorded clock"));
    assert_eq!(reads.load(Ordering::Relaxed), 1);
}

#[test]
fn corrupt_session_restore_remains_unsaved_and_source_initialization_cannot_replace_it() {
    let inner = muniment::MemoryBackend::default();
    let backend: HostBackend = Box::new(inner.clone());
    let storage = SessionStore::new(backend);
    storage.try_save("not a session").unwrap();
    let mut ui = UiState::new();
    ui.seal = Some(woodshed_views::persona::PracticeSeal::Sealed {
        persona: "test".into(),
        protection: "test".into(),
    });
    restore(&storage, &mut ui);
    assert!(!ui.practice_saved);
    assert!(
        ui.practice_save_notice
            .as_ref()
            .unwrap()
            .contains("existing files")
    );
    ensure_personal_relation_source(&storage, &mut ui);
    assert!(ui.personal_relations.is_none());
    assert!(
        storage
            .try_save(&serde_json::to_string(&ui.to_persisted()).unwrap())
            .is_err()
    );
    assert_eq!(storage.load().as_deref(), Some("not a session"));
}

#[test]
fn early_settings_decode_refusal_survives_later_restore() {
    let backend: HostBackend = Box::new(muniment::MemoryBackend::default());
    let storage = SessionStore::new(backend);
    storage.try_save_settings("invalid settings").unwrap();
    assert!(load_settings(&storage).is_none());
    let mut ui = UiState::new();
    restore(&storage, &mut ui);
    assert!(!ui.practice_saved);
    assert!(ui.practice_save_notice.is_some());
    assert!(storage.try_save_settings("{}").is_err());
    assert_eq!(storage.load_settings().as_deref(), Some("invalid settings"));
}

#[test]
fn cold_restore_unseal_failure_reports_unsaved_before_automatic_source_creation() {
    let inner = muniment::MemoryBackend::default();
    let first = SessionStore::new(woodshed_core::sealed_backend::SealedBackend::new(
        inner.clone(),
        [1; 32],
    ));
    first
        .try_save(&serde_json::to_string(&UiState::new().to_persisted()).unwrap())
        .unwrap();
    let backend: HostBackend = Box::new(woodshed_core::sealed_backend::SealedBackend::new(
        inner, [2; 32],
    ));
    let second = SessionStore::new(backend);
    let mut ui = UiState::new();
    restore(&second, &mut ui);
    assert!(!ui.practice_saved);
    assert!(
        ui.practice_save_notice
            .as_ref()
            .unwrap()
            .contains("existing files")
    );
    ensure_personal_relation_source(&second, &mut ui);
    assert!(ui.personal_relations.is_none());
    assert!(second.try_save("default").is_err());
    assert!(first.try_load().unwrap().is_some());
}

#[test]
fn unknown_workspace_is_quarantined_without_replacing_its_representation() {
    let backend: HostBackend = Box::new(muniment::MemoryBackend::default());
    let storage = SessionStore::new(backend);
    let mut saved = UiState::new().to_persisted();
    saved.workspace_json = Some(r#"{"version":999,"future":"retained"}"#.into());
    let original = serde_json::to_string(&saved).unwrap();
    storage.try_save(&original).unwrap();
    let mut ui = UiState::new();
    restore(&storage, &mut ui);
    assert!(!ui.practice_saved);
    assert!(
        storage
            .try_save(&serde_json::to_string(&ui.to_persisted()).unwrap())
            .is_err()
    );
    assert_eq!(storage.load().as_deref(), Some(original.as_str()));
}

#[test]
fn settings_apply_without_a_practice_session() {
    let backend: HostBackend = Box::new(muniment::MemoryBackend::default());
    let storage = SessionStore::new(backend);
    let mut settings = AppSettings::default();
    settings.appearance.theme = "Ember".into();
    storage.save_settings(&serde_json::to_string(&settings).unwrap());

    let mut ui = UiState::new();
    restore(&storage, &mut ui);

    assert_eq!(ui.app_settings, settings);
}

#[test]
fn host_session_restores_the_workspace_policy() {
    let backend: HostBackend = Box::new(muniment::MemoryBackend::default());
    let storage = SessionStore::new(backend);
    let mut saved = UiState::new();
    saved.activate_workspace_panel(WorkspacePanel::Related);
    let saved_session = saved.to_persisted();
    let saved_workspace = saved_session.workspace_json.clone();
    storage.save(&serde_json::to_string(&saved_session).unwrap());

    let mut restored = UiState::new();
    restore(&storage, &mut restored);

    assert_eq!(
        restored.workspace.active_panel(),
        Some(WorkspacePanel::Related)
    );
    assert_eq!(restored.section, woodshed_core::storage::AppSection::Stage);
    assert!(restored.related_expanded);
    assert_eq!(restored.to_persisted().workspace_json, saved_workspace);
}

#[test]
fn legacy_four_panel_workspace_and_session_restore_without_a_retained_library() {
    // Freeze the pre-overview workspace wire contract rather than deriving
    // this fixture from today's list of product panels.
    let tabs = [
        (
            "practice",
            0x574f_5052u64,
            "Practice",
            "woodshed.practice",
            "stage",
        ),
        ("set", 0x574f_5345, "Set", "woodshed.set", "rehearsal"),
        (
            "related",
            0x574f_5245,
            "Related",
            "woodshed.related",
            "stage",
        ),
        (
            "settings",
            0x574f_5347,
            "Settings",
            "woodshed.settings",
            "settings",
        ),
    ]
    .map(|(panel, id, title, kind, content_id)| {
        serde_json::json!({
            "panel": panel, "id": id, "title": title,
            "kind": kind, "content_id": content_id,
        })
    });
    let workspace = serde_json::json!({
        "tree": {"stack": {"tabs": tabs, "active": 1}},
        "active_panel": "set",
    });
    let mut legacy =
        serde_json::to_value(woodshed_core::storage::PersistedSession::default()).unwrap();
    legacy.as_object_mut().unwrap().remove("retained_sets");
    legacy["workspace_json"] = serde_json::Value::String(workspace.to_string());
    legacy["section"] = serde_json::json!("Rehearsal");
    let backend: HostBackend = Box::new(muniment::MemoryBackend::default());
    let storage = SessionStore::new(backend);
    storage.save(&legacy.to_string());
    let mut ui = UiState::new();
    restore(&storage, &mut ui);
    assert_eq!(ui.workspace.active_panel(), Some(WorkspacePanel::Set));
    assert_eq!(ui.section, woodshed_core::storage::AppSection::Rehearsal);
    assert!(!ui.rehearsal_running);
    assert!(!ui.song_playing);
    assert!(ui.to_persisted().retained_sets.entries.is_empty());
}

#[test]
fn legacy_single_owner_session_adopts_instances_without_changing_authored_work() {
    use woodshedding::rehearsal::{FretWindow, Hold};
    let mut old = UiState::new();
    old.stage.set_lens(woodshed_core::Lens::Scales);
    old.stage.set_root(6);
    old.stage_current(None);
    old.stage.set_root(9);
    old.stage_current(None);
    old.set.cursor = 1;
    old.set.cards[1].setting.instrument = "Ukulele".into();
    old.set.cards[1].setting.tuning = Some("Standard (high-G)".into());
    old.set.cards[1].setting.capo = Some(2);
    old.set.cards[1].setting.fret_window = Some(FretWindow { start: 2, span: 12 });
    old.set.cards[1].timing.bpm = Some(83.0);
    old.set.cards[1].timing.hold = Hold::Bars(2);
    old.app_settings.fretboard.neck_start = 4;
    old.app_settings.fretboard.neck_end = Some(17);
    let expected_set = serde_json::to_value(&old.set).unwrap();
    let mut legacy = serde_json::to_value(woodshed_core::storage::PersistedSession::capture(
        &old.stage,
        woodshed_core::storage::AppSection::Rehearsal,
        &old.set,
        &old.song,
        &old.practice_history,
    ))
    .unwrap();
    for key in ["working_sets", "catalog_explorations", "active_exploration"] {
        legacy.as_object_mut().unwrap().remove(key);
    }
    let backend: HostBackend = Box::new(muniment::MemoryBackend::default());
    let storage = SessionStore::new(backend);
    storage.save(&legacy.to_string());
    storage.save_settings(&serde_json::to_string(&old.app_settings).unwrap());
    let mut ui = UiState::new();
    restore(&storage, &mut ui);
    assert_eq!(serde_json::to_value(&ui.set).unwrap(), expected_set);
    assert_eq!(ui.working_sets.summaries(&ui.set).len(), 1);
    assert!(ui.working_sets.inactive.is_empty());
    assert!(ui.catalog_explorations.inactive.is_empty());
    assert_eq!(ui.catalog_explorations.summaries().len(), 1);
    assert_eq!(ui.stage.root_idx, 9);
    assert_eq!(ui.stage.lens, woodshed_core::Lens::Scales);
    assert_eq!(ui.app_settings.fretboard.neck_start, 4);
    assert_eq!(ui.app_settings.fretboard.neck_end, Some(17));
    assert!(!ui.rehearsal_running);
    assert!(ui.rehearsal_owner.is_none());
    let persisted = ui.to_persisted();
    assert!(persisted.active_exploration.is_some());
    assert_eq!(persisted.working_sets.active_id, ui.working_sets.active_id);
}

include!("tests/catalog_reopen.rs");

include!("tests/ordered_reopen.rs");

include!("tests/library_reopen.rs");
