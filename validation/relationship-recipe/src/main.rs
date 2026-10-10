//! Bounded shared-compiler and Woodshed persistence adoption instrument.
use std::collections::BTreeMap;
use std::path::Path;

use scenograph::relationship::{
    RecipeEdit, RelationshipRecipeDraft, RelationshipSnapshot, relationship_recipe,
};
use scenograph::{ProjectionInputBinding, RevisionEvidence};
use scenomise::projection::{
    CompileIssue, CompiledRelationshipProjection, ItemSizes, ProjectionCompiler,
    RelationshipDataset,
};
use woodshed_core::settings::AppSettings;
use woodshed_core::storage::{PersistedSession, SessionStore};
use woodshed_views::stage::UiState;

#[allow(dead_code)]
#[path = "../../../crates/woodshed-core/examples/relationship_disclosure_export.rs"]
mod export;

#[allow(dead_code)]
#[rustfmt::skip]
#[path = "../../../crates/woodshed-genet/src/storage.rs"]
mod host_storage;

// Scenomise compiles through a consumer-sized compiler; Woodshed's card footprint.
fn compile_relationship_snapshot(
    snapshot: &RelationshipSnapshot,
    data: &RelationshipDataset,
) -> Result<CompiledRelationshipProjection, Vec<CompileIssue>> {
    ProjectionCompiler::new(ItemSizes {
        card: sceno::Size2::new(164.0, 68.0),
    })
    .compile_relationship_snapshot(snapshot, data)
}

fn dataset() -> RelationshipDataset {
    serde_json::from_value(export::fixture_json()).expect("Woodshed exports the shared schema")
}

fn snapshot(data: &RelationshipDataset) -> RelationshipSnapshot {
    let binding = ProjectionInputBinding {
        source: data.dataset.source.clone(),
        expects_generation: Some(data.dataset.revision.clone()),
        revision_evidence: RevisionEvidence::PublicGeneration,
    };
    let recipe = relationship_recipe(
        "selected-material-comparison-v1",
        "Selected material",
        "woodshed-proof",
        "recipe-v1",
        BTreeMap::from([("woodshed".into(), binding)]),
    );
    let mut draft = RelationshipRecipeDraft::new(recipe);
    draft.apply(RecipeEdit::SetLabel("Explained selected material".into()));
    let mut arrangement = draft.recipe().definition.arrangement.clone();
    arrangement.spacing = 48;
    draft.apply(RecipeEdit::SetArrangement(arrangement));
    RelationshipSnapshot {
        recipe: draft.to_recipe().unwrap(),
        source_name: "woodshed".into(),
        selected_occurrence: Some(data.dataset.occurrences[1].occurrence_id.clone()),
        selected_relationship: Some(data.relationships[0].id.clone()),
    }
}

fn check(reading: &RelationshipSnapshot, data: &RelationshipDataset) {
    let compiled =
        compile_relationship_snapshot(reading, data).expect("compatible reading compiles");
    assert_eq!(compiled.projection.instance_by_occurrence.len(), 3);
    assert_eq!(compiled.relationships.len(), 1);
    assert_eq!(compiled.projection.scene.relations.len(), 1);
    assert_eq!(
        compiled.relationships[0].disclosure.explanation,
        data.relationships[0].explanation
    );
    assert_eq!(
        compiled.selected_relationship,
        reading.selected_relationship
    );
    assert!(compiled.projection.selected.is_some());
    assert_eq!(
        data.dataset.occurrences[0].source,
        data.dataset.occurrences[1].source
    );
    assert_ne!(
        data.dataset.occurrences[0].occurrence_id,
        data.dataset.occurrences[1].occurrence_id
    );
}

fn seed(path: &Path) {
    assert!(
        !path.exists(),
        "the proof never overwrites an existing session"
    );
    let data = dataset();
    let reading = snapshot(&data);
    check(&reading, &data);
    let mut ui = UiState::new();
    ui.set = export::fixture_set();
    let before = serde_json::to_value(&ui.set).unwrap();
    ui.relationship_reading_json =
        Some(serde_json::json!({"snapshot": reading, "dataset": data}).to_string());
    assert_eq!(before, serde_json::to_value(&ui.set).unwrap());
    let store = SessionStore::new(host_storage::FsBackend::default());
    let persisted = serde_json::to_string(&ui.to_persisted()).unwrap();
    store.save(&persisted);
    assert_eq!(store.load().as_deref(), Some(persisted.as_str()));
    println!(
        "seed: compiled three occurrences and one explained relationship; retained shared edits and selection"
    );
}

fn reopen(path: &Path) {
    assert!(
        path.is_file(),
        "reopening requires an existing isolated session"
    );
    let store = SessionStore::new(host_storage::FsBackend::default());
    let session: PersistedSession = serde_json::from_str(&store.load().unwrap()).unwrap();
    let mut ui = UiState::new();
    assert!(
        ui.apply_persisted(&session, AppSettings::default())
            .is_none()
    );
    let payload = ui
        .relationship_reading_json
        .as_ref()
        .expect("the application restores its retained reading");
    let value: serde_json::Value = serde_json::from_str(payload).unwrap();
    let reading: RelationshipSnapshot = serde_json::from_value(value["snapshot"].clone()).unwrap();
    let data: RelationshipDataset = serde_json::from_value(value["dataset"].clone()).unwrap();
    check(&reading, &data);
    assert_eq!(reading.recipe.definition.arrangement.spacing, 48);
    assert_eq!(
        reading.recipe.definition.label,
        "Explained selected material"
    );
    assert_eq!(
        ui.to_persisted().relationship_reading_json,
        session.relationship_reading_json
    );
    assert_eq!(
        serde_json::to_value(&ui.set).unwrap(),
        serde_json::to_value(export::fixture_set()).unwrap()
    );
    assert!(!ui.rehearsal_running);
    println!(
        "reopen: shared edits, selections, musical evidence and source Set survive a new process; no runner started"
    );
}

fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    assert_eq!(
        args.len(),
        3,
        "usage: woodshed-relationship-recipe-proof seed|reopen /isolated/session.json"
    );
    std::env::set_var("WOODSHED_STATE", &args[2]);
    std::env::set_var("WOODSHED_SETTINGS", format!("{}.settings", args[2]));
    match args[1].as_str() {
        "seed" => seed(Path::new(&args[2])),
        "reopen" => reopen(Path::new(&args[2])),
        _ => panic!("unknown proof action"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_domain_disclosure_compiles_and_has_routed_occurrence_endpoints() {
        let data = dataset();
        let reading = snapshot(&data);
        check(&reading, &data);
        let compiled = compile_relationship_snapshot(&reading, &data).unwrap();
        let relation = &compiled.relationships[0];
        assert_eq!(
            relation.from,
            compiled.projection.instance_by_occurrence[&data.relationships[0].from_occurrence]
        );
        assert_eq!(
            relation.to,
            compiled.projection.instance_by_occurrence[&data.relationships[0].to_occurrence]
        );
    }

    #[test]
    fn checked_fixture_is_the_current_owner_generated_disclosure() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../scenarios/woodshed_relationships.json"
        ))
        .unwrap();
        assert_eq!(fixture, export::fixture_json());
        let typed: RelationshipDataset = serde_json::from_value(fixture).unwrap();
        check(&snapshot(&typed), &typed);
    }

    #[test]
    fn numeric_field_cannot_replace_an_unavailable_semantic_role() {
        let mut data = dataset();
        let reading = snapshot(&data);
        data.facets.remove("authored_order");
        let issues = compile_relationship_snapshot(&reading, &data).unwrap_err();
        assert!(
            issues
                .iter()
                .any(|issue| issue.message.contains("authored_order"))
        );
    }

    #[test]
    fn unknown_relationship_endpoint_and_stale_provenance_are_refused() {
        let mut data = dataset();
        let reading = snapshot(&data);
        data.relationships[0].to_occurrence = "missing".into();
        assert!(compile_relationship_snapshot(&reading, &data).is_err());
        data = dataset();
        data.relationships[0].provenance.source_revision = "another-revision".into();
        assert!(compile_relationship_snapshot(&reading, &data).is_err());
    }

    #[test]
    fn compatible_rebind_preserves_recipe_but_unavailable_facet_refuses() {
        let data = dataset();
        let reading = snapshot(&data);
        let set = export::fixture_set();
        let ids = set.cards.iter().map(|card| card.id).collect::<Vec<_>>();
        let facts = woodshed_core::comparison_disclosure::disclose(
            woodshed_core::working_sets::WorkingSetId(2),
            "woodshed-relationship-fixture-v1",
            &set,
            &ids,
            (ids[0], ids[2]),
        )
        .unwrap();
        let mut other: RelationshipDataset = serde_json::from_value(
            woodshed_core::comparison_disclosure::projection_json(&facts),
        )
        .unwrap();
        let mut draft = RelationshipRecipeDraft::new(reading.recipe.clone());
        draft.apply(RecipeEdit::BindInput {
            name: "other".into(),
            binding: ProjectionInputBinding {
                source: other.dataset.source.clone(),
                expects_generation: Some(other.dataset.revision.clone()),
                revision_evidence: RevisionEvidence::PublicGeneration,
            },
        });
        let rebound = RelationshipSnapshot {
            recipe: draft.to_recipe().unwrap(),
            source_name: "other".into(),
            selected_occurrence: Some(other.dataset.occurrences[1].occurrence_id.clone()),
            selected_relationship: Some(other.relationships[0].id.clone()),
        };
        check(&rebound, &other);
        assert_eq!(rebound.recipe.definition.id, reading.recipe.definition.id);
        assert_eq!(
            rebound.recipe.definition.arrangement,
            reading.recipe.definition.arrangement
        );
        other.facets.remove("explained_relationships");
        assert!(compile_relationship_snapshot(&rebound, &other).is_err());
    }
}
