use super::*;
#[test]
#[ignore = "run against an isolated absent/locked custody keeper with WOODSHED_DATA_DIR"]
fn dr_c_woodshed_identity_stays_pending() {
    let mode = std::env::var("DR_C_RECEIPT_MODE").expect("receipt mode");
    assert!(mode == "absent" || mode == "locked");
    let root = configured_root().expect("isolated application root");
    assert!(std::env::var_os("WOODSHED_DATA_DIR").is_some());
    std::fs::create_dir_all(&root).unwrap();
    let legacy = root.join("genet-state.json");
    std::fs::write(&legacy, b"already-public practice").unwrap();
    let opened = open_store_at(&root, None);
    assert!(matches!(opened.seal, PracticeSeal::Pending { .. }));
    assert!(opened.binding.is_none());
    assert!(opened.storage.try_load().unwrap().is_none());
    assert!(
        opened
            .storage
            .try_save("must never be written in the clear")
            .is_err()
    );
    assert!(pollster::block_on(opened.storage.backend().delete("session")).is_err());
    assert_eq!(std::fs::read(&legacy).unwrap(), b"already-public practice");
    assert!(!root.join("personas").exists());
    let chosen = open_store_at(&root, Some(&ProfileId("default".into())));
    assert!(matches!(chosen.seal, PracticeSeal::Pending { .. }));
    assert!(chosen.binding.is_none());
}
