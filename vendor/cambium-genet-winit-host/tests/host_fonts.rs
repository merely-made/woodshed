// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! The host font seam: a face the application bundles is registered into the
//! text system the host builds, and a sheet naming that family shapes with it.
//!
//! Ahem is the instrument. Every glyph in it advances exactly one em, so a run
//! of five characters at 20px is 100px wide and nothing else is. A system
//! fallback cannot produce that number by accident, which is what makes the
//! assertion a positive control rather than a "text got laid out" check.

use cambium::{AnyView, GenetCtx, GenetElement, el, text};
use cambium_genet_winit_host::{Harness, HostFont, Init, inert_hooks};
use taproot::Selector;

#[derive(Default)]
struct App;

type Child = Box<dyn AnyView<App, (), GenetCtx, GenetElement>>;

/// One absolutely positioned block, so its width shrinks to the run's advance.
fn root(_state: &App) -> Child {
    Box::new(el("span", text("XXXXX")).attr("class", "run").attr(
        "style",
        "display:block;position:absolute;left:0px;top:0px;font-size:20px;",
    ))
}

/// The bundled family the sheet names. No system font can answer to it.
const SHEET: &str = ".run { font-family: 'RootstockSeamFace'; }";

fn ahem() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../ports/pelt/examples/Ahem.ttf");
    std::fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

fn harness(fonts: Vec<HostFont>) -> Harness<App, fn(&App) -> Child, Child> {
    let mut h = Harness::with_hooks(
        Init {
            state: App,
            logic: root as fn(&App) -> Child,
            sheet: SHEET.into(),
            fonts,
            images: Vec::new(),
        },
        inert_hooks(),
    );
    h.layout_at(400.0, 200.0);
    h
}

fn run_width(h: &Harness<App, fn(&App) -> Child, Child>) -> f32 {
    let node = h
        .with_dom(|dom| {
            taproot::matching_estimated_fixture(dom, &Selector::class("run"))
                .first()
                .copied()
        })
        .expect("the run element is in the DOM");
    h.painted_rect(node).expect("the run element is laid out").2
}

#[test]
fn bundled_face_shapes_the_family_the_sheet_names() {
    let pinned = harness(vec![HostFont {
        family: Some("RootstockSeamFace".into()),
        bytes: ahem(),
    }]);
    assert_eq!(
        run_width(&pinned),
        100.0,
        "five Ahem glyphs at 20px advance one em each"
    );

    // The same sheet without the face falls back, and the fallback is not Ahem.
    let fallback = harness(Vec::new());
    assert_ne!(
        run_width(&fallback),
        100.0,
        "without the bundled face the family cannot resolve to Ahem"
    );
}

#[test]
fn registration_survives_a_relayout() {
    let mut h = harness(vec![HostFont {
        family: Some("RootstockSeamFace".into()),
        bytes: ahem(),
    }]);
    // A resize drops the retained session and builds a new text system.
    h.layout_at(600.0, 300.0);
    assert_eq!(run_width(&h), 100.0, "the face is registered again");
}
