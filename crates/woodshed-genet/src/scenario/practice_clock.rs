//! Native scenario controls and receipts for the device-owned practice clock.

use std::{cell::RefCell, path::PathBuf, rc::Rc, time::Instant};

use taproot::ProbeSnapshot;
use woodshed_audio::output_runtime::{OutputSnapshot, ProgramSnapshot, ProgramStatus};
use woodshed_core::Lens;
use woodshed_views::stage::UiState;

use super::{Product, Shared};

pub(super) struct Wait {
    pub(super) card_index: usize,
    pub(super) status: ProgramStatus,
    pub(super) started: Instant,
}

impl Wait {
    pub(super) fn matches(&self, snapshot: Option<OutputSnapshot>) -> bool {
        snapshot.is_some_and(|snapshot| {
            snapshot.program.card_index == self.card_index && snapshot.program.status == self.status
        })
    }
}

pub(super) fn product_act(
    product: &mut Product,
    ctx: &mut super::Ctx<'_>,
    label: &str,
) -> Option<bool> {
    if let Some(target) = wait_target(label) {
        product.practice_clock_wait.replace(Some(Wait {
            card_index: target.0,
            status: target.1,
            started: Instant::now(),
        }));
        return Some(true);
    }
    match label {
        "p5clock-save-paused" => Some(save_program_point(
            &product.shared,
            "p5clock-paused.json",
            |point| point.status == "paused",
        )),
        "p5clock-check-paused" => Some(check_saved_position(
            &product.shared,
            "p5clock-paused.json",
            |before, now| {
                same_position(before, now)
                    && now.status == "paused"
                    && now.device_frame > before.device_frame
            },
        )),
        "p5clock-check-paused-retime" => Some(check_saved_position(
            &product.shared,
            "p5clock-paused.json",
            |before, now| {
                same_position(before, now) && now.status == "paused" && now.bpm == Some(30.0)
            },
        )),
        "p5clock-save-resume-point" => Some(save_program_point(
            &product.shared,
            "p5clock-resume.json",
            |point| point.status == "paused",
        )),
        "p5clock-check-resumed" => Some(check_saved_position(
            &product.shared,
            "p5clock-resume.json",
            |before, now| {
                now.epoch == before.epoch
                    && now.card_index == before.card_index
                    && now.status == "playing"
                    && now.beat_ticks > before.beat_ticks
                    && now.card_frames > before.card_frames
            },
        )),
        "p5clock-save-seconds-point" => Some(save_program_point(
            &product.shared,
            "p5clock-seconds.json",
            |point| point.card_index == 1 && point.status == "playing",
        )),
        "p5clock-check-seconds-retime" => Some(check_saved_position(
            &product.shared,
            "p5clock-seconds.json",
            |before, now| {
                now.epoch == before.epoch
                    && now.card_index == 1
                    && now.status == "playing"
                    && now.card_frames >= before.card_frames
                    && now.bpm == Some(30.0)
            },
        )),
        "p5clock-save-manual-point" => Some(save_program_point(
            &product.shared,
            "p5clock-manual.json",
            |point| point.card_index == 2 && point.status == "waiting_manual",
        )),
        "p5clock-check-manual-wait" => Some(check_saved_position(
            &product.shared,
            "p5clock-manual.json",
            |before, now| {
                same_transport_position(before, now)
                    && now.status == "waiting_manual"
                    && now.device_frame > before.device_frame
                    && now.active_frames > before.active_frames
            },
        )),
        "p5clock-save-final" => Some(save_final(ctx, &product.shared)),
        "p5clock-verify-reopen" => Some(verify_reopen(ctx, &product.shared)),
        _ => None,
    }
}

fn wait_target(label: &str) -> Option<(usize, ProgramStatus)> {
    match label {
        "p5clock-await-card-0" => Some((0, ProgramStatus::Playing)),
        "p5clock-await-card-1" => Some((1, ProgramStatus::Playing)),
        "p5clock-await-manual" => Some((2, ProgramStatus::WaitingManual)),
        "p5clock-await-complete" => Some((2, ProgramStatus::Complete)),
        _ => None,
    }
}

pub(super) fn probe_act(
    ctx: &mut super::Ctx<'_>,
    shared: &mut Shared,
    label: &str,
) -> Option<bool> {
    if !matches!(label, "p5clock-device-clock-example") {
        return None;
    }
    let mut result = true;
    ctx.runner.update(|ui| {
        use woodshed_core::harmony::KeyedCatalogRef;
        use woodshedding::pitch::PitchClass;
        use woodshedding::rehearsal::{Hold, Set, Timing, Touch};

        ui.stop_rehearsal();
        ui.set = Set::default();
        ui.practice_history = Default::default();
        ui.retained_sets = Default::default();
        ui.working_sets = Default::default();
        ui.catalog_explorations = Default::default();
        ui.stage.set_lens(Lens::Scales);
        for (label, root, bpm, hold) in [
            ("Beat clock 20 BPM", 0, 20.0, Hold::Bars(1)),
            ("Seconds clock 20 BPM", 4, 20.0, Hold::Seconds(2.0)),
            ("Manual clock 400 BPM", 7, 400.0, Hold::Manual),
        ] {
            let Some(mut card) = (KeyedCatalogRef {
                formula_id: "scale:Major".into(),
                root: PitchClass::new(root),
            })
            .to_card() else {
                eprintln!("P5 clock fixture refused: the source scale is unavailable");
                result = false;
                return;
            };
            card.label = label.into();
            card.touch = Touch::Walk;
            card.timing = Timing {
                bpm: Some(bpm),
                hold,
            };
            ui.set.push(card);
        }
        ui.set.cursor = 0;
        ui.set.loop_mode = woodshedding::rehearsal::LoopMode::Off;
        ui.set_tray_expanded = true;
        ui.set_graph_card_expanded = true;
        ui.activate_workspace_panel(woodshed_views::workspace::WorkspacePanel::Practice);
        ui.select_app_section(woodshed_core::storage::AppSection::Stage);
    });
    if !result {
        shared.events.push("p5clock-fixture-refused".into());
    }
    Some(result)
}

pub(super) fn extend_snapshot(
    mut snapshot: ProbeSnapshot,
    ui: &UiState,
    shared: &Shared,
) -> ProbeSnapshot {
    let output = shared
        .backend
        .as_ref()
        .and_then(|backend| backend.output_snapshot());
    let program = output.map(|snapshot| snapshot.program);
    snapshot = snapshot
        .with_field(
            "p5clock-status",
            program
                .map_or("none", |p| status_label(p.status))
                .to_string(),
        )
        .with_field(
            "p5clock-epoch",
            program.map_or(0, |p| p.epoch.0).to_string(),
        )
        .with_field(
            "p5clock-card-index",
            program.map_or(0, |p| p.card_index).to_string(),
        )
        .with_field(
            "p5clock-card-token",
            program
                .and_then(|p| p.card_token)
                .map_or(0, |t| t)
                .to_string(),
        )
        .with_field(
            "p5clock-event-index",
            program.map_or(0, |p| p.event_index).to_string(),
        )
        .with_field(
            "p5clock-event-token",
            program
                .and_then(|p| p.event_token)
                .map_or(0, |t| t)
                .to_string(),
        )
        .with_field(
            "p5clock-device-frame",
            program.map_or(0, |p| p.device_frame).to_string(),
        )
        .with_field(
            "p5clock-active-frames",
            program.map_or(0, |p| p.active_frames).to_string(),
        )
        .with_field(
            "p5clock-beat-ticks",
            program.map_or(0, |p| p.beat_ticks).to_string(),
        )
        .with_field(
            "p5clock-card-frames",
            program.map_or(0, |p| p.card_frames).to_string(),
        )
        .with_field(
            "p5clock-bpm",
            program.map_or("none".into(), |p| f64::from_bits(p.bpm_bits).to_string()),
        )
        .with_field(
            "p5clock-click-subdivision",
            program.map_or(0, |p| p.click_subdivision_ticks).to_string(),
        )
        .with_field("p5clock-running", ui.rehearsal_running.to_string())
        .with_field("p5clock-paused", ui.rehearsal_paused.to_string())
        .with_field(
            "p5clock-source-owner",
            ui.rehearsal_owner.map_or(0, |owner| owner.0).to_string(),
        )
        .with_field(
            "p5clock-projection-event",
            ui.practice_clock_projection
                .as_ref()
                .and_then(|p| p.event_ordinal)
                .map_or("none".into(), |event| event.to_string()),
        )
        .with_field(
            "p5clock-projection-contacts",
            ui.practice_clock_projection
                .as_ref()
                .map_or(0, |p| p.contacts.len())
                .to_string(),
        )
        .with_field(
            "p5clock-stage-event",
            ui.stage_clock_event
                .map_or("none".into(), |event| event.to_string()),
        )
        .with_field(
            "p5clock-stage-contacts",
            ui.stage_clock_contacts.len().to_string(),
        )
        .with_field(
            "p5clock-history-count",
            ui.practice_history.len().to_string(),
        )
        .with_field(
            "p5clock-sample-rate",
            shared
                .backend
                .as_ref()
                .and_then(|backend| backend.output_sample_rate())
                .map_or(0, |rate| rate)
                .to_string(),
        );
    snapshot
}

fn status_label(status: ProgramStatus) -> &'static str {
    match status {
        ProgramStatus::Stopped => "stopped",
        ProgramStatus::Playing => "playing",
        ProgramStatus::Paused => "paused",
        ProgramStatus::WaitingManual => "waiting_manual",
        ProgramStatus::Refused => "refused",
        ProgramStatus::Complete => "complete",
        ProgramStatus::WaitingObserver => "waiting_observer",
    }
}

#[derive(Clone)]
struct ClockPoint {
    epoch: u64,
    status: String,
    card_index: usize,
    card_token: Option<u64>,
    event_token: Option<u64>,
    device_frame: u64,
    active_frames: u64,
    beat_ticks: u64,
    card_frames: u64,
    bpm: Option<f64>,
}

fn point(snapshot: ProgramSnapshot) -> ClockPoint {
    ClockPoint {
        epoch: snapshot.epoch.0,
        status: status_label(snapshot.status).to_string(),
        card_index: snapshot.card_index,
        card_token: snapshot.card_token,
        event_token: snapshot.event_token,
        device_frame: snapshot.device_frame,
        active_frames: snapshot.active_frames,
        beat_ticks: snapshot.beat_ticks,
        card_frames: snapshot.card_frames,
        bpm: (snapshot.bpm_bits != 0).then(|| f64::from_bits(snapshot.bpm_bits)),
    }
}

fn same_position(before: &ClockPoint, now: &ClockPoint) -> bool {
    same_transport_position(before, now) && before.active_frames == now.active_frames
}

fn same_transport_position(before: &ClockPoint, now: &ClockPoint) -> bool {
    before.epoch == now.epoch
        && before.card_index == now.card_index
        && before.card_token == now.card_token
        && before.event_token == now.event_token
        && before.beat_ticks == now.beat_ticks
        && before.card_frames == now.card_frames
}

fn save_program_point(
    shared: &Rc<RefCell<Shared>>,
    file: &str,
    accept: impl FnOnce(&ClockPoint) -> bool,
) -> bool {
    let Some(program) = shared
        .borrow()
        .backend
        .as_ref()
        .and_then(|backend| backend.output_snapshot())
        .map(|snapshot| snapshot.program)
    else {
        return fail("P5 clock checkpoint refused: no output program snapshot");
    };
    let value = point(program);
    if !accept(&value) {
        return fail("P5 clock checkpoint refused: unexpected program state");
    }
    let Some(path) = receipt_path(file) else {
        return fail("P5 clock checkpoint refused: no isolated receipt directory");
    };
    write_new_json(
        &path,
        &serde_json::json!({
            "epoch": value.epoch,
            "status": &value.status,
            "card_index": value.card_index,
            "card_token": value.card_token,
            "event_token": value.event_token,
            "device_frame": value.device_frame,
            "active_frames": value.active_frames,
            "beat_ticks": value.beat_ticks,
            "card_frames": value.card_frames,
            "bpm": value.bpm,
        }),
    )
    .is_ok()
}

fn check_saved_position(
    shared: &Rc<RefCell<Shared>>,
    file: &str,
    check: impl FnOnce(&ClockPoint, &ClockPoint) -> bool,
) -> bool {
    let Some(path) = receipt_path(file) else {
        return fail("P5 clock comparison refused: no isolated receipt directory");
    };
    let expected = std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
    let Some(expected) = expected else {
        return fail("P5 clock comparison refused: expected checkpoint missing");
    };
    let Some(program) = shared
        .borrow()
        .backend
        .as_ref()
        .and_then(|backend| backend.output_snapshot())
        .map(|snapshot| snapshot.program)
    else {
        return fail("P5 clock comparison refused: no output program snapshot");
    };
    let before = ClockPoint {
        epoch: json_u64(&expected, "epoch").unwrap_or(u64::MAX),
        status: json_str(&expected, "status")
            .unwrap_or("invalid")
            .to_owned(),
        card_index: json_u64(&expected, "card_index").unwrap_or(usize::MAX as u64) as usize,
        card_token: json_u64_opt(&expected, "card_token"),
        event_token: json_u64_opt(&expected, "event_token"),
        device_frame: json_u64(&expected, "device_frame").unwrap_or(u64::MAX),
        active_frames: json_u64(&expected, "active_frames").unwrap_or(u64::MAX),
        beat_ticks: json_u64(&expected, "beat_ticks").unwrap_or(u64::MAX),
        card_frames: json_u64(&expected, "card_frames").unwrap_or(u64::MAX),
        bpm: expected.get("bpm").and_then(serde_json::Value::as_f64),
    };
    let now = point(program);
    if check(&before, &now) {
        true
    } else {
        fail("P5 clock comparison refused: processor position or tempo differs")
    }
}

fn save_final(ctx: &mut super::Ctx<'_>, shared: &Rc<RefCell<Shared>>) -> bool {
    let ui = ctx.runner.state();
    if !ui.practice_clock_managed
        || ui.rehearsal_running
        || ui.practice_clock_projection.is_some()
        || ui.practice_history.len() != 6
        || !ui.practice_saved
    {
        return fail("P5 clock final receipt refused: run, history, or save state is incomplete");
    }
    let shared_state = shared.borrow();
    let Some(owner) = shared_state.seal.as_ref() else {
        return fail("P5 clock final receipt refused: no sealed session");
    };
    if !matches!(owner, woodshed_views::persona::PracticeSeal::Sealed { .. }) {
        return fail("P5 clock final receipt refused: session is not sealed");
    }
    let Some(stored) = shared_state
        .storage
        .as_ref()
        .and_then(|storage| storage.load())
    else {
        return fail("P5 clock final receipt refused: sealed session is unreadable");
    };
    let persisted = match woodshed_core::storage::decode_session(&stored) {
        Ok(decoded) => decoded.session,
        Err(error) => return fail(&format!("P5 clock final receipt refused: {error}")),
    };
    let cards = match serde_json::to_value(&ui.set.cards) {
        Ok(value) => value,
        Err(error) => return fail(&format!("P5 clock final receipt refused: {error}")),
    };
    let persisted_cards = match serde_json::to_value(&persisted.set.cards) {
        Ok(value) => value,
        Err(error) => return fail(&format!("P5 clock final receipt refused: {error}")),
    };
    let history = ui.practice_history.recent(ui.practice_history.len());
    let persisted_history = persisted
        .practice_history
        .recent(persisted.practice_history.len());
    if cards != persisted_cards || history != persisted_history {
        return fail("P5 clock final receipt refused: live and sealed Cards/history differ");
    }
    let expected_labels = [
        "Beat clock 20 BPM",
        "Seconds clock 20 BPM",
        "Manual clock 400 BPM",
    ];
    let mut event_pairs = serde_json::Map::new();
    let mut occurrence_ids = std::collections::BTreeSet::new();
    for event in &history {
        let Some(snapshot) = event
            .provenance
            .as_ref()
            .and_then(|provenance| provenance.card_snapshot.as_object())
        else {
            return fail(
                "P5 clock final receipt refused: accepted event lacks exact Card provenance",
            );
        };
        let Some(label) = snapshot.get("label").and_then(serde_json::Value::as_str) else {
            return fail("P5 clock final receipt refused: accepted Card label is missing");
        };
        if !expected_labels.contains(&label) {
            return fail("P5 clock final receipt refused: unexpected Card in accepted history");
        }
        if event
            .provenance
            .as_ref()
            .and_then(|provenance| provenance.working_set_id)
            != Some(ui.working_sets.active_id)
        {
            return fail("P5 clock final receipt refused: event owner differs from active Set");
        }
        let entry = event_pairs.entry(label.to_string()).or_insert_with(
            || serde_json::json!({"rehearsed": null, "completed_ms": null, "occurrence_id": null}),
        );
        let occurrence_id = event.provenance.as_ref().unwrap().occurrence_id;
        occurrence_ids.insert(occurrence_id.0);
        if !entry["occurrence_id"].is_null()
            && entry["occurrence_id"] != serde_json::json!(occurrence_id)
        {
            return fail(
                "P5 clock final receipt refused: Started and Completed Card identities differ",
            );
        }
        entry["occurrence_id"] = serde_json::json!(occurrence_id);
        match event.kind {
            woodshed_core::history::EngagementKind::Rehearsed => {
                if event.practiced_ms.is_some()
                    || !entry["rehearsed"].is_null()
                    || !entry["completed_ms"].is_null()
                {
                    return fail(
                        "P5 clock final receipt refused: duplicate or malformed Started event",
                    );
                }
                entry["rehearsed"] = serde_json::json!(true);
            },
            woodshed_core::history::EngagementKind::Completed => {
                if entry["rehearsed"].is_null() {
                    return fail(
                        "P5 clock final receipt refused: Completed precedes its Started event",
                    );
                }
                let Some(duration) = event.practiced_ms else {
                    return fail(
                        "P5 clock final receipt refused: completed Card has no measured frames",
                    );
                };
                if !entry["completed_ms"].is_null() {
                    return fail("P5 clock final receipt refused: duplicate Completed event");
                }
                if label == "Seconds clock 20 BPM" && duration != 2_000 {
                    return fail(
                        "P5 clock final receipt refused: authored two-second hold did not measure 2000 ms",
                    );
                }
                entry["completed_ms"] = serde_json::json!(duration);
            },
            _ => return fail("P5 clock final receipt refused: unexpected history event kind"),
        }
    }
    if event_pairs.len() != expected_labels.len()
        || occurrence_ids.len() != expected_labels.len()
        || expected_labels.iter().any(|label| {
            event_pairs.get(*label).is_none_or(|entry| {
                entry["rehearsed"].is_null()
                    || entry["completed_ms"].as_u64().is_none()
                    || entry["occurrence_id"].is_null()
            })
        })
    {
        return fail(
            "P5 clock final receipt refused: each Card needs one Started and Completed event",
        );
    }
    let Some(output) = shared_state
        .backend
        .as_ref()
        .and_then(|backend| backend.output_snapshot())
    else {
        return fail("P5 clock final receipt refused: output snapshot is unavailable");
    };
    if output.program.status != ProgramStatus::Complete {
        return fail("P5 clock final receipt refused: processor has not completed the Manual Card");
    }
    let report = serde_json::json!({
        "owner": ui.working_sets.active_id.0,
        "cards": cards,
        "history": history,
        "events_by_card": event_pairs,
        "persisted_cards_match": true,
        "persisted_history_match": true,
        "program_status": status_label(output.program.status),
        "program_epoch": output.program.epoch.0,
        "sample_rate": shared_state.backend.as_ref().and_then(|backend| backend.output_sample_rate()),
    });
    let Some(path) = receipt_path("p5clock-final.json") else {
        return fail("P5 clock final receipt refused: no isolated receipt directory");
    };
    write_new_json(&path, &report).is_ok()
}

fn verify_reopen(ctx: &mut super::Ctx<'_>, shared: &Rc<RefCell<Shared>>) -> bool {
    let Some(path) = receipt_path("p5clock-final.json") else {
        return fail("P5 clock reopen receipt refused: no isolated receipt directory");
    };
    let expected = std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
    let Some(expected) = expected else {
        return fail("P5 clock reopen receipt refused: seed checkpoint is missing");
    };
    let ui = ctx.runner.state();
    if !ui.practice_clock_managed
        || ui.rehearsal_running
        || ui.rehearsal_paused
        || ui.rehearsal_clock_program.is_some()
        || ui.practice_clock_projection.is_some()
        || ui.practice_clock_commands.len() != 0
    {
        return fail("P5 clock reopen receipt refused: transient program state was restored");
    }
    let Some(output) = shared
        .borrow()
        .backend
        .as_ref()
        .and_then(|backend| backend.output_snapshot())
    else {
        return fail("P5 clock reopen receipt refused: output snapshot is unavailable");
    };
    if output.program.status != ProgramStatus::Stopped {
        return fail("P5 clock reopen receipt refused: device program is not stopped");
    }
    let cards = match serde_json::to_value(&ui.set.cards) {
        Ok(value) => value,
        Err(error) => return fail(&format!("P5 clock reopen receipt refused: {error}")),
    };
    let history = ui.practice_history.recent(ui.practice_history.len());
    if history.len() != 6 {
        return fail("P5 clock reopen receipt refused: expected six accepted history events");
    }
    let Some(stored) = shared
        .borrow()
        .storage
        .as_ref()
        .and_then(|storage| storage.load())
    else {
        return fail("P5 clock reopen receipt refused: sealed session is unreadable");
    };
    let persisted = match woodshed_core::storage::decode_session(&stored) {
        Ok(decoded) => decoded.session,
        Err(error) => return fail(&format!("P5 clock reopen receipt refused: {error}")),
    };
    let persisted_cards = match serde_json::to_value(&persisted.set.cards) {
        Ok(value) => value,
        Err(error) => return fail(&format!("P5 clock reopen receipt refused: {error}")),
    };
    let persisted_history = persisted
        .practice_history
        .recent(persisted.practice_history.len());
    let history_value = match serde_json::to_value(&history) {
        Ok(value) => value,
        Err(error) => return fail(&format!("P5 clock reopen receipt refused: {error}")),
    };
    if Some(&cards) != expected.get("cards")
        || Some(&history_value) != expected.get("history")
        || expected.get("owner") != Some(&serde_json::json!(ui.working_sets.active_id.0))
        || cards != persisted_cards
        || history != persisted_history
    {
        return fail(
            "P5 clock reopen receipt refused: owner, authored Cards, or accepted history differs",
        );
    }
    let report = serde_json::json!({
        "owner": ui.working_sets.active_id.0,
        "cards": cards,
        "history": history,
        "program_status": status_label(output.program.status),
        "transient_program_cleared": true,
        "persisted_cards_match": true,
        "persisted_history_match": true,
    });
    let Some(path) = receipt_path("p5clock-reopened.json") else {
        return fail("P5 clock reopen receipt refused: no isolated receipt directory");
    };
    write_new_json(&path, &report).is_ok()
}

fn receipt_path(name: &str) -> Option<PathBuf> {
    std::env::var_os("WOODSHED_CAPTURE_DIR").map(|dir| PathBuf::from(dir).join(name))
}

fn write_new_json(path: &std::path::Path, value: &serde_json::Value) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(serde_json::to_string_pretty(value).unwrap().as_bytes())
}

fn json_u64(value: &serde_json::Value, key: &str) -> Option<u64> {
    value.get(key).and_then(serde_json::Value::as_u64)
}

fn json_u64_opt(value: &serde_json::Value, key: &str) -> Option<u64> {
    value.get(key).and_then(serde_json::Value::as_u64)
}

fn json_str<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(serde_json::Value::as_str)
}

fn fail(message: &str) -> bool {
    eprintln!("{message}");
    false
}
