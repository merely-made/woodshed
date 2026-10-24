//! Woodshed routes layered over Cambium's bounded navigation model.
//!
//! These addresses name existing owners and occurrences. They never carry a
//! copy of the Set or of an audio/editor process.
use super::{UiState, WorkspacePanel};
use cambium::scope_navigation::{
    ScopeCamera, ScopeCommand, ScopeControls, ScopeCrumb, ScopeDirection, ScopeNavigation,
    ScopeVisit, scope_controls, scope_trail_with_location_edit,
};
use woodshed_core::{
    session_overview::{OverviewNodeId, SessionArtifactId},
    working_sets::WorkingSetId,
};
use woodshedding::rehearsal::CardId;
use serde::{Deserialize, Serialize};

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
        if matches!(self.section, super::AppSection::Stage | super::AppSection::Rehearsal)
            && matches!(
                self.workspace.active_panel(),
                Some(WorkspacePanel::Overview | WorkspacePanel::Set | WorkspacePanel::Practice)
            )
        {
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
mod tests {
    use super::*;

    fn instance_node(owner: WorkingSetId) -> OverviewNodeId {
        OverviewNodeId::Artifact(SessionArtifactId::WorkingSetInstance(owner))
    }
    fn two_sets_with_the_same_occurrence() -> (UiState, WorkingSetId, WorkingSetId, CardId, CardId)
    {
        let mut ui = UiState::new();
        ui.stage_current(None);
        let first = ui.working_sets.active_id;
        let first_card = ui.set.cards[0].id;
        let same_material = ui.set.clone();
        let second = ui
            .working_sets
            .create(&mut ui.set, "Same material", same_material);
        let second_card = ui.set.cards[0].id;
        assert_eq!(
            first_card, second_card,
            "the Set owner qualifies an occurrence even when Card IDs coincide"
        );
        (ui, first, second, first_card, second_card)
    }
    fn open_owner(ui: &mut UiState, owner: WorkingSetId) {
        ui.activate_workspace_panel(WorkspacePanel::Overview);
        ui.overview_focus = Some(instance_node(owner));
        ui.open_overview_node(instance_node(owner));
    }

    #[test]
    fn up_and_down_move_between_overview_set_and_card_without_changing_runner_cursor() {
        let (mut ui, owner, _, card, _) = two_sets_with_the_same_occurrence();
        open_owner(&mut ui, owner);
        assert_eq!(ui.workspace.active_panel(), Some(WorkspacePanel::Set));
        let runner_cursor = ui.set.cursor_id();
        ui.remember_card_scope(card);
        ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Down));
        assert_eq!(
            ui.scope_navigation.current().unwrap().place,
            WoodshedScopePlace::CardOccurrence { owner, card }
        );
        assert_eq!(ui.workspace.active_panel(), Some(WorkspacePanel::Practice));
        assert_eq!(
            ui.set.cursor_id(),
            runner_cursor,
            "Card navigation must not seek the Set runner"
        );
        ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Up));
        assert_eq!(
            ui.scope_navigation.current().unwrap().place,
            WoodshedScopePlace::WorkingSet {
                owner,
                panel: WorkspacePanel::Set
            }
        );
        ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Up));
        assert_eq!(
            ui.scope_navigation.current().unwrap().place,
            WoodshedScopePlace::Overview
        );
        assert_eq!(ui.workspace.active_panel(), Some(WorkspacePanel::Overview));
        ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Down));
        assert_eq!(
            ui.scope_navigation.current().unwrap().place,
            WoodshedScopePlace::WorkingSet {
                owner,
                panel: WorkspacePanel::Set
            }
        );
        ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Down));
        assert_eq!(
            ui.scope_navigation.current().unwrap().place,
            WoodshedScopePlace::CardOccurrence { owner, card }
        );
    }

    #[test]
    fn back_and_forward_revisit_owner_card_occurrences_independently_of_scope_depth() {
        let (mut ui, first, second, first_card, second_card) = two_sets_with_the_same_occurrence();
        open_owner(&mut ui, first);
        ui.remember_card_scope(first_card);
        ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Down));
        assert_eq!(
            ui.scope_navigation.current().unwrap().place,
            WoodshedScopePlace::CardOccurrence {
                owner: first,
                card: first_card
            }
        );
        assert!(ui.visit_scope(
            WoodshedScopePlace::CardOccurrence {
                owner: second,
                card: second_card
            },
            Some(WoodshedScopeAnchor::CardOccurrence {
                owner: second,
                card: second_card
            }),
        ));
        assert_eq!(ui.working_sets.active_id, second);
        ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Back));
        assert_eq!(ui.working_sets.active_id, first);
        assert_eq!(
            ui.scope_navigation.current().unwrap().place,
            WoodshedScopePlace::CardOccurrence {
                owner: first,
                card: first_card
            }
        );
        ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Forward));
        assert_eq!(ui.working_sets.active_id, second);
        assert_eq!(
            ui.scope_navigation.current().unwrap().place,
            WoodshedScopePlace::CardOccurrence {
                owner: second,
                card: second_card
            }
        );
    }

    #[test]
    fn edits_made_to_an_inactive_set_survive_return_navigation() {
        let (mut ui, first, second, _, _) = two_sets_with_the_same_occurrence();
        open_owner(&mut ui, first);
        assert!(ui.visit_scope(
            WoodshedScopePlace::WorkingSet {
                owner: second,
                panel: WorkspacePanel::Set
            },
            None,
        ));
        let inactive = ui.working_sets.get_mut(first, &mut ui.set).unwrap();
        inactive.cards[0].label = "Edited while away".into();
        assert!(ui.visit_scope(
            WoodshedScopePlace::WorkingSet {
                owner: first,
                panel: WorkspacePanel::Set
            },
            None,
        ));
        assert_eq!(ui.working_sets.active_id, first);
        assert_eq!(ui.set.cards[0].label, "Edited while away");
    }

    #[test]
    fn history_restores_the_original_practice_panel_and_graph_camera() {
        let (mut ui, owner, _, _, _) = two_sets_with_the_same_occurrence();
        open_owner(&mut ui, owner);
        ui.activate_workspace_panel(WorkspacePanel::Practice);
        ui.set_graph_viewport = cambium::GraphViewport {
            pan: (0.23, -0.17),
            zoom: 1.4,
        };

        assert!(ui.visit_scope(
            WoodshedScopePlace::Overview,
            Some(WoodshedScopeAnchor::OverviewNode(instance_node(owner))),
        ));
        ui.set_graph_viewport = cambium::GraphViewport::default();
        ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Back));

        assert_eq!(ui.workspace.active_panel(), Some(WorkspacePanel::Practice));
        assert_eq!(
            ui.set_graph_viewport,
            cambium::GraphViewport {
                pan: (0.23, -0.17),
                zoom: 1.4,
            }
        );
        assert_eq!(
            ui.scope_navigation.current().unwrap().place,
            WoodshedScopePlace::WorkingSet {
                owner,
                panel: WorkspacePanel::Practice,
            }
        );
    }

    #[test]
    fn reopened_card_history_restores_the_exact_overview_camera_bookmark() {
        let (mut ui, first, second, first_card, second_card) =
            two_sets_with_the_same_occurrence();
        open_owner(&mut ui, first);
        ui.remember_card_scope(first_card);
        ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Down));
        ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Up));
        ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Up));
        ui.overview_viewport = cambium::GraphViewport {
            pan: (0.16, -0.12),
            zoom: 1.1,
        };
        assert!(ui.visit_scope(
            WoodshedScopePlace::WorkingSet {
                owner: second,
                panel: WorkspacePanel::Set,
            },
            Some(WoodshedScopeAnchor::OverviewNode(instance_node(second))),
        ));
        ui.remember_card_scope(second_card);
        ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Down));
        ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Up));
        ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Up));
        ui.overview_viewport = cambium::GraphViewport {
            pan: (0.31, -0.22),
            zoom: 1.25,
        };
        ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Down));
        ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Down));

        let persisted = ui.to_persisted();
        let mut reopened = UiState::new();
        assert!(reopened
            .apply_persisted(&persisted, ui.app_settings.clone())
            .is_none());
        assert_eq!(reopened.working_sets.active_id, second);
        assert_eq!(reopened.workspace.active_panel(), Some(WorkspacePanel::Practice));
        assert_eq!(reopened.overview_viewport.zoom, 1.25);

        reopened.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Back));
        reopened.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Back));
        assert_eq!(reopened.workspace.active_panel(), Some(WorkspacePanel::Overview));
        assert_eq!(reopened.overview_viewport, cambium::GraphViewport {
            pan: (0.31, -0.22),
            zoom: 1.25,
        });
        assert_eq!(reopened.scope_navigation.current().unwrap().place, WoodshedScopePlace::Overview);

        for _ in 0..10 {
            if reopened.scope_navigation.current().is_some_and(|visit| {
                visit.place == WoodshedScopePlace::Overview
                    && visit.anchor
                        == Some(WoodshedScopeAnchor::OverviewNode(instance_node(first)))
            }) {
                break;
            }
            reopened.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Back));
        }
        assert_eq!(reopened.workspace.active_panel(), Some(WorkspacePanel::Overview));
        assert_eq!(
            reopened.scope_navigation.current().unwrap().anchor,
            Some(WoodshedScopeAnchor::OverviewNode(instance_node(first)))
        );
        assert_eq!(reopened.overview_viewport, cambium::GraphViewport {
            pan: (0.16, -0.12),
            zoom: 1.1,
        });
        assert_eq!(reopened.scope_navigation.current().unwrap().place, WoodshedScopePlace::Overview);
    }

    #[test]
    fn session_save_reopens_contextual_history_and_owner_qualified_card_route() {
        let (mut ui, first, second, first_card, second_card) = two_sets_with_the_same_occurrence();
        open_owner(&mut ui, first);
        ui.remember_card_scope(first_card);
        ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Down));
        assert!(ui.visit_scope(
            WoodshedScopePlace::CardOccurrence {
                owner: second,
                card: second_card,
            },
            Some(WoodshedScopeAnchor::CardOccurrence {
                owner: second,
                card: second_card,
            }),
        ));
        ui.set_graph_viewport = cambium::GraphViewport {
            pan: (0.31, -0.22),
            zoom: 1.6,
        };
        ui.music_example(false);
        ui.practice_music();
        assert_eq!(ui.music_editor.practice.owner(), Some(second));

        let saved = ui.to_persisted();
        assert!(saved.scope_navigation_json.is_some());
        let encoded = serde_json::to_string(&saved).unwrap();
        let reopened_session = woodshed_core::storage::decode_session(&encoded)
            .unwrap()
            .session;
        let mut reopened = UiState::new();
        assert!(reopened
            .apply_persisted(&reopened_session, ui.app_settings.clone())
            .is_none());

        assert_eq!(reopened.working_sets.active_id, second);
        assert!(reopened.music_editor.practice.owner().is_none());
        assert_eq!(reopened.workspace.active_panel(), Some(WorkspacePanel::Practice));
        assert_eq!(
            reopened.scope_navigation.current().unwrap().place,
            WoodshedScopePlace::CardOccurrence {
                owner: second,
                card: second_card,
            }
        );
        assert_eq!(
            reopened.set_graph_viewport,
            cambium::GraphViewport {
                pan: (0.31, -0.22),
                zoom: 1.6,
            }
        );
        reopened.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Back));
        assert_eq!(reopened.working_sets.active_id, first);
        assert_eq!(
            reopened.scope_navigation.current().unwrap().place,
            WoodshedScopePlace::CardOccurrence {
                owner: first,
                card: first_card,
            }
        );
    }

    #[test]
    fn corrupt_stale_and_legacy_scope_snapshots_fail_closed() {
        let legacy = woodshed_core::storage::PersistedSession::default();
        assert!(legacy.scope_navigation_json.is_none());
        let mut reopened = UiState::new();
        reopened.apply_persisted(&legacy, Default::default());
        assert!(reopened.scope_navigation.current().is_none());
        assert!(reopened.scope_navigation_notice.is_none());

        let (mut ui, owner, _, card, _) = two_sets_with_the_same_occurrence();
        open_owner(&mut ui, owner);
        ui.remember_card_scope(card);
        ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Down));

        let mut corrupt = ui.to_persisted();
        corrupt.scope_navigation_json = Some("{".into());
        let mut reopened = UiState::new();
        reopened.apply_persisted(&corrupt, ui.app_settings.clone());
        assert!(reopened.scope_navigation.current().is_none());
        assert!(reopened
            .scope_navigation_notice
            .as_deref()
            .is_some_and(|notice| notice.contains("malformed")));
        assert_eq!(
            reopened.to_persisted().scope_navigation_json.as_deref(),
            Some("{")
        );
        assert!(reopened.visit_scope(
            WoodshedScopePlace::WorkingSet {
                owner: ui.working_sets.active_id,
                panel: WorkspacePanel::Set,
            },
            None,
        ));
        assert!(reopened.scope_navigation_notice.is_none());
        assert_ne!(
            reopened.to_persisted().scope_navigation_json.as_deref(),
            Some("{")
        );

        let mut stale = ui.to_persisted();
        stale.set.cards.clear();
        let stale_navigation_json = stale.scope_navigation_json.clone().unwrap();
        let mut reopened = UiState::new();
        reopened.apply_persisted(&stale, ui.app_settings.clone());
        assert!(reopened.scope_navigation.current().is_none());
        assert_eq!(
            reopened.to_persisted().scope_navigation_json.as_deref(),
            Some(stale_navigation_json.as_str())
        );
        assert!(reopened
            .scope_navigation_notice
            .as_deref()
            .is_some_and(|notice| notice.contains("missing material")));
    }

    #[test]
    fn navigation_preserves_the_music_practice_owner_transport_and_capture_state() {
        let (mut ui, owner, other, _, _) = two_sets_with_the_same_occurrence();
        assert!(ui.working_sets.activate(owner, &mut ui.set));
        ui.music_example(false);
        ui.practice_music();
        let practice_owner = ui.music_editor.practice.owner();
        let practice_status = ui.music_editor.practice.status();
        let practice_epoch = ui.music_editor.practice.run_epoch();
        assert_eq!(practice_owner, Some(owner));
        ui.song_playing = true;
        ui.song_recording = true;
        ui.rehearsal_running = true;
        ui.rehearsal_owner = Some(owner);
        let transport = ui.transport.clone();
        let captures = ui.loop_captures.clone();
        let audio_requests = ui.audio_requests.clone();

        open_owner(&mut ui, other);

        assert_eq!(ui.working_sets.active_id, other);
        assert_eq!(ui.music_editor.practice.owner(), practice_owner);
        assert_eq!(ui.music_editor.practice.status(), practice_status);
        assert_eq!(ui.music_editor.practice.run_epoch(), practice_epoch);
        assert_eq!(ui.rehearsal_owner, Some(owner));
        assert!(ui.rehearsal_running);
        assert!(ui.song_playing && ui.song_recording);
        assert_eq!(ui.transport, transport);
        assert_eq!(ui.loop_captures, captures);
        assert_eq!(ui.audio_requests, audio_requests);
        assert_eq!(ui.workspace.active_panel(), Some(WorkspacePanel::Set));
        assert_ne!(ui.rehearsal_owner, Some(ui.working_sets.active_id));
        assert_eq!(ui.music_editor.practice.owner(), Some(owner));
    }

    #[test]
    fn stale_history_destination_does_not_move_the_route_or_open_a_replacement() {
        let (mut ui, first, second, first_card, second_card) = two_sets_with_the_same_occurrence();
        open_owner(&mut ui, first);
        ui.remember_card_scope(first_card);
        ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Down));
        assert!(ui.visit_scope(
            WoodshedScopePlace::CardOccurrence {
                owner: second,
                card: second_card
            },
            Some(WoodshedScopeAnchor::CardOccurrence {
                owner: second,
                card: second_card
            }),
        ));
        let destination_owner = ui.working_sets.active_id;
        let current_place = ui.scope_navigation.current().unwrap().place.clone();
        ui.working_sets
            .get_mut(first, &mut ui.set)
            .unwrap()
            .cards
            .clear();
        ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Back));
        assert_eq!(ui.working_sets.active_id, destination_owner);
        assert_eq!(ui.scope_navigation.current().unwrap().place, current_place);
        assert!(
            matches!(current_place, WoodshedScopePlace::CardOccurrence { owner, .. } if owner == second)
        );
    }
}
