#![cfg(feature = "portable-scenes")]
use woodshed_core::{
    Lens, ROOT_NAMES, StageState,
    arrangement_scene::saved_arrangement_scene,
    saved_arrangements::{SessionSet, save_progression_arrangement, saved_arrangement_stage},
    sealed_backend::SealedBackend,
    settings::AppSettings,
    storage::{AppSection, PersistedSession, SessionStore, decode_session},
    working_sets::WorkingSetId,
};
use woodshedding::rehearsal::{Set, arrangement::*};

fn fixture() -> (StageState, Set, ProgressionArrangement) {
    let mut stage = StageState::new();
    stage.lens = Lens::Progressions;
    stage.set_root(ROOT_NAMES.iter().position(|s| *s == "C").unwrap());
    stage.select_progression(
        stage
            .progressions()
            .iter()
            .position(|p| p.name == "ii-V-I (Jazz)")
            .unwrap(),
    );
    let mut set = Set::default();
    for i in 0..3 {
        stage.progression_expand(i);
        let mut card = stage.card_from_lens().unwrap();
        stage.select_card_shape_index(&mut card, 0).unwrap();
        set.push(card);
    }
    set.duplicate(2);
    stage.select_card_shape_index(&mut set.cards[3], 1).unwrap();
    let a = ProgressionArrangement {
        name: "Saved practice passage".into(),
        extent: 64,
        role_spans: vec![
            Span {
                start: 0,
                length: 16,
            },
            Span {
                start: 16,
                length: 16,
            },
            Span {
                start: 32,
                length: 32,
            },
        ],
        cards: set
            .cards
            .iter()
            .enumerate()
            .map(|(i, c)| CardAlignment {
                card: c.id,
                role: Some(i.min(2)),
                placement: Some(Placement {
                    lane: 0,
                    span: Span {
                        start: [0, 16, 32, 48][i],
                        length: [16, 8, 16, 16][i],
                    },
                }),
            })
            .collect(),
        rests: vec![ArrangementRest {
            key: "breath".into(),
            role: None,
            placement: Placement {
                lane: 0,
                span: Span {
                    start: 24,
                    length: 8,
                },
            },
        }],
    };
    (stage, set, a)
}
fn session(stage: &StageState, set: &Set) -> PersistedSession {
    PersistedSession::capture(
        stage,
        AppSection::Stage,
        set,
        &Default::default(),
        &Default::default(),
    )
}
fn json(value: &impl serde::Serialize) -> serde_json::Value {
    serde_json::to_value(value).unwrap()
}

#[test]
fn sealed_save_reopen_exports_same_ids_spans_and_voicings() {
    let (stage, mut set, a) = fixture();
    save_progression_arrangement(&stage, &mut set, a).unwrap();
    let expected = saved_arrangement_scene(&set, "practice-session/working/1").unwrap();
    let backend = muniment::MemoryBackend::default();
    {
        let store = SessionStore::new(SealedBackend::new(backend.clone(), [7; 32]));
        store
            .try_save(&serde_json::to_string(&session(&stage, &set)).unwrap())
            .unwrap();
    }
    let store = SessionStore::new(SealedBackend::new(backend, [7; 32]));
    let reopened = decode_session(&store.load().unwrap()).unwrap().session;
    assert_eq!(json(&set), json(&reopened.set));
    assert_eq!(
        expected,
        saved_arrangement_scene(&reopened.set, "practice-session/working/1").unwrap()
    );
    assert_eq!(expected.linked.len(), 4);
    assert_ne!(expected.linked[2].scene, expected.linked[3].scene);
}

#[test]
fn navigation_and_working_set_switch_do_not_retarget_template() {
    let (mut stage, mut set, a) = fixture();
    save_progression_arrangement(&stage, &mut set, a).unwrap();
    let expected = saved_arrangement_scene(&set, "stable").unwrap();
    let mut s = session(&stage, &set);
    let first = s.working_sets.active_id;
    let other = s.working_sets.create(&mut s.set, "Other", Set::default());
    assert_ne!(first, other);
    stage.set_root(7);
    stage.select_progression(0);
    stage.set_tuning(2);
    stage.apply_neck(8, Some(16));
    let stored = SessionSet::Working(first).get(&s).unwrap();
    assert_eq!(expected, saved_arrangement_scene(stored, "stable").unwrap());
    let reopened = decode_session(&serde_json::to_string(&s).unwrap())
        .unwrap()
        .session;
    assert_eq!(
        expected,
        saved_arrangement_scene(SessionSet::Working(first).get(&reopened).unwrap(), "stable")
            .unwrap()
    );
    assert!(SessionSet::Working(WorkingSetId(999)).get(&s).is_none());
}

#[test]
fn snapshots_and_duplicates_remap_live_and_missing_alignment_without_aliasing() {
    let (stage, mut set, a) = fixture();
    save_progression_arrangement(&stage, &mut set, a).unwrap();
    let mut s = session(&stage, &set);
    let saved = s.retained_sets.save_snapshot(&set, "Study");
    let frozen = json(s.retained_sets.get(saved).unwrap());
    let original: Vec<_> = set.cards.iter().map(|c| c.id).collect();
    set.remove(1); // Its alignment remains authored and unresolved.
    let mut bank = woodshed_core::working_sets::WorkingSets::default();
    let duplicate = bank.duplicate(bank.active_id, &mut set, "Copy").unwrap();
    let missing = set
        .progression_arrangement
        .as_ref()
        .unwrap()
        .alignment
        .cards[1]
        .card;
    assert!(set.card(missing).is_none());
    for _ in 0..8 {
        set.push(stage.card_from_lens().unwrap());
    }
    assert!(
        set.card(missing).is_none(),
        "missing reference is reserved against later insertions"
    );
    assert!(
        saved_arrangement_stage(&set)
            .err()
            .unwrap()
            .contains("Card")
    );
    assert!(bank.get(duplicate, &set).is_some());
    assert!(s.retained_sets.restore_into(saved, &mut set));
    assert!(set.cards.iter().all(|c| !original.contains(&c.id)));
    let bindings = &set
        .progression_arrangement
        .as_ref()
        .unwrap()
        .alignment
        .cards;
    assert!(bindings.iter().all(|b| set.card(b.card).is_some()));
    assert_eq!(
        saved_arrangement_scene(&set, "copy").unwrap().linked.len(),
        4
    );
    assert_eq!(frozen, json(s.retained_sets.get(saved).unwrap()));
    let id = bank
        .duplicate(bank.active_id, &mut set, "Resolved copy")
        .unwrap();
    assert!(
        bank.get(id, &set)
            .unwrap()
            .progression_arrangement
            .is_some()
    );
}

#[test]
fn unresolved_or_changed_data_is_retained_and_refused_without_mutation() {
    let (stage, mut set, a) = fixture();
    save_progression_arrangement(&stage, &mut set, a.clone()).unwrap();
    let good = set.clone();
    let mut invalid = a;
    invalid.role_spans[0].length = u32::MAX;
    assert!(save_progression_arrangement(&stage, &mut set, invalid).is_err());
    assert_eq!(json(&good), json(&set));
    for fault in 0..5 {
        let mut broken = good.clone();
        let saved = broken.progression_arrangement.as_mut().unwrap();
        match fault {
            0 => saved.version = 99,
            1 => saved.roles[0].degree = 7,
            2 => saved.progression = "Absent catalog entry".into(),
            3 => saved.open_midi[0] += 1,
            _ => {
                broken.remove(0);
            },
        }
        let before = json(&broken);
        assert!(saved_arrangement_scene(&broken, "refusal").is_err());
        let reopened: Set = serde_json::from_value(before.clone()).unwrap();
        assert_eq!(before, json(&reopened));
        assert!(saved_arrangement_scene(&reopened, "refusal").is_err());
        assert_eq!(before, json(&broken));
    }
    let legacy: Set = serde_json::from_str(r#"{"cards":[],"cursor":0}"#).unwrap();
    assert!(legacy.progression_arrangement.is_none());
    let mut stale = good;
    stale.cards[0].setting.voicing_fingerprint = Some("stale".into());
    assert!(
        saved_arrangement_scene(&stale, "stale")
            .unwrap_err()
            .contains("voicing")
    );
}

fn cli(args: &[&str]) -> std::process::Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_woodshed-saved-arrangement"))
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn fresh_process_attach_and_export_support_each_owner_and_preserve_source_keys() {
    let (stage, set, a) = fixture();
    let mut s = session(&stage, &set);
    let mut retained = set.clone();
    save_progression_arrangement(&stage, &mut retained, a.clone()).unwrap();
    let saved = s.retained_sets.save_snapshot(&retained, "Retained passage");
    let active = s.working_sets.active_id;
    let inactive = s.working_sets.create(&mut s.set, "Other", Set::default());
    assert!(s.working_sets.activate(active, &mut s.set));
    // Give the inactive owner actual material through the public bank seam.
    *s.working_sets.get_mut(inactive, &mut s.set).unwrap() = set.clone();
    let dir =
        std::env::temp_dir().join(format!("woodshed-saved-arrangement-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let path = |name: &str| dir.join(name).to_str().unwrap().to_string();
    let mut source = json(&s);
    source["future_host_field"] = json(&"preserve");
    source["set"]["future_set_field"] = json(&42);
    source["set"]["cards"][0]["future_card_field"] = json(&true);
    let input = path("input.json");
    let settings = path("settings.json");
    let alignment = path("alignment.json");
    std::fs::write(&input, serde_json::to_vec(&source).unwrap()).unwrap();
    std::fs::write(
        &settings,
        serde_json::to_vec(&AppSettings::default()).unwrap(),
    )
    .unwrap();
    std::fs::write(&alignment, serde_json::to_vec(&a).unwrap()).unwrap();
    let original = std::fs::read(&input).unwrap();
    for (i, (kind, id)) in [
        ("working", active.0),
        ("working", inactive.0),
        ("retained", saved.0),
    ]
    .iter()
    .enumerate()
    {
        let attached = path(&format!("attached-{i}.json"));
        let exported = path(&format!("scene-{i}.json"));
        let id = id.to_string();
        if *kind == "working" {
            let attach = cli(&[
                "attach", &input, &settings, kind, &id, &alignment, &attached,
            ]);
            assert!(
                attach.status.success(),
                "{}",
                String::from_utf8_lossy(&attach.stderr)
            );
            let after: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&attached).unwrap()).unwrap();
            let target = if i == 0 {
                "/set"
            } else {
                "/working_sets/inactive/0/set"
            };
            let mut normalized = after;
            normalized
                .pointer_mut(target)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove("progression_arrangement");
            assert_eq!(source, normalized);
        } else {
            let refused = path("refused-retained-edit.json");
            assert!(
                !cli(&["attach", &input, &settings, kind, &id, &alignment, &refused])
                    .status
                    .success()
            );
            assert!(!std::path::Path::new(&refused).exists());
            std::fs::copy(&input, &attached).unwrap();
        }
        let export = cli(&["export", &attached, kind, &id, "session/study", &exported]);
        assert!(
            export.status.success(),
            "{}",
            String::from_utf8_lossy(&export.stderr)
        );
        let bytes = std::fs::read(&exported).unwrap();
        let doc = scenomise::document::SceneDocument::parse(std::str::from_utf8(&bytes).unwrap())
            .unwrap();
        assert_eq!(doc.linked.len(), 4);
        assert!(
            !cli(&["export", &attached, kind, &id, "session/study", &exported])
                .status
                .success()
        );
        assert_eq!(bytes, std::fs::read(&exported).unwrap());
    }
    assert_eq!(original, std::fs::read(&input).unwrap());
    let absent = path("absent.json");
    assert!(
        !cli(&["export", &input, "working", "999", "session/study", &absent])
            .status
            .success()
    );
    assert!(!std::path::Path::new(&absent).exists());
    if let Some(destination) = std::env::var_os("WOODSHED_SAVED_ARRANGEMENT_RECEIPT_DIR") {
        let destination = std::path::PathBuf::from(destination);
        std::fs::create_dir(&destination).unwrap();
        for entry in std::fs::read_dir(&dir).unwrap() {
            let entry = entry.unwrap();
            std::fs::copy(entry.path(), destination.join(entry.file_name())).unwrap();
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}
