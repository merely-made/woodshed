use super::*;
use crate::loop_capture::decode_pcm;
const NAMESPACE: &str = "0123456789abcdef0123456789abcdef";

#[test]
fn edited_pack_roundtrip_and_invalid_asset_refusal() {
    let mut session = decode_session("{}").unwrap().session;
    assert!(session.audio_pack.is_none());
    let pack = audio_packs::files::load(std::path::Path::new("bundled"), None).unwrap();
    let pack = pack
        .edit(
            &pack.revision(),
            audio_packs::PackEdit::CycleStep { track: 0, step: 1 },
        )
        .unwrap();
    let midi = pack.midi().unwrap().bytes;
    session.audio_pack = Some(pack);
    let json = serde_json::to_string(&session).unwrap();
    let reopened = decode_session(&json).unwrap().session;
    assert!(session.audio_pack == reopened.audio_pack);
    assert_eq!(midi, reopened.audio_pack.unwrap().midi().unwrap().bytes);
    let mut invalid: serde_json::Value = serde_json::from_str(&json).unwrap();
    invalid["audio_pack"]["snapshot"]["assets"]["samples/kick.wav"][0] = serde_json::json!(0);
    assert!(decode_session(&invalid.to_string()).is_err());
}

#[test]
fn session_store_roundtrips_opaque_scope_navigation_bytes() {
    let store = SessionStore::new(muniment::MemoryBackend::default());
    let mut session = PersistedSession::default();
    let route = r#"{"future_scope_payload":{"owner":"opaque","camera":[0.2,-0.1,1.4]}}"#;
    session.scope_navigation_json = Some(route.into());
    let written = serde_json::to_string(&session).unwrap();
    store.try_save(&written).unwrap();

    let read = store.try_load().unwrap().unwrap();
    assert_eq!(read, written);
    let reopened = decode_session(&read).unwrap().session;
    assert_eq!(reopened.scope_navigation_json.as_deref(), Some(route));
}

#[test]
fn missing_session_initializes_but_unreadable_slot_quarantines_every_clone() {
    let inner = muniment::MemoryBackend::default();
    let store = SessionStore::new(inner.clone());
    let sibling = store.clone();
    assert!(store.try_load().unwrap().is_none());
    store.try_save("original session").unwrap();
    pollster::block_on(inner.put(SETTINGS_SLOT, &[0xff])).unwrap();
    assert!(store.load_settings().is_none());
    assert!(sibling.write_refusal().is_some());
    assert!(sibling.try_save("default replacement").is_err());
    assert!(sibling.try_save_settings("{}").is_err());
    assert!(
        sibling
            .try_write_capture(NAMESPACE, 48_000, &[0.25])
            .is_err()
    );
    store.save("another default");
    assert_eq!(
        pollster::block_on(inner.get(SESSION_SLOT))
            .unwrap()
            .unwrap(),
        b"original session"
    );
    assert_eq!(
        pollster::block_on(inner.get(SETTINGS_SLOT))
            .unwrap()
            .unwrap(),
        [0xff]
    );
}

#[test]
fn wrong_persona_quarantines_without_clobbering_original_session() {
    let inner = muniment::MemoryBackend::default();
    let first = SessionStore::new(crate::sealed_backend::SealedBackend::new(
        inner.clone(),
        [1; 32],
    ));
    first.try_save("original session").unwrap();
    let ciphertext = pollster::block_on(inner.get(SESSION_SLOT)).unwrap();
    let second = SessionStore::new(crate::sealed_backend::SealedBackend::new(
        inner.clone(),
        [2; 32],
    ));
    assert!(second.try_load().is_err());
    second.save("default session");
    assert!(second.try_save_settings("{}").is_err());
    assert_eq!(
        pollster::block_on(inner.get(SESSION_SLOT)).unwrap(),
        ciphertext
    );
    assert_eq!(
        first.try_load().unwrap().as_deref(),
        Some("original session")
    );
}

#[test]
fn dropped_settings_write_is_not_acknowledged() {
    let store = SessionStore::new(FaultBackend {
        inner: muniment::MemoryBackend::default(),
        fault: Fault::Drop,
    });
    assert!(store.try_save_settings("{}").is_err());
    assert!(store.save_problem().unwrap().contains("settings"));
}

fn capture_ref(digest: String, frames: usize) -> LoopCaptureRef {
    LoopCaptureRef {
        seconds: None,
        clock: None,
        id: crate::loop_plan::LoopSegmentId {
            set: crate::working_sets::WorkingSetId(3),
            card: woodshedding::rehearsal::CardId(7),
        },
        bpm: 83.75,
        beats: 7,
        bars: 3,
        sample_rate: 48_000,
        frames,
        digest,
        codec: LoopCaptureCodec::LegacyRawF32V1,
    }
}

#[derive(Clone)]
enum Fault {
    Drop,
    DropSettings(std::sync::Arc<std::sync::atomic::AtomicBool>),
    CorruptCapture,
    RefuseSession,
}

#[derive(Clone)]
struct FaultBackend {
    inner: muniment::MemoryBackend,
    fault: Fault,
}

#[async_trait::async_trait]
impl Backend for FaultBackend {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StoreError> {
        self.inner.get(key).await
    }
    async fn put(&self, key: &str, bytes: &[u8]) -> Result<(), StoreError> {
        match &self.fault {
            Fault::Drop => Ok(()),
            Fault::DropSettings(drop)
                if key == SETTINGS_SLOT && drop.load(std::sync::atomic::Ordering::Relaxed) =>
            {
                Ok(())
            },
            Fault::RefuseSession if key == SESSION_SLOT => {
                Err(StoreError::Backend("refused session replacement".into()))
            },
            Fault::CorruptCapture if key.starts_with("loop-pcm/") => {
                let mut bad = bytes.to_vec();
                *bad.last_mut().unwrap() ^= 1;
                self.inner.put(key, &bad).await
            },
            _ => self.inner.put(key, bytes).await,
        }
    }
    async fn delete(&self, key: &str) -> Result<(), StoreError> {
        self.inner.delete(key).await
    }
    async fn list(&self, prefix: &str) -> Result<Vec<String>, StoreError> {
        self.inner.list(prefix).await
    }
    async fn scan(&self, start: &str, end: &str) -> Result<Vec<String>, StoreError> {
        self.inner.scan(start, end).await
    }
    async fn apply(&self, _ops: &[muniment::backend::WriteOp]) -> Result<(), StoreError> {
        Err(StoreError::Backend("no multi-file transaction".into()))
    }
}

#[test]
fn legacy_session_defaults_authored_musical_context_and_roundtrips_it() {
    let mut value = serde_json::to_value(PersistedSession::default()).unwrap();
    value.as_object_mut().unwrap().remove("musical_context");
    let mut session: PersistedSession = serde_json::from_value(value).unwrap();
    assert!(session.musical_context.items().is_empty());
    let subject = crate::harmony::KeyedCatalogRef {
        formula_id: "scale:Major".into(),
        root: woodshedding::pitch::PitchClass::new(2),
    };
    let id = session
        .musical_context
        .keep(crate::working_sets::WorkingSetId(1), subject.clone())
        .unwrap();
    let reopened: PersistedSession =
        serde_json::from_str(&serde_json::to_string(&session).unwrap()).unwrap();
    assert_eq!(reopened.musical_context.get(id).unwrap().subject, subject);
}

#[test]
fn session_round_trips_through_json() {
    let mut stage = StageState::new();
    stage.set_lens(Lens::Arpeggios);
    stage.set_tuning(3);
    stage.set_root(3);
    stage.select_arpeggio(5);
    stage.arpeggio_inversion = 2;
    stage.select_progression(1);
    let mut set = woodshedding::rehearsal::Set::default();
    set.push(stage.card_from_lens().expect("arpeggio card"));
    let mut song = crate::song::SongDoc::default();
    song.name = "My Song".into();
    song.bars.push(crate::song::SongBar::default());
    song.one_shot = true;
    let mut history = crate::history::PracticeHistory::default();
    history.record(
        Some(1_000),
        woodshed_graph::chord_id("Minor 7"),
        crate::history::EngagementKind::Staged,
        Some(woodshed_graph::scale_id("Dorian")),
        None,
    );
    let related = RelatedSettings {
        use_history: false,
        show_neighborhood: false,
        dismissed_ids: vec![woodshed_graph::chord_id("Minor 7")],
        ..Default::default()
    };
    let settings = AppSettings {
        page: crate::settings::SettingsPage::Tuning,
        appearance: crate::settings::AppearanceSettings {
            theme: "Ember".into(),
            theme_choice: None,
        },
        tuning: crate::settings::TuningSettings { tuning_idx: 3 },
        stage: crate::settings::StageSettings {
            related: related.clone(),
            ..Default::default()
        },
        fretboard: crate::settings::FretboardSettings {
            board_layout: "Hero".into(),
            marker_style: "Sharp".into(),
            ..Default::default()
        },
        metronome: crate::settings::MetronomeSettings { bpm: 96.0 },
        ..AppSettings::default()
    };
    let snap = PersistedSession::capture(&stage, AppSection::Settings, &set, &song, &history);
    let json = serde_json::to_string(&snap).unwrap();
    let wire: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(
        wire.get("theme").is_none(),
        "application settings leave the session wire"
    );
    assert!(wire.get("tuning_idx").is_none());
    let settings_json = serde_json::to_string(&settings).unwrap();
    let settings_wire: serde_json::Value = serde_json::from_str(&settings_json).unwrap();
    assert_eq!(settings_wire["theme"], "Ember");
    assert_eq!(settings_wire["tuning_idx"], 3);
    assert_eq!(settings_wire["bpm"], 96.0);
    let back: PersistedSession = serde_json::from_str(&json).unwrap();
    let mut restored = StageState::new();
    back.restore(&mut restored, &settings);
    assert_eq!(restored.lens, Lens::Arpeggios);
    assert_eq!(restored.tuning_idx, 3);
    assert_eq!(restored.root_idx, 3);
    assert_eq!(restored.arpeggio_idx, 5);
    assert_eq!(restored.arpeggio_inversion, 2);
    assert_eq!(restored.progression_idx, Some(1));
    assert_eq!(back.section, AppSection::Settings);
    assert_eq!(back.set.cards.len(), 1, "the rehearsal set round-trips");
    assert_eq!(back.set.cards[0].label, "Csus4 arpeggio");
    assert_eq!(back.song.name, "My Song", "the song doc round-trips");
    assert_eq!(back.song.bars.len(), 1);
    assert!(back.song.one_shot);
    // The practice lineage round-trips through the session wire: the
    // engagement, its subject, and the traversal that produced it.
    assert_eq!(back.practice_history.len(), history.len());
    assert_eq!(
        back.practice_history.recent(1)[0].subject_id,
        history.recent(1)[0].subject_id
    );
    assert_eq!(settings.stage.related, related);
}

#[test]
fn history_admission_failures_propagate_from_session_decode() {
    assert_eq!(
        decode_session("{}").unwrap().session.practice_history.len(),
        0
    );
    for history in [
        serde_json::json!({"entries": [], "visits": "damaged", "owners": []}),
        serde_json::json!({"entries": [], "events": []}),
        serde_json::json!({"entries": [], "visits": [], "owners": [{
            "identity": "practitioner", "origin": null, "current": 0,
            "creator": null, "pending_origin_parent": null, "owned_visits": []
        }]}),
    ] {
        let contents = serde_json::json!({"practice_history": history}).to_string();
        assert!(decode_session(&contents).is_err());
    }
    let legacy = serde_json::json!({"practice_history": {"events": [{
        "subject_id": "scale:Major", "kind": "Rehearsed", "sequence": 0
    }], "next_sequence": 1}})
    .to_string();
    let session = decode_session(&legacy).unwrap().session;
    let events = session.practice_history.recent(10);
    let current = serde_json::to_string(&session).unwrap();
    assert_eq!(
        decode_session(&current)
            .unwrap()
            .session
            .practice_history
            .recent(10),
        events
    );
}

#[test]
fn rejected_history_can_be_quarantined_without_replacing_saved_bytes() {
    let contents = r#"{"practice_history":{"entries":[],"visits":"damaged","owners":[]}}"#;
    let store = SessionStore::new(muniment::MemoryBackend::default());
    store.try_save(contents).unwrap();
    let loaded = store.try_load().unwrap().unwrap();
    let error = decode_session(&loaded).unwrap_err();
    // The desktop host owns this refusal decision. Its ordinary decode
    // error path now receives malformed history instead of a default.
    store.refuse_writes(error.to_string());
    assert!(store.try_save("{}").is_err());
    assert_eq!(store.try_load().unwrap().as_deref(), Some(contents));
}

fn session_with_all_set_banks() -> PersistedSession {
    let mut session = PersistedSession::default();
    let card = StageState::new().card_from_lens().unwrap();
    session.set.push(card.clone());
    session.set.push(card);
    session.retained_sets.save_snapshot(&session.set, "Saved");
    session
        .working_sets
        .inactive
        .push(crate::working_sets::WorkingSet {
            id: crate::working_sets::WorkingSetId(2),
            name: "Inactive".into(),
            set: session.set.clone(),
        });
    session
}

#[test]
fn duplicate_card_identity_is_rejected_in_every_loaded_set_bank() {
    for (path, owner) in [
        ("/set", "Active working Set 1"),
        ("/working_sets/inactive/0/set", "Inactive working Set 2"),
        ("/retained_sets/entries/0/set", "Retained saved Set 1"),
    ] {
        let mut wire = serde_json::to_value(session_with_all_set_banks()).unwrap();
        let set = wire.pointer_mut(path).unwrap();
        set["cards"][1]["id"] = set["cards"][0]["id"].clone();
        let error = decode_session(&wire.to_string()).unwrap_err().to_string();
        assert!(error.contains(owner), "{error}");
        assert!(error.contains("Duplicate Card identity"), "{error}");
    }
}

#[test]
fn exhausted_card_mints_are_decode_errors_in_every_set_bank() {
    for (path, owner) in [
        ("/set", "Active working Set 1"),
        ("/working_sets/inactive/0/set", "Inactive working Set 2"),
        ("/retained_sets/entries/0/set", "Retained saved Set 1"),
    ] {
        for case in [
            "mint only",
            "maximum card",
            "migration consumes final id",
            "empty mint",
        ] {
            let mut wire = serde_json::to_value(session_with_all_set_banks()).unwrap();
            let set = wire.pointer_mut(path).unwrap();
            match case {
                "mint only" => set["last_card_id"] = serde_json::json!(u64::MAX),
                "maximum card" => set["cards"][0]["id"] = serde_json::json!(u64::MAX),
                "migration consumes final id" => {
                    set["last_card_id"] = serde_json::json!(u64::MAX - 1);
                    set["cards"][0]["id"] = serde_json::json!(0);
                },
                "empty mint" => {
                    set["last_card_id"] = serde_json::json!(u64::MAX);
                    set["cards"] = serde_json::json!([]);
                },
                _ => unreachable!(),
            }
            let error = decode_session(&wire.to_string()).unwrap_err().to_string();
            assert!(error.contains(owner), "{case}: {error}");
            assert!(
                error.contains("Card identity mint exhausted"),
                "{case}: {error}"
            );
        }
    }
}

#[test]
fn every_set_bank_migrates_legacy_ids_and_current_snapshots_round_trip() {
    let original = serde_json::to_value(session_with_all_set_banks()).unwrap();
    // Snapshot/source IDs intentionally overlap across Sets and remain valid.
    let current = decode_session(&original.to_string()).unwrap().session;
    assert_eq!(serde_json::to_value(current).unwrap(), original);
    let mut legacy = original;
    for path in [
        "/set",
        "/working_sets/inactive/0/set",
        "/retained_sets/entries/0/set",
    ] {
        let set = legacy.pointer_mut(path).unwrap();
        set["cards"][0].as_object_mut().unwrap().remove("id");
        set["cards"][1]["id"] = serde_json::json!(0);
        set.as_object_mut().unwrap().remove("last_card_id");
        set["cursor"] = serde_json::json!(1);
    }
    let session = decode_session(&legacy.to_string()).unwrap().session;
    for set in [
        &session.set,
        &session.working_sets.inactive[0].set,
        &session.retained_sets.entries[0].set,
    ] {
        assert!(set.cards[0].id.is_assigned());
        assert!(set.cards[1].id.is_assigned());
        assert_ne!(set.cards[0].id, set.cards[1].id);
        assert_eq!(set.cursor_id(), Some(set.cards[1].id));
    }
    let saved = serde_json::to_value(session).unwrap();
    let reopened = decode_session(&saved.to_string()).unwrap().session;
    assert_eq!(serde_json::to_value(reopened).unwrap(), saved);
}

#[test]
fn unknown_fields_and_missing_fields_tolerated() {
    let loaded = decode_session(r#"{"lens":"Chords","bpm":88.0}"#).unwrap();
    assert_eq!(loaded.session.lens, Lens::Chords);
    assert_eq!(loaded.legacy_settings.as_ref().unwrap().metronome.bpm, 88.0);
    assert_eq!(
        loaded.session.section,
        AppSection::Stage,
        "missing fields default"
    );
    // Out-of-range indices clamp through the setters.
    let huge: PersistedSession = serde_json::from_str(r#"{"scale_idx":99999}"#).unwrap();
    let mut s = StageState::new();
    huge.restore(&mut s, &AppSettings::default());
    assert!(s.scale_idx < s.scales().len());
}

#[test]
fn the_legacy_sequence_edge_toggle_becomes_a_relation_set() {
    use woodshedding::rehearsal::SetGraphEdgeKind;

    // A session written while Set-graph edge visibility was one boolean.
    let mut off: AppSettings =
        serde_json::from_str(r#"{"show_set_sequence_edges":false}"#).unwrap();
    assert!(
        off.stage.shows_relation(SetGraphEdgeKind::Next),
        "the default set is unchanged until the legacy value is adopted"
    );

    off.stage.adopt_legacy_relation_visibility();
    assert!(!off.stage.shows_relation(SetGraphEdgeKind::Next));

    // The legacy key is consumed, so the next save writes only the set.
    let json = serde_json::to_string(&off).unwrap();
    assert!(!json.contains("show_set_sequence_edges"));
    assert!(json.contains("visible_set_relations"));

    // A session written after the migration is untouched by it.
    let mut on: AppSettings = serde_json::from_str(&json).unwrap();
    on.stage.adopt_legacy_relation_visibility();
    assert!(!on.stage.shows_relation(SetGraphEdgeKind::Next));
}

#[test]
fn multiple_owner_banks_preserve_legacy_active_content_and_round_trip() {
    let mut session = decode_session("{}").unwrap().session;
    assert_eq!(
        session.working_sets.active_id,
        crate::working_sets::WorkingSetId(1)
    );
    assert!(session.working_sets.inactive.is_empty());
    assert!(session.active_exploration.is_none());
    session
        .set
        .push(StageState::new().card_from_lens().unwrap());
    let original_id = session.set.cards[0].id;
    let original_owner = session.working_sets.active_id;
    let second = session.working_sets.create(
        &mut session.set,
        "Other",
        woodshedding::rehearsal::Set::default(),
    );
    let mut context = crate::catalog_explorations::CatalogExplorationState::default();
    context.search_query = "minor".into();
    session.catalog_explorations.create(
        &mut context,
        "Other exploration",
        crate::catalog_explorations::CatalogExplorationState::default(),
    );
    session.active_exploration = Some(context);
    let mut reopened = decode_session(&serde_json::to_string(&session).unwrap())
        .unwrap()
        .session;
    assert_eq!(reopened.working_sets.active_id, second);
    assert!(
        reopened
            .working_sets
            .activate(original_owner, &mut reopened.set)
    );
    assert_eq!(reopened.set.cards[0].id, original_id);
    assert_eq!(
        reopened.catalog_explorations.inactive[0].state.search_query,
        "minor"
    );
    let wire = serde_json::to_value(&reopened.working_sets).unwrap();
    assert!(wire.get("set").is_none());
}

#[test]
fn retained_sets_default_for_legacy_and_round_trip_separately_from_working_set() {
    let legacy = decode_session("{}").unwrap();
    assert!(legacy.session.retained_sets.entries.is_empty());
    let mut session = PersistedSession::default();
    session
        .set
        .push(StageState::new().card_from_lens().unwrap());
    let saved_id = session
        .retained_sets
        .save_snapshot(&session.set, "My study");
    session.set.cards[0].label = "Current edited instruction".into();
    let saved_before = serde_json::to_value(session.retained_sets.get(saved_id).unwrap()).unwrap();
    let mut loaded = decode_session(&serde_json::to_string(&session).unwrap())
        .unwrap()
        .session;
    assert_eq!(loaded.set.cards[0].label, "Current edited instruction");
    assert_eq!(
        serde_json::to_value(loaded.retained_sets.get(saved_id).unwrap()).unwrap(),
        saved_before
    );
    let old_id = loaded.set.cards[0].id;
    assert!(loaded.retained_sets.restore_into(saved_id, &mut loaded.set));
    assert_ne!(loaded.set.cards[0].id, old_id);
    assert_eq!(
        serde_json::to_value(loaded.retained_sets.get(saved_id).unwrap()).unwrap(),
        saved_before
    );
}

#[test]
fn legacy_tabs_migrate_to_current_sections() {
    let practice: PersistedSession = serde_json::from_str(r#"{"tab":"Practice"}"#).unwrap();
    assert_eq!(practice.section, AppSection::Stage);
    let song: PersistedSession = serde_json::from_str(r#"{"tab":"Song"}"#).unwrap();
    assert_eq!(song.section, AppSection::Looper);
}

#[path = "tests/durability.rs"]
mod durability;
