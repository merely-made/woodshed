//! View-owned atmosphere beneath the Mere graph. These simulations never own
//! graph nodes, relationships, hit targets, or musical state.
use pictograph::canvas::{AmbientSim, GameOfLife, NBody};
use serde::{Deserialize, Serialize};
use sprigging::PaintCmd;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AtmosphereKind {
    #[default]
    None,
    Orbits,
    Cells,
}

impl AtmosphereKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Orbits => "orbits",
            Self::Cells => "cells",
        }
    }
}

/// Only the authored recipe is retained. Reopening starts the same seeded
/// pattern; transient simulation history does not become session truth.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct OverviewAtmosphere {
    pub kind: AtmosphereKind,
    pub seed: u32,
}

impl Default for OverviewAtmosphere {
    fn default() -> Self {
        Self {
            kind: AtmosphereKind::None,
            seed: 1,
        }
    }
}

#[derive(Default)]
pub struct OverviewAmbient {
    descriptor: Option<OverviewAtmosphere>,
    sim: Option<Box<dyn AmbientSim>>,
    revision: u64,
}

impl OverviewAmbient {
    /// Rebuild only when the authored recipe changes, never on ordinary paint.
    pub fn reconcile(&mut self, descriptor: OverviewAtmosphere) -> bool {
        if self.descriptor == Some(descriptor) {
            return false;
        }
        self.descriptor = Some(descriptor);
        self.sim = match descriptor.kind {
            AtmosphereKind::None => None,
            AtmosphereKind::Orbits => Some(Box::new(NBody::seeded(64, descriptor.seed))),
            AtmosphereKind::Cells => Some(Box::new(GameOfLife::seeded(32, 24, descriptor.seed))),
        };
        self.revision = self.revision.wrapping_add(1);
        true
    }

    /// The host supplies visibility, motion, and reduced-motion policy together.
    /// A disabled frame preserves both the pattern and its revision exactly.
    pub fn tick(&mut self, enabled: bool) -> bool {
        if !enabled {
            return false;
        }
        let Some(sim) = self.sim.as_mut() else {
            return false;
        };
        sim.advance(1.0 / 60.0);
        self.revision = self.revision.wrapping_add(1);
        true
    }

    pub fn paint(&self, width: f32, height: f32) -> Vec<PaintCmd> {
        if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
            return Vec::new();
        }
        let Some(sim) = self.sim.as_ref() else {
            return Vec::new();
        };
        let mut tincture = sim.default_tincture();
        tincture.a *= 0.65;
        sim.paint(width, height, tincture)
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn painted(runtime: &OverviewAmbient) -> String {
        format!("{:?}", runtime.paint(640.0, 320.0))
    }

    #[test]
    fn disabled_motion_preserves_seeded_paint_and_revision() {
        for kind in [AtmosphereKind::Orbits, AtmosphereKind::Cells] {
            let descriptor = OverviewAtmosphere { kind, seed: 37 };
            let mut runtime = OverviewAmbient::default();
            assert!(runtime.reconcile(descriptor));
            let before = painted(&runtime);
            let revision = runtime.revision();
            for _ in 0..120 {
                assert!(!runtime.tick(false));
            }
            assert_eq!(runtime.revision(), revision);
            assert_eq!(painted(&runtime), before);
            assert!(!runtime.reconcile(descriptor));
            for _ in 0..120 {
                assert!(runtime.tick(true));
            }
            assert_ne!(painted(&runtime), before);
        }
    }

    #[test]
    fn reopening_reconstructs_pattern_without_solver_history() {
        for kind in [AtmosphereKind::Orbits, AtmosphereKind::Cells] {
            let descriptor = OverviewAtmosphere { kind, seed: 23 };
            let mut first = OverviewAmbient::default();
            first.reconcile(descriptor);
            let seeded = painted(&first);
            first.tick(true);
            let mut reopened = OverviewAmbient::default();
            reopened.reconcile(descriptor);
            assert_eq!(painted(&reopened), seeded);
            first.reconcile(OverviewAtmosphere {
                seed: 25,
                ..descriptor
            });
            assert_ne!(painted(&first), seeded);
            assert!(first.reconcile(OverviewAtmosphere::default()));
            assert!(first.paint(640.0, 320.0).is_empty());
            assert!(!first.tick(true));
        }
    }

    #[test]
    fn invalid_viewport_is_not_painted() {
        let mut runtime = OverviewAmbient::default();
        runtime.reconcile(OverviewAtmosphere {
            kind: AtmosphereKind::Orbits,
            seed: 1,
        });
        for (width, height) in [(f32::NAN, 320.0), (640.0, f32::INFINITY), (0.0, 320.0)] {
            assert!(runtime.paint(width, height).is_empty());
        }
    }
}
