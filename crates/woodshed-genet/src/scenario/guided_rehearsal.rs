//! Isolated mixed-Set fixtures and read-only native acceptance checkpoints.
//! These operations run only through an explicitly supplied scenario. Playback
//! and history facts always come from the shipping host and output processor.
use std::{cell::RefCell, path::PathBuf, rc::Rc};

use serde_json::{Value, json};
use woodshed_core::{Lens, history::EngagementKind};
use woodshed_views::stage::UiState;
use woodshedding::rehearsal::{Hold, LoopMode, Set, Timing, Touch};

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
    if let Some(name) = label.strip_prefix("p6-checkpoint ") {
        return Some(checkpoint(ctx.runner.state(), &shared.borrow(), name));
    }
    if label == "p6-verify-reopen" {
        return Some(verify_reopen(ctx.runner.state(), &shared.borrow()));
    }
    None
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
