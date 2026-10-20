//! Explicit, scenario-only advancement of the real software output processor.
//! Frame counts and PCM come from OutputRuntime; wall time never drives this lane.
use super::{Ctx, Product, Shared};
use taproot::ProbeSnapshot;
use woodshed_views::stage::{UiState, music_editor::practice::PracticeStatus};

const MAX_ADVANCE: usize = 480_000;
const BLOCK: usize = 8192;

pub(super) fn enabled() -> bool {
    receipt_enabled(
        std::env::var("WOODSHED_MUSIC_PRACTICE_RECEIPT")
            .ok()
            .as_deref(),
        std::env::var_os("WOODSHED_SCENARIO").is_some(),
    )
}
fn receipt_enabled(flag: Option<&str>, scenario: bool) -> bool {
    flag == Some("1") && scenario
}

pub(super) struct Receipt {
    pcm: blake3::Hasher,
    frames: u64,
    samples: u64,
    nonzero: u64,
    advances: Vec<serde_json::Value>,
}
impl Default for Receipt {
    fn default() -> Self {
        Self {
            pcm: blake3::Hasher::new(),
            frames: 0,
            samples: 0,
            nonzero: 0,
            advances: vec![],
        }
    }
}

fn advance_frames(label: &str) -> Option<Result<usize, String>> {
    label.strip_prefix("music-clock-advance-").map(|number| {
        number
            .parse::<usize>()
            .ok()
            .filter(|n| (1..=MAX_ADVANCE).contains(n))
            .ok_or_else(|| format!("Music clock advance requires 1..={MAX_ADVANCE} output frames"))
    })
}

pub(super) fn act(product: &mut Product, ctx: &mut Ctx<'_>, label: &str) -> Option<bool> {
    if let Some(frames) = advance_frames(label) {
        return Some(report((|| {
            if !enabled() {
                return Err("Music practice receipt frame pump is not enabled".into());
            }
            let frames = frames?;
            let mut shared = product.shared.borrow_mut();
            let backend = shared.backend.as_mut().ok_or("No receipt output backend")?;
            let start = backend
                .output_snapshot()
                .ok_or("No output snapshot")?
                .device_frames;
            let mut hasher = blake3::Hasher::new();
            let mut samples = 0_u64;
            let mut nonzero = 0_u64;
            let mut blocks = 0_u64;
            let receipt = &mut product.music_practice_receipt;
            let result = ctx.runner.update_deferred(|ui| -> Result<(), String> {
                let mut left = frames;
                while left > 0 {
                    backend.service();
                    backend.sync_practice_clock(ui);
                    let count = left.min(BLOCK);
                    let pcm = backend.advance_receipt_frames(count)?;
                    if pcm.len() != count * 2 {
                        return Err(
                            "Output processor returned an unexpected stereo PCM length".into()
                        );
                    }
                    for sample in &pcm {
                        let bytes = sample.to_bits().to_le_bytes();
                        hasher.update(&bytes);
                        receipt.pcm.update(&bytes);
                        nonzero += u64::from(*sample != 0.0);
                    }
                    samples += pcm.len() as u64;
                    receipt.samples += pcm.len() as u64;
                    receipt.frames += count as u64;
                    receipt.nonzero += pcm.iter().filter(|sample| **sample != 0.0).count() as u64;
                    blocks += 1;
                    left -= count;
                    backend.service();
                    backend.sync_practice_clock(ui);
                }
                Ok(())
            });
            result?;
            let end = backend
                .output_snapshot()
                .ok_or("No output snapshot after rendering")?
                .device_frames;
            if end.checked_sub(start) != Some(frames as u64) {
                return Err(
                    "Output device frame delta differs from the rendered receipt frames".into(),
                );
            }
            receipt.advances.push(serde_json::json!({"requested_frames":frames,"rendered_samples":samples,"nonzero_samples":nonzero,"processor_blocks":blocks,"device_frame_start":start,"device_frame_end":end,"pcm_f32le_blake3":hasher.finalize().to_hex().to_string()}));
            drop(shared);
            ctx.runner.update(|_| {});
            Ok(())
        })()));
    }
    if let Some(name) = label.strip_prefix("music-practice-checkpoint-") {
        return Some(report(checkpoint(product, ctx, name)));
    }
    None
}

fn status(status: PracticeStatus) -> &'static str {
    match status {
        PracticeStatus::Stopped => "stopped",
        PracticeStatus::Playing => "playing",
        PracticeStatus::Paused => "paused",
        PracticeStatus::Complete => "complete",
    }
}
fn score_hash(score: Option<&woodshedding::notation::Score>) -> Option<String> {
    score
        .and_then(|s| woodshedding::notation::MusicDocument::from_score(s).ok())
        .map(|doc| blake3::hash(doc.musicxml.as_bytes()).to_hex().to_string())
}
fn saved_hash(ui: &UiState) -> Option<String> {
    ui.set
        .music
        .as_ref()
        .map(|doc| blake3::hash(doc.musicxml.as_bytes()).to_hex().to_string())
}

pub(super) fn extend_snapshot(
    mut snapshot: ProbeSnapshot,
    ui: &UiState,
    shared: &Shared,
) -> ProbeSnapshot {
    let practice = &ui.music_editor.practice;
    let head = practice.playhead();
    snapshot = snapshot
        .with_field("music-practice-receipt", enabled().to_string())
        .with_field("music-practice-status", status(practice.status()))
        .with_field(
            "music-practice-requested",
            ui.music_practice_requested().to_string(),
        )
        .with_field(
            "music-playback-bar",
            head.map(|p| p.bar_index.to_string())
                .unwrap_or_else(|| "none".into()),
        )
        .with_field(
            "music-playback-event",
            head.map(|p| p.event_index.to_string())
                .unwrap_or_else(|| "none".into()),
        )
        .with_field(
            "music-playback-rest",
            head.is_some_and(|p| p.is_rest).to_string(),
        )
        .with_field(
            "music-playback-ticks",
            head.map_or(0, |p| p.bar_beat_ticks).to_string(),
        )
        .with_field(
            "music-applied-revision",
            practice.applied_revision().to_string(),
        )
        .with_field(
            "music-desired-revision",
            practice.desired_revision().to_string(),
        )
        .with_field(
            "music-pending-update",
            practice.pending_update().to_string(),
        )
        .with_field(
            "music-practice-bpm",
            practice
                .tempo_bpm()
                .map(|p| p.to_string())
                .unwrap_or_else(|| "none".into()),
        )
        .with_field(
            "music-source-hash",
            score_hash(ui.music_draft().map(|d| &d.score)).unwrap_or_else(|| "none".into()),
        )
        .with_field(
            "music-applied-source-hash",
            score_hash(practice.applied_score()).unwrap_or_else(|| "none".into()),
        )
        .with_field(
            "music-saved-hash",
            saved_hash(ui).unwrap_or_else(|| "none".into()),
        );
    let output = shared
        .backend
        .as_ref()
        .and_then(|backend| backend.output_snapshot());
    snapshot
        .with_field(
            "output-frame",
            output.map_or(0, |o| o.device_frames).to_string(),
        )
        .with_field(
            "music-device-revision",
            output.map_or(0, |o| o.program.revision).to_string(),
        )
        .with_field(
            "music-device-ticks",
            output.map_or(0, |o| o.program.beat_ticks).to_string(),
        )
}

fn checkpoint(product: &Product, ctx: &Ctx<'_>, name: &str) -> Result<(), String> {
    if !enabled() {
        return Err("Music practice receipt checkpoint is not enabled".into());
    }
    if name.is_empty()
        || name.len() > 80
        || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err("Checkpoint needs a simple name of at most 80 characters".into());
    }
    let directory = std::env::var_os("WOODSHED_CAPTURE_DIR")
        .ok_or("Music practice checkpoint requires an isolated capture directory")?;
    let ui = ctx.runner.state();
    let shared = product.shared.borrow();
    let backend = shared.backend.as_ref().ok_or("No receipt output backend")?;
    let output = backend.output_snapshot().ok_or("No output snapshot")?;
    let p = output.program;
    let practice = &ui.music_editor.practice;
    let stored = shared
        .storage
        .as_ref()
        .and_then(|store| store.load())
        .map(|text| woodshed_core::storage::decode_session(&text).map(|decoded| decoded.session))
        .transpose()
        .map_err(|e| e.to_string())?;
    let stored_hash = stored
        .as_ref()
        .and_then(|session| session.set.music.as_ref())
        .map(|doc| blake3::hash(doc.musicxml.as_bytes()).to_hex().to_string());
    let receipt = &product.music_practice_receipt;
    let value = serde_json::json!({
        "kind":"woodshed-native-music-practice-output-runtime",
        "owner":ui.working_sets.active_id.0,
        "authoring_selection":ui.music_draft().map(|d|d.selected),
        "playback":{"status":status(practice.status()),"owner":practice.owner().map(|owner|owner.0),"run_epoch":practice.run_epoch(),"bar":practice.playhead().map(|p|p.bar_index),"event":practice.playhead().map(|p|p.event_index),"rest":practice.playhead().map(|p|p.is_rest),"bar_beat_ticks":practice.playhead().map(|p|p.bar_beat_ticks),"applied_revision":practice.applied_revision(),"desired_revision":practice.desired_revision(),"pending_update":practice.pending_update(),"bpm":practice.tempo_bpm()},
        "device":{"output_frame":output.device_frames,"processing_blocks":output.processing_blocks,"sample_rate":backend.output_sample_rate(),"native_stream_count":backend.output_stream_count(),"program_status":format!("{:?}",p.status),"program_epoch":p.epoch.0,"program_revision":p.revision,"bar":p.card_index,"event":p.event_index,"beat_ticks":p.beat_ticks,"card_frames":p.card_frames,"active_frames":p.active_frames,"bpm":f64::from_bits(p.bpm_bits)},
        "music":{"draft_canonical_xml_blake3":score_hash(ui.music_draft().map(|d|&d.score)),"applied_canonical_xml_blake3":score_hash(practice.applied_score()),"saved_exact_xml_blake3":saved_hash(ui),"stored_exact_xml_blake3":stored_hash,"draft_dirty":ui.music_draft().is_some_and(|d|d.dirty())},
        "pcm":{"encoding":"f32 IEEE-754 bit patterns, little endian, interleaved stereo","frames":receipt.frames,"samples":receipt.samples,"nonzero_samples":receipt.nonzero,"cumulative_blake3":receipt.pcm.finalize().to_hex().to_string(),"advances":receipt.advances}
    });
    use std::io::Write;
    let path = std::path::PathBuf::from(directory).join(format!("music-practice-{name}.json"));
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    file.write_all(&serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
fn report(result: Result<(), String>) -> bool {
    match result {
        Ok(()) => true,
        Err(error) => {
            eprintln!("Music practice receipt refused: {error}");
            false
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn receipt_requires_both_explicit_flag_and_scenario() {
        assert!(receipt_enabled(Some("1"), true));
        for flag in [None, Some("0"), Some("true")] {
            assert!(!receipt_enabled(flag, true));
        }
        assert!(!receipt_enabled(Some("1"), false));
    }
    #[test]
    fn frame_actions_are_bounded() {
        assert_eq!(
            advance_frames("music-clock-advance-480000")
                .unwrap()
                .unwrap(),
            480000
        );
        for suffix in ["0", "480001", "-1", "nan", ""] {
            assert!(
                advance_frames(&format!("music-clock-advance-{suffix}"))
                    .unwrap()
                    .is_err()
            );
        }
        assert!(advance_frames("ordinary-action").is_none());
    }
}
