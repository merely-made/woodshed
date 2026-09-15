//! Header, destinations, layout switch, work area, phone bar. Lane S1 owns this.
//!
//! One markup for both dock families: `Layout::Dock` puts the destinations in
//! the header and the dock under the work area (with the phone bar under that,
//! shown only by the phone media query); `Layout::Rail` puts the transport rail
//! beside a column whose top is the tab strip.

use crate::{
    CompactCommand, CompactPlayerState, FullView, Layout, RedshankSurfaceState, SurfaceTab, dock,
    rail, scene, tabs,
};
use cambium::{button_with, el, lens, text};

fn tab_button(active: SurfaceTab, tab: SurfaceTab, class: &str) -> FullView {
    let class = if tab == active {
        format!("{class} rs-segment-on")
    } else {
        class.to_owned()
    };
    Box::new(
        button_with(
            vec![Box::new(text(tab.label())) as FullView],
            move |state: &mut RedshankSurfaceState, _| {
                state.active_tab = tab;
                state.request(CompactCommand::SelectTab(tab));
            },
        )
        .attr("class", class)
        .attr("role", "tab")
        .attr("aria-label", tab.label())
        .attr(
            "aria-selected",
            if tab == active { "true" } else { "false" },
        )
        .attr("aria-controls", tab.panel_id()),
    )
}

fn destinations(active: SurfaceTab, class: &str, button_class: &str, label: &str) -> FullView {
    let tabs: Vec<FullView> = SurfaceTab::ALL
        .into_iter()
        .map(|tab| tab_button(active, tab, button_class))
        .collect();
    Box::new(
        el("nav", tabs)
            .attr("class", class)
            .attr("role", "tablist")
            .attr("aria-label", label),
    )
}

/// The header: the Merely mark reduction, the name, and the destinations.
fn header(state: &RedshankSurfaceState) -> FullView {
    let children: Vec<FullView> = vec![
        Box::new(
            el("span", ())
                .attr("class", "rs-mark")
                .attr("aria-label", "Merely"),
        ),
        Box::new(el("span", text("Redshank")).attr("class", "rs-wordmark")),
        destinations(state.active_tab, "rs-segment", "rs-tab", "Destinations"),
    ];
    Box::new(el("header", children).attr("class", "rs-header"))
}

fn panel(state: &RedshankSurfaceState) -> FullView {
    match state.active_tab {
        SurfaceTab::Listen => tabs::listen::panel(state),
        SurfaceTab::Library => tabs::library::panel(state),
        SurfaceTab::Notes => tabs::notes::panel(state),
        SurfaceTab::Mere => scene::panel(state),
        SurfaceTab::Settings => tabs::settings::panel(state),
    }
}

/// Only the active tab's panel is in the tree; hidden panels are not rendered.
fn work(state: &RedshankSurfaceState) -> FullView {
    Box::new(
        el("div", panel(state))
            .attr("class", "rs-work")
            .attr("role", "tabpanel")
            .attr("id", state.active_tab.panel_id())
            .attr("aria-label", state.active_tab.label()),
    )
}

fn notice(state: &RedshankSurfaceState) -> Vec<FullView> {
    state
        .notice
        .as_ref()
        .map(|notice| {
            Box::new(
                el("div", text(notice.clone()))
                    .attr("class", "rs-notice")
                    .attr("role", "status"),
            ) as FullView
        })
        .into_iter()
        .collect()
}

/// The dock, lensed onto the compact state so it is the same view a host can
/// mount alone. The slot is `display: contents`, so the DOM holds exactly one
/// `.rs-dock`.
fn dock_slot() -> FullView {
    Box::new(
        el(
            "footer",
            Box::new(lens(
                |compact: &mut CompactPlayerState| dock::compact_surface(compact),
                |state: &mut RedshankSurfaceState| &mut state.compact,
            )),
        )
        .attr("class", "rs-dock-slot"),
    )
}

fn rail_slot() -> FullView {
    Box::new(
        el(
            "div",
            Box::new(lens(
                |compact: &mut CompactPlayerState| rail::rail_surface(compact),
                |state: &mut RedshankSurfaceState| &mut state.compact,
            )),
        )
        .attr("class", "rs-rail-slot"),
    )
}

pub fn surface(state: &RedshankSurfaceState) -> FullView {
    let layout_class = match state.layout {
        Layout::Dock => "rs-dock-layout",
        Layout::Rail => "rs-rail-layout",
    };
    let mut children: Vec<FullView> = Vec::new();
    match state.layout {
        Layout::Dock => {
            children.push(header(state));
            children.extend(notice(state));
            children.push(work(state));
            children.push(dock_slot());
            children.push(destinations(
                state.active_tab,
                "rs-phone-bar",
                "rs-phone-tab",
                "Destinations",
            ));
        },
        Layout::Rail => {
            children.push(rail_slot());
            let mut column: Vec<FullView> = vec![destinations(
                state.active_tab,
                "rs-rail-tabs",
                "rs-tab",
                "Destinations",
            )];
            column.extend(notice(state));
            column.push(work(state));
            children.push(Box::new(
                el("div", column).attr("class", "rs-column rs-work-column"),
            ));
        },
    }
    Box::new(
        el("main", children)
            .attr(
                "class",
                format!("rs-app {} {layout_class}", state.scope_class()),
            )
            .attr("data-tab", state.active_tab.panel_id()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Mode, Seed, theme};
    use cambium::{DomHandle, GenetAppRunner, PointerClick};
    use cambium_genet_winit_host::Harness;
    use genet_scripted_dom::{NodeId, ScriptedDom};
    use layout_dom_api::{LayoutDom, LocalName, Namespace};
    use redshank_model::ItemId;
    use std::cell::RefCell;
    use std::rc::Rc;

    use crate::{Face, NoteKind, NoteMarker, NowPlaying, Recording, SourceKind, TransportState};
    use redshank_model::AnnotationId;

    type FullLogic = fn(&RedshankSurfaceState) -> FullView;
    type FullRunner = GenetAppRunner<RedshankSurfaceState, FullLogic, FullView, ()>;

    fn now_playing() -> NowPlaying {
        NowPlaying {
            item_id: ItemId("episode-42".into()),
            title: "Wetland".into(),
            feed_title: Some("The Allusionist".into()),
            face: Face::Tag("m4a".into()),
            source: SourceKind::Local,
            position_ms: 84_000,
            duration_ms: Some(121_000),
            resumed_from_ms: Some(81_000),
            buffered_percent: 100,
            markers: vec![NoteMarker {
                id: AnnotationId("note-1".into()),
                offset_ms: 46_000,
                end_offset_ms: None,
                kind: NoteKind::Text,
            }],
        }
    }

    fn state_with(transport: TransportState) -> RedshankSurfaceState {
        RedshankSurfaceState {
            compact: CompactPlayerState {
                transport,
                now_playing: Some(now_playing()),
                ..CompactPlayerState::default()
            },
            ..Default::default()
        }
    }

    fn runner(state: RedshankSurfaceState) -> FullRunner {
        let dom: DomHandle = Rc::new(RefCell::new(ScriptedDom::new()));
        FullRunner::new(dom, crate::surface, state)
    }

    fn node_with_control(dom: &ScriptedDom, root: NodeId, control: &str) -> NodeId {
        let attribute = LocalName::from("aria-controls");
        let empty = Namespace::from("");
        let mut pending = vec![root];
        while let Some(node) = pending.pop() {
            if dom
                .attribute(node, &empty, &attribute)
                .is_some_and(|value| value == control)
            {
                return node;
            }
            pending.extend(dom.dom_children(node));
        }
        panic!("missing tab for {control}");
    }

    #[test]
    fn shell_carries_the_scope_layout_and_one_panel() {
        let runner = runner(state_with(TransportState::Playing));
        let markup = runner.dom().borrow().outer_html(runner.root());
        assert!(markup.contains("class=\"rs-app t-redshank rs-dock-layout\""));
        assert!(markup.contains("role=\"tablist\""));
        assert!(markup.contains("id=\"redshank-listen-panel\""));
        assert!(!markup.contains("id=\"redshank-library-panel\""));
        assert!(markup.contains("class=\"rs-dock\""));
        assert!(markup.contains("class=\"rs-phone-bar\""));
    }

    /// Segment law: the selected destination is `--t-text` on `--t-bg`, which
    /// the sheet paints from `rs-segment-on` alone.
    #[test]
    fn segment_law_marks_the_selected_destination() {
        let mut runner = runner(state_with(TransportState::Playing));
        let markup = runner.dom().borrow().outer_html(runner.root());
        assert!(markup.contains("class=\"rs-tab rs-segment-on\" role=\"tab\" aria-label=\"Listen\" aria-selected=\"true\""));
        let library = node_with_control(
            &runner.dom().borrow(),
            runner.root(),
            "redshank-library-panel",
        );
        runner.dispatch_click(library, PointerClick::at((1.0, 1.0)));
        let mut commands = Vec::new();
        runner.update(|state| {
            assert_eq!(state.active_tab, SurfaceTab::Library);
            commands.extend(state.drain_commands());
        });
        assert_eq!(commands, [CompactCommand::SelectTab(SurfaceTab::Library)]);
        let markup = runner.dom().borrow().outer_html(runner.root());
        assert!(markup.contains("aria-label=\"Library\" aria-selected=\"true\""));
        assert!(markup.contains("aria-label=\"Listen\" aria-selected=\"false\""));
    }

    #[test]
    fn rail_layout_puts_the_rail_beside_the_tab_strip() {
        let mut state = state_with(TransportState::Playing);
        state.layout = Layout::Rail;
        let runner = runner(state);
        let markup = runner.dom().borrow().outer_html(runner.root());
        assert!(markup.contains("rs-rail-layout"));
        assert!(markup.contains("class=\"rs-rail\""));
        assert!(markup.contains("class=\"rs-rail-tabs\""));
        assert!(!markup.contains("class=\"rs-dock\""));
    }

    #[test]
    fn notice_is_a_status_line_under_the_header() {
        let mut state = state_with(TransportState::Playing);
        state.notice = Some("Feed refreshed".into());
        let runner = runner(state);
        let markup = runner.dom().borrow().outer_html(runner.root());
        assert!(markup.contains("class=\"rs-notice\" role=\"status\""));
        assert!(markup.contains("Feed refreshed"));
    }

    /// Does Livery mount an `svg` element at all? The mark is a styled span
    /// until this says otherwise.
    #[test]
    fn svg_elements_mount() {
        let dom: DomHandle = Rc::new(RefCell::new(ScriptedDom::new()));
        type Logic =
            fn(&()) -> Box<dyn cambium::AnyView<(), (), cambium::GenetCtx, cambium::GenetElement>>;
        let logic: Logic = |_| {
            Box::new(el(
                "svg",
                Box::new(el("path", ()).attr("d", "M0 0 L16 0 L8 16 Z")),
            ))
        };
        let runner = GenetAppRunner::<(), Logic, _, ()>::new(dom, logic, ());
        let markup = runner.dom().borrow().outer_html(runner.root());
        println!("svg probe: {markup}");
        assert!(markup.contains("<svg"));
    }

    // --- dock height invariance -------------------------------------------

    fn fixtures() -> Vec<(&'static str, RedshankSurfaceState)> {
        let mut empty = RedshankSurfaceState::default();
        empty.compact.transport = TransportState::Empty;
        let mut recording = state_with(TransportState::Playing);
        recording.compact.recording = Some(Recording {
            elapsed_ms: 4_000,
            anchor_ms: 84_000,
            paused_for_capture: true,
        });
        vec![
            ("empty", empty),
            ("playing", state_with(TransportState::Playing)),
            ("paused", state_with(TransportState::Paused)),
            ("buffering", state_with(TransportState::Buffering)),
            ("recording", recording),
            ("completed", state_with(TransportState::Completed)),
            (
                "unavailable",
                state_with(TransportState::Unavailable(
                    "Output device unavailable".into(),
                )),
            ),
        ]
    }

    fn dock_node(dom: &ScriptedDom, root: NodeId) -> NodeId {
        let class = LocalName::from("class");
        let empty = Namespace::from("");
        let mut pending = vec![root];
        while let Some(node) = pending.pop() {
            if dom
                .attribute(node, &empty, &class)
                .is_some_and(|value| value.split_whitespace().any(|name| name == "rs-dock"))
            {
                return node;
            }
            pending.extend(dom.dom_children(node));
        }
        panic!("missing dock");
    }

    /// The dock's laid-out height is identical in every transport state, in a
    /// real headless layout (the winit host's windowless `Harness`).
    #[test]
    fn dock_height_is_identical_in_every_transport_state() {
        let mut heights = Vec::new();
        for (name, state) in fixtures() {
            let logic: FullLogic = crate::surface;
            let mut harness = Harness::new(theme::sheet(), state, logic);
            harness.layout_at(960.0, 640.0);
            let dock = harness.with_dom(|dom| dock_node(dom, dom.document()));
            let rect = harness.painted_rect(dock).expect("dock paints");
            println!("{name}: dock rect {rect:?}");
            heights.push((name, rect.3));
        }
        let first = heights[0].1;
        for (name, height) in &heights {
            assert!(
                (height - first).abs() < 0.5,
                "{name} dock height {height} != {first}"
            );
        }
        assert!(first > 100.0, "dock collapsed to {first}");
    }

    /// The same invariance at phone width, where the transport row re-flows
    /// into the thumb grid.
    #[test]
    fn dock_height_is_identical_at_phone_width() {
        let mut heights = Vec::new();
        for (name, state) in fixtures() {
            let logic: FullLogic = crate::surface;
            let mut harness = Harness::new(theme::sheet(), state, logic);
            harness.layout_at(412.0, 892.0);
            let dock = harness.with_dom(|dom| dock_node(dom, dom.document()));
            let rect = harness.painted_rect(dock).expect("dock paints");
            println!("phone {name}: dock rect {rect:?}");
            heights.push((name, rect.3));
        }
        let first = heights[0].1;
        for (name, height) in &heights {
            assert!(
                (height - first).abs() < 0.5,
                "{name} phone dock height {height} != {first}"
            );
        }
    }

    #[test]
    fn every_scope_class_is_reachable_from_the_state() {
        for (seed, mode) in theme::SCOPES {
            let state = RedshankSurfaceState {
                seed,
                mode,
                ..Default::default()
            };
            assert_eq!(state.scope_class(), theme::scope_class(seed, mode));
        }
        let _ = (Seed::Wetland, Mode::Dark);
    }

    // --- artwork probe -----------------------------------------------------

    /// Does a `.rs-face` with `background-image: url(<a real PNG>)` paint any
    /// differently from one without?
    ///
    /// **It does not, and it cannot as the stack stands.** Livery parses
    /// `url(...)` into `BackgroundImage::Url` and `paint.rs::image_key_for`
    /// resolves it one of two ways: a `data:` URL is decoded inline, and
    /// anything else is looked up in the caller-supplied
    /// `ImageSources = HashMap<String, Vec<u8>>`. Cambium's only paint call —
    /// `cambium-rootstock/src/owned_layout.rs::emit_paint_list_with_leaves` —
    /// passes `&HashMap::new()`, a hardcoded empty map with no `Init` field or
    /// setter behind it. So a file or http URL finds no bytes, `image_key_for`
    /// returns `None`, and `emit_background_image_in` returns having emitted
    /// nothing. Silently: no error, no fallback paint.
    ///
    /// This test can only show the half a windowless harness can see — that
    /// the declaration changes no geometry. `Harness` exposes `painted_rect`
    /// and no paint list; `read_frame` needs a wgpu `Surface`, which a
    /// windowless harness has not got, so there is no headless pixel readback
    /// to compare. The finding above is read off the source, and the seam it
    /// asks for is an `Init.images` field forwarded into that call.
    #[test]
    fn artwork_background_image_changes_no_geometry_and_paints_nothing() {
        const ARTWORK: &str = "C:/Users/mark_/Code/testing/woodshed/c1.png";
        assert!(
            std::path::Path::new(ARTWORK).is_file(),
            "the probe wants a real PNG at {ARTWORK}"
        );
        let face = |artwork: bool| -> (f32, f32, f32, f32) {
            let mut state = state_with(TransportState::Playing);
            if artwork {
                state.compact.now_playing.as_mut().unwrap().face =
                    Face::Artwork(ARTWORK.to_owned());
            }
            let sheet = if artwork {
                format!(
                    "{}
.rs-dock-identity .rs-face {{ background-image: url({ARTWORK}); }}
",
                    theme::sheet()
                )
            } else {
                theme::sheet()
            };
            let logic: FullLogic = crate::surface;
            let mut harness = Harness::new(sheet, state, logic);
            harness.layout_at(960.0, 640.0);
            let node = harness.with_dom(|dom| face_node(dom, dom.document()));
            harness.painted_rect(node).expect("the face paints")
        };
        let plain = face(false);
        let with_artwork = face(true);
        println!("face without artwork: {plain:?}");
        println!("face with background-image: {with_artwork:?}");
        assert_eq!(
            plain, with_artwork,
            "a background image must not move anything"
        );
    }

    fn face_node(dom: &ScriptedDom, root: NodeId) -> NodeId {
        let class = LocalName::from("class");
        let empty = Namespace::from("");
        let mut pending = vec![root];
        while let Some(node) = pending.pop() {
            if dom
                .attribute(node, &empty, &class)
                .is_some_and(|value| value.split_whitespace().any(|name| name == "rs-face"))
            {
                return node;
            }
            pending.extend(dom.dom_children(node));
        }
        panic!("missing face");
    }
}
