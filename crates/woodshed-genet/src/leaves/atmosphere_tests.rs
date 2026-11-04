use super::*;

fn commands(leaf: &mut dyn Leaf, size: Size) -> Vec<sprigging::PaintCmd> {
    let mut commands = Vec::new();
    leaf.paint(&mut PaintCx::new(&mut commands, size));
    commands
}

#[test]
fn atmosphere_is_clipped_resized_and_beneath_graph_independently_of_camera() {
    let source_size = Size {
        width: 100.0,
        height: 100.0,
    };
    let actual_size = Size {
        width: 200.0,
        height: 50.0,
    };
    let mut ambient = Vec::new();
    PaintCx::new(&mut ambient, source_size).fill_rect(
        10.0,
        20.0,
        4.0,
        6.0,
        sprigging::ColorF {
            r: 0.1,
            g: 0.2,
            b: 0.3,
            a: 0.5,
        },
    );
    let graph = sprigging::GraphCanvas::new(
        vec![sprigging::GraphGlyphNode {
            x: 0.5,
            y: 0.5,
            color: sprigging::ColorF {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
        }],
        Vec::new(),
        source_size,
    );
    let mut leaf = OverviewAmbientLeaf {
        graph,
        ambient,
        source_size,
    };
    let painted = commands(&mut leaf, actual_size);
    assert!(matches!(
        painted.first(),
        Some(sprigging::PaintCmd::PushClip(_))
    ));
    match &painted[1] {
        sprigging::PaintCmd::DrawRect(rect) => {
            assert_eq!(
                (rect.placement.bounds.min.x, rect.placement.bounds.min.y),
                (20.0, 10.0)
            );
            assert_eq!(
                (rect.placement.bounds.max.x, rect.placement.bounds.max.y),
                (28.0, 13.0)
            );
        },
        other => panic!("shared atmosphere rectangle expected, got {other:?}"),
    }
    assert!(matches!(painted[2], sprigging::PaintCmd::PopClip));
    assert!(matches!(painted[3], sprigging::PaintCmd::PushClip(_)));
    assert!(
        painted[4..]
            .iter()
            .any(|cmd| matches!(cmd, sprigging::PaintCmd::DrawPath(_)))
    );
    leaf.graph.set_viewport(sprigging::GraphViewport {
        pan: (0.1, 0.2),
        zoom: 1.25,
    });
    let panned = commands(&mut leaf, actual_size);
    assert_eq!(
        format!("{:?}", &painted[..3]),
        format!("{:?}", &panned[..3])
    );
    assert_ne!(
        format!("{:?}", &painted[3..]),
        format!("{:?}", &panned[3..])
    );
}
