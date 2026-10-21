// SPDX-License-Identifier: MPL-2.0
use super::*;
use audio_pattern::Step;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "audio-pack-ap3-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn kit(&self) -> PathBuf {
        let target = self.0.join("installed");
        std::fs::create_dir(&target).unwrap();
        std::fs::create_dir(target.join("samples")).unwrap();
        let source =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/audio-packs/basic-drums");
        for f in [
            "audio-pack.json",
            "samples/kick.wav",
            "samples/snare.wav",
            "samples/hat.wav",
        ] {
            std::fs::copy(source.join(f), target.join(f)).unwrap();
        }
        target
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn bundled() -> PackSelection {
    files::load(std::path::Path::new("bundled"), None).unwrap()
}
#[test]
fn installed_edited_rhythm_survives_uninstallation_bit_exact() {
    let d = Directory::new();
    let kit = d.kit();
    let original = files::load(&kit, None).unwrap();
    let edited = original
        .edit(
            &original.revision(),
            PackEdit::MoveHit {
                track: 0,
                from: 0,
                to: 1,
            },
        )
        .unwrap();
    let midi = edited.midi().unwrap().bytes;
    let pcm = edited.render_bar(48_000).unwrap();
    let bytes = serde_json::to_vec(&edited).unwrap();
    std::fs::remove_dir_all(&kit).unwrap();
    let reopened: PackSelection = serde_json::from_slice(&bytes).unwrap();
    reopened.validate().unwrap();
    assert!(edited == reopened);
    assert_eq!(midi, reopened.midi().unwrap().bytes);
    assert_eq!(pcm, reopened.render_bar(48_000).unwrap());
}
#[test]
fn inactive_edits_survive_switch_and_serialization() {
    let d = Directory::new();
    let kit = d.kit();
    let p = kit.join("audio-pack.json");
    let mut v: serde_json::Value = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
    let id = v["patterns"]
        .as_object()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();
    v["patterns"]["alternate"] = v["patterns"][&id].clone();
    std::fs::write(p, serde_json::to_vec(&v).unwrap()).unwrap();
    let a = files::load(&kit, Some(&id)).unwrap();
    let a = a
        .edit(&a.revision(), PackEdit::CycleStep { track: 0, step: 1 })
        .unwrap();
    let b = a.select_pattern("alternate").unwrap();
    let saved = serde_json::to_vec(&b).unwrap();
    std::fs::remove_dir_all(kit).unwrap();
    let reopened: PackSelection = serde_json::from_slice(&saved).unwrap();
    let restored = reopened.select_pattern(&id).unwrap();
    assert_eq!(a.pattern(), restored.pattern());
    assert_eq!(a.midi().unwrap().bytes, restored.midi().unwrap().bytes);
    assert_eq!(
        a.render_bar(48_000).unwrap(),
        restored.render_bar(48_000).unwrap()
    );
}
#[test]
fn positional_edits_refuse_stale_occupied_or_invalid_targets() {
    let a = bundled();
    let revision = a.revision();
    let b = a
        .edit(&revision, PackEdit::CycleStep { track: 0, step: 1 })
        .unwrap();
    assert!(b.edit(&revision, PackEdit::MuteTrack(0)).is_err());
    assert!(
        a.edit(
            &revision,
            PackEdit::MoveHit {
                track: 0,
                from: 0,
                to: 8
            }
        )
        .is_err()
    );
    assert!(
        a.edit(&revision, PackEdit::CycleStep { track: 99, step: 0 })
            .is_err()
    );
    assert!(
        a.edit(
            &revision,
            PackEdit::MoveHit {
                track: 0,
                from: 1,
                to: 2
            }
        )
        .is_err()
    );
    assert_eq!(a.pattern().tracks[0].steps[1], Step::Empty);
}
#[test]
fn corrupted_assets_and_missing_files_are_refused() {
    let d = Directory::new();
    let kit = d.kit();
    let a = files::load(&kit, None).unwrap();
    std::fs::remove_file(kit.join("samples/kick.wav")).unwrap();
    assert!(files::load(&kit, None).is_err());
    let mut v = serde_json::to_value(&a).unwrap();
    v["snapshot"]["assets"]["samples/kick.wav"][0] = serde_json::json!(0);
    let b: PackSelection = serde_json::from_value(v).unwrap();
    assert!(b.validate().is_err());
    a.validate().unwrap();
}
#[test]
fn invalid_retained_drafts_tempo_gain_and_render_bounds_are_refused() {
    let a = bundled();
    assert!(a.at_tempo(f32::NAN).is_err());
    assert!(
        a.edit(
            &a.revision(),
            PackEdit::Gain {
                track: 0,
                delta: f32::INFINITY
            }
        )
        .is_err()
    );
    assert!(a.render_bars(0, 1).is_err());
    assert!(a.render_bars(48_000, 0).is_err());
    assert!(a.render_bars(192_000, 255).is_err());
    let mut v = serde_json::to_value(&a).unwrap();
    v["inactive_patterns"]["unknown"] = v["pattern"].clone();
    let b: PackSelection = serde_json::from_value(v).unwrap();
    assert!(b.validate().is_err());
}
#[test]
fn exports_refuse_existing_destination_without_modification() {
    let d = Directory::new();
    let p = d.0.join("keep.mid");
    std::fs::write(&p, b"keep").unwrap();
    assert!(files::export_midi(&bundled(), &p).is_err());
    assert_eq!(std::fs::read(p).unwrap(), b"keep");
}
