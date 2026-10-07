//! Retained Mere presentation, separate from owner facts and musical instructions.
use super::{UiState, overview_atmosphere::OverviewAtmosphere};
use cambium::GraphViewport;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use woodshed_core::session_overview::OverviewNodeId;

const VERSION: u32 = 1;
const MAX_ENTRIES: usize = 512;
const MAX_JSON_BYTES: usize = 512 * 1024;
const MAX_COORDINATE: f32 = 100_000.0;

/// Versioned view-owned state. Tuple entries keep structured identities valid in
/// JSON; identity is never flattened into a display label or a JSON map key.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OverviewPresentation {
    pub version: u32,
    pub pan: (f32, f32),
    pub zoom: f32,
    pub motion: bool,
    pub reduced_motion: bool,
    pub atmosphere: OverviewAtmosphere,
    pub positions: Vec<(OverviewNodeId, (f32, f32))>,
    pub focus: Option<OverviewNodeId>,
    pub roles: Vec<(OverviewNodeId, String)>,
    pub background: Vec<OverviewNodeId>,
}

impl Default for OverviewPresentation {
    fn default() -> Self {
        Self {
            version: VERSION,
            pan: (0.0, 0.0),
            zoom: 1.0,
            motion: false,
            reduced_motion: true,
            atmosphere: OverviewAtmosphere::default(),
            positions: Vec::new(),
            focus: None,
            roles: Vec::new(),
            background: Vec::new(),
        }
    }
}

fn coordinate(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(-MAX_COORDINATE, MAX_COORDINATE)
    } else {
        0.0
    }
}

fn point(value: (f32, f32)) -> (f32, f32) {
    (coordinate(value.0), coordinate(value.1))
}

impl OverviewPresentation {
    pub fn capture(ui: &UiState) -> Self {
        let current: BTreeSet<_> = super::overview::overview_snapshot(ui)
            .nodes
            .into_iter()
            .map(|node| node.id)
            .collect();
        let mut positions = ui.overview_positions.clone();
        // Only a reconciled runtime reports positions; its latest arrangement
        // overlays manual seeds without feeding each animation frame back in.
        positions.extend(ui.overview_dynamics.positions());
        positions.retain(|id, _| current.contains(id));
        Self {
            version: VERSION,
            pan: ui.overview_viewport.pan,
            zoom: ui.overview_viewport.zoom,
            motion: ui.overview_motion,
            reduced_motion: ui.overview_reduced_motion,
            atmosphere: ui.overview_atmosphere,
            positions: positions.into_iter().take(MAX_ENTRIES).collect(),
            focus: ui.overview_focus.clone(),
            roles: ui
                .overview_roles
                .iter()
                .take(MAX_ENTRIES)
                .map(|(id, role)| (id.clone(), role.clone()))
                .collect(),
            background: ui
                .overview_background
                .iter()
                .take(MAX_ENTRIES)
                .cloned()
                .collect(),
        }
        .sanitized()
        .expect("capture uses the current version")
    }

    /// Reject a future schema rather than interpreting its fields as version 1.
    pub fn sanitized(mut self) -> Option<Self> {
        if self.version != VERSION {
            return None;
        }
        self.pan = point(self.pan);
        self.zoom = if self.zoom.is_finite() {
            self.zoom.clamp(0.25, 8.0)
        } else {
            1.0
        };
        let positions: BTreeMap<_, _> = self
            .positions
            .into_iter()
            .take(MAX_ENTRIES)
            .map(|(id, pos)| (id, point(pos)))
            .collect();
        self.positions = positions.into_iter().collect();
        let background: BTreeSet<_> = self.background.into_iter().take(MAX_ENTRIES).collect();
        self.background = background.into_iter().collect();
        self.roles = self
            .roles
            .into_iter()
            .take(MAX_ENTRIES)
            .filter(|(_, role)| pictograph::canvas::Role::parse(role).is_some())
            .collect();
        Some(self)
    }

    pub fn is_unknown_version(json: &str) -> bool {
        json.len() <= MAX_JSON_BYTES
            && serde_json::from_str::<serde_json::Value>(json)
                .ok()
                .and_then(|value| value.get("version").and_then(serde_json::Value::as_u64))
                .is_some_and(|version| version != u64::from(VERSION))
    }

    pub fn from_json(json: Option<&str>) -> Option<Self> {
        let json = json?;
        if json.len() > MAX_JSON_BYTES {
            return None;
        }
        serde_json::from_str::<Self>(json).ok()?.sanitized()
    }

    pub fn to_json(&self) -> Option<String> {
        let value = self.clone().sanitized()?;
        let json = serde_json::to_string(&value).ok()?;
        (json.len() <= MAX_JSON_BYTES).then_some(json)
    }

    pub fn apply(self, ui: &mut UiState) {
        let value = self.sanitized().unwrap_or_default();
        ui.overview_viewport = GraphViewport {
            pan: value.pan,
            zoom: value.zoom,
        };
        ui.overview_motion = value.motion;
        ui.overview_reduced_motion = value.reduced_motion;
        ui.overview_atmosphere = value.atmosphere;
        ui.overview_ambient.reconcile(value.atmosphere);
        ui.overview_focus = value.focus;
        ui.overview_positions = value.positions.into_iter().collect();
        ui.overview_background = value.background.into_iter().collect();
        ui.overview_roles = value.roles.into_iter().collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use woodshed_core::{
        session_overview::{OverviewProcess, SessionArtifactId},
        settings::AppSettings,
    };

    #[test]
    fn atmosphere_recipe_roundtrip_restarts_seed_and_keeps_owner_facts() {
        use super::super::overview_atmosphere::AtmosphereKind;
        let mut ui = UiState::new();
        ui.overview_atmosphere = OverviewAtmosphere {
            kind: AtmosphereKind::Cells,
            seed: 93,
        };
        ui.overview_ambient.reconcile(ui.overview_atmosphere);
        let seeded = format!("{:?}", ui.overview_ambient.paint(640.0, 320.0));
        for _ in 0..120 {
            ui.overview_ambient.tick(true);
        }
        let owner = serde_json::to_value(&ui.set).unwrap();
        let session = ui.to_persisted();
        let mut reopened = UiState::new();
        reopened.overview_movement_controls = true;
        reopened.apply_persisted(&session, AppSettings::default());
        assert_eq!(reopened.overview_atmosphere, ui.overview_atmosphere);
        assert_eq!(
            format!("{:?}", reopened.overview_ambient.paint(640.0, 320.0)),
            seeded
        );
        assert!(!reopened.overview_movement_controls);
        assert_eq!(serde_json::to_value(&reopened.set).unwrap(), owner);
        let old = OverviewPresentation::from_json(Some(r#"{"version":1,"zoom":2.0}"#)).unwrap();
        assert_eq!(old.atmosphere, OverviewAtmosphere::default());
        assert_eq!(old.zoom, 2.0);
    }

    #[test]
    fn session_roundtrip_preserves_camera_typed_identity_and_roles() {
        let mut ui = UiState::new();
        let id = OverviewNodeId::Artifact(SessionArtifactId::Song);
        let background = OverviewNodeId::Process(OverviewProcess::Looper);
        ui.overview_viewport = GraphViewport {
            pan: (12.5, -36.0),
            zoom: 2.0,
        };
        ui.overview_positions.insert(id.clone(), (0.15, 0.85));
        ui.overview_focus = Some(id.clone());
        ui.overview_motion = true;
        ui.overview_reduced_motion = false;
        ui.overview_background.insert(background.clone());
        ui.overview_roles.insert(id.clone(), "pinned".into());
        let session = ui.to_persisted();
        let wire = serde_json::to_string(&session).unwrap();
        let decoded = serde_json::from_str(&wire).unwrap();
        let mut restored = UiState::new();
        restored.apply_persisted(&decoded, AppSettings::default());
        assert_eq!(restored.overview_viewport.pan, (12.5, -36.0));
        assert_eq!(restored.overview_viewport.zoom, 2.0);
        assert_eq!(restored.overview_focus, Some(id.clone()));
        assert!(restored.overview_motion);
        assert!(!restored.overview_reduced_motion);
        assert_eq!(restored.overview_positions.get(&id), Some(&(0.15, 0.85)));
        assert!(restored.overview_background.contains(&background));
        assert_eq!(
            restored.overview_roles.get(&id).map(String::as_str),
            Some("pinned")
        );
        assert_eq!(
            serde_json::to_value(&restored.set).unwrap(),
            serde_json::to_value(&ui.set).unwrap()
        );
    }

    #[test]
    fn capture_keeps_current_runtime_positions_without_changing_manual_seeds() {
        let mut ui = UiState::new();
        let id = OverviewNodeId::Artifact(SessionArtifactId::Song);
        let missing = OverviewNodeId::View("no-longer-mounted".into());
        ui.overview_positions.insert(id.clone(), (0.1, 0.2));
        ui.overview_positions.insert(missing.clone(), (0.5, 0.5));
        ui.overview_dynamics.reconcile(
            &[(id.clone(), (0.7, 0.8), "Song".into())],
            false,
            true,
            "still.default",
            &BTreeMap::new(),
        );
        let value = OverviewPresentation::capture(&ui);
        let pos = value
            .positions
            .iter()
            .find(|(node, _)| *node == id)
            .unwrap()
            .1;
        assert!((pos.0 - 0.7).abs() < 0.001);
        assert!((pos.1 - 0.8).abs() < 0.001);
        assert!(!value.positions.iter().any(|(node, _)| *node == missing));
        assert_eq!(ui.overview_positions.get(&id), Some(&(0.1, 0.2)));
    }

    #[test]
    fn invalid_and_future_payloads_only_reset_presentation() {
        for payload in ["{", r#"{"version":2,"zoom":2.0}"#, r#"{"zoom":null}"#] {
            let mut ui = UiState::new();
            ui.stage.set_root(7);
            let mut session = ui.to_persisted();
            session.overview_presentation_json = Some(payload.into());
            ui.overview_viewport.pan = (100.0, 200.0);
            ui.overview_background
                .insert(OverviewNodeId::Artifact(SessionArtifactId::Song));
            ui.apply_persisted(&session, AppSettings::default());
            assert_eq!(ui.stage.root_idx, 7);
            assert_eq!(ui.overview_viewport.pan, (0.0, 0.0));
            assert_eq!(ui.overview_viewport.zoom, 1.0);
            assert!(ui.overview_background.is_empty());
            if OverviewPresentation::is_unknown_version(payload) {
                assert_eq!(
                    ui.to_persisted().overview_presentation_json.as_deref(),
                    Some(payload)
                );
            }
        }
    }

    #[test]
    fn nonfinite_values_and_extremes_are_sanitized_before_encoding() {
        let id = OverviewNodeId::Artifact(SessionArtifactId::Song);
        let value = OverviewPresentation {
            pan: (f32::NAN, f32::INFINITY),
            zoom: f32::NEG_INFINITY,
            positions: vec![(id.clone(), (f32::NAN, f32::MAX))],
            ..Default::default()
        };
        let decoded = OverviewPresentation::from_json(value.to_json().as_deref()).unwrap();
        assert_eq!(decoded.pan, (0.0, 0.0));
        assert_eq!(decoded.zoom, 1.0);
        assert_eq!(decoded.positions, vec![(id, (0.0, MAX_COORDINATE))]);
        for (zoom, expected) in [(0.01, 0.25), (999.0, 8.0)] {
            let value = OverviewPresentation {
                zoom,
                ..Default::default()
            };
            assert_eq!(value.sanitized().unwrap().zoom, expected);
        }
    }

    #[test]
    fn arrangement_and_roles_are_bounded_and_legacy_sessions_default() {
        let mut value = OverviewPresentation::default();
        for index in 0..600 {
            let id = OverviewNodeId::View(index.to_string());
            value.positions.push((id.clone(), (1.0, 2.0)));
            value.background.push(id);
        }
        let value = value.sanitized().unwrap();
        assert_eq!(value.positions.len(), MAX_ENTRIES);
        assert_eq!(value.background.len(), MAX_ENTRIES);
        assert!(OverviewPresentation::from_json(None).is_none());
        assert_eq!(
            OverviewPresentation::from_json(Some("{}")).unwrap(),
            OverviewPresentation::default()
        );
        assert!(OverviewPresentation::from_json(Some(&" ".repeat(MAX_JSON_BYTES + 1))).is_none());
    }
}
