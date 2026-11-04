use super::*;

/// Lower the typed read model into the shared portable scene contract. Identity
/// stays with the DTO; placement is presentation only and may be replaced.
pub fn overview_scene(overview: &OverviewSnapshot) -> SceneSnapshot {
    let mut scene = Scene::new();
    let mut indices = BTreeMap::new();
    let mut row_counts = [0usize; 4];
    let mut points = Vec::new();
    for node in &overview.nodes {
        let row = match node.id {
            OverviewNodeId::Artifact(_) => 0,
            OverviewNodeId::View(_) => 1,
            OverviewNodeId::Process(_) => 2,
            OverviewNodeId::Catalog(_) => 3,
        };
        let point = Vec2::new(row_counts[row] as f32 * 220.0, row as f32 * 180.0);
        row_counts[row] += 1;
        let instance = InstanceId(scene.items.len() as u32);
        indices.insert(node.id.clone(), instance);
        points.push(point);
        let source = scene.intern_source(node.id.source_ref());
        scene.items.push(ProjectedItem {
            source,
            space: Scene::WORLD,
            transform: Transform2::translation(point.x, point.y),
            footprint: Footprint::Rect {
                size: Size2::new(180.0, 100.0),
            },
            representation: Representation::Card,
            layer: 0,
            visible: true,
            hit: None,
            channels: Vec::new(),
        });
    }
    for relation in &overview.relations {
        let (Some(&from), Some(&to)) = (indices.get(&relation.from), indices.get(&relation.to))
        else {
            continue;
        };
        scene.relations.push(RoutedRelation {
            link_key: None,
            marks: Default::default(),
            strength: None,
            from,
            to,
            space: Scene::WORLD,
            points: vec![points[from.0 as usize], points[to.0 as usize]],
            kind: Some(relation.kind.stable_id().into()),
            weight: None,
        });
    }
    scene.bounds = Rect::new(
        Vec2::new(-120.0, -80.0),
        Size2::new(
            row_counts.into_iter().max().unwrap_or(1).max(1) as f32 * 220.0 + 20.0,
            720.0,
        ),
    );
    // Dense identities are projection-local. Changed read-model contents use a
    // new epoch so a retained consumer cannot interpret old instance references
    // against a freshly ordered overview.
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for node in &overview.nodes {
        for value in [node.id.wire_key(), node.label.clone(), node.detail.clone()] {
            for byte in (value.len() as u64)
                .to_le_bytes()
                .into_iter()
                .chain(value.bytes())
            {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x100_0000_01b3);
            }
        }
    }
    for relation in &overview.relations {
        for value in [
            relation.from.wire_key(),
            relation.to.wire_key(),
            relation.kind.stable_id().into(),
        ] {
            for byte in (value.len() as u64)
                .to_le_bytes()
                .into_iter()
                .chain(value.bytes())
            {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x100_0000_01b3);
            }
        }
    }
    SceneSnapshot::from_dense(SceneEpoch(hash), Revision(0), scene).expect("valid overview scene")
}
