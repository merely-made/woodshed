use super::*;
#[test]
#[ignore = "run mere/scripts/dr_c_receipts.py against the isolated receipt keeper"]
fn dr_c_hocket_identity_stays_pending() {
    let mode = std::env::var("DR_C_RECEIPT_MODE").expect("receipt mode");
    assert!(mode == "absent" || mode == "locked");
    if mode == "locked" {
        let mut client = castellan_client::custody_client::BlockingCustodyClient::open_declared(
            ostiary::app_admission::AppId::new(CUSTODY_APP),
            app_declaration(),
        )
        .unwrap();
        assert_eq!(
            client.status().unwrap().lock,
            dramatis::view::VaultLockView::Locked
        );
        assert!(
            !client.roster().unwrap().entries.is_empty(),
            "public persona roster remains readable"
        );
    }
    let temp = tempfile::tempdir().unwrap();
    let public = temp.path().join("public-session");
    std::fs::write(&public, b"public session retained").unwrap();
    let result = LocalIdentity::open_at(temp.path().to_path_buf());
    assert!(
        matches!(result, Err(ref error) if error.to_string().contains("pending")),
        "Hocket must expose no contact token or signing identity"
    );
    assert_eq!(std::fs::read(&public).unwrap(), b"public session retained");
    assert_eq!(
        std::fs::read_dir(temp.path()).unwrap().count(),
        1,
        "no identity, adoption or consent is written by Hocket while pending"
    );
    // Also exercise the previously-owned identity branch. These synthetic
    // bytes are never decoded by Hocket: only djinn may adopt them.
    let legacy = temp.path().join(LEGACY_ROOT);
    std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
    std::fs::write(&legacy, b"opaque legacy root").unwrap();
    assert!(
        matches!(LocalIdentity::open_at(temp.path().to_path_buf()),
        Err(ref error) if error.to_string().contains("pending")),
        "a legacy identity must not be decoded or used as a fallback while pending"
    );
    assert_eq!(std::fs::read(legacy).unwrap(), b"opaque legacy root");
    assert!(!temp.path().join(MARKER).exists());
    assert!(!consent_path(temp.path()).exists());
}
