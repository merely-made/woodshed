use super::*;
use personae::InMemoryProvider;
use woodshed_views::stage::UiState;

fn sealed_at(
    root: &Path,
    provider: &InMemoryProvider,
) -> (SealedBackend<FsBackend>, String, PathBuf) {
    let owner = owner_for_provider(provider);
    let directory = persona_directory(root, &owner);
    let backend = SealedBackend::for_provider(FsBackend::at(&directory), provider).unwrap();
    (backend, owner, directory)
}

fn open_fixture(
    root: &Path,
    provider: &InMemoryProvider,
) -> SessionStore<SealedBackend<FsBackend>> {
    let (backend, owner, directory) = sealed_at(root, provider);
    initialize_owner(&backend, &directory, &owner).unwrap();
    SessionStore::new(backend)
}

#[test]
fn cryptographic_personas_isolate_session_settings_and_pcm_across_fresh_handles() {
    let root = tempfile::tempdir().unwrap();
    let first_provider = InMemoryProvider::from_seed([1; 32]);
    let second_provider = InMemoryProvider::from_seed([2; 32]);
    let first_owner = owner_for_provider(&first_provider);
    let second_owner = owner_for_provider(&second_provider);
    assert_ne!(first_owner, second_owner);
    // Both vaults can use this local label. It never participates in paths.
    let label = ProfileId("default".into());
    let first_binding =
        StoreBinding::for_test(root.path().into(), first_owner.clone(), label.clone());
    let second_binding = StoreBinding::for_test(root.path().into(), second_owner.clone(), label);
    assert_ne!(first_binding.directory(), second_binding.directory());
    let first = open_fixture(root.path(), &first_provider);
    let second = open_fixture(root.path(), &second_provider);
    first.try_save("first practice").unwrap();
    first.try_save_settings("first settings").unwrap();
    second.try_save("second practice").unwrap();
    second.try_save_settings("second settings").unwrap();
    let namespace = "0123456789abcdef0123456789abcdef";
    let first_pcm = [0.25, -0.0];
    let second_pcm = [0.75, 0.0];
    let (first_digest, _) = first
        .try_write_capture(namespace, 48_000, &first_pcm)
        .unwrap();
    let (second_digest, _) = second
        .try_write_capture(namespace, 48_000, &second_pcm)
        .unwrap();
    drop(first);
    drop(second);
    let first = open_fixture(root.path(), &InMemoryProvider::from_seed([1; 32]));
    let second = open_fixture(root.path(), &InMemoryProvider::from_seed([2; 32]));
    assert_eq!(first.try_load().unwrap().as_deref(), Some("first practice"));
    assert_eq!(
        second.try_load().unwrap().as_deref(),
        Some("second practice")
    );
    assert_eq!(first.load_settings().as_deref(), Some("first settings"));
    assert_eq!(second.load_settings().as_deref(), Some("second settings"));
    for (store, digest, expected) in [
        (&first, first_digest, first_pcm),
        (&second, second_digest, second_pcm),
    ] {
        let reference = woodshed_core::loop_capture::LoopCaptureRef {
            id: woodshed_core::loop_plan::LoopSegmentId {
                set: woodshed_core::working_sets::WorkingSetId(3),
                card: woodshedding::rehearsal::CardId(7),
            },
            bpm: 120.0,
            beats: 4,
            bars: 1,
            sample_rate: 48_000,
            frames: 2,
            digest,
            codec: woodshed_core::loop_capture::LoopCaptureCodec::LegacyRawF32V1,
            clock: None,
            seconds: None,
        };
        let decoded = store.try_read_capture(namespace, &reference).unwrap();
        assert_eq!(
            decoded
                .iter()
                .map(|sample| sample.to_bits())
                .collect::<Vec<_>>(),
            expected
                .iter()
                .map(|sample| sample.to_bits())
                .collect::<Vec<_>>()
        );
    }
    let renamed_binding = StoreBinding::for_test(
        root.path().into(),
        owner_for_provider(&InMemoryProvider::from_seed([1; 32])),
        ProfileId("renamed".into()),
    );
    assert_eq!(first_binding.directory(), renamed_binding.directory());
    assert_eq!(
        std::fs::read_dir(root.path().join("personas/v1"))
            .unwrap()
            .count(),
        2
    );
    assert!(!root.path().join("genet-state.json").exists());
}

#[test]
fn scope_navigation_payload_reopens_from_filesystem_and_corrupt_route_bytes_are_retained() {
    let root = tempfile::tempdir().unwrap();
    let provider = InMemoryProvider::from_seed([13; 32]);
    let store = open_fixture(root.path(), &provider);
    let ui = UiState::new();
    let saved = ui.to_persisted();
    let route = saved.scope_navigation_json.clone().unwrap();
    let bytes = serde_json::to_string(&saved).unwrap();
    store.try_save(&bytes).unwrap();
    drop(store);

    let reopened_store = open_fixture(root.path(), &provider);
    let reopened_bytes = reopened_store.try_load().unwrap().unwrap();
    assert_eq!(reopened_bytes, bytes);
    let reopened_session = woodshed_core::storage::decode_session(&reopened_bytes)
        .unwrap()
        .session;
    assert_eq!(
        reopened_session.scope_navigation_json.as_deref(),
        Some(route.as_str())
    );
    let mut reopened_ui = UiState::new();
    reopened_ui.apply_persisted(&reopened_session, Default::default());
    assert_eq!(
        reopened_ui.to_persisted().scope_navigation_json.as_deref(),
        Some(route.as_str())
    );

    let mut corrupt_session = reopened_session;
    corrupt_session.scope_navigation_json = Some("{".into());
    let corrupt_bytes = serde_json::to_string(&corrupt_session).unwrap();
    reopened_store.try_save(&corrupt_bytes).unwrap();
    drop(reopened_store);

    let fresh_store = open_fixture(root.path(), &provider);
    let retained_corrupt_bytes = fresh_store.try_load().unwrap().unwrap();
    let mut fresh_ui = UiState::new();
    let corrupt_loaded = woodshed_core::storage::decode_session(&retained_corrupt_bytes)
        .unwrap()
        .session;
    fresh_ui.apply_persisted(&corrupt_loaded, Default::default());
    assert_eq!(
        fresh_ui.to_persisted().scope_navigation_json.as_deref(),
        Some("{")
    );
    assert_eq!(
        fresh_store.try_load().unwrap().as_deref(),
        Some(corrupt_bytes.as_str())
    );
    assert_eq!(retained_corrupt_bytes, corrupt_bytes);
}

#[test]
fn owner_initialization_authenticates_schema_identity_and_is_immutable() {
    let provider = InMemoryProvider::from_seed([3; 32]);
    for (version, purpose, foreign_owner) in [
        (2, OWNER_PURPOSE, false),
        (OWNER_VERSION, "another-purpose", false),
        (OWNER_VERSION, OWNER_PURPOSE, true),
    ] {
        let root = tempfile::tempdir().unwrap();
        let (backend, owner, directory) = sealed_at(root.path(), &provider);
        let bytes = serde_json::to_vec(&OwnerManifest {
            version,
            purpose: purpose.into(),
            owner: if foreign_owner {
                "0".repeat(64)
            } else {
                owner.clone()
            },
        })
        .unwrap();
        pollster::block_on(backend.put("owner", &bytes)).unwrap();
        let before = std::fs::read(directory.join("owner")).unwrap();
        assert!(initialize_owner(&backend, &directory, &owner).is_err());
        assert_eq!(std::fs::read(directory.join("owner")).unwrap(), before);
        assert!(!directory.join("session").exists());
    }
    let root = tempfile::tempdir().unwrap();
    let (backend, owner, directory) = sealed_at(root.path(), &provider);
    initialize_owner(&backend, &directory, &owner).unwrap();
    let original = std::fs::read(directory.join("owner")).unwrap();
    initialize_owner(&backend, &directory, &owner).unwrap();
    assert!(pollster::block_on(backend.put("owner", b"replacement")).is_err());
    assert!(pollster::block_on(backend.delete("owner")).is_err());
    assert_eq!(std::fs::read(directory.join("owner")).unwrap(), original);
    let foreign = SealedBackend::for_provider(
        FsBackend::at(&directory),
        &InMemoryProvider::from_seed([4; 32]),
    )
    .unwrap();
    assert!(initialize_owner(&foreign, &directory, &owner).is_err());
    assert_eq!(std::fs::read(directory.join("owner")).unwrap(), original);
}

#[test]
fn incomplete_unowned_directories_and_plaintext_never_become_owned_stores() {
    let provider = InMemoryProvider::from_seed([5; 32]);
    for name in ["session", "settings", "loop-pcm", ".owner.interrupted.tmp"] {
        let root = tempfile::tempdir().unwrap();
        let (backend, owner, directory) = sealed_at(root.path(), &provider);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join(name), b"unowned bytes").unwrap();
        assert!(initialize_owner(&backend, &directory, &owner).is_err());
        assert!(!directory.join("owner").exists());
        assert_eq!(
            std::fs::read(directory.join(name)).unwrap(),
            b"unowned bytes"
        );
    }
    let root = tempfile::tempdir().unwrap();
    let (backend, owner, directory) = sealed_at(root.path(), &provider);
    std::fs::create_dir_all(&directory).unwrap();
    let plain = serde_json::to_vec(&OwnerManifest {
        version: OWNER_VERSION,
        purpose: OWNER_PURPOSE.into(),
        owner: owner.clone(),
    })
    .unwrap();
    std::fs::write(directory.join("owner"), &plain).unwrap();
    assert!(initialize_owner(&backend, &directory, &owner).is_err());
    assert_eq!(std::fs::read(directory.join("owner")).unwrap(), plain);
}

#[test]
fn valid_owner_does_not_adopt_plaintext_session_or_settings() {
    let provider = InMemoryProvider::from_seed([6; 32]);
    let root = tempfile::tempdir().unwrap();
    let store = open_fixture(root.path(), &provider);
    let directory = persona_directory(root.path(), &owner_for_provider(&provider));
    std::fs::write(directory.join("session"), b"{}").unwrap();
    std::fs::write(directory.join("settings"), b"{}").unwrap();
    assert!(store.try_load().is_err());
    assert!(store.load_settings().is_none());
    assert!(store.write_refusal().is_some());
    assert!(store.try_save("default").is_err());
    assert!(store.try_save_settings("default").is_err());
    assert_eq!(std::fs::read(directory.join("session")).unwrap(), b"{}");
    assert_eq!(std::fs::read(directory.join("settings")).unwrap(), b"{}");
}

#[test]
fn family_choice_requires_stable_unlocked_profile_and_cryptographic_identity() {
    let provider = InMemoryProvider::from_seed([11; 32]);
    let master = provider.master_public_key().to_bytes();
    let owner = owner_for_provider(&provider);
    let profile = ProfileId("default".into());
    let stable = || ChoiceIdentity {
        profile: Some(profile.clone()),
        unlocked: true,
        public_key: Some(master),
    };
    assert!(admit_choice_identity(stable(), &profile, master, &owner).is_ok());
    let changed_seed = InMemoryProvider::from_seed([12; 32])
        .master_public_key()
        .to_bytes();
    for current in [
        ChoiceIdentity {
            unlocked: false,
            ..stable()
        },
        ChoiceIdentity {
            profile: None,
            ..stable()
        },
        ChoiceIdentity {
            profile: Some(ProfileId("another-profile".into())),
            ..stable()
        },
        ChoiceIdentity {
            public_key: None,
            ..stable()
        },
        ChoiceIdentity {
            public_key: Some(changed_seed),
            ..stable()
        },
    ] {
        assert!(admit_choice_identity(current, &profile, master, &owner).is_err());
    }
    assert!(
        admit_choice_identity(stable(), &profile, master, &"0".repeat(64)).is_err(),
        "a current public key must also match the installed cryptographic owner"
    );
    assert!(
        admit_choice_identity(stable(), &profile, changed_seed, &owner).is_err(),
        "a matching binding owner cannot substitute for the retained provider identity"
    );
}

#[test]
fn pending_store_is_empty_and_refuses_every_mutation() {
    let opened = pending_store("the vault is locked".into());
    assert!(matches!(opened.seal, PracticeSeal::Pending { .. }));
    assert!(opened.binding.is_none());
    assert!(opened.storage.try_load().unwrap().is_none());
    assert!(opened.storage.load_settings().is_none());
    assert!(
        pollster::block_on(opened.storage.backend().list(""))
            .unwrap()
            .is_empty()
    );
    assert!(opened.storage.try_save("must remain unsaved").is_err());
    assert!(pollster::block_on(opened.storage.backend().delete("session")).is_err());
    assert!(
        pollster::block_on(opened.storage.backend().apply(&[WriteOp::Put {
            key: "session".into(),
            value: vec![1]
        }]))
        .is_err()
    );
    assert!(opened.storage.try_load().unwrap().is_none());
}

#[cfg(unix)]
#[test]
fn root_resolution_preserves_symlink_parent_semantics_for_missing_destinations() {
    let directory = tempfile::tempdir().unwrap();
    let actual = directory.path().join("actual");
    std::fs::create_dir_all(actual.join("sub")).unwrap();
    std::os::unix::fs::symlink(actual.join("sub"), directory.path().join("link")).unwrap();
    let root = directory.path().join("link/../private");
    assert_eq!(
        absolute_root(&root).unwrap(),
        actual.canonicalize().unwrap().join("private")
    );
    assert!(
        !actual.join("private").exists(),
        "resolving isolation must not create storage"
    );
}

#[test]
fn configured_root_freezes_relative_paths_and_refuses_obsolete_or_empty_overrides() {
    for mode in ["relative", "obsolete-state", "obsolete-settings", "empty"] {
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "storage::persona_store_tests::configured_root_worker",
                "--nocapture",
            ])
            .env_remove("WOODSHED_STATE")
            .env_remove("WOODSHED_SETTINGS")
            .env_remove("WOODSHED_DATA_DIR")
            .env("WOODSHED_ROOT_TEST_MODE", mode);
        match mode {
            "relative" => {
                command.env("WOODSHED_DATA_DIR", "isolated-root");
            },
            "obsolete-state" => {
                command.env("WOODSHED_STATE", "");
            },
            "obsolete-settings" => {
                command.env("WOODSHED_SETTINGS", "");
            },
            "empty" => {
                command.env("WOODSHED_DATA_DIR", "");
            },
            _ => unreachable!(),
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "root test {mode} failed: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn configured_root_worker() {
    let Ok(mode) = std::env::var("WOODSHED_ROOT_TEST_MODE") else {
        return;
    };
    // Configuration is supplied before this isolated process starts; the
    // test runner's other threads never observe environment mutation.
    match mode.as_str() {
        "relative" => {
            let root = configured_root().unwrap();
            assert!(root.is_absolute());
            assert_eq!(
                root,
                std::env::current_dir()
                    .unwrap()
                    .canonicalize()
                    .unwrap()
                    .join("isolated-root")
            );
        },
        "obsolete-state" | "obsolete-settings" => {
            let name = if mode == "obsolete-state" {
                "WOODSHED_STATE"
            } else {
                "WOODSHED_SETTINGS"
            };
            let error = configured_root().unwrap_err();
            assert!(error.contains(name));
            assert!(error.contains("WOODSHED_DATA_DIR"));
        },
        "empty" => {
            assert!(configured_root().unwrap_err().contains("empty"));
        },
        _ => panic!("unknown root test mode"),
    }
}
