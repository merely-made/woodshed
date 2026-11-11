//! Pictograph's lifted canvas controls over Woodshed's Cambium graph canvases.
//!
//! The Graphshell stack lift (Mere P2) moved the camera commands, gestures and
//! face measurement into `cartography::canvas`, generic over a host camera.
//! Woodshed's session Overview, Related graph and relationship editor
//! keep Cambium's [`GraphViewport`] as their camera; [`SurfaceCamera`] presents
//! that viewport as a pictograph [`CameraView`] so the same commands drive it.
//! Cameras are presentation only: no command here edits a Set or a mere.

use cambium::{GraphCanvasSwatch, GraphViewport, WheelEvent, clickable, el, text};
use cartography::canvas::CameraView;
use cartography::canvas::camera_host::{CameraHost, FaceGeometry, FitHost, ScreenRect};
use cartography::canvas::controls::CanvasCommand;
use cartography::canvas::face_alignment::FaceAlignment;
use cartography::canvas::gestures::{PAN_GLIDE, WheelMode, WheelUse, wheel_use, wheel_zoom};
use sprigging::Size;

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
pub fn pan_by_fraction(
    ui: &UiState,
    surface: CanvasSurface,
    fraction: (f32, f32),
) -> CanvasCommand {
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

/// One wheel notch over a surface, under pictograph's wheel policy: plain
/// wheel zooms at the pointer, Ctrl/Meta always zoom (trackpad pinch), and a
/// notch the policy leaves to the page keeps the host's scroll default.
pub fn apply_wheel(ui: &mut UiState, surface: CanvasSurface, event: &WheelEvent) {
    match wheel_use(
        WheelMode::default(),
        event.modifiers.ctrl,
        event.modifiers.meta,
    ) {
        WheelUse::Page => {},
        WheelUse::Zoom => {
            if event.delta.1 == 0.0 {
                return;
            }
            event.prevent_default();
            // Cambium's positive y scrolls toward the top; pictograph takes
            // the DOM sign, positive down.
            wheel_zoom(
                &mut SurfaceCamera::new(ui, surface),
                event.local,
                -event.delta.1,
            );
        },
        WheelUse::Pan => {
            event.prevent_default();
            let mut camera = SurfaceCamera::new(ui, surface);
            let view = camera.camera();
            camera.set_camera(CameraView {
                offset: (view.offset.0 + event.delta.0, view.offset.1 + event.delta.1),
                ..view
            });
        },
    }
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
#[path = "canvas_camera/tests.rs"]
mod tests;
