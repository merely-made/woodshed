//! Viewer-owned dynamics over Woodshed's disclosed Mere occurrences.
//!
//! Scenomise remains the arrangement authority. Pictograph's graph-free board
//! supplies motion and its advertised curation actions; neither edits a Set.
use std::collections::BTreeMap;

use pictograph::canvas::{
    AdvertisedAction, BoardItem, PhysicsBoard, PhysicsChoice, PhysicsLaw, Role,
};
use woodshed_core::session_overview::OverviewNodeId;

/// Fixed score-space scale. Paint and native targets read the same inverse;
/// fitting every moving frame would make a stationary item appear to move.
const WORLD_SIZE: f32 = 1000.0;
pub type DynamicsItem = (OverviewNodeId, (f32, f32), String);

pub struct OverviewDynamics {
    board: PhysicsBoard,
    items: Vec<DynamicsItem>,
    roles: BTreeMap<String, String>,
    law: PhysicsLaw,
    motion: bool,
    dragging: Option<OverviewNodeId>,
    held_position: Option<(f32, f32)>,
    move_origin: Option<(f32, f32)>,
}

impl std::fmt::Debug for OverviewDynamics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OverviewDynamics")
            .field("items", &self.items.len())
            .field("law", &self.law)
            .field("motion", &self.motion)
            .finish()
    }
}

impl Default for OverviewDynamics {
    fn default() -> Self {
        Self {
            board: PhysicsBoard::new(),
            items: Vec::new(),
            roles: BTreeMap::new(),
            law: PhysicsLaw::Still,
            motion: false,
            dragging: None,
            held_position: None,
            move_origin: None,
        }
    }
}

fn world(at: (f32, f32)) -> (f32, f32) {
    ((at.0 - 0.5) * WORLD_SIZE, (at.1 - 0.5) * WORLD_SIZE)
}

fn normalized(at: (f32, f32)) -> (f32, f32) {
    (at.0 / WORLD_SIZE + 0.5, at.1 / WORLD_SIZE + 0.5)
}

impl OverviewDynamics {
    /// Reconcile only when disclosed slots, roles or motion intent change.
    /// Motion is opt-in; reduced motion always wins over the saved preference.
    /// Unknown law/role ids safely read as Still/Seeded.
    pub fn reconcile(
        &mut self,
        items: &[DynamicsItem],
        motion: bool,
        reduced_motion: bool,
        law_id: &str,
        roles: &BTreeMap<String, String>,
    ) {
        let law = PhysicsLaw::parse(law_id).unwrap_or(PhysicsLaw::Still);
        let motion = motion && !reduced_motion;
        if self.items == items && self.roles == *roles && self.law == law && self.motion == motion {
            return;
        }
        // A changed arrangement supplies new homes. Rebuild rather than keep
        // the board's previous slot coordinates after an explicit restore.
        if self.items != items {
            self.board = PhysicsBoard::new();
            self.dragging = None;
            self.held_position = None;
            self.move_origin = None;
        }
        self.items = items.to_vec();
        self.roles = roles.clone();
        self.law = law;
        self.motion = motion;
        self.board
            .set_stage(
                &PhysicsChoice {
                    law,
                    ..PhysicsChoice::default()
                }
                .into_spec(),
            )
            .expect("built-in Woodshed physics laws have valid stages");
        self.board.sync(
            items
                .iter()
                .filter(|(_, at, _)| at.0.is_finite() && at.1.is_finite())
                .map(|(id, at, group)| BoardItem {
                    id: id.wire_key(),
                    slot: world(*at),
                    site: group.clone(),
                })
                .collect(),
        );
        for (id, _, _) in items {
            let role = roles.get(&id.wire_key()).and_then(|id| Role::parse(id));
            self.board.set_item_role(&id.wire_key(), role);
        }
        if !motion {
            self.board.halt();
        }
    }

    pub fn positions(&self) -> BTreeMap<OverviewNodeId, (f32, f32)> {
        self.items
            .iter()
            .filter_map(|(id, _, _)| {
                let at = if self.dragging.as_ref() == Some(id) {
                    self.held_position
                        .unwrap_or(normalized(self.board.position(&id.wire_key())?))
                } else {
                    normalized(self.board.position(&id.wire_key())?)
                };
                (at.0.is_finite() && at.1.is_finite()).then(|| (id.clone(), at))
            })
            .collect()
    }

    /// The host calls this once per presented frame and requests another only
    /// while true. Paused/reduced-motion scenes never advance the simulation.
    pub fn tick(&mut self) -> bool {
        self.motion && self.board.tick()
    }

    pub fn actions(&self, id: &OverviewNodeId) -> Vec<AdvertisedAction> {
        self.board.advertised_actions(&id.wire_key())
    }

    pub fn role(&self, id: &OverviewNodeId) -> Option<Role> {
        self.board.role_of(&id.wire_key())
    }

    pub fn drag_start(&mut self, id: &OverviewNodeId) -> bool {
        if self.board.drag_start(&id.wire_key()) {
            self.held_position = self.positions().get(id).copied();
            self.move_origin = self.held_position;
            self.dragging = Some(id.clone());
            true
        } else {
            false
        }
    }

    pub fn drag_move(&mut self, at: (f32, f32)) -> bool {
        if !at.0.is_finite() || !at.1.is_finite() {
            return false;
        }
        let (x, y) = world(at);
        let moved = self.board.drag_move(x, y);
        if moved {
            self.held_position = Some(at);
        }
        moved
    }

    pub fn drag_end(&mut self) -> bool {
        if self.motion && self.dragging.is_some() {
            self.board.tick();
        }
        let ended = self.board.drag_end();
        self.finish_move();
        ended
    }

    pub fn begin_key_move(&mut self, id: &OverviewNodeId) -> bool {
        if self.board.begin_key_move(&id.wire_key()) {
            self.held_position = self.positions().get(id).copied();
            self.move_origin = self.held_position;
            self.dragging = Some(id.clone());
            true
        } else {
            false
        }
    }

    /// `normalized_delta` already inverts the host's viewport; camera zoom
    /// must be divided out exactly once by the caller.
    pub fn key_move_by(&mut self, normalized_delta: (f32, f32)) -> bool {
        if !normalized_delta.0.is_finite() || !normalized_delta.1.is_finite() {
            return false;
        }
        let moved = self.board.key_move_by(
            normalized_delta.0 * WORLD_SIZE,
            normalized_delta.1 * WORLD_SIZE,
        );
        if moved {
            if let Some(at) = self.held_position.as_mut() {
                at.0 += normalized_delta.0;
                at.1 += normalized_delta.1;
            }
        }
        moved
    }

    pub fn end_key_move(&mut self, drop: bool) -> bool {
        if self.motion && self.dragging.is_some() {
            self.board.tick();
        }
        let ended = self.board.end_key_move(drop);
        if !drop {
            self.held_position = self.move_origin;
        }
        self.finish_move();
        ended
    }

    fn finish_move(&mut self) {
        if !self.motion && self.dragging.is_some() {
            // Pin writes the next kinematic translation; reading the board
            // before a tick otherwise drops at the stale position. A paused
            // host must not advance unrelated dynamics to make input visible.
            // Spawn a replacement board at the exact viewer positions, then
            // reconcile original homes without stepping. Existing seeded
            // positions survive the second sync; anchors return immediately.
            let mut positions = self.positions();
            if let Some(id) = self.dragging.as_ref() {
                if self.board.role_of(&id.wire_key()) == Some(Role::Anchored) {
                    if let Some((_, at, _)) = self.items.iter().find(|(item, _, _)| item == id) {
                        positions.insert(id.clone(), *at);
                    }
                }
            }
            self.board = PhysicsBoard::new();
            self.board
                .set_stage(
                    &PhysicsChoice {
                        law: self.law,
                        ..PhysicsChoice::default()
                    }
                    .into_spec(),
                )
                .expect("built-in Woodshed physics laws have valid stages");
            self.board.sync(
                self.items
                    .iter()
                    .filter_map(|(id, _, group)| {
                        Some(BoardItem {
                            id: id.wire_key(),
                            slot: world(*positions.get(id)?),
                            site: group.clone(),
                        })
                    })
                    .collect(),
            );
            self.board.sync(
                self.items
                    .iter()
                    .filter(|(_, at, _)| at.0.is_finite() && at.1.is_finite())
                    .map(|(id, at, group)| BoardItem {
                        id: id.wire_key(),
                        slot: world(*at),
                        site: group.clone(),
                    })
                    .collect(),
            );
            for (id, _, _) in &self.items {
                self.board.set_item_role(
                    &id.wire_key(),
                    self.roles
                        .get(&id.wire_key())
                        .and_then(|role| Role::parse(role)),
                );
            }
            self.board.halt();
        }
        self.dragging = None;
        self.held_position = None;
        self.move_origin = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn node() -> OverviewNodeId {
        OverviewNodeId::View("set".into())
    }
    fn items() -> Vec<DynamicsItem> {
        vec![(node(), (0.2, 0.7), "view".into())]
    }

    fn assert_home(adapter: &OverviewDynamics) {
        let at = adapter.positions()[&node()];
        assert!(
            (at.0 - 0.2).abs() < 1e-6 && (at.1 - 0.7).abs() < 1e-6,
            "{at:?}"
        );
    }

    #[test]
    fn paused_move_is_visible_drops_exactly_and_leaves_other_nodes_still() {
        let other = OverviewNodeId::View("other".into());
        let inputs = vec![
            (node(), (0.2, 0.7), "view".into()),
            (other.clone(), (0.8, 0.2), "view".into()),
        ];
        let mut adapter = OverviewDynamics::default();
        adapter.reconcile(
            &inputs,
            true,
            true,
            PhysicsLaw::Flock.id(),
            &BTreeMap::new(),
        );
        let before = adapter.positions()[&other];
        assert!(adapter.drag_start(&node()));
        assert!(adapter.drag_move((0.4, 0.6)));
        assert_eq!(adapter.positions()[&node()], (0.4, 0.6));
        assert!(adapter.drag_end());
        let at = adapter.positions()[&node()];
        assert!((at.0 - 0.4).abs() < 1e-6 && (at.1 - 0.6).abs() < 1e-6);
        assert_eq!(adapter.positions()[&other], before);
        adapter.reconcile(
            &inputs,
            true,
            true,
            PhysicsLaw::Flock.id(),
            &BTreeMap::new(),
        );
        assert_eq!(adapter.positions()[&node()], at);
        assert!(adapter.begin_key_move(&node()));
        assert!(adapter.key_move_by((0.03, 0.0)));
        assert!(adapter.end_key_move(true));
        assert!((adapter.positions()[&node()].0 - at.0 - 0.03).abs() < 1e-6);
        assert_eq!(adapter.positions()[&other], before);
        let dropped = adapter.positions()[&node()];
        assert!(adapter.begin_key_move(&node()));
        assert!(adapter.key_move_by((0.1, 0.0)));
        assert!(adapter.end_key_move(false));
        assert_eq!(adapter.positions()[&node()], dropped);
    }

    #[test]
    fn reduced_motion_prevents_ticks_and_preserves_arrangement() {
        let mut adapter = OverviewDynamics::default();
        adapter.reconcile(
            &items(),
            true,
            true,
            PhysicsLaw::Flock.id(),
            &BTreeMap::new(),
        );
        for _ in 0..30 {
            assert!(!adapter.tick());
        }
        assert_home(&adapter);
    }

    #[test]
    fn pin_advertisement_refuses_pointer_and_keyboard_move() {
        let mut adapter = OverviewDynamics::default();
        adapter.reconcile(
            &items(),
            false,
            false,
            PhysicsLaw::Still.id(),
            &BTreeMap::from([(node().wire_key(), "pinned".into())]),
        );
        assert!(!adapter.drag_start(&node()));
        assert!(!adapter.begin_key_move(&node()));
        assert!(
            !adapter
                .actions(&node())
                .iter()
                .any(|a| a.intent.0 == pictograph::canvas::DRAG_INTENT)
        );
    }

    #[test]
    fn keyboard_cancel_returns_origin_and_paused_anchor_returns_home() {
        let mut adapter = OverviewDynamics::default();
        adapter.reconcile(
            &items(),
            false,
            false,
            PhysicsLaw::Still.id(),
            &BTreeMap::new(),
        );
        assert!(adapter.begin_key_move(&node()));
        adapter.key_move_by((0.1, 0.0));
        adapter.end_key_move(false);
        assert_home(&adapter);
        adapter.reconcile(
            &items(),
            false,
            false,
            PhysicsLaw::Still.id(),
            &BTreeMap::from([(node().wire_key(), "anchored".into())]),
        );
        assert!(adapter.drag_start(&node()));
        adapter.drag_move((0.6, 0.6));
        adapter.drag_end();
        assert_home(&adapter);
    }
}
