//! Isolated mixed-Set fixtures and read-only native acceptance checkpoints.
//! These operations run only through an explicitly supplied scenario. Playback
//! and history facts always come from the shipping host and output processor.
use std::{cell::RefCell, path::PathBuf, rc::Rc};

use serde_json::{Value, json};
use woodshed_core::{Lens, harmony::KeyedCatalogRef, history::EngagementKind, working_sets::WorkingSetId};
use woodshed_views::stage::UiState;
use woodshedding::rehearsal::{CardId, Hold, LoopMode, Set, Timing, Touch};

use super::{Ctx, Shared};

pub(super) fn extend_snapshot(
    snapshot: taproot::ProbeSnapshot,
    ui: &UiState,
) -> taproot::ProbeSnapshot {
    snapshot
        .with_field(
            "p6-loop-mode",
            match ui.set.loop_mode {
                LoopMode::Off => "off",
                LoopMode::All => "all",
                LoopMode::Card => "card",
            }
            .to_string(),
        )
        .with_field("p6-cursor", ui.set.cursor.to_string())
        .with_field("p6-close-pending", ui.practice_close_pending.to_string())
        .with_field(
            "p6-notice",
            ui.card_timing_notice.clone().unwrap_or_default(),
        )
}

pub(super) fn act(ctx: &mut Ctx<'_>, shared: &Rc<RefCell<Shared>>, label: &str) -> Option<bool> {
    if label == "preservation-request-close" {
        // Exercise the shipping close admission while keeping the scenario
        // alive to capture its decision UI. A native window request and frame
        // completion both call this same production function.
        return Some(
            crate::prepare_close(&mut shared.borrow_mut(), ctx)
                == cambium_genet_winit_host::CloseDisposition::KeepVisible,
        );
    }
    if label == "p6-mixed-set-example" {
        ctx.runner.update(seed);
        return Some(true);
    }
    if label == "p6-context-native-seed" {
        ctx.runner.update(seed_two_context_sets);
        return Some(true);
    }
    if let Some(name) = label.strip_prefix("p6-context-observation ") {
        return Some(write_context_observation(
            ctx.runner.state(),
            &shared.borrow(),
            name,
        ));
    }
    if let Some(name) = label.strip_prefix("p6-audio-observation ") {
        return Some(write_audio_observation(
            ctx.runner.state(),
            &shared.borrow(),
            name,
        ));
    }
    if let Some(name) = label.strip_prefix("p6-checkpoint ") {
        return Some(checkpoint(ctx.runner.state(), &shared.borrow(), name));
    }
    if label == "p6-verify-reopen" {
        return Some(verify_reopen(ctx.runner.state(), &shared.borrow()));
    }
    None
}

fn write_audio_observation(ui: &UiState, shared: &Shared, name: &str) -> bool {
    let Some(path) = path(name) else {
        return false;
    };
    let Some(backend) = shared.backend.as_ref() else {
        return false;
    };
    let Some(sample_rate) = backend.output_sample_rate() else {
        return false;
    };
    let Some(snapshot) = backend.output_snapshot() else {
        return false;
    };
    let program = snapshot.program;
    let value = json!({
        "sample_rate": sample_rate,
        "rehearsal_running": ui.rehearsal_running,
        "rehearsal_paused": ui.rehearsal_paused,
        "program": {
            "status": format!("{:?}", program.status),
            "card_index": program.card_index,
            "card_token": program.card_token,
            "event_index": program.event_index,
            "event_token": program.event_token,
            "device_frame": program.device_frame,
            "card_frames": program.card_frames,
            "active_frames": program.active_frames,
            "bpm": f64::from_bits(program.bpm_bits),
        }
    });
    let Ok(bytes) = serde_json::to_vec_pretty(&value) else {
        return false;
    };
    let Ok(mut file) = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    else {
        return false;
    };
    use std::io::Write;
    file.write_all(&bytes).is_ok()
}

fn seed(ui: &mut UiState) {
    ui.stop_rehearsal();
    ui.set = Set::default();
    ui.practice_history = Default::default();
    ui.retained_sets = Default::default();
    ui.working_sets = Default::default();
    for (lens, label, hold) in [
        (Lens::Chords, "Guided chord", Hold::Bars(1)),
        (Lens::Scales, "Guided scale", Hold::Seconds(2.0)),
        (Lens::Arpeggios, "Guided arpeggio", Hold::Reps(1)),
    ] {
        ui.stage.set_lens(lens);
        let mut card = ui.stage.card_from_lens().expect("built-in fixture lens");
        card.label = label.into();
        card.timing = Timing {
            bpm: Some(240.0),
            hold,
        };
        if lens == Lens::Scales {
            card.touch = Touch::Walk;
        }
        ui.set.push(card);
    }
    ui.set.cursor = 0;
    ui.set.loop_mode = LoopMode::Off;
    ui.set_tray_expanded = true;
    ui.set_graph_card_expanded = true;
    ui.select_app_section(woodshed_core::storage::AppSection::Rehearsal);
}

fn seed_two_context_sets(ui: &mut UiState) {
    seed(ui);
    let shared_material = ui.set.clone();
    ui.working_sets
        .create(&mut ui.set, "Working Set 2", shared_material);
    ui.set.cursor = 0;
    ui.set.loop_mode = LoopMode::Off;
    ui.select_app_section(woodshed_core::storage::AppSection::Rehearsal);
}

/// Read the actual sealed session and current product projection at a native
/// scenario checkpoint. This does not save or alter the session.
fn write_context_observation(ui: &UiState, shared: &Shared, name: &str) -> bool {
    if !matches!(
        name,
        "seed" | "reopened" | "back-overview" | "back-older-overview" | "returned"
    )
        || !ui.practice_saved
        || !matches!(
            shared.seal.as_ref(),
            Some(woodshed_views::persona::PracticeSeal::Sealed { .. })
        )
        || ui.working_sets.inactive.len() + 1 != 2
    {
        return false;
    }
    let Some(inactive) = ui.working_sets.inactive.first() else {
        return false;
    };
    let same_card_occurrences = ui.set.cards.len() == inactive.set.cards.len()
        && ui
            .set
            .cards
            .iter()
            .zip(&inactive.set.cards)
            .all(|(active, inactive)| active.id == inactive.id);
    if !same_card_occurrences {
        return false;
    }
    let Some(storage) = shared.storage.as_ref() else {
        return false;
    };
    let Some(stored_bytes) = storage.load() else {
        return false;
    };
    let Ok(stored) = woodshed_core::storage::decode_session(&stored_bytes) else {
        return false;
    };
    let live = ui.to_persisted();
    let Ok(live_value) = serde_json::to_value(&live) else {
        return false;
    };
    let Ok(stored_value) = serde_json::to_value(&stored.session) else {
        return false;
    };

    let music = serde_json::json!({
        "sets_by_owner": persisted_set_bank(&live_value),
        "retained_sets": live_value["retained_sets"].clone(),
        "catalog_explorations": live_value["catalog_explorations"].clone(),
        "song": live_value["song"].clone(),
        "musical_context": live_value["musical_context"].clone(),
        "looper": live_value["looper"].clone(),
        "loop_captures": live_value["loop_captures"].clone(),
        "practice_history": live_value["practice_history"].clone(),
    });
    let fields = [
        "set",
        "working_sets",
        "retained_sets",
        "catalog_explorations",
        "song",
        "musical_context",
        "looper",
        "loop_captures",
        "practice_history",
        "workspace_json",
        "scope_navigation_json",
        "overview_presentation_json",
    ];
    let stored_matches_live = fields
        .iter()
        .all(|field| live_value[*field] == stored_value[*field]);
    let route: serde_json::Value = live
        .scope_navigation_json
        .as_deref()
        .and_then(|value| serde_json::from_str(value).ok())
        .unwrap_or(serde_json::Value::Null);
    let card_context = card_context_facts(ui, &route);
    let workspace: serde_json::Value = live
        .workspace_json
        .as_deref()
        .and_then(|value| serde_json::from_str(value).ok())
        .unwrap_or(serde_json::Value::Null);
    let overview_presentation: serde_json::Value = live
        .overview_presentation_json
        .as_deref()
        .and_then(|value| serde_json::from_str(value).ok())
        .unwrap_or(serde_json::Value::Null);
    let program = shared
        .backend
        .as_ref()
        .and_then(|backend| backend.output_snapshot())
        .map(|snapshot| snapshot.program);
    let program_status = program.map(|snapshot| format!("{:?}", snapshot.status));
    let playback_stopped = !ui.rehearsal_running
        && !ui.rehearsal_paused
        && program_status.as_deref() == Some("Stopped");

    let mut cross_process = serde_json::Value::Null;
    if name != "seed" {
        let Some(expected_path) = path("context-seed") else {
            return false;
        };
        let expected = std::fs::read(expected_path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
        let Some(expected) = expected else {
            return false;
        };
        let same_music = expected.get("music") == Some(&music);
        let same_route = expected.get("route") == Some(&route);
        let same_view = expected.get("workspace") == Some(&workspace)
            && expected.get("overview_presentation") == Some(&overview_presentation)
            && expected.get("card_context") == Some(&card_context);
        cross_process = serde_json::json!({
            "seed_music_exact": same_music,
            "seed_route_exact": same_route,
            "seed_view_exact": same_view,
        });
        if !same_music || (name == "reopened" && (!same_route || !same_view)) {
            return false;
        }
    }

    let report = serde_json::json!({
        "phase": name,
        "sealed": true,
        "practice_saved": ui.practice_saved,
        "stored_matches_live": stored_matches_live,
        "music": music,
        "route": route,
        "current_visit_ordinal": route
            .pointer("/lineage/owners/0/current")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        "card_context": card_context,
        "workspace": workspace,
        "overview_presentation": overview_presentation,
        "active_working_set_id": ui.working_sets.active_id.0,
        "active_working_set_name": ui.working_sets.active_name,
        "working_set_count": ui.working_sets.inactive.len() + 1,
        "same_card_occurrences_in_both_sets": same_card_occurrences,
        "overview_zoom": ui.overview_viewport.zoom,
        "runner": {
            "running": ui.rehearsal_running,
            "paused": ui.rehearsal_paused,
            "program_status": program_status,
            "playback_stopped": playback_stopped,
        },
        "cross_process": cross_process,
    });
    let Ok(bytes) = serde_json::to_vec_pretty(&report) else {
        return false;
    };
    let Some(path) = path(&format!("context-{name}")) else {
        return false;
    };
    let Ok(mut file) = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    else {
        return false;
    };
    use std::io::Write;
    if file.write_all(&bytes).is_err() {
        return false;
    }
    stored_matches_live && playback_stopped && !route.is_null() && same_card_occurrences
}

/// Read the owner-qualified Card summary expected in the visible context strip.
/// This is derived from the current persisted navigation bookmark and model only.
fn card_context_facts(ui: &UiState, route: &Value) -> Value {
    let Some(ordinal) = route
        .pointer("/lineage/owners/0/current")
        .and_then(Value::as_u64)
        .and_then(|ordinal| usize::try_from(ordinal).ok())
    else {
        return Value::Null;
    };
    let Some(place) = route
        .pointer("/bookmarks")
        .and_then(Value::as_array)
        .and_then(|bookmarks| bookmarks.get(ordinal))
        .and_then(|bookmark| bookmark.get("value"))
        .and_then(|visit| visit.get("place"))
        .and_then(|place| place.get("CardOccurrence"))
    else {
        return Value::Null;
    };
    let (Some(owner), Some(card)) = (
        place.get("owner").and_then(Value::as_u64),
        place.get("card").and_then(Value::as_u64),
    ) else {
        return Value::Null;
    };
    let owner = WorkingSetId(owner);
    let card_id = CardId(card);
    let Some(set) = ui.working_sets.get(owner, &ui.set) else {
        return Value::Null;
    };
    let Some(card) = set.cards.iter().find(|candidate| candidate.id == card_id) else {
        return Value::Null;
    };
    let Some(owner_name) = ui
        .working_sets
        .summaries(&ui.set)
        .into_iter()
        .find(|summary| summary.id == owner)
        .map(|summary| summary.name)
    else {
        return Value::Null;
    };
    let material = KeyedCatalogRef::from_card(card)
        .and_then(|reference| reference.label())
        .unwrap_or_else(|| card.material.tag().to_owned());
    json!({
        "owner_id": owner.0,
        "owner_name": owner_name,
        "card_id": card.id.0,
        "card_label": card.label,
        "material_label": material,
    })
}

fn persisted_set_bank(session: &serde_json::Value) -> Vec<serde_json::Value> {
    let working_sets = &session["working_sets"];
    let mut sets = vec![serde_json::json!({
        "id": working_sets["active_id"].clone(),
        "name": working_sets["active_name"].clone(),
        "set": session["set"].clone(),
    })];
    if let Some(inactive) = working_sets["inactive"].as_array() {
        sets.extend(inactive.iter().cloned());
    }
    sets.sort_by_key(|set| set["id"].as_u64().unwrap_or_default());
    sets
}

fn path(name: &str) -> Option<PathBuf> {
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return None;
    }
    Some(PathBuf::from(std::env::var_os("WOODSHED_CAPTURE_DIR")?).join(format!("p6-{name}.json")))
}

fn report(ui: &UiState, shared: &Shared) -> Result<Value, String> {
    if !matches!(
        shared.seal,
        Some(woodshed_views::persona::PracticeSeal::Sealed { .. })
    ) {
        return Err("A real sealed isolated profile is required".into());
    }
    let stored = shared
        .storage
        .as_ref()
        .and_then(|storage| storage.load())
        .ok_or("The sealed session is unreadable")?;
    let stored = woodshed_core::storage::decode_session(&stored)
        .map_err(|e| e.to_string())?
        .session;
    let set = serde_json::to_value(&ui.set).map_err(|e| e.to_string())?;
    let history = serde_json::to_value(ui.practice_history.recent(ui.practice_history.len()))
        .map_err(|e| e.to_string())?;
    let saved_set = serde_json::to_value(&stored.set).map_err(|e| e.to_string())?;
    let saved_history = serde_json::to_value(
        stored
            .practice_history
            .recent(stored.practice_history.len()),
    )
    .map_err(|e| e.to_string())?;
    if !ui.practice_saved
        || ui.practice_save_notice.is_some()
        || set != saved_set
        || history != saved_history
    {
        return Err("Live Set/history and sealed saved state must match exactly".into());
    }
    let output = shared.backend.as_ref().and_then(|b| b.output_snapshot());
    let program = output.map(|out| out.program);
    Ok(json!({
        "set": set, "history": history, "owner": ui.working_sets.active_id.0,
        "history_count": ui.practice_history.len(), "sealed_exact": true,
        "running": ui.rehearsal_running, "paused": ui.rehearsal_paused,
        "transient_cleared": !ui.rehearsal_running && !ui.rehearsal_paused
            && ui.rehearsal_clock_program.is_none() && ui.practice_clock_projection.is_none()
            && ui.practice_clock_commands.is_empty(),
        "program": program.map(|p| json!({"epoch":p.epoch.0, "status":format!("{:?}",p.status),
            "card_index":p.card_index, "event_token":p.event_token, "active_frames":p.active_frames,
            "card_frames":p.card_frames, "beat_ticks":p.beat_ticks, "device_frame":p.device_frame})),
        "sample_rate": shared.backend.as_ref().and_then(|b| b.output_sample_rate()),
    }))
}

fn checkpoint(ui: &UiState, shared: &Shared, name: &str) -> bool {
    let result = (|| -> Result<(), String> {
        let path = path(name).ok_or("Invalid checkpoint name or directory")?;
        let value = report(ui, shared)?;
        if name == "set-loop" {
            let events = ui.practice_history.recent(ui.practice_history.len());
            for card in &ui.set.cards {
                if !events.iter().any(|event| {
                    event.kind == EngagementKind::Completed
                        && event
                            .provenance
                            .as_ref()
                            .is_some_and(|p| p.occurrence_id == card.id)
                }) {
                    return Err(format!(
                        "Card {} never completed before the Set loop",
                        card.id.0
                    ));
                }
            }
        }
        if name == "card-loop" {
            let completed = ui
                .practice_history
                .recent(ui.practice_history.len())
                .into_iter()
                .filter(|e| {
                    e.kind == EngagementKind::Completed
                        && e.provenance
                            .as_ref()
                            .is_some_and(|p| p.occurrence_id == ui.set.cards[0].id)
                })
                .count();
            if ui.set.loop_mode != LoopMode::Card || ui.set.cursor != 0 || completed < 3 {
                return Err("Focused Card must repeat from accepted device completions".into());
            }
        }
        if name == "final"
            && (!value["transient_cleared"].as_bool().unwrap_or(false)
                || ui.practice_history.is_empty())
        {
            return Err(
                "Final checkpoint requires a stopped run and accepted practice history".into(),
            );
        }
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| e.to_string())?;
        file.write_all(serde_json::to_string_pretty(&value).unwrap().as_bytes())
            .map_err(|e| e.to_string())?;
        Ok(())
    })();
    if let Err(error) = &result {
        eprintln!("P6 native checkpoint refused: {error}");
    }
    result.is_ok()
}

fn verify_reopen(ui: &UiState, shared: &Shared) -> bool {
    let result = (|| -> Result<(), String> {
        let expected: Value = serde_json::from_slice(
            &std::fs::read(path("final").ok_or("No expected checkpoint")?)
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let actual = report(ui, shared)?;
        for field in ["set", "history", "owner"] {
            if expected[field] != actual[field] {
                return Err(format!("Reopened {field} differs"));
            }
        }
        if !actual["transient_cleared"].as_bool().unwrap_or(false) {
            return Err("Runtime state did not reopen cold".into());
        }
        std::fs::write(
            path("reopened").ok_or("No capture directory")?,
            serde_json::to_vec_pretty(&actual).unwrap(),
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })();
    if let Err(error) = &result {
        eprintln!("P6 native reopen refused: {error}");
    }
    result.is_ok()
}
