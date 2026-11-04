use super::*;

pub(super) fn save_final(ctx: &mut super::super::Ctx<'_>, shared: &Rc<RefCell<Shared>>) -> bool {
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
    // `recent` is newest first; validate lifecycle order chronologically while
    // retaining the exact original order in the sealed comparison/report.
    for event in history.iter().rev() {
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

pub(super) fn verify_reopen(ctx: &mut super::super::Ctx<'_>, shared: &Rc<RefCell<Shared>>) -> bool {
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
