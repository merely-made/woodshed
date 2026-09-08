//! The custom-paint leaves: Related and Set graph swatches, and the two
//! fretboards.
//!
//! Each is rebuilt only when the model behind it moves, keyed by a signature on
//! [`Shared`]. The host owns the registry and renders whatever is in it at the
//! boxes layout gives them; what belongs to woodshed is which leaf, from which
//! model, in which palette.

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

use sprigging::{Leaf, LeafRegistry, PaintCx, Size, SizeHint};
use woodshed_views::fretboard_leaf::{
    Dot, FRETBOARD_LEAF_KEY, FretboardLeaf, MarkerStyle, Orientation, REHEARSAL_FRETBOARD_LEAF_KEY,
};
use woodshed_views::stage::{NEIGHBORHOOD_LEAF_KEY, SET_GRAPH_LEAF_KEY, UiState};

use crate::shared::Shared;

struct StageGuideSegment {
    a: (f32, f32),
    b: (f32, f32),
    active: bool,
}

struct StageGuidesLeaf {
    graph: sprigging::GraphCanvas,
    segments: Vec<StageGuideSegment>,
    viewport: cambium::GraphViewport,
    inset: f32,
}

impl Leaf for StageGuidesLeaf {
    fn accessibility(&mut self, node: &mut accesskit::Node) {
        self.graph.accessibility(node);
    }

    fn measure(&mut self, known: SizeHint, available: SizeHint) -> Size {
        self.graph.measure(known, available)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let size = cx.size();
        cx.push_clip_rect(0.0, 0.0, size.width, size.height);
        let color = sprigging::ColorF {
            r: 0.47,
            g: 0.53,
            b: 0.70,
            a: 0.28,
        };
        let active_color = sprigging::ColorF {
            r: 0.93,
            g: 0.70,
            b: 0.28,
            a: 0.72,
        };
        for segment in &self.segments {
            let a = self.viewport.project(segment.a, size, self.inset);
            let b = self.viewport.project(segment.b, size, self.inset);
            cx.stroke_path(
                sprigging::Path::polyline(&[a, b]),
                sprigging::round_stroke(
                    if segment.active { active_color } else { color },
                    if segment.active { 2.0 } else { 1.0 },
                ),
            );
        }
        cx.pop_clip();
        self.graph.paint(cx);
    }

    fn event(&mut self, event: &sprigging::LeafEvent) -> Option<sprigging::LeafAction> {
        self.graph.event(event)
    }

    fn paint_dirty(&self) -> bool {
        self.graph.paint_dirty()
    }

    fn layout_dirty(&self) -> bool {
        self.graph.layout_dirty()
    }
}

/// The product palette for a Related node kind. Lives host-side so Cambium's
/// graph component stays palette-neutral; the same mapping drove the old glyph.
fn related_kind_color(kind: &str) -> sprigging::ColorF {
    let quiet = kind.starts_with("context:");
    let base = kind
        .strip_prefix("staged:")
        .or_else(|| kind.strip_prefix("context:"))
        .unwrap_or(kind);
    let [r, g, b] = match base {
        "Scale" => [0.30, 0.67, 0.76],
        "Chord" => [0.91, 0.38, 0.25],
        "Arpeggio" => [0.70, 0.46, 0.86],
        "Progression" => [0.91, 0.68, 0.28],
        "Exercise" => [0.40, 0.72, 0.42],
        _ => [0.72, 0.74, 0.78],
    };
    let scale = if quiet { 0.62 } else { 1.0 };
    sprigging::ColorF {
        r: r * scale,
        g: g * scale,
        b: b * scale,
        a: if quiet { 0.58 } else { 1.0 },
    }
}

fn hasher() -> std::collections::hash_map::DefaultHasher {
    std::collections::hash_map::DefaultHasher::new()
}

/// Refresh every leaf from the current state. Called from the frame hook,
/// before the host lays out and paints.
pub fn sync_all(shared: &mut Shared, ui: &UiState, leaves: &mut LeafRegistry<u64>) {
    sync_related_swatch(shared, ui, leaves);
    sync_set_graph_swatch(shared, ui, leaves);
    sync_fretboard(shared, ui, leaves);
    sync_fretboard_active(ui, leaves);
    sync_rehearsal_fretboard(shared, ui, leaves);
}

/// Refresh only the Set graph leaf during a view-local node drag.
///
/// The pointer dispatch already rebuilt Woodshed's retained tree. Related and
/// both fretboards cannot change from this gesture, so reconstructing their
/// projections before every presented move only adds latency.
pub fn sync_set_graph(shared: &mut Shared, ui: &UiState, leaves: &mut LeafRegistry<u64>) {
    sync_set_graph_swatch(shared, ui, leaves);
}

/// Paint the Related graph swatch's leaf from the shared swatch model (built by
/// the view layer), rebuilding only when the paint changes (node
/// positions/kinds, hovered emphasis, or size). The palette lives here so
/// product colours stay app-side; the geometry and interactivity are the
/// component's.
fn sync_related_swatch(shared: &mut Shared, ui: &UiState, leaves: &mut LeafRegistry<u64>) {
    let swatch = woodshed_views::stage::related_swatch(ui);
    let mut h = hasher();
    swatch.width.hash(&mut h);
    swatch.height.hash(&mut h);
    for node in &swatch.graph.nodes {
        node.id.hash(&mut h);
        node.position.0.to_bits().hash(&mut h);
        node.position.1.to_bits().hash(&mut h);
        node.kind.hash(&mut h);
    }
    for relation in &swatch.relations {
        relation.id.hash(&mut h);
        relation.visible.hash(&mut h);
        relation.emphasized.hash(&mut h);
        for point in &relation.route {
            point.0.to_bits().hash(&mut h);
            point.1.to_bits().hash(&mut h);
        }
    }
    swatch.selected.hash(&mut h);
    let hovered_idx = swatch
        .hovered
        .as_ref()
        .and_then(|hovered| swatch.graph.nodes.iter().position(|n| &n.id == hovered));
    hovered_idx.hash(&mut h);
    let sig = h.finish();
    if sig == shared.neighborhood_sig {
        return;
    }
    shared.neighborhood_sig = sig;
    let leaf = swatch.paint_leaf(|kind: &&str| related_kind_color(kind));
    leaves.insert(NEIGHBORHOOD_LEAF_KEY, Box::new(leaf));
}

fn tonnetz_segments(ui: &UiState) -> Vec<StageGuideSegment> {
    if ui.app_settings.stage.set_graph_reading
        == woodshed_core::settings::StageGraphReading::PitchMotion
    {
        let Some(anchor) = ui.pitch_motion_anchor.as_ref() else {
            return Vec::new();
        };
        let snapshot = woodshed_views::stage::set_graph_snapshot(ui);
        let bounds = snapshot.snapshot.tables.bounds;
        let normalize = |x: f32, y: f32| {
            (
                (x - bounds.origin.x) / bounds.size.w,
                (y - bounds.origin.y) / bounds.size.h,
            )
        };
        let center = woodshed_core::pitch_motion_reading::CENTER;
        let mut segments = Vec::new();
        for (_, radius) in woodshed_core::pitch_motion_reading::guides(anchor) {
            for index in 0..96 {
                let a = index as f32 / 96.0 * std::f32::consts::TAU;
                let b = (index + 1) as f32 / 96.0 * std::f32::consts::TAU;
                segments.push(StageGuideSegment {
                    a: normalize(center.0 + radius * a.cos(), center.1 + radius * a.sin()),
                    b: normalize(center.0 + radius * b.cos(), center.1 + radius * b.sin()),
                    active: false,
                });
            }
        }
        segments.push(StageGuideSegment {
            a: normalize(540.0, -750.0),
            b: normalize(540.0, 750.0),
            active: false,
        });
        return segments;
    }
    if ui.app_settings.stage.set_graph_reading
        != woodshed_core::settings::StageGraphReading::Tonnetz
    {
        return Vec::new();
    }
    let snapshot = woodshed_views::stage::set_graph_snapshot(ui);
    let bounds = snapshot.snapshot.tables.bounds;
    let normalize = |(x, y): (f32, f32)| {
        (
            ((x - bounds.origin.x) / bounds.size.w.max(1.0)).clamp(0.0, 1.0),
            ((y - bounds.origin.y) / bounds.size.h.max(1.0)).clamp(0.0, 1.0),
        )
    };
    let selected = ui
        .set
        .cursor_id()
        .and_then(|id| ui.set.cards.iter().find(|card| card.id == id))
        .and_then(|card| woodshed_core::harmony::KeyedCatalogRef::from_material(&card.material));
    let materials = snapshot
        .nodes
        .iter()
        .filter_map(|node| node.keyed.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let mut edges = BTreeMap::<((i32, i32), (i32, i32)), StageGuideSegment>::new();
    for keyed in materials {
        let Some(vertices) = woodshed_core::tonnetz::triangle(&keyed) else {
            continue;
        };
        let points = vertices.map(|(x, y, _)| normalize((x, y)));
        let active = ui.context_focus.as_ref() == Some(&keyed) || selected.as_ref() == Some(&keyed);
        let lattice = vertices.map(|(x, y, _)| (x.round() as i32, y.round() as i32));
        for (from, to) in [(0usize, 1usize), (1, 2), (2, 0)] {
            let mut key = (lattice[from], lattice[to]);
            if key.1 < key.0 {
                key = (key.1, key.0);
            }
            edges
                .entry(key)
                .and_modify(|segment| segment.active |= active)
                .or_insert(StageGuideSegment {
                    a: points[from],
                    b: points[to],
                    active,
                });
        }
    }
    edges.into_values().collect()
}

/// Paint the Set graph projection into the same leaf registry as the Related
/// swatch. Its identity and edge filtering are owned by Woodshed; Cambium
/// supplies the shared canvas and native hit targets.
fn sync_set_graph_swatch(shared: &mut Shared, ui: &UiState, leaves: &mut LeafRegistry<u64>) {
    let swatch = woodshed_views::stage::set_graph_swatch(ui);
    let mut h = hasher();
    swatch.width.hash(&mut h);
    swatch.height.hash(&mut h);
    swatch.viewport.pan.0.to_bits().hash(&mut h);
    swatch.viewport.pan.1.to_bits().hash(&mut h);
    swatch.viewport.zoom.to_bits().hash(&mut h);
    for node in &swatch.graph.nodes {
        node.id.hash(&mut h);
        node.position.0.to_bits().hash(&mut h);
        node.position.1.to_bits().hash(&mut h);
        node.kind.hash(&mut h);
    }
    for relation in &swatch.relations {
        relation.id.hash(&mut h);
        relation.visible.hash(&mut h);
        relation.emphasized.hash(&mut h);
        for point in &relation.route {
            point.0.to_bits().hash(&mut h);
            point.1.to_bits().hash(&mut h);
        }
    }
    for footprint in &swatch.node_footprints {
        footprint.id.hash(&mut h);
        footprint.width.to_bits().hash(&mut h);
        footprint.height.to_bits().hash(&mut h);
    }
    swatch.selected.hash(&mut h);
    swatch.hovered.hash(&mut h);
    let segments = tonnetz_segments(ui);
    segments.len().hash(&mut h);
    for segment in &segments {
        segment.a.0.to_bits().hash(&mut h);
        segment.a.1.to_bits().hash(&mut h);
        segment.b.0.to_bits().hash(&mut h);
        segment.b.1.to_bits().hash(&mut h);
        segment.active.hash(&mut h);
    }
    let sig = h.finish();
    if sig == shared.set_graph_sig {
        return;
    }
    shared.set_graph_sig = sig;
    let graph = swatch.paint_leaf(|kind: &&str| related_kind_color(kind));
    let leaf = StageGuidesLeaf {
        graph,
        segments,
        viewport: swatch.viewport,
        inset: swatch.node_radius + swatch.edge_width,
    };
    leaves.insert(SET_GRAPH_LEAF_KEY, Box::new(leaf));
}

/// The board's dots plus its shape are the model; the leaf paints the neck and
/// markers. Rebuilt only on a board change — the run's live position rides
/// [`sync_fretboard_active`].
fn sync_fretboard(shared: &mut Shared, ui: &UiState, leaves: &mut LeafRegistry<u64>) {
    let st = &ui.stage;
    let marker_style = ui.app_settings.fretboard.marker_style.clone();
    let orientation = Orientation::from_name(&ui.app_settings.fretboard.orientation);
    let distinguish_root = ui.app_settings.accessibility.distinguish_root;
    // Scale / Chord carry their richer FretDots (pins, detail cards); the
    // transport lenses (Arpeggio / Exercise / Progression) feed uniform
    // LensMarkers. Either way the leaf needs only the position and whether it is
    // the root — the sounding step rides `set_active`.
    let dots: Vec<Dot> = match st.lens {
        woodshed_core::Lens::Scales | woodshed_core::Lens::Chords => st
            .dots()
            .into_iter()
            .map(|d| Dot {
                string_index: d.string_index,
                fret: d.fret,
                is_root: d.is_root,
                marked: false,
                excluded: false,
            })
            .collect(),
        _ => st
            .lens_markers()
            .0
            .into_iter()
            .map(|m| Dot {
                string_index: m.string_index,
                fret: m.fret,
                is_root: m.is_root,
                marked: false,
                // The Exercise's fading trail rides the leaf's faint "excluded"
                // paint, so the eye stays on the bright current step.
                excluded: m.is_trail,
            })
            .collect(),
    };
    let mut h = hasher();
    (st.lens as usize).hash(&mut h);
    st.string_count().hash(&mut h);
    st.fret_start.hash(&mut h);
    st.fret_count.hash(&mut h);
    marker_style.hash(&mut h);
    matches!(orientation, Orientation::Vertical).hash(&mut h);
    distinguish_root.hash(&mut h);
    for d in &dots {
        d.string_index.hash(&mut h);
        d.fret.hash(&mut h);
        d.is_root.hash(&mut h);
    }
    let sig = h.finish();
    if sig == shared.fretboard_sig {
        return;
    }
    shared.fretboard_sig = sig;
    leaves.insert(
        FRETBOARD_LEAF_KEY,
        Box::new(FretboardLeaf::new(
            st.string_count(),
            st.fret_start,
            st.fret_count,
            orientation,
            distinguish_root,
            dots,
            MarkerStyle::from_name(&marker_style),
        )),
    );
}

/// Push the run's live values into the fretboard leaf each frame: the active
/// (sounding) note, and the touch's path trail plus whether to draw it. The leaf
/// is otherwise only rebuilt on a board change; the run and the Path toggle move
/// between those.
fn sync_fretboard_active(ui: &UiState, leaves: &mut LeafRegistry<u64>) {
    let st = &ui.stage;
    // Scale / Chord: the running step. Transport lenses (Arpeggio / Exercise):
    // the board's current position, which advances live as the transport steps,
    // so it is read here each frame rather than baked into the leaf.
    let active = match st.lens {
        woodshed_core::Lens::Scales | woodshed_core::Lens::Chords => st
            .scale_run_playing
            .then_some(st.scale_run_active)
            .flatten(),
        _ => st.lens_markers().1,
    };
    // Draw mode always shows the trail (you're editing it).
    let show_path = st.path_shown || st.draw_mode;
    let path = if show_path {
        st.run_positions()
    } else {
        Vec::new()
    };
    if let Some(leaf) = leaves.get_mut_as::<FretboardLeaf>(&FRETBOARD_LEAF_KEY) {
        leaf.set_active(active);
        leaf.set_path(path, show_path);
    }
}

/// The Rehearsal board is a second fretboard leaf, fed from the card under the
/// cursor (not the live Stage lens) and carrying that card's marks and mark
/// mode. Only rebuilt while Rehearsal is on screen and the model changes; the
/// label overlay drives click-to-mark.
fn sync_rehearsal_fretboard(shared: &mut Shared, ui: &UiState, leaves: &mut LeafRegistry<u64>) {
    if ui.section != woodshed_core::storage::AppSection::Rehearsal || ui.set.cards.is_empty() {
        return;
    }
    let cursor = ui.set.cursor.min(ui.set.cards.len() - 1);
    let card = &ui.set.cards[cursor];
    let st = &ui.stage;
    let geom = ui.rehearsal_board_geometry();
    let marker_style = ui.app_settings.fretboard.marker_style.clone();
    let orientation = Orientation::from_name(&ui.app_settings.fretboard.orientation);
    let distinguish_root = ui.app_settings.accessibility.distinguish_root;
    // marked / excluded come from the same UiState helpers the view uses, so the
    // mode logic (Off / Solo / Mute) lives in exactly one place.
    let dots: Vec<Dot> = st
        .dots_for_card(card)
        .into_iter()
        .map(|d| Dot {
            string_index: d.string_index,
            fret: d.fret,
            is_root: d.is_root,
            marked: ui.card_marked(d.string_index, d.fret),
            excluded: ui.card_excluded(d.string_index, d.fret),
        })
        .collect();
    let mut h = hasher();
    cursor.hash(&mut h);
    geom.string_count.hash(&mut h);
    geom.fret_start.hash(&mut h);
    geom.fret_count.hash(&mut h);
    marker_style.hash(&mut h);
    matches!(orientation, Orientation::Vertical).hash(&mut h);
    distinguish_root.hash(&mut h);
    for d in &dots {
        d.string_index.hash(&mut h);
        d.fret.hash(&mut h);
        d.is_root.hash(&mut h);
        d.marked.hash(&mut h);
        d.excluded.hash(&mut h);
    }
    let sig = h.finish();
    if sig == shared.rehearsal_fretboard_sig {
        return;
    }
    shared.rehearsal_fretboard_sig = sig;
    leaves.insert(
        REHEARSAL_FRETBOARD_LEAF_KEY,
        Box::new(FretboardLeaf::new(
            geom.string_count,
            geom.fret_start,
            geom.fret_count,
            orientation,
            distinguish_root,
            dots,
            MarkerStyle::from_name(&marker_style),
        )),
    );
}
