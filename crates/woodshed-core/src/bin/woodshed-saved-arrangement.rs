//! Explicit file exchange over normal session/Set data, separate from the
//! desktop's sealed store. Inputs are read-only; outputs never replace files.
use std::{
    io::{Read, Write},
    path::Path,
};
use woodshed_core::{
    arrangement_scene::saved_arrangement_scene,
    retained_sets::SavedSetId,
    saved_arrangements::{SessionSet, save_progression_arrangement},
    settings::AppSettings,
    storage::decode_session,
    working_sets::WorkingSetId,
};
use woodshedding::rehearsal::arrangement::ProgressionArrangement;

const USAGE: &str = "usage:\n  woodshed-saved-arrangement attach SESSION.json SETTINGS.json working SET_ID ALIGNMENT.json NEW_SESSION.json\n  woodshed-saved-arrangement export SESSION.json working|retained SET_ID READING_ID NEW_SCENE.json";
fn read(path: &str, max: u64) -> Result<String, Box<dyn std::error::Error>> {
    let mut text = String::new();
    std::fs::File::open(path)?
        .take(max + 1)
        .read_to_string(&mut text)?;
    if text.len() as u64 > max {
        return Err("Input exceeds its byte budget".into());
    }
    Ok(text)
}
fn output(path: &str, value: &impl serde::Serialize) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = serde_json::to_vec_pretty(value)?;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(Path::new(path))?
        .write_all(&bytes)?;
    Ok(())
}
fn selector(kind: &str, id: &str) -> Result<SessionSet, Box<dyn std::error::Error>> {
    let id: u64 = id.parse()?;
    if id == 0 {
        return Err("Set identity must be nonzero".into());
    }
    match kind {
        "working" => Ok(SessionSet::Working(WorkingSetId(id))),
        "retained" => Ok(SessionSet::Retained(SavedSetId(id))),
        _ => Err(USAGE.into()),
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = std::env::args().skip(1).collect();
    match a.first().map(String::as_str) {
        Some("attach") if a.len() == 7 => {
            if a[3] != "working" {
                return Err("Retained snapshots are read-only; open a working copy before authoring alignment".into());
            }
            let raw = read(&a[1], 16 * 1024 * 1024)?;
            let mut value: serde_json::Value = serde_json::from_str(&raw)?;
            let mut session = decode_session(&raw)?.session;
            let settings: AppSettings = serde_json::from_str(&read(&a[2], 1024 * 1024)?)?;
            let owner = selector(&a[3], &a[4])?;
            let alignment: ProgressionArrangement =
                serde_json::from_str(&read(&a[5], 1024 * 1024)?)?;
            let mut stage = woodshed_core::StageState::new();
            session.restore(&mut stage, &settings);
            stage.apply_neck(settings.fretboard.neck_start, settings.fretboard.neck_end);
            let set = owner
                .working_mut(&mut session)
                .ok_or("Set identity is absent from this session")?;
            save_progression_arrangement(&stage, set, alignment)?;
            // Refuse bad geometry or stale selected shapes before publishing.
            saved_arrangement_scene(set, "attachment-validation")?;
            let attachment = serde_json::to_value(&set.progression_arrangement)?;
            let id: u64 = a[4].parse()?;
            let raw_set = match owner {
                SessionSet::Working(_) if id == session.working_sets.active_id.0 => {
                    value.get_mut("set")
                },
                SessionSet::Working(_) => value
                    .pointer_mut("/working_sets/inactive")
                    .and_then(|v| v.as_array_mut())
                    .and_then(|items| items.iter_mut().find(|v| v["id"].as_u64() == Some(id)))
                    .and_then(|v| v.get_mut("set")),
                SessionSet::Retained(_) => None,
            }
            .and_then(|v| v.as_object_mut())
            .ok_or("Source has no explicit Set object")?;
            // Preserve every other source key, including fields from newer hosts.
            raw_set.insert("progression_arrangement".into(), attachment);
            output(&a[6], &value)?;
            println!("Saved arrangement in a new session copy: {}", a[6]);
        },
        Some("export") if a.len() == 6 => {
            let session = decode_session(&read(&a[1], 16 * 1024 * 1024)?)?.session;
            let owner = selector(&a[2], &a[3])?;
            let set = owner
                .get(&session)
                .ok_or("Set identity is absent from this session")?;
            let document = saved_arrangement_scene(set, &a[4])?;
            output(&a[5], &document)?;
            println!(
                "Exported {} with {} saved voicings",
                document.primary.title,
                document.linked.len()
            );
        },
        _ => return Err(USAGE.into()),
    }
    Ok(())
}
