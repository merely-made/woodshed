//! Woodshed routes layered over Cambium's bounded navigation model.
//!
//! These addresses name existing owners and occurrences. They never carry a
//! copy of the Set or of an audio/editor process.
use super::{UiState, WorkspacePanel};
use cambium::scope_navigation::{
    ScopeCamera, ScopeCommand, ScopeControls, ScopeCrumb, ScopeDirection, ScopeNavigation,
    ScopeVisit, scope_controls, scope_trail_with_location_edit,
};
use serde::{Deserialize, Serialize};
use woodshed_core::{
    session_overview::{OverviewNodeId, SessionArtifactId},
    working_sets::WorkingSetId,
};
use woodshedding::rehearsal::CardId;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum WoodshedScopePlace {
    Overview,
    WorkingSet {
        owner: WorkingSetId,
        panel: WorkspacePanel,
    },
    CardOccurrence {
        owner: WorkingSetId,
        card: CardId,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum WoodshedScopeAnchor {
    CardOccurrence { owner: WorkingSetId, card: CardId },
    OverviewNode(OverviewNodeId),
}

pub(crate) type Navigation = ScopeNavigation<WoodshedScopePlace, WoodshedScopeAnchor>;

impl UiState {
    pub(super) fn persist_scope_navigation(&self) -> Result<String, serde_json::Error> {
        if let Some(rejected) = &self.scope_navigation_rejected_json {
            return Ok(rejected.clone());
        }
        let mut navigation = self.scope_navigation.clone();
        if let Some(current) = self.current_scope_visit() {
            navigation.capture(current);
        }
        serde_json::to_string(&navigation)
    }

    pub(super) fn restore_scope_navigation(&mut self, json: Option<&str>) {
        self.scope_navigation = Navigation::default();
        self.scope_navigation_notice = None;
        self.scope_navigation_rejected_json = None;
        let Some(json) = json else {
            return;
        };
        let Ok(navigation) = serde_json::from_str::<Navigation>(json) else {
            self.scope_navigation_rejected_json = Some(json.to_owned());
            self.scope_navigation_notice =
                Some("Saved navigation history was malformed and was ignored.".into());
            return;
        };
        if navigation
            .validate(
                |place| self.scope_place_valid(place),
                |anchor| self.scope_anchor_valid(anchor),
            )
            .is_err()
        {
            self.scope_navigation_rejected_json = Some(json.to_owned());
            self.scope_navigation_notice = Some(
                "Saved navigation history pointed to missing material or an invalid view and was ignored."
                    .into(),
            );
            return;
        }

        self.scope_navigation = navigation;
        if matches!(
            self.section,
            super::AppSection::Stage | super::AppSection::Rehearsal
        ) && matches!(
            self.workspace.active_panel(),
            Some(WorkspacePanel::Overview | WorkspacePanel::Set | WorkspacePanel::Practice)
        ) {
            if let Some(current) = self.scope_navigation.current().cloned() {
                if !self.restore_scope_visit(&current) {
                    self.scope_navigation = Navigation::default();
                    self.scope_navigation_rejected_json = Some(json.to_owned());
                    self.scope_navigation_notice = Some(
                        "Saved navigation history could not be restored and was ignored.".into(),
                    );
                }
            }
        }
    }

    pub(super) fn current_scope_visit(
        &self,
    ) -> Option<ScopeVisit<WoodshedScopePlace, WoodshedScopeAnchor>> {
        let panel = self.workspace.active_panel()?;
        let owner = self.working_sets.active_id;
        let selected = self
            .scope_card_anchors
            .get(&owner)
            .copied()
            .filter(|card| {
                self.working_sets
                    .get(owner, &self.set)
                    .is_some_and(|set| set.cards.iter().any(|c| c.id == *card))
            })
            .or_else(|| self.set.cursor_id());
        let saved_place =
            self.scope_navigation
                .current()
                .and_then(|visit| match (&visit.place, panel) {
                    (
                        WoodshedScopePlace::WorkingSet {
                            owner: saved,
                            panel: saved_panel,
                        },
                        WorkspacePanel::Set | WorkspacePanel::Practice,
                    ) if *saved == owner && *saved_panel == panel => Some(visit.place.clone()),
                    (
                        WoodshedScopePlace::CardOccurrence { owner: saved, .. },
                        WorkspacePanel::Practice,
                    ) if *saved == owner => Some(visit.place.clone()),
                    (WoodshedScopePlace::Overview, WorkspacePanel::Overview) => {
                        Some(visit.place.clone())
                    },
                    _ => None,
                });
        let (place, anchor, camera) = match panel {
            WorkspacePanel::Overview => (
                WoodshedScopePlace::Overview,
                self.overview_focus
                    .clone()
                    .map(WoodshedScopeAnchor::OverviewNode),
                Some(scope_camera(self.overview_viewport)),
            ),
            WorkspacePanel::Practice | WorkspacePanel::Set => {
                let place = saved_place.unwrap_or(WoodshedScopePlace::WorkingSet { owner, panel });
                let anchor =
                    selected.map(|card| WoodshedScopeAnchor::CardOccurrence { owner, card });
                let camera = (panel == WorkspacePanel::Practice)
                    .then(|| scope_camera(self.set_graph_viewport));
                (place, anchor, camera)
            },
            _ => return None,
        };
        Some(ScopeVisit {
            place,
            camera,
            anchor,
        })
    }

    pub(super) fn scope_controls(&self) -> ScopeControls {
        let current = self.current_scope_visit();
        let back = self
            .scope_navigation
            .destination(false)
            .and_then(|v| self.scope_place_label(&v.place));
        let forward = self
            .scope_navigation
            .destination(true)
            .and_then(|v| self.scope_place_label(&v.place));
        let up = current
            .as_ref()
            .and_then(|v| self.scope_parent(&v.place))
            .and_then(|p| self.scope_place_label(&p));
        let down = current
            .as_ref()
            .and_then(|v| self.scope_child(&v.place, v.anchor.as_ref()))
            .and_then(|p| self.scope_place_label(&p));
        ScopeControls {
            back,
            forward,
            up,
            down,
            hint: self.scope_navigation_hint,
        }
    }

    pub(super) fn scope_trail(&self) -> Vec<ScopeCrumb<WoodshedScopePlace>> {
        let Some(visit) = self.current_scope_visit() else {
            return Vec::new();
        };
        let mut places = vec![WoodshedScopePlace::Overview];
        match visit.place {
            WoodshedScopePlace::Overview => {},
            WoodshedScopePlace::WorkingSet { owner, panel } => {
                places.push(WoodshedScopePlace::WorkingSet { owner, panel })
            },
            WoodshedScopePlace::CardOccurrence { owner, card } => {
                places.push(WoodshedScopePlace::WorkingSet {
                    owner,
                    panel: WorkspacePanel::Set,
                });
                places.push(WoodshedScopePlace::CardOccurrence { owner, card });
            },
        }
        let last = places.len().saturating_sub(1);
        places
            .into_iter()
            .enumerate()
            .filter_map(|(index, place)| {
                self.scope_place_label(&place).map(|label| ScopeCrumb {
                    label,
                    target: place,
                    current: index == last,
                })
            })
            .collect()
    }

    pub(super) fn scope_navigation_widget(&self) -> super::UiChild {
        Box::new(scope_controls(
            self.scope_controls(),
            |ui: &mut UiState, command: ScopeCommand<WoodshedScopePlace>| {
                ui.handle_scope_command(command);
            },
        ))
    }

    pub(super) fn scope_trail_widget(&self) -> super::UiChild {
        Box::new(scope_trail_with_location_edit(
            self.scope_trail(),
            false,
            |ui: &mut UiState, command: ScopeCommand<WoodshedScopePlace>| {
                ui.handle_scope_command(command);
            },
        ))
    }

    pub(super) fn remember_card_scope(&mut self, card: CardId) {
        let owner = self.working_sets.active_id;
        self.scope_card_anchors.insert(owner, card);
        match self.scope_navigation.current().map(|v| v.place.clone()) {
            Some(WoodshedScopePlace::CardOccurrence { owner: current, .. }) if current == owner => {
                self.scope_navigation.visit(self.make_scope_visit(
                    WoodshedScopePlace::CardOccurrence { owner, card },
                    Some(WoodshedScopeAnchor::CardOccurrence { owner, card }),
                ));
            },
            _ => {
                if let Some(current) = self.current_scope_visit() {
                    self.scope_navigation.capture(current);
                }
            },
        }
    }

    pub(super) fn visit_scope(
        &mut self,
        place: WoodshedScopePlace,
        anchor: Option<WoodshedScopeAnchor>,
    ) -> bool {
        if !self.scope_place_valid(&place)
            || anchor.as_ref().is_some_and(|a| !self.scope_anchor_valid(a))
        {
            return false;
        }
        if let Some(current) = self.current_scope_visit() {
            self.scope_navigation.capture(current);
        }
        let mut visit = self
            .scope_saved_visit(&place)
            .unwrap_or_else(|| self.make_scope_visit(place, anchor.clone()));
        if anchor.is_some() {
            visit.anchor = anchor;
        }
        if !self.restore_scope_visit(&visit) {
            return false;
        }
        self.scope_navigation.visit(visit);
        self.scope_navigation_rejected_json = None;
        self.scope_navigation_notice = None;
        true
    }

    pub(super) fn handle_scope_command(&mut self, command: ScopeCommand<WoodshedScopePlace>) {
        match command {
            ScopeCommand::Hint(direction) => self.scope_navigation_hint = direction,
            ScopeCommand::EditLocation => {},
            ScopeCommand::To(place) => {
                self.visit_scope(place, None);
            },
            ScopeCommand::Navigate(
                direction @ (ScopeDirection::Back | ScopeDirection::Forward),
            ) => {
                let forward = direction == ScopeDirection::Forward;
                let Some(destination) = self.scope_navigation.destination(forward).cloned() else {
                    return;
                };
                if !self.scope_place_valid(&destination.place)
                    || destination
                        .anchor
                        .as_ref()
                        .is_some_and(|a| !self.scope_anchor_valid(a))
                {
                    return;
                }
                if let Some(current) = self.current_scope_visit() {
                    self.scope_navigation.capture(current);
                }
                if self.restore_scope_visit(&destination) {
                    if self.scope_navigation.step(forward) {
                        self.scope_navigation_rejected_json = None;
                        self.scope_navigation_notice = None;
                    }
                }
            },
            ScopeCommand::Navigate(direction @ (ScopeDirection::Up | ScopeDirection::Down)) => {
                let Some(current) = self.current_scope_visit() else {
                    return;
                };
                let (target, anchor) = match direction {
                    ScopeDirection::Up => self
                        .scope_parent_visit(&current.place)
                        .unwrap_or((WoodshedScopePlace::Overview, None)),
                    _ => (
                        self.scope_child(&current.place, current.anchor.as_ref())
                            .unwrap_or_else(|| current.place.clone()),
                        current.anchor.clone(),
                    ),
                };
                if target != current.place {
                    self.visit_scope(target, anchor);
                }
            },
        }
    }

    fn scope_place_valid(&self, place: &WoodshedScopePlace) -> bool {
        match place {
            WoodshedScopePlace::Overview => true,
            WoodshedScopePlace::WorkingSet { owner, panel } => {
                self.working_sets.get(*owner, &self.set).is_some()
                    && matches!(panel, WorkspacePanel::Set | WorkspacePanel::Practice)
            },
            WoodshedScopePlace::CardOccurrence { owner, card } => self
                .working_sets
                .get(*owner, &self.set)
                .is_some_and(|set| set.cards.iter().any(|c| c.id == *card)),
        }
    }
    fn scope_anchor_valid(&self, anchor: &WoodshedScopeAnchor) -> bool {
        match anchor {
            WoodshedScopeAnchor::CardOccurrence { owner, card } => {
                self.scope_place_valid(&WoodshedScopePlace::CardOccurrence {
                    owner: *owner,
                    card: *card,
                })
            },
            WoodshedScopeAnchor::OverviewNode(id) => super::overview::overview_snapshot(self)
                .nodes
                .iter()
                .any(|node| node.id == *id),
        }
    }
    fn scope_place_label(&self, place: &WoodshedScopePlace) -> Option<String> {
        match place {
            WoodshedScopePlace::Overview => Some("Session overview".into()),
            WoodshedScopePlace::WorkingSet { owner, panel } => {
                let name = self
                    .working_sets
                    .summaries(&self.set)
                    .into_iter()
                    .find(|s| s.id == *owner)?
                    .name;
                Some(if *panel == WorkspacePanel::Practice {
                    format!("Practice · {name}")
                } else {
                    name
                })
            },
            WoodshedScopePlace::CardOccurrence { owner, card } => {
                let set = self.working_sets.get(*owner, &self.set)?;
                let occurrence = set.cards.iter().find(|c| c.id == *card)?;
                Some(if occurrence.label.trim().is_empty() {
                    "Card".into()
                } else {
                    occurrence.label.clone()
                })
            },
        }
    }
    fn scope_parent(&self, place: &WoodshedScopePlace) -> Option<WoodshedScopePlace> {
        match place {
            WoodshedScopePlace::Overview => None,
            WoodshedScopePlace::WorkingSet { .. } => Some(WoodshedScopePlace::Overview),
            WoodshedScopePlace::CardOccurrence { owner, .. } => {
                Some(WoodshedScopePlace::WorkingSet {
                    owner: *owner,
                    panel: WorkspacePanel::Set,
                })
            },
        }
    }
    fn scope_parent_visit(
        &self,
        place: &WoodshedScopePlace,
    ) -> Option<(WoodshedScopePlace, Option<WoodshedScopeAnchor>)> {
        match place {
            WoodshedScopePlace::Overview => None,
            WoodshedScopePlace::WorkingSet { owner, .. } => Some((
                WoodshedScopePlace::Overview,
                Some(WoodshedScopeAnchor::OverviewNode(OverviewNodeId::Artifact(
                    SessionArtifactId::WorkingSetInstance(*owner),
                ))),
            )),
            WoodshedScopePlace::CardOccurrence { owner, card } => Some((
                WoodshedScopePlace::WorkingSet {
                    owner: *owner,
                    panel: WorkspacePanel::Set,
                },
                Some(WoodshedScopeAnchor::CardOccurrence {
                    owner: *owner,
                    card: *card,
                }),
            )),
        }
    }
    fn scope_child(
        &self,
        place: &WoodshedScopePlace,
        anchor: Option<&WoodshedScopeAnchor>,
    ) -> Option<WoodshedScopePlace> {
        match place {
            WoodshedScopePlace::Overview => {
                if let Some(WoodshedScopeAnchor::OverviewNode(OverviewNodeId::Artifact(
                    SessionArtifactId::WorkingSetInstance(owner),
                ))) = anchor
                {
                    let place = WoodshedScopePlace::WorkingSet {
                        owner: *owner,
                        panel: WorkspacePanel::Set,
                    };
                    return self.scope_place_valid(&place).then_some(place);
                }
                let place = WoodshedScopePlace::WorkingSet {
                    owner: self.working_sets.active_id,
                    panel: WorkspacePanel::Set,
                };
                self.scope_place_valid(&place).then_some(place)
            },
            WoodshedScopePlace::WorkingSet { owner, .. } => {
                let card = match anchor {
                    Some(WoodshedScopeAnchor::CardOccurrence {
                        owner: anchored_owner,
                        card,
                    }) if anchored_owner == owner => Some(*card),
                    _ => self
                        .scope_card_anchors
                        .get(owner)
                        .copied()
                        .or_else(|| self.working_sets.get(*owner, &self.set)?.cursor_id()),
                }?;
                self.scope_place_valid(&WoodshedScopePlace::CardOccurrence {
                    owner: *owner,
                    card,
                })
                .then_some(WoodshedScopePlace::CardOccurrence {
                    owner: *owner,
                    card,
                })
            },
            WoodshedScopePlace::CardOccurrence { .. } => None,
        }
    }
    fn scope_saved_visit(
        &self,
        place: &WoodshedScopePlace,
    ) -> Option<ScopeVisit<WoodshedScopePlace, WoodshedScopeAnchor>> {
        self.scope_navigation
            .saved_matching(|candidate| candidate == place)
            .cloned()
    }
    fn make_scope_visit(
        &self,
        place: WoodshedScopePlace,
        anchor: Option<WoodshedScopeAnchor>,
    ) -> ScopeVisit<WoodshedScopePlace, WoodshedScopeAnchor> {
        let camera = match &place {
            WoodshedScopePlace::Overview => Some(scope_camera(self.overview_viewport)),
            WoodshedScopePlace::CardOccurrence { .. } => {
                Some(scope_camera(self.set_graph_viewport))
            },
            WoodshedScopePlace::WorkingSet { panel, .. } => {
                (*panel == WorkspacePanel::Practice).then(|| scope_camera(self.set_graph_viewport))
            },
        };
        ScopeVisit {
            place,
            camera,
            anchor,
        }
    }
    fn restore_scope_visit(
        &mut self,
        visit: &ScopeVisit<WoodshedScopePlace, WoodshedScopeAnchor>,
    ) -> bool {
        if !self.scope_place_valid(&visit.place)
            || visit
                .anchor
                .as_ref()
                .is_some_and(|a| !self.scope_anchor_valid(a))
        {
            return false;
        }
        match &visit.place {
            WoodshedScopePlace::Overview => {
                self.activate_workspace_panel(WorkspacePanel::Overview);
                if let Some(camera) = visit.camera {
                    self.overview_viewport = graph_viewport(camera);
                }
                if let Some(WoodshedScopeAnchor::OverviewNode(id)) = &visit.anchor {
                    self.overview_focus = Some(id.clone());
                }
            },
            WoodshedScopePlace::WorkingSet { owner, panel } => {
                if !self.activate_working_set_for_scope(*owner) {
                    return false;
                }
                self.activate_workspace_panel(*panel);
                if *panel == WorkspacePanel::Practice {
                    if let Some(camera) = visit.camera {
                        self.set_graph_viewport = graph_viewport(camera);
                    }
                }
                if let Some(WoodshedScopeAnchor::CardOccurrence { owner, card }) = &visit.anchor {
                    self.scope_card_anchors.insert(*owner, *card);
                }
            },
            WoodshedScopePlace::CardOccurrence { owner, card } => {
                if !self.activate_working_set_for_scope(*owner) {
                    return false;
                }
                self.activate_workspace_panel(WorkspacePanel::Practice);
                self.scope_card_anchors.insert(*owner, *card);
                if let Some(camera) = visit.camera {
                    self.set_graph_viewport = graph_viewport(camera);
                }
            },
        }
        true
    }
}

fn scope_camera(viewport: cambium::GraphViewport) -> ScopeCamera {
    ScopeCamera {
        x: viewport.pan.0,
        y: viewport.pan.1,
        zoom: viewport.zoom,
    }
}
fn graph_viewport(camera: ScopeCamera) -> cambium::GraphViewport {
    cambium::GraphViewport {
        pan: (camera.x, camera.y),
        zoom: camera.zoom,
    }
}

#[cfg(test)]
#[path = "contextual_navigation/tests.rs"]
mod tests;
