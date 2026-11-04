use super::*;

fn leaf() -> FretboardLeaf {
    let dots = vec![
        Dot {
            string_index: 0,
            fret: 5,
            is_root: true,
            marked: false,
            excluded: false,
        },
        Dot {
            string_index: 1,
            fret: 7,
            is_root: false,
            marked: false,
            excluded: false,
        },
    ];
    FretboardLeaf::new(
        6,
        0,
        12,
        Orientation::Horizontal,
        false,
        dots,
        MarkerStyle::Sharp,
    )
}

#[test]
fn leaf_announces_itself_as_a_described_graphic() {
    let mut l = leaf();
    let mut node = accesskit::Node::new(accesskit::Role::GenericContainer);
    l.accessibility(&mut node);
    assert_eq!(node.role(), accesskit::Role::GraphicsObject);
    let label = node.label().expect("a fallback summary");
    assert!(label.contains("fretboard"), "names what it is: {label}");
    assert!(label.contains("2 notes"), "counts its notes: {label}");
}

#[test]
fn an_authored_name_outranks_the_leaf_fallback() {
    let mut l = leaf();
    let mut node = accesskit::Node::new(accesskit::Role::GenericContainer);
    node.set_label("A Minor Pentatonic fretboard");
    l.accessibility(&mut node);
    // The author named the board; the leaf must not overwrite it.
    assert_eq!(node.label(), Some("A Minor Pentatonic fretboard"));
    // But role is the leaf's to state.
    assert_eq!(node.role(), accesskit::Role::GraphicsObject);
}
