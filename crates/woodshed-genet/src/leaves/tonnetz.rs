use super::*;

pub(super) fn tonnetz_segments(ui: &UiState) -> Vec<StageGuideSegment> {
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
