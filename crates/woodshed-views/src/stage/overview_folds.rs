//! View-local selection folds over Woodshed's typed session overview.
//! Member stand-ins preserve source identity; neither owner facts nor layout seeds change.
use super::{UiChild, UiState, overview};
use cambium::{GraphCanvasSwatch, clickable, el, text};
use curation::{FoldId, FoldRecord, FoldRule, FoldStandIn};
use std::collections::{BTreeMap, BTreeSet};
use woodshed_core::session_overview::OverviewNodeId;

pub const SCOPE: &str = "woodshed:session-overview";
const MAX_FOLDS: usize = 128;
const MAX_MEMBERS: usize = 512;

#[derive(Default)]
pub struct OverviewFolds {
    pub records: Vec<FoldRecord>,
    pub selecting: bool,
    pub selected: BTreeSet<OverviewNodeId>,
    pub notice: Option<String>,
    pub(super) undo: Vec<Vec<FoldRecord>>,
    pub(super) redo: Vec<Vec<FoldRecord>>,
}

/// Admission bounds retained presentation, never rewrites the source model.
pub(super) fn admit_records(records: Vec<FoldRecord>) -> Vec<FoldRecord> {
    let mut admitted: Vec<FoldRecord> = Vec::new();
    for record in records.into_iter().take(MAX_FOLDS) {
        if record.validate().is_ok()
            && record.source_scope == SCOPE
            && record.members.len() <= MAX_MEMBERS
            && record.stand_in_member().is_some()
            && !admitted
                .iter()
                .any(|old| old.id == record.id || old.overlaps(&record))
        {
            admitted.push(record);
        }
    }
    admitted
}

impl OverviewFolds {
    pub fn contains(&self, id: &OverviewNodeId) -> bool {
        self.records.iter().any(|r| r.contains(&id.wire_key()))
    }
    fn record(&mut self, previous: Vec<FoldRecord>) {
        self.undo.push(previous);
        if self.undo.len() > 64 {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.selected.clear();
        self.selecting = false;
    }
}

impl UiState {
    /// A normal graph/roster activation expands a stand-in, or toggles deliberate selection.
    pub(super) fn activate_overview_fold(&mut self, id: &OverviewNodeId) -> bool {
        let snapshot = overview::overview_snapshot(self);
        let present: BTreeSet<_> = snapshot.nodes.iter().map(|n| n.id.wire_key()).collect();
        let key = id.wire_key();
        if let Some(index) = self.overview_folds.records.iter().position(|r| {
            r.stand_in_member() == Some(key.as_str())
                && r.members.iter().all(|m| present.contains(m))
        }) {
            let old = self.overview_folds.records.clone();
            self.overview_folds.records.remove(index);
            self.overview_folds.record(old);
            self.overview_folds.notice =
                Some("Fold expanded. Its members keep their places.".into());
            return true;
        }
        if !self.overview_folds.selecting {
            return false;
        }
        if present.contains(&key) && !self.overview_folds.contains(id) {
            if !self.overview_folds.selected.remove(id) {
                self.overview_folds.selected.insert(id.clone());
            }
        }
        true
    }

    pub fn create_overview_fold(&mut self) -> Result<(), String> {
        if self.overview_unrecognized_presentation_json.is_some() {
            return Err("The retained graph presentation cannot be read by this version. It is preserved; new folds cannot be saved.".into());
        }
        if self.overview_folds.records.len() >= MAX_FOLDS {
            return Err("Expand a fold before adding another.".into());
        }
        let present: BTreeSet<_> = overview::overview_snapshot(self)
            .nodes
            .into_iter()
            .map(|n| n.id)
            .collect();
        let selected = &self.overview_folds.selected;
        if selected.len() < 2 || selected.len() > MAX_MEMBERS || !selected.is_subset(&present) {
            return Err("Select at least two current graph members to fold.".into());
        }
        if selected.iter().any(|id| self.overview_folds.contains(id)) {
            return Err("Expand an overlapping fold before selecting those members.".into());
        }
        let number = (1..)
            .find(|n| {
                !self
                    .overview_folds
                    .records
                    .iter()
                    .any(|r| r.id.as_str() == format!("session-fold-{n}"))
            })
            .unwrap();
        let members: Vec<_> = selected.iter().map(OverviewNodeId::wire_key).collect();
        let record = FoldRecord::new(
            FoldId::new(format!("session-fold-{number}")),
            SCOPE,
            members.clone(),
            FoldStandIn::Member(members[0].clone()),
            Some(FoldRule::Selection),
            Some(format!("Fold {number}")),
        )
        .map_err(|e| e.to_string())?;
        let old = self.overview_folds.records.clone();
        self.overview_folds.records.push(record);
        self.overview_folds.record(old);
        self.overview_folds.notice = Some("Fold created. Activate its +N member to expand.".into());
        Ok(())
    }

    pub fn expand_overview_folds(&mut self) {
        if self.overview_folds.records.is_empty() {
            return;
        }
        let old = std::mem::take(&mut self.overview_folds.records);
        self.overview_folds.record(old);
        self.overview_folds.notice = Some("All folds expanded.".into());
    }

    pub fn undo_overview_fold(&mut self, redo: bool) {
        let state = &mut self.overview_folds;
        let next = if redo {
            state.redo.pop()
        } else {
            state.undo.pop()
        };
        if let Some(next) = next {
            let old = std::mem::replace(&mut state.records, next);
            if redo {
                state.undo.push(old);
            } else {
                state.redo.push(old);
            }
            state.selected.clear();
            state.selecting = false;
            state.notice = Some(if redo { "Fold redone." } else { "Fold undone." }.into());
        }
    }
}

/// The complete active fold set maps each source endpoint once, including fold-to-fold cells.
pub(super) fn project(
    mut swatch: GraphCanvasSwatch<OverviewNodeId, &'static str>,
    state: &OverviewFolds,
) -> GraphCanvasSwatch<OverviewNodeId, &'static str> {
    if state.records.is_empty() && !state.selecting {
        return swatch;
    }
    let source: BTreeMap<_, _> = swatch
        .graph
        .nodes
        .iter()
        .map(|n| (n.id.wire_key(), n.id.clone()))
        .collect();
    let mut owners = BTreeMap::new();
    let mut labels = BTreeMap::new();
    for record in admit_records(state.records.clone()) {
        if record
            .members
            .iter()
            .any(|key| !source.contains_key(key) || owners.contains_key(&source[key]))
        {
            continue;
        }
        let stand_in = source[record.stand_in_member().unwrap()].clone();
        labels.insert(
            stand_in.clone(),
            format!(
                "{} +{}, activate to expand",
                record.label.as_deref().unwrap_or("Fold"),
                record.hidden_count()
            ),
        );
        for key in &record.members {
            owners.insert(source[key].clone(), stand_in.clone());
        }
    }
    swatch
        .graph
        .nodes
        .retain(|node| owners.get(&node.id).is_none_or(|owner| owner == &node.id));
    for node in &mut swatch.graph.nodes {
        if let Some(label) = labels.get(&node.id) {
            node.label = label.clone();
            swatch
                .node_attrs
                .push((node.id.clone(), "aria-expanded".into(), "false".into()));
        } else if state.selecting && state.selected.contains(&node.id) {
            node.label.push_str(", selected for fold");
        }
    }
    for selected in [&mut swatch.selected, &mut swatch.focus] {
        if let Some(owner) = selected.as_ref().and_then(|id| owners.get(id)) {
            *selected = Some(owner.clone());
        }
    }
    let mut bundles = BTreeMap::new();
    let mut retained = Vec::new();
    for mut relation in swatch.relations {
        if !owners.contains_key(&relation.from) && !owners.contains_key(&relation.to) {
            retained.push(relation);
            continue;
        }
        if !relation.visible {
            continue;
        }
        relation.from = owners.get(&relation.from).cloned().unwrap_or(relation.from);
        relation.to = owners.get(&relation.to).cloned().unwrap_or(relation.to);
        if relation.from == relation.to {
            continue;
        }
        relation.route.clear();
        relation.link_key = None;
        relation.marks = Default::default();
        let entry = bundles
            .entry((
                relation.from.clone(),
                relation.to.clone(),
                relation.kind.clone(),
            ))
            .or_insert((relation, 0usize));
        entry.1 += 1;
    }
    for ((from, to, kind), (mut relation, count)) in bundles {
        let a = from.wire_key();
        let b = to.wire_key();
        relation.id = format!(
            "fold-bundle:{}:{a}{}:{b}{}:{kind}",
            a.len(),
            b.len(),
            kind.len()
        );
        relation.label = format!(
            "{count} {kind} relation{}",
            if count == 1 { "" } else { "s" }
        );
        relation.emphasized = count > 1;
        retained.push(relation);
    }
    swatch.relations = retained;
    swatch.graph.edges.clear();
    swatch
}

pub(super) fn controls(ui: &UiState) -> UiChild {
    let buttons: Vec<UiChild> = [
        (
            if ui.overview_folds.selecting {
                "Stop selecting folds"
            } else {
                "Select members for fold"
            },
            "overview-fold-select",
            0,
        ),
        ("Fold selected", "overview-fold-create", 1),
        ("Expand all folds", "overview-fold-expand", 2),
        ("Undo fold", "overview-fold-undo", 3),
        ("Redo fold", "overview-fold-redo", 4),
    ]
    .into_iter()
    .map(|(label, class, action)| {
        Box::new(clickable(
            el("button", text(label)).attr("class", format!("t-btn {class}")),
            move |ui: &mut UiState, _| match action {
                0 => {
                    ui.overview_folds.selecting = !ui.overview_folds.selecting;
                    ui.overview_folds.notice = None;
                },
                1 => {
                    if let Err(why) = ui.create_overview_fold() {
                        ui.overview_folds.notice = Some(why);
                    }
                },
                2 => ui.expand_overview_folds(),
                3 => ui.undo_overview_fold(false),
                _ => ui.undo_overview_fold(true),
            },
        )) as UiChild
    })
    .collect();
    Box::new(el("div", (
        el("div", buttons).attr("class", "overview-instance-actions"),
        el("p", text(if ui.overview_folds.selecting {
            format!("{} members selected. Activate graph nodes or roster entries to toggle selection.", ui.overview_folds.selected.len())
        } else { ui.overview_folds.notice.clone().unwrap_or_else(|| "Folds change this view; Sets, catalog relations and live activity keep their meaning.".into()) })).attr("class", "overview-detail overview-fold-notice"),
    )).attr("class", "overview-fold-controls"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cambium::{GraphCanvasNode, GraphCanvasRelation, GraphCanvasSubgraph};
    use woodshed_core::settings::AppSettings;

    fn source() -> GraphCanvasSwatch<OverviewNodeId, &'static str> {
        let id = |n| OverviewNodeId::View(format!("opaque-{n}"));
        GraphCanvasSwatch::new(
            1,
            GraphCanvasSubgraph {
                nodes: (0..4)
                    .map(|n| GraphCanvasNode {
                        id: id(n),
                        kind: "view",
                        position: (n as f32 / 4., 0.5),
                        label: n.to_string(),
                        key: Some(id(n).wire_key()),
                    })
                    .collect(),
                edges: vec![],
            },
        )
        .with_relations(
            [(0, 1), (0, 2), (1, 3), (3, 0)]
                .into_iter()
                .enumerate()
                .map(|(i, (from, to))| GraphCanvasRelation {
                    id: format!("source-{i}"),
                    from: id(from),
                    to: id(to),
                    kind: "contains".into(),
                    label: "source evidence".into(),
                    route: vec![],
                    link_key: None,
                    marks: Default::default(),
                    strength: 1.,
                    visible: true,
                    emphasized: false,
                })
                .collect(),
        )
    }
    fn record(id: &str, members: &[OverviewNodeId]) -> FoldRecord {
        FoldRecord::new(
            FoldId::new(id),
            SCOPE,
            members.iter().map(OverviewNodeId::wire_key),
            FoldStandIn::Member(members[0].wire_key()),
            Some(FoldRule::Selection),
            Some(id.into()),
        )
        .unwrap()
    }
    #[test]
    fn two_typed_folds_map_every_endpoint_once_without_changing_source_or_member_positions() {
        let source = source();
        let before = source.clone();
        let ids: Vec<_> = source.graph.nodes.iter().map(|n| n.id.clone()).collect();
        let state = OverviewFolds {
            records: vec![record("one", &ids[..2]), record("two", &ids[2..])],
            ..Default::default()
        };
        let folded = project(source.clone(), &state);
        assert_eq!(source, before);
        assert_eq!(folded.graph.nodes.len(), 2);
        assert_eq!(
            folded.graph.nodes[0].position,
            source.graph.nodes[0].position
        );
        assert_eq!(
            folded.graph.nodes[1].position,
            source.graph.nodes[2].position
        );
        assert_eq!(folded.relations.len(), 2);
        let forward = folded.relations.iter().find(|r| r.from == ids[0]).unwrap();
        assert_eq!(forward.to, ids[2]);
        assert_eq!(forward.label, "2 contains relations");
        let reverse = folded.relations.iter().find(|r| r.from == ids[2]).unwrap();
        assert_eq!(reverse.to, ids[0]);
        assert_eq!(reverse.label, "1 contains relation");
        assert_eq!(project(source.clone(), &OverviewFolds::default()), source);
        let mut stale = state;
        stale.records[0].members.push("view:missing".into());
        stale.records[0].members.sort();
        assert_eq!(project(source.clone(), &stale).graph.nodes.len(), 3);
    }
    #[test]
    fn fold_history_and_session_reopen_keep_owner_payload_and_source_truth() {
        let mut ui = UiState::new();
        ui.stage_current(None);
        let snapshot = overview::overview_snapshot(&ui);
        let positions: BTreeMap<_, _> = overview::overview_swatch(&ui)
            .graph
            .nodes
            .into_iter()
            .map(|n| (n.id, n.position))
            .collect();
        let before = serde_json::to_value(ui.to_persisted()).unwrap();
        ui.overview_folds
            .selected
            .extend(snapshot.nodes.iter().take(2).map(|n| n.id.clone()));
        ui.create_overview_fold().unwrap();
        let records = ui.overview_folds.records.clone();
        ui.undo_overview_fold(false);
        assert!(ui.overview_folds.records.is_empty());
        ui.undo_overview_fold(true);
        assert_eq!(ui.overview_folds.records, records);
        let session = ui.to_persisted();
        let mut after = serde_json::to_value(&session).unwrap();
        after["overview_presentation_json"] = before["overview_presentation_json"].clone();
        assert_eq!(before, after);
        assert_eq!(snapshot, overview::overview_snapshot(&ui));
        let mut reopened = UiState::new();
        reopened.apply_persisted(&session, AppSettings::default());
        assert_eq!(reopened.overview_folds.records, records);
        assert!(reopened.overview_folds.undo.is_empty());
        assert!(reopened.overview_folds.redo.is_empty());
        let stand_in = snapshot
            .nodes
            .iter()
            .find(|n| Some(n.id.wire_key().as_str()) == records[0].stand_in_member())
            .unwrap()
            .id
            .clone();
        assert!(reopened.activate_overview_fold(&stand_in));
        assert!(reopened.overview_folds.records.is_empty());
        for node in overview::overview_swatch(&reopened).graph.nodes {
            assert_eq!(Some(&node.position), positions.get(&node.id));
        }
    }
    #[test]
    fn stale_selection_overlaps_wrong_scopes_and_future_records_cannot_replace_owner_facts() {
        let mut ui = UiState::new();
        assert!(ui.create_overview_fold().is_err());
        let ids: Vec<_> = overview::overview_snapshot(&ui)
            .nodes
            .into_iter()
            .take(2)
            .map(|n| n.id)
            .collect();
        ui.overview_folds.selected.extend(ids.clone());
        ui.create_overview_fold().unwrap();
        ui.overview_folds.selected.extend(ids);
        assert!(ui.create_overview_fold().is_err());
        let mut foreign = ui.overview_folds.records[0].clone();
        foreign.source_scope = "other".into();
        assert!(admit_records(vec![foreign]).is_empty());
        let mut session = ui.to_persisted();
        let mut wire: serde_json::Value =
            serde_json::from_str(session.overview_presentation_json.as_ref().unwrap()).unwrap();
        wire["folds"][0]["version"] = serde_json::json!(99);
        let raw = wire.to_string();
        session.overview_presentation_json = Some(raw.clone());
        let mut reopened = UiState::new();
        reopened.apply_persisted(&session, AppSettings::default());
        assert!(reopened.overview_folds.records.is_empty());
        reopened.overview_folds.selected.extend(
            overview::overview_snapshot(&reopened)
                .nodes
                .into_iter()
                .take(2)
                .map(|n| n.id),
        );
        assert!(
            reopened
                .create_overview_fold()
                .unwrap_err()
                .contains("retained graph presentation")
        );
        assert_eq!(
            reopened.to_persisted().overview_presentation_json,
            Some(raw)
        );
    }
}
