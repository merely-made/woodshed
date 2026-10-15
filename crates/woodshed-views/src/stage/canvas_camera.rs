//! Pictograph's lifted canvas controls over Woodshed's Cambium graph canvases.
//!
//! The Graphshell stack lift (Mere P2) moved the camera commands, gestures and
//! face measurement into `pictograph::canvas`, generic over a host camera.
//! Woodshed's Session Mere overview, Related graph and relationship editor
//! keep Cambium's [`GraphViewport`] as their camera; [`SurfaceCamera`] presents
//! that viewport as a pictograph [`CameraView`] so the same commands drive it.
//! Cameras are presentation only: no command here edits a Set or a Mere.

use cambium::{GraphCanvasSwatch, GraphViewport, clickable, el, text};
use sprigging::Size;
use pictograph::canvas::CameraView;
use pictograph::canvas::camera_host::{CameraHost, FaceGeometry, FitHost, ScreenRect};
use pictograph::canvas::controls::CanvasCommand;
use pictograph::canvas::face_alignment::FaceAlignment;
use pictograph::canvas::gestures::PAN_GLIDE;

use super::{UiChild, UiState};

/// The zoom range every Woodshed graph canvas has used.
pub const ZOOM_RANGE: (f32, f32) = (0.25, 8.0);

/// The Woodshed surfaces that host pictograph's canvas controls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanvasSurface {
    Overview,
    Related,
    Relationship,
}

/// The leaf box [`GraphViewport::project`] maps normalized points into.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LeafGeometry {
    pub size: (f32, f32),
    pub inset: f32,
}

impl LeafGeometry {
    pub fn of<Id, Kind>(swatch: &GraphCanvasSwatch<Id, Kind>) -> Self {
        Self {
            size: (swatch.width as f32, swatch.height as f32),
            inset: swatch.node_radius + swatch.edge_width,
        }
    }

    fn inner(self) -> (f32, f32) {
        (
            (self.size.0 - 2.0 * self.inset).max(1.0),
            (self.size.1 - 2.0 * self.inset).max(1.0),
        )
    }

    pub fn viewport_px(self) -> (u32, u32) {
        (self.size.0.max(1.0) as u32, self.size.1.max(1.0) as u32)
    }
}

/// A [`GraphViewport`] read as pictograph's top-down camera. World space is a
/// normalized point scaled by the inner box, so leaf-local px are
/// `world * zoom + offset` with `offset = inset + (0.5 - 0.5 zoom + pan) * inner`.
pub fn camera_of(viewport: GraphViewport, leaf: LeafGeometry) -> CameraView {
    let zoom = viewport.zoom.max(0.01);
    let inner = leaf.inner();
    CameraView {
        offset: (
            leaf.inset + (0.5 - 0.5 * zoom + viewport.pan.0) * inner.0,
            leaf.inset + (0.5 - 0.5 * zoom + viewport.pan.1) * inner.1,
        ),
        zoom,
    }
}

/// The inverse of [`camera_of`], clamping zoom to [`ZOOM_RANGE`].
pub fn viewport_of(camera: CameraView, leaf: LeafGeometry) -> GraphViewport {
    let zoom = if camera.zoom.is_finite() {
        camera.zoom.clamp(ZOOM_RANGE.0, ZOOM_RANGE.1)
    } else {
        1.0
    };
    let inner = leaf.inner();
    GraphViewport {
        pan: (
            (camera.offset.0 - leaf.inset) / inner.0 - 0.5 + 0.5 * zoom,
            (camera.offset.1 - leaf.inset) / inner.1 - 0.5 + 0.5 * zoom,
        ),
        zoom,
    }
}

/// Frame normalized positions with a small margin, about their centre.
pub fn fit_viewport(positions: impl IntoIterator<Item = (f32, f32)>) -> GraphViewport {
    let mut min = (f32::INFINITY, f32::INFINITY);
    let mut max = (f32::NEG_INFINITY, f32::NEG_INFINITY);
    for (x, y) in positions {
        min = (min.0.min(x), min.1.min(y));
        max = (max.0.max(x), max.1.max(y));
    }
    if !min.0.is_finite() {
        return GraphViewport::default();
    }
    let zoom =
        (0.82 / (max.0 - min.0).max(max.1 - min.1).max(0.1)).clamp(ZOOM_RANGE.0, ZOOM_RANGE.1);
    GraphViewport {
        pan: (
            (0.5 - (min.0 + max.0) * 0.5) * zoom,
            (0.5 - (min.1 + max.1) * 0.5) * zoom,
        ),
        zoom,
    }
}

/// One surface's camera, borrowed from the UI state for a single command.
pub struct SurfaceCamera<'a> {
    ui: &'a mut UiState,
    surface: CanvasSurface,
    leaf: LeafGeometry,
}

impl<'a> SurfaceCamera<'a> {
    pub fn new(ui: &'a mut UiState, surface: CanvasSurface) -> Self {
        let leaf = surface_leaf(ui, surface);
        Self { ui, surface, leaf }
    }

    pub fn leaf(&self) -> LeafGeometry {
        self.leaf
    }

    fn viewport(&self) -> GraphViewport {
        match self.surface {
            CanvasSurface::Overview => self.ui.overview_viewport,
            CanvasSurface::Related => self.ui.related_viewport,
            CanvasSurface::Relationship => self.ui.relationship_viewport,
        }
    }

    fn viewport_mut(&mut self) -> &mut GraphViewport {
        match self.surface {
            CanvasSurface::Overview => &mut self.ui.overview_viewport,
            CanvasSurface::Related => &mut self.ui.related_viewport,
            CanvasSurface::Relationship => &mut self.ui.relationship_viewport,
        }
    }
}

fn surface_leaf(ui: &UiState, surface: CanvasSurface) -> LeafGeometry {
    match surface {
        CanvasSurface::Overview => LeafGeometry::of(&super::overview::overview_swatch(ui)),
        CanvasSurface::Related => LeafGeometry::of(&super::related_swatch(ui)),
        CanvasSurface::Relationship => super::relationship::relationship_swatch(ui)
            .map(|swatch| LeafGeometry::of(&swatch))
            .unwrap_or(LeafGeometry {
                size: (1.0, 1.0),
                inset: 0.0,
            }),
    }
}

fn surface_positions(ui: &UiState, surface: CanvasSurface) -> Vec<(f32, f32)> {
    fn positions<Id, Kind>(nodes: &[cambium::GraphCanvasNode<Id, Kind>]) -> Vec<(f32, f32)> {
        nodes.iter().map(|node| node.position).collect()
    }
    match surface {
        CanvasSurface::Overview => positions(&super::overview::overview_swatch(ui).graph.nodes),
        CanvasSurface::Related => positions(&super::related_swatch(ui).graph.nodes),
        CanvasSurface::Relationship => super::relationship::relationship_swatch(ui)
            .map(|swatch| positions(&swatch.graph.nodes))
            .unwrap_or_default(),
    }
}

impl CameraHost for SurfaceCamera<'_> {
    fn camera(&self) -> CameraView {
        camera_of(self.viewport(), self.leaf)
    }

    fn set_camera(&mut self, view: CameraView) {
        let leaf = self.leaf;
        *self.viewport_mut() = viewport_of(view, leaf);
    }

    fn zoom_range(&self) -> (f32, f32) {
        ZOOM_RANGE
    }
}

impl FitHost for SurfaceCamera<'_> {
    fn fit_to_content(&mut self) {
        let fitted = fit_viewport(surface_positions(self.ui, self.surface));
        *self.viewport_mut() = fitted;
    }

    fn fit_visible(&mut self) {
        self.fit_to_content();
    }

    /// Centre the overview's focused node at the current zoom. Related and
    /// the relationship editor have no camera selection to frame.
    fn fit_selection(&mut self) {
        if self.surface != CanvasSurface::Overview {
            return;
        }
        let Some(focus) = self.ui.overview_focus.clone() else {
            return;
        };
        let swatch = super::overview::overview_swatch(self.ui);
        let Some(node) = swatch.graph.nodes.iter().find(|node| node.id == focus) else {
            return;
        };
        let zoom = self.ui.overview_viewport.zoom;
        self.ui.overview_viewport.pan = (
            (0.5 - node.position.0) * zoom,
            (0.5 - node.position.1) * zoom,
        );
    }

    fn restore_arrangement(&mut self) {
        match self.surface {
            CanvasSurface::Overview => self.ui.restore_overview_arrangement(),
            _ => *self.viewport_mut() = GraphViewport::default(),
        }
    }

    fn physics_paused(&self) -> bool {
        self.surface != CanvasSurface::Overview || !self.ui.overview_motion
    }

    fn set_physics_paused(&mut self, paused: bool) {
        if self.surface == CanvasSurface::Overview {
            self.ui.overview_motion = !paused;
            self.ui.reconcile_overview_dynamics();
        }
    }
}

/// Run one pictograph canvas command against a Woodshed surface's camera.
pub fn apply_canvas_command(ui: &mut UiState, surface: CanvasSurface, command: CanvasCommand) {
    let mut camera = SurfaceCamera::new(ui, surface);
    let viewport = camera.leaf().viewport_px();
    command.apply(&mut camera, viewport);
}

/// Zoom by `factor` about the canvas centre, as the surfaces' zoom buttons do.
pub fn zoom_by(ui: &UiState, surface: CanvasSurface, factor: f32) -> CanvasCommand {
    let zoom = match surface {
        CanvasSurface::Overview => ui.overview_viewport.zoom,
        CanvasSurface::Related => ui.related_viewport.zoom,
        CanvasSurface::Relationship => ui.relationship_viewport.zoom,
    };
    CanvasCommand::SetZoom {
        zoom: zoom * factor,
    }
}

/// A keyboard-style pan that travels `fraction` of the canvas's inner box
/// once its glide completes.
pub fn pan_by_fraction(ui: &UiState, surface: CanvasSurface, fraction: (f32, f32)) -> CanvasCommand {
    let inner = surface_leaf(ui, surface).inner();
    CanvasCommand::Pan {
        dx: fraction.0 * inner.0 / PAN_GLIDE,
        dy: fraction.1 * inner.1 / PAN_GLIDE,
    }
}

/// The camera buttons the Related graph and relationship editor share: the
/// same commands the overview's camera controls send.
pub fn camera_controls(surface: CanvasSurface, prefix: &'static str) -> Vec<UiChild> {
    [
        ("Fit", "fit"),
        ("Restore view", "restore"),
        ("Zoom in", "zoom-in"),
        ("Zoom out", "zoom-out"),
        ("Pan left", "pan-left"),
        ("Pan right", "pan-right"),
        ("Pan up", "pan-up"),
        ("Pan down", "pan-down"),
    ]
    .into_iter()
    .map(|(label, verb)| {
        Box::new(clickable(
            el("button", text(label)).attr("class", format!("t-btn {prefix}-{verb}")),
            move |ui: &mut UiState, _| {
                let command = match verb {
                    "fit" => CanvasCommand::Fit,
                    "restore" => CanvasCommand::RestoreArrangement,
                    "zoom-in" => zoom_by(ui, surface, 1.25),
                    "zoom-out" => zoom_by(ui, surface, 1.0 / 1.25),
                    "pan-left" => pan_by_fraction(ui, surface, (0.1, 0.0)),
                    "pan-right" => pan_by_fraction(ui, surface, (-0.1, 0.0)),
                    "pan-up" => pan_by_fraction(ui, surface, (0.0, 0.1)),
                    _ => pan_by_fraction(ui, surface, (0.0, -0.1)),
                };
                apply_canvas_command(ui, surface, command);
            },
        )) as UiChild
    })
    .collect()
}

/// A swatch's view-layer hit targets (faces) against the nodes its painted
/// leaf draws (bodies). The two come from Cambium's view projection and
/// Sprigging's paint projection respectively.
pub struct SwatchFaces {
    zoom: f32,
    faces: Vec<(ScreenRect, Option<ScreenRect>)>,
}

impl SwatchFaces {
    pub fn of<Id: Clone + PartialEq, Kind>(swatch: &GraphCanvasSwatch<Id, Kind>) -> Self {
        let leaf = swatch.paint_leaf(|_| Default::default());
        let size = Size {
            width: swatch.width as f32,
            height: swatch.height as f32,
        };
        let (half_hit, radius) = (swatch.hit_size.max(1.0) * 0.5, swatch.node_radius);
        let faces = swatch
            .projected_positions()
            .into_iter()
            .enumerate()
            .map(|(index, (_, at))| {
                let face = (
                    at.0 - half_hit,
                    at.1 - half_hit,
                    at.0 + half_hit,
                    at.1 + half_hit,
                );
                let body = leaf
                    .node_local_position(index, size)
                    .map(|(x, y)| (x - radius, y - radius, x + radius, y + radius));
                (face, body)
            })
            .collect();
        Self {
            zoom: swatch.viewport.zoom,
            faces,
        }
    }

    pub fn measure(&self, viewport: (u32, u32)) -> FaceAlignment {
        FaceAlignment::measure(self, viewport)
    }
}

impl FaceGeometry for SwatchFaces {
    fn zoom(&self) -> f32 {
        self.zoom
    }

    fn faces(&self) -> Vec<(ScreenRect, Option<ScreenRect>)> {
        self.faces.clone()
    }
}

/// Measure one surface's current face alignment.
pub fn measure_surface_faces(ui: &UiState, surface: CanvasSurface) -> Option<FaceAlignment> {
    let measure = |faces: SwatchFaces, leaf: LeafGeometry| faces.measure(leaf.viewport_px());
    match surface {
        CanvasSurface::Overview => {
            let swatch = super::overview::overview_swatch(ui);
            Some(measure(SwatchFaces::of(&swatch), LeafGeometry::of(&swatch)))
        },
        CanvasSurface::Related => {
            let swatch = super::related_swatch(ui);
            Some(measure(SwatchFaces::of(&swatch), LeafGeometry::of(&swatch)))
        },
        CanvasSurface::Relationship => super::relationship::relationship_swatch(ui)
            .map(|swatch| measure(SwatchFaces::of(&swatch), LeafGeometry::of(&swatch))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEAF: LeafGeometry = LeafGeometry {
        size: (640.0, 360.0),
        inset: 6.0,
    };

    fn project(viewport: GraphViewport, point: (f32, f32)) -> (f32, f32) {
        viewport.project(
            point,
            Size {
                width: LEAF.size.0,
                height: LEAF.size.1,
            },
            LEAF.inset,
        )
    }

    fn close(a: (f32, f32), b: (f32, f32)) -> bool {
        (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3
    }

    /// A camera that owns a viewport, for exercising the conversion alone.
    struct Owned(GraphViewport);

    impl CameraHost for Owned {
        fn camera(&self) -> CameraView {
            camera_of(self.0, LEAF)
        }
        fn set_camera(&mut self, view: CameraView) {
            self.0 = viewport_of(view, LEAF);
        }
        fn zoom_range(&self) -> (f32, f32) {
            ZOOM_RANGE
        }
    }

    impl FitHost for Owned {}

    #[test]
    fn the_camera_reads_back_the_viewport_projection() {
        let viewport = GraphViewport {
            pan: (0.12, -0.3),
            zoom: 1.7,
        };
        let camera = camera_of(viewport, LEAF);
        let inner = LEAF.inner();
        for point in [(0.0, 0.0), (0.5, 0.5), (1.0, 0.25), (0.8, 1.0)] {
            let world = (point.0 * inner.0, point.1 * inner.1);
            let screen = (
                world.0 * camera.zoom + camera.offset.0,
                world.1 * camera.zoom + camera.offset.1,
            );
            assert!(close(screen, project(viewport, point)));
        }
        let back = viewport_of(camera, LEAF);
        assert!(close(back.pan, viewport.pan) && (back.zoom - viewport.zoom).abs() < 1e-6);
    }

    #[test]
    fn set_zoom_keeps_the_centre_and_clamps_to_the_range() {
        let mut host = Owned(GraphViewport {
            pan: (0.2, 0.1),
            zoom: 1.0,
        });
        let centre = (LEAF.size.0 * 0.5, LEAF.size.1 * 0.5);
        let world_at_centre = |host: &Owned| {
            let c = host.camera();
            ((centre.0 - c.offset.0) / c.zoom, (centre.1 - c.offset.1) / c.zoom)
        };
        let before = world_at_centre(&host);
        CanvasCommand::SetZoom { zoom: 2.5 }.apply(&mut host, LEAF.viewport_px());
        assert_eq!(host.0.zoom, 2.5);
        assert!(close(world_at_centre(&host), before));
        CanvasCommand::SetZoom { zoom: 100.0 }.apply(&mut host, LEAF.viewport_px());
        assert_eq!(host.0.zoom, ZOOM_RANGE.1);
    }

    #[test]
    fn a_pan_command_travels_its_glide_distance_in_pixels() {
        let mut host = Owned(GraphViewport::default());
        let before = project(host.0, (0.5, 0.5));
        CanvasCommand::Pan { dx: 12.0, dy: -6.0 }.apply(&mut host, LEAF.viewport_px());
        let after = project(host.0, (0.5, 0.5));
        assert!(close(
            (after.0 - before.0, after.1 - before.1),
            (12.0 * PAN_GLIDE, -6.0 * PAN_GLIDE)
        ));
    }

    #[test]
    fn verbs_a_surface_lacks_leave_the_camera_alone() {
        let viewport = GraphViewport {
            pan: (0.1, 0.1),
            zoom: 1.3,
        };
        let mut host = Owned(viewport);
        for command in [
            CanvasCommand::FitSelection,
            CanvasCommand::TogglePhysics,
            CanvasCommand::SetPhysicsPaused(true),
        ] {
            command.apply(&mut host, LEAF.viewport_px());
            assert_eq!(host.0, viewport);
        }
    }

    #[test]
    fn fitting_centres_and_frames_the_positions() {
        let fitted = fit_viewport([(0.2, 0.3), (0.6, 0.5)]);
        let centre = project(fitted, (0.4, 0.4));
        assert!(close(
            centre,
            (
                LEAF.inset + 0.5 * LEAF.inner().0,
                LEAF.inset + 0.5 * LEAF.inner().1
            )
        ));
        assert!((fitted.zoom - 0.82 / 0.4).abs() < 1e-4);
        assert_eq!(fit_viewport([]), GraphViewport::default());
    }

    fn relationship_fixture() -> UiState {
        use woodshed_core::harmony::KeyedCatalogRef;
        use woodshedding::pitch::PitchClass;
        let mut ui = UiState::new();
        for (formula, tonic) in [("chord:Major", 0), ("chord:Major", 0), ("chord:Minor", 9)] {
            ui.set.push(
                KeyedCatalogRef {
                    formula_id: formula.into(),
                    root: PitchClass::new(tonic),
                }
                .to_card()
                .unwrap(),
            );
        }
        ui.open_relationship_recipe();
        ui.bind_relationship_recipe(false).unwrap();
        ui
    }

    fn surfaces() -> Vec<(CanvasSurface, UiState)> {
        let mut overview = UiState::new();
        overview.stage_current(None);
        let mut related = UiState::new();
        related.stage.set_lens(woodshed_core::Lens::Chords);
        vec![
            (CanvasSurface::Overview, overview),
            (CanvasSurface::Related, related),
            (CanvasSurface::Relationship, relationship_fixture()),
        ]
    }

    #[test]
    fn every_surface_keeps_hit_targets_on_painted_nodes_through_commands() {
        for (surface, mut ui) in surfaces() {
            let set = serde_json::to_value(&ui.set).unwrap();
            apply_canvas_command(&mut ui, surface, CanvasCommand::Fit);
            let fitted = measure_surface_faces(&ui, surface).unwrap();
            assert!(fitted.nodes > 0, "{surface:?} measured nothing after Fit");
            for command in [
                CanvasCommand::SetZoom { zoom: 0.5 },
                CanvasCommand::SetZoom { zoom: 1.5 },
                CanvasCommand::Pan { dx: 9.0, dy: -4.0 },
                CanvasCommand::Zoom { delta: 40.0 },
                CanvasCommand::FitSelection,
                CanvasCommand::Fit,
            ] {
                apply_canvas_command(&mut ui, surface, command);
                let faces = measure_surface_faces(&ui, surface).unwrap();
                assert_eq!(faces.missing, 0, "{surface:?} after {command:?}");
                assert!(faces.offset_max < 1e-3, "{surface:?} after {command:?}: {faces:?}");
            }
            assert_eq!(serde_json::to_value(&ui.set).unwrap(), set, "{surface:?} edited the Set");
            apply_canvas_command(&mut ui, surface, CanvasCommand::RestoreArrangement);
            let restored = match surface {
                CanvasSurface::Overview => ui.overview_viewport,
                CanvasSurface::Related => ui.related_viewport,
                CanvasSurface::Relationship => ui.relationship_viewport,
            };
            assert_eq!(restored, GraphViewport::default());
        }
    }

    #[test]
    fn only_the_overview_has_physics_to_pause() {
        for (surface, mut ui) in surfaces() {
            let before = ui.overview_motion;
            apply_canvas_command(&mut ui, surface, CanvasCommand::TogglePhysics);
            assert_eq!(
                ui.overview_motion,
                if surface == CanvasSurface::Overview { !before } else { before }
            );
        }
    }
}
