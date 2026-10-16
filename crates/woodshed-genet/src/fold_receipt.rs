//! Read-only native fold checkpoints from the normal sealed session path.
use crate::shared::Shared;
use serde_json::{Value, json};
use woodshed_views::stage::{
    OverviewPresentation, UiState, overview_snapshot, overview_source_swatch, overview_swatch,
};

fn owner(mut session: Value) -> Value {
    let object = session.as_object_mut().unwrap();
    object.remove("overview_presentation_json");
    object.remove("workspace_json");
    // Stemma serializes owned_visits from a HashSet and bindings from a HashMap.
    // Compare membership canonically without dropping any event or payload value.
    if let Some(history) = object.get_mut("practice_history") {
        if let Some(owners) = history.get_mut("owners").and_then(Value::as_array_mut) {
            for owner in owners {
                if let Some(visits) = owner.get_mut("owned_visits").and_then(Value::as_array_mut) {
                    visits.sort_by_key(|value| value.as_u64().unwrap_or(u64::MAX));
                }
            }
        }
        if let Some(visits) = history.get_mut("visits").and_then(Value::as_array_mut) {
            for visit in visits {
                if let Some(bindings) = visit.get_mut("bindings").and_then(Value::as_array_mut) {
                    bindings.sort_by_key(|value| {
                        value
                            .get("owner")
                            .and_then(Value::as_u64)
                            .unwrap_or(u64::MAX)
                    });
                }
            }
        }
    }
    session
}
fn hash(value: &impl serde::Serialize) -> String {
    blake3::hash(&serde_json::to_vec(value).unwrap())
        .to_hex()
        .to_string()
}
fn checkpoint(ui: &UiState, shared: &Shared) -> Result<Value, String> {
    if !matches!(
        shared.seal,
        Some(woodshed_views::persona::PracticeSeal::Sealed { .. })
    ) {
        return Err("fold checkpoint requires isolated sealed custody".into());
    }
    let json = shared
        .storage
        .as_ref()
        .and_then(|s| s.load())
        .ok_or("sealed session unreadable")?;
    let persisted = woodshed_core::storage::decode_session(&json)
        .map_err(|e| e.to_string())?
        .session;
    let presentation =
        OverviewPresentation::from_json(persisted.overview_presentation_json.as_deref())
            .ok_or("sealed presentation unreadable")?;
    let owners = owner(serde_json::to_value(ui.to_persisted()).map_err(|e| e.to_string())?);
    let persisted_owners = owner(serde_json::to_value(persisted).map_err(|e| e.to_string())?);
    if owners != persisted_owners
        || presentation.folds != ui.overview_folds.records
        || !ui.practice_saved
    {
        let changed: Vec<_> = owners
            .as_object()
            .unwrap()
            .keys()
            .filter(|key| owners.get(*key) != persisted_owners.get(*key))
            .cloned()
            .collect();
        return Err(format!(
            "live/sealed agreement: owners={}, changed owner keys={changed:?}, folds={}, confirmed={}",
            owners == persisted_owners,
            presentation.folds == ui.overview_folds.records,
            ui.practice_saved
        ));
    }
    let source = overview_source_swatch(ui);
    let projected = overview_swatch(ui);
    let mut positions: Vec<_> = source
        .graph
        .nodes
        .iter()
        .map(|n| (n.id.wire_key(), n.position))
        .collect();
    positions.sort_by(|a, b| a.0.cmp(&b.0));
    let truth = overview_snapshot(ui);
    let boundary: Vec<_> = projected
        .relations
        .iter()
        .filter(|r| r.id.starts_with("fold-bundle:"))
        .map(|r| {
            json!({
                "from":r.from.wire_key(),"to":r.to.wire_key(),"kind":r.kind,"label":r.label,
                "source_link_cleared":r.link_key.is_none(),"route":r.route,
            })
        })
        .collect();
    Ok(json!({
        "schema":"woodshed.fold-checkpoint/v1","sealed":true,"practice_saved":true,
        "folds":ui.overview_folds.records,"persisted_folds_match":true,
        "owner_digest":hash(&owners),"owner":owners,"persisted_owners_match":true,
        "truth_digest":hash(&truth),"truth":truth,"source_positions":positions,
        "positions_digest":hash(&positions),"fold_digest":hash(&ui.overview_folds.records),
        "visible_nodes":projected.graph.nodes.iter().map(|n|json!({"key":n.id.wire_key(),"label":n.label,"position":n.position})).collect::<Vec<_>>(),
        "boundary_bundles":boundary,"source_node_count":source.graph.nodes.len(),
        "source_relation_count":source.relations.len(),
        "source_relations":source.relations.iter().map(|r|json!({"from":r.from.wire_key(),"to":r.to.wire_key(),"kind":r.kind,"visible":r.visible})).collect::<Vec<_>>(),"projected_relation_count":projected.relations.len(),
        "rehearsal_running":ui.rehearsal_running,"song_playing":ui.song_playing,
        "presentation_version":presentation.version,
    }))
}

pub fn write(ui: &UiState, shared: &Shared, stem: &str) -> bool {
    let result = || -> Result<(), String> {
        if stem.is_empty() || !stem.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-') {
            return Err("invalid checkpoint stem".into());
        }
        let directory = std::env::var_os("WOODSHED_CAPTURE_DIR").ok_or("no receipt directory")?;
        let report = checkpoint(ui, shared)?;
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(std::path::PathBuf::from(directory).join(format!("fold-{stem}.json")))
            .map_err(|e| e.to_string())?;
        file.write_all(serde_json::to_string_pretty(&report).unwrap().as_bytes())
            .map_err(|e| e.to_string())?;
        Ok(())
    };
    match result() {
        Ok(()) => true,
        Err(why) => {
            eprintln!("Fold checkpoint refused: {why}");
            false
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_membership_order_is_canonical_but_event_edits_still_differ() {
        let first = json!({"set":{"cards":[1,2]}, "practice_history":{
            "owners":[{"identity":"practitioner","owned_visits":[1,0]}],
            "visits":[{"created_at_ms":123,"context":{"kind":"Staged"},
                "bindings":[{"owner":1,"forward_child":null},{"owner":0,"forward_child":1}]}]
        }});
        let mut reordered = first.clone();
        reordered["practice_history"]["owners"][0]["owned_visits"] = json!([0, 1]);
        reordered["practice_history"]["visits"][0]["bindings"]
            .as_array_mut()
            .unwrap()
            .reverse();
        assert_eq!(owner(first.clone()), owner(reordered.clone()));
        let mut edited = reordered.clone();
        edited["practice_history"]["visits"][0]["context"]["kind"] = json!("Completed");
        assert_ne!(owner(first.clone()), owner(edited));
        let mut edited = reordered.clone();
        edited["practice_history"]["visits"][0]["created_at_ms"] = json!(124);
        assert_ne!(owner(first.clone()), owner(edited));
        let mut edited = reordered.clone();
        edited["practice_history"]["owners"][0]["owned_visits"] = json!([0]);
        assert_ne!(owner(first.clone()), owner(edited));
        let mut edited = reordered;
        edited["set"]["cards"] = json!([2, 1]);
        assert_ne!(owner(first), owner(edited));
    }
}
