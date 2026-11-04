use super::*;

/// The selected material's neighbors, each carrying every relation that
/// connects them. Deterministic: derived from catalog truth alone, with no
/// history and no inference. Practice evidence is layered by the consumer that
/// owns it, onto these same records.
pub fn related_neighbors(selected_id: &str, limit: usize) -> Vec<RelatedNeighbor> {
    relation_index()
        .get(selected_id)
        .into_iter()
        .flatten()
        .take(limit)
        .cloned()
        .collect()
}

/// Every relation between two materials, in display order. This is what
/// selecting an edge should show: all applicable reasons, each with its own
/// authority, rather than the one that happened to rank highest.
pub fn relations_between(source_id: &str, target_id: &str) -> Vec<MaterialRelation> {
    relation_index()
        .get(source_id)
        .into_iter()
        .flatten()
        .find(|neighbor| neighbor.id == target_id)
        .map(|neighbor| neighbor.relations.clone())
        .unwrap_or_default()
}
