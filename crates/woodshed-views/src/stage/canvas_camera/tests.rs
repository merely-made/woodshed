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
        (
            (centre.0 - c.offset.0) / c.zoom,
            (centre.1 - c.offset.1) / c.zoom,
        )
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
            assert!(
                faces.offset_max < 1e-3,
                "{surface:?} after {command:?}: {faces:?}"
            );
        }
        assert_eq!(
            serde_json::to_value(&ui.set).unwrap(),
            set,
            "{surface:?} edited the Set"
        );
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
            if surface == CanvasSurface::Overview {
                !before
            } else {
                before
            }
        );
    }
}

#[test]
fn a_wheel_notch_toward_the_top_zooms_in_about_the_pointer() {
    for (surface, mut ui) in surfaces() {
        apply_canvas_command(&mut ui, surface, CanvasCommand::Fit);
        let leaf = SurfaceCamera::new(&mut ui, surface).leaf();
        let at = (leaf.size.0 * 0.3, leaf.size.1 * 0.6);
        let world = |ui: &mut UiState| {
            let view = SurfaceCamera::new(ui, surface).camera();
            (
                (at.0 - view.offset.0) / view.zoom,
                (at.1 - view.offset.1) / view.zoom,
            )
        };
        let (before_world, before_zoom) = (
            world(&mut ui),
            SurfaceCamera::new(&mut ui, surface).camera().zoom,
        );
        let event = WheelEvent::new((0.0, 40.0), at, leaf.size);
        apply_wheel(&mut ui, surface, &event);
        let after_zoom = SurfaceCamera::new(&mut ui, surface).camera().zoom;
        assert!(
            after_zoom > before_zoom || after_zoom == ZOOM_RANGE.1,
            "{surface:?}"
        );
        assert!(close(world(&mut ui), before_world), "{surface:?}");
        let faces = measure_surface_faces(&ui, surface).unwrap();
        assert_eq!(faces.missing, 0);
        assert!(faces.offset_max < 1e-3);
    }
}
