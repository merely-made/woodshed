//! Explicitly invoked synthetic native/reopen fixture; never a normal startup path.
use crate::storage::{configured_root, open_store_at};
use woodshed_core::loop_capture::{
    CAPTURE_VERSION, LoopCaptureCodec, LoopCaptureRef, PER_BEAT_CLOCK,
};
use woodshed_core::loop_plan::LoopSegmentId;
use woodshed_views::stage::UiState;

#[test]
#[ignore = "requires an owned isolated custody keeper and WOODSHED_DATA_DIR"]
fn seed_or_verify_native_persona_stores() {
    assert!(
        std::env::var_os("GRAPHSHELL_APP_ENDPOINT").is_some(),
        "explicit isolated keeper required"
    );
    assert!(
        std::env::var_os("WOODSHED_DATA_DIR").is_some(),
        "explicit isolated data root required"
    );
    let root = configured_root().unwrap();
    let mode = std::env::var("WOODSHED_PERSONA_RECEIPT_MODE").unwrap();
    assert!(matches!(mode.as_str(), "seed" | "verify"));
    let output =
        std::path::PathBuf::from(std::env::var_os("WOODSHED_PERSONA_RECEIPT_OUTPUT").unwrap());
    let mut reports = Vec::new();
    for (index, profile) in ["default", "d1-b"].iter().enumerate() {
        let opened = open_store_at(&root, Some(&personae::ProfileId((*profile).into())));
        let binding = opened
            .binding
            .as_ref()
            .expect("owned isolated persona must open");
        let value = if index == 0 { 0.25_f32 } else { -0.5_f32 };
        let theme = if index == 0 { "Ember" } else { "Dusk" };
        let label = if index == 0 {
            "D1 A phrase"
        } else {
            "D1 B phrase"
        };
        if mode == "seed" {
            assert!(
                opened.storage.try_load().unwrap().is_none(),
                "seed only a new fixture"
            );
            let mut ui = UiState::new();
            for _ in 0..=index {
                ui.stage_current(None);
            }
            for card in &mut ui.set.cards {
                card.label = label.into();
            }
            ui.app_settings.appearance.theme = theme.into();
            ui.app_settings.metronome.bpm = if index == 0 { 87.0 } else { 143.0 };
            let namespace = format!("{:032x}", index + 1);
            let (digest, frames) = opened
                .storage
                .try_write_capture(&namespace, 1_000, &vec![value; 2_000])
                .unwrap();
            ui.loop_captures.version = CAPTURE_VERSION;
            ui.loop_captures.namespace = Some(namespace);
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
            ui.seal = Some(opened.seal.clone());
            ui.practice_saved = true;
            crate::session::ensure_personal_relation_source(&opened.storage, &mut ui);
            opened
                .storage
                .try_save_settings(&serde_json::to_string(&ui.app_settings).unwrap())
                .unwrap();
            opened
                .storage
                .try_save(&serde_json::to_string(&ui.to_persisted()).unwrap())
                .unwrap();
        }
        let ui = crate::session::admit(&opened.storage).unwrap();
        assert_eq!(ui.set.cards.len(), index + 1);
        assert!(ui.set.cards.iter().all(|card| card.label == label));
        assert_eq!(ui.app_settings.appearance.theme, theme);
        assert_eq!(ui.loop_captures.takes.len(), 1);
        let samples = opened
            .storage
            .try_read_capture(
                ui.loop_captures.namespace.as_deref().unwrap(),
                &ui.loop_captures.takes[0],
            )
            .unwrap();
        assert_eq!(samples.len(), 2_000);
        assert!(
            samples
                .iter()
                .all(|sample| sample.to_bits() == value.to_bits())
        );
        let mut bits = Vec::new();
        for sample in &samples {
            bits.extend_from_slice(&sample.to_bits().to_le_bytes());
        }
        reports.push(serde_json::json!({
            "profile":profile,"owner":binding.owner(),"cards":ui.set.cards,
            "settings":ui.app_settings,"captures":ui.loop_captures,
            "personal_relations":ui.to_persisted().personal_relations,
            "pcm_frames":samples.len(),"pcm_bits_blake3":blake3::hash(&bits).to_hex().to_string(),
        }));
    }
    let default = open_store_at(&root, Some(&personae::ProfileId("default".into())));
    default
        .binding
        .unwrap()
        .choose_profile(&personae::ProfileId("default".into()))
        .unwrap();
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .unwrap();
    serde_json::to_writer_pretty(file, &reports).unwrap();
}
