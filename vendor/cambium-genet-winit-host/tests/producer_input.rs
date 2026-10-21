// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use cambium::{AnyView, GenetCtx, GenetElement, custom_leaf, el, on_click};
use cambium_genet_winit_host::{
    Harness, HostHooks, Init, ProducedTexture, ProducerContext, TextureProducer, inert_hooks,
};

type View = Box<dyn AnyView<State, (), GenetCtx, GenetElement>>;
type Logic = fn(&State) -> View;

struct State {
    transform: &'static str,
    overlay: bool,
    hits: Vec<(f32, f32)>,
    overlay_hits: usize,
}

struct Unrendered;
impl TextureProducer for Unrendered {
    fn render(&mut self, _: &ProducerContext<'_>) -> Option<ProducedTexture> {
        panic!("the native input harness does not own a GPU")
    }
}

fn view(state: &State) -> View {
    let leaf = on_click(
        custom_leaf::<State, ()>(7, 80, 40).attr(
            "style",
            format!(
                "display:block;position:absolute;left:20px;top:30px;width:80px;height:40px;\
             padding:7px;border:5px solid black;transform:{};transform-origin:0 0;",
                state.transform
            ),
        ),
        |state: &mut State, event| state.hits.push(event.local),
    );
    let overlay = state.overlay.then(|| {
        on_click(
            el("button", ()).attr(
                "style",
                "position:absolute;left:50px;top:60px;width:20px;height:20px;z-index:1;",
            ),
            |state: &mut State, _| state.overlay_hits += 1,
        )
    });
    Box::new(
        el("div", (leaf, overlay)).attr("style", "position:relative;width:300px;height:240px;"),
    )
}

fn harness() -> Harness<State, Logic, View> {
    let mut hooks: HostHooks<State, Logic, View> = inert_hooks();
    hooks.frame = Box::new(|cx| {
        if !cx.producers.contains(7) {
            cx.producers.register(7, Unrendered, &["color"]).unwrap();
        }
        false
    });
    let mut host = Harness::with_hooks(
        Init {
            state: State {
                transform: "scale(2)",
                overlay: false,
                hits: vec![],
                overlay_hits: 0,
            },
            logic: view as Logic,
            sheet: String::new(),
            fonts: Vec::new(),
            images: Vec::new(),
        },
        hooks,
    );
    host.prepare_frame();
    host.layout_at(300.0, 240.0);
    host
}

fn close(actual: (f32, f32), expected: (f32, f32)) {
    assert!(
        (actual.0 - expected.0).abs() < 1e-4 && (actual.1 - expected.1).abs() < 1e-4,
        "actual={actual:?} expected={expected:?}"
    );
}

#[test]
fn content_local_clicks_respect_transforms_padding_overlay_and_singular_rejection() {
    let mut host = harness();
    host.click_at(54.0, 68.0);
    close(host.state().hits[0], (5.0, 7.0));
    host.click_at(42.0, 60.0);
    assert_eq!(
        host.state().hits.len(),
        1,
        "border/padding are outside producer content"
    );
    host.update(|state| state.overlay = true);
    host.click_at(54.0, 68.0);
    assert_eq!(host.state().overlay_hits, 1);
    assert_eq!(
        host.state().hits.len(),
        1,
        "the ordinary overlaid control owns its hit"
    );
    host.update(|state| {
        state.overlay = false;
        state.transform = "rotate(90deg)";
    });
    host.click_at(1.0, 47.0);
    close(host.state().hits[1], (5.0, 7.0));
    host.update(|state| state.transform = "scale(0)");
    host.click_at(20.0, 30.0);
    assert_eq!(host.state().hits.len(), 2);
}

#[test]
fn window_coordinates_cross_host_zoom_once_before_content_mapping() {
    let mut host = harness();
    host.set_ui_zoom(1.5);
    host.click_at_window(81.0, 102.0);
    close(host.state().hits[0], (5.0, 7.0));
}
