//! Native binding of the shared portable scene surface to the host renderer.
use crate::sync::Ctx;
use cambium_genet_winit_host::SceneProducer;
use cambium_rootstock::{ProducerRole, ProducerSemantics};
use std::{cell::RefCell, rc::Rc};
use woodshed_views::{
    stage::{
        UiState,
        arrangement_editor::{ArrangementSurfaces, PRIMARY_KEY, RELATED_KEY},
    },
    workspace::WorkspacePanel,
};

type Bound = Rc<RefCell<SceneProducer<ArrangementSurfaces>>>;
#[derive(Default)]
pub struct Bindings {
    primary: Option<Bound>,
    related: Option<Bound>,
}
impl Bindings {
    pub fn sync(&mut self, ctx: &mut Ctx<'_>) {
        let ui = ctx.runner.state();
        let visible = ui.workspace.active_panel() == Some(WorkspacePanel::Arrangement)
            && ui.persona.is_none()
            && !ui.appearance.workshop_open;
        let source = ui.arrangement_editor.surfaces.clone();
        for related in [false, true] {
            let key = if related { RELATED_KEY } else { PRIMARY_KEY };
            let present = {
                let s = source.borrow();
                if related {
                    s.related.is_some()
                } else {
                    s.primary.is_some()
                }
            };
            if !visible || !present {
                ctx.producers.remove(key);
                continue;
            }
            let slot = if related {
                &mut self.related
            } else {
                &mut self.primary
            };
            let producer = slot.get_or_insert_with(|| {
                let callback: fn(&mut ArrangementSurfaces, u32, u32) -> _ = if related {
                    |s, w, h| {
                        let theme = s.theme.clone();
                        match s
                            .related
                            .as_mut()
                            .unwrap()
                            .paint(w.max(49), h.max(49), &theme)
                        {
                            Ok(scene) => scene,
                            Err(e) => {
                                s.error = Some(e);
                                Default::default()
                            },
                        }
                    }
                } else {
                    |s, w, h| {
                        let theme = s.theme.clone();
                        match s
                            .primary
                            .as_mut()
                            .unwrap()
                            .paint(w.max(49), h.max(49), &theme)
                        {
                            Ok(scene) => scene,
                            Err(e) => {
                                s.error = Some(e);
                                Default::default()
                            },
                        }
                    }
                };
                let semantics: fn(&ArrangementSurfaces) -> Option<ProducerSemantics> = if related {
                    |_| {
                        Some(ProducerSemantics {
                            role: Some(ProducerRole::Image),
                            name: Some("Selected saved voicing".into()),
                            children: vec![],
                        })
                    }
                } else {
                    |_| {
                        Some(ProducerSemantics {
                            role: Some(ProducerRole::Image),
                            name: Some("Progression arrangement".into()),
                            children: vec![],
                        })
                    }
                };
                Rc::new(RefCell::new(SceneProducer::new(
                    source.clone(),
                    key,
                    callback,
                    |s| s.revision,
                    semantics,
                )))
            });
            producer.borrow_mut().set_source(source.clone());
            if !ctx.producers.contains(key) {
                ctx.producers
                    .register(key, producer.clone(), &[])
                    .expect("arrangement owns distinct producer keys");
            }
        }
    }
}

/// Scenario fixture uses real catalog and saved-shape APIs; arrangement edits
/// remain ordinary visible controls in the native qualification.
pub(crate) fn seed_example(ui: &mut UiState) {
    ui.stop_rehearsal();
    ui.set = Default::default();
    ui.working_sets = Default::default();
    ui.arrangement_editor = Default::default();
    ui.stage.set_lens(woodshed_core::Lens::Progressions);
    ui.stage.set_root(
        woodshed_core::ROOT_NAMES
            .iter()
            .position(|s| *s == "C")
            .unwrap(),
    );
    ui.root_dd.selected = ui.stage.root_idx;
    ui.stage.select_progression(
        ui.stage
            .progressions()
            .iter()
            .position(|p| p.name == "ii-V-I (Jazz)")
            .unwrap(),
    );
    for i in 0..3 {
        ui.stage.progression_expand(i);
        let mut card = ui.stage.card_from_lens().unwrap();
        ui.stage.select_card_shape_index(&mut card, 0).unwrap();
        ui.set.push(card);
    }
    ui.set.duplicate(2);
    ui.stage
        .select_card_shape_index(&mut ui.set.cards[3], 1)
        .unwrap();
    ui.activate_workspace_panel(WorkspacePanel::Practice);
}
/// Explicit synthetic 4/4 Set fixture shared with the Graphshell block study.
/// Only the scenario lane invokes it; starting an ordinary arrangement still
/// authors no meter. Subsequent grouping must use the native form controls.
pub(crate) fn seed_bar_example(ui: &mut UiState) {
    seed_example(ui);
    let value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../support/scene-editing/progression-set.json"
    )).expect("portable Set fixture");
    let set: woodshedding::rehearsal::Set = serde_json::from_value(value["set"].clone())
        .expect("typed portable Set fixture");
    woodshed_core::saved_arrangements::saved_arrangement_stage(&set)
        .expect("fixture source still resolves in Woodshed");
    ui.set = set;
    ui.activate_workspace_panel(WorkspacePanel::Arrangement);
    ui.sync_arrangement_scene();
}

#[cfg(test)]
mod tests {
    use super::*;
    use taproot::Selector;
    #[test]
    fn native_arrangement_controls_route_text_save_and_preserve_drafts_on_close() {
        let store = woodshed_core::storage::SessionStore::new(Box::new(
            muniment::MemoryBackend::default(),
        )
            as crate::storage::HostBackend);
        let (_, mut h) = crate::tests::save_close_harness(store);
        h.update(seed_example);
        h.after_dispatch();
        h.layout_at(1180.0, 1100.0);
        assert!(h.click_on(&Selector::class("workspace-panel").containing("Arrangement")));
        assert!(h.click_on(&Selector::class("music-legacy")));
        assert!(h.click_on(&Selector::class("arrangement-start")));
        assert!(h.click_on(&Selector::class("arrangement-place-order")));
        let before = serde_json::to_value(&h.state().set.cards).unwrap();
        assert!(h.click_on(
            &Selector::class("arrangement-field").with_attr("data-arrangement-field", "name")
        ));
        h.key_injected(" Native proof");
        let owner = h.state().working_sets.active_id;
        assert!(
            h.state()
                .arrangement_editor
                .field(owner, "name")
                .unwrap()
                .text()
                .contains("Native proof")
        );
        // Native text injection and IME can change a field before the ordinary
        // dispatch tail. A prepared frame must never retain a stale scene.
        h.update(|ui| {
            *ui.arrangement_editor.field_mut(owner, "extent").unwrap() =
                cambium::TextInput::new("-1");
        });
        h.prepare_frame();
        h.relayout();
        assert!(
            h.state()
                .arrangement_editor
                .surfaces
                .borrow()
                .primary
                .is_none()
        );
        h.with_dom(|dom| {
            assert_eq!(
                taproot::matching_classes(dom, &Selector::class("arrangement-error")).len(),
                1
            );
        });
        h.update(|ui| {
            *ui.arrangement_editor.field_mut(owner, "extent").unwrap() =
                cambium::TextInput::new("64");
        });
        h.prepare_frame();
        h.relayout();
        assert!(
            h.state()
                .arrangement_editor
                .surfaces
                .borrow()
                .primary
                .is_some()
        );
        h.request_close(cambium_genet_winit_host::CloseRequest::Native);
        h.relayout();
        assert!(!h.close_requested());
        assert!(h.state().arrangement_editor.close_pending);
        assert!(h.click_on(&Selector::class("arrangement-close-cancel")));
        assert!(h.click_on(&Selector::class("arrangement-save")));
        assert!(!h.state().arrangement_editor.dirty());
        assert_eq!(before, serde_json::to_value(&h.state().set.cards).unwrap());
        assert!(h.state().set.progression_arrangement.is_some());
    }
    #[test]
    fn native_arrangement_scene_routes_captured_spans_and_keyboard_edits() {
        use cambium_genet_winit_host::{Harness, Modifiers};
        use winit::keyboard::NamedKey;
        for width in [1180.0, 420.0] {
            let mut ui = UiState::new();
            seed_example(&mut ui);
            ui.set_viewport_width(width);
            ui.activate_workspace_panel(WorkspacePanel::Arrangement);
            ui.start_arrangement();
            ui.place_arrangement_cards_in_order();
            ui.save_arrangement();
            let original_cards = serde_json::to_value(&ui.set.cards).unwrap();
            let logic: crate::sync::Logic = Box::new(woodshed_views::stage::stage_root);
            let mut h = Harness::new(crate::appearance::stylesheet(&ui), ui, logic);
            h.layout_at(width, 1100.0);
            let selector = Selector::class("arrangement-scene");
            // Scrolling to the actual custom leaf uses the same path as native clicks.
            assert!(h.click_on(&selector));
            let node = h.with_dom(|dom| taproot::matching_classes(dom, &selector)[0]);
            let (x, y, w, height) = h.painted_rect(node).unwrap();
            let (at, beat) = {
                let surfaces = h.state().arrangement_editor.surfaces.borrow();
                let surface = surfaces.primary.as_ref().unwrap();
                let rect = surface
                    .occurrence_rect("card-1", w.round() as u32, height.round() as u32)
                    .unwrap();
                let a = surface
                    .scene_point(0.0, 0.0, w.round() as u32, height.round() as u32)
                    .unwrap();
                let b = surface
                    .scene_point(1.0, 0.0, w.round() as u32, height.round() as u32)
                    .unwrap();
                (
                    (
                        x + rect.x + rect.width / 2.0,
                        y + rect.y + rect.height / 2.0,
                    ),
                    47.0 / (b.x - a.x),
                )
            };
            let owner = h.state().working_sets.active_id;
            h.press_at(at.0, at.1);
            assert_eq!(
                h.pointer_capture(),
                Some(node),
                "width={width}, point={at:?}"
            );
            h.move_to(at.0 + beat, at.1);
            h.relayout();
            assert_eq!(
                h.state()
                    .arrangement_editor
                    .field(owner, "card-1-start")
                    .unwrap()
                    .text(),
                "0"
            );
            // Escape must reach the focused scene while capture is still active.
            h.key_named(NamedKey::Escape);
            h.release_at(at.0 + beat, at.1);
            h.relayout();
            assert!(!h.state().arrangement_editor.dirty());
            // A status line or linked reading can change scroll/layout after
            // cancellation. Resolve the painted coordinates for each gesture.
            let (next_x, next_y, _, _) = h.painted_rect(node).unwrap();
            let at = (at.0 + next_x - x, at.1 + next_y - y);
            h.press_at(at.0, at.1);
            assert!(
                h.state().arrangement_editor.preview_active(),
                "second down width={width} at={at:?}, rect={:?}",
                h.painted_rect(node)
            );
            h.move_to(at.0 + beat, at.1);
            h.relayout();
            assert!(
                h.state().arrangement_editor.preview_active(),
                "second move width={width} capture={:?}",
                h.pointer_capture()
            );
            h.release_at(at.0 + beat, at.1);
            h.relayout();
            assert_eq!(
                h.state()
                    .arrangement_editor
                    .field(owner, "card-1-start")
                    .unwrap()
                    .text(),
                "4"
            );
            h.key_named(NamedKey::ArrowRight);
            assert_eq!(
                h.state()
                    .arrangement_editor
                    .field(owner, "card-1-start")
                    .unwrap()
                    .text(),
                "8"
            );
            h.set_modifiers(Modifiers {
                shift: true,
                ..Default::default()
            });
            h.key_named(NamedKey::ArrowRight);
            assert_eq!(
                h.state()
                    .arrangement_editor
                    .field(owner, "card-1-length")
                    .unwrap()
                    .text(),
                "20"
            );
            h.set_modifiers(Modifiers {
                alt: true,
                ..Default::default()
            });
            h.key_named(NamedKey::ArrowLeft);
            assert_eq!(
                h.state()
                    .arrangement_editor
                    .field(owner, "card-1-start")
                    .unwrap()
                    .text(),
                "4"
            );
            assert_eq!(
                h.state()
                    .arrangement_editor
                    .field(owner, "card-1-length")
                    .unwrap()
                    .text(),
                "24"
            );
            h.set_modifiers(Default::default());
            let move_selector = Selector::class("arrangement-span-edit").containing("Move earlier");
            let move_node = h.with_dom(|dom| taproot::matching_classes(dom, &move_selector)[0]);
            assert!(h.click_on(&move_selector));
            assert_eq!(
                h.state()
                    .arrangement_editor
                    .field(owner, "card-1-start")
                    .unwrap()
                    .text(),
                "0",
                "width={width} button={:?} visible={:?} focus={:?} selected={:?} notice={:?}",
                h.painted_rect(move_node),
                h.visible_rect(move_node),
                h.focus(),
                h.state()
                    .arrangement_editor
                    .surfaces
                    .borrow()
                    .primary
                    .as_ref()
                    .and_then(|s| s.selected())
                    .map(str::to_owned),
                h.state().arrangement_editor.notice
            );
            // Save is still explicit and leaves the original Card instructions alone.
            assert_eq!(
                h.state()
                    .set
                    .progression_arrangement
                    .as_ref()
                    .unwrap()
                    .alignment
                    .cards[0]
                    .placement
                    .as_ref()
                    .unwrap()
                    .span
                    .start,
                0
            );
            assert!(h.click_on(&Selector::class("arrangement-save")));
            assert_eq!(
                serde_json::to_value(&h.state().set.cards).unwrap(),
                original_cards
            );
            assert!(!h.state().arrangement_editor.dirty());
        }
    }
    #[test]
    fn native_arrangement_layout_keeps_preview_in_wide_and_narrow_viewports() {
        for width in [1180.0, 420.0] {
            let mut ui = UiState::new();
            seed_example(&mut ui);
            let initial_width = if width < 1000.0 { 1180.0 } else { width };
            ui.set_viewport_width(initial_width);
            ui.activate_workspace_panel(WorkspacePanel::Arrangement);
            ui.start_arrangement();
            ui.place_arrangement_cards_in_order();
            let sheet = crate::appearance::stylesheet(&ui);
            let logic: crate::sync::Logic = Box::new(woodshed_views::stage::stage_root);
            let mut h = cambium_genet_winit_host::Harness::new(sheet, ui, logic);
            h.layout_at(initial_width, 1100.0);
            if width != initial_width {
                h.update(|ui| {
                    ui.set_viewport_width(width);
                });
                h.layout_at(width, 1100.0);
            }
            let dom = h.runner().dom();
            let dom = dom.borrow();
            let nodes = taproot::matching_classes(&dom, &Selector::class("arrangement-preview"));
            assert_eq!(nodes.len(), 1);
            let (_, _, w, _) = h.painted_rect(nodes[0]).unwrap();
            assert!(w <= width, "preview {w} exceeds viewport {width}");
            drop(dom);

            let leaf_selector = Selector::class("arrangement-scene");
            let viewport_selector = Selector::class("arrangement-scene-scroll");
            assert!(h.click_on(&leaf_selector), "scene leaf should scroll into view at {width}");
            let (leaf, viewport) = h.with_dom(|dom| {
                (
                    taproot::matching_classes(dom, &leaf_selector)[0],
                    taproot::matching_classes(dom, &viewport_selector)[0],
                )
            });
            let leaf_rect = h.painted_rect(leaf).unwrap();
            let viewport_rect = h.painted_rect(viewport).unwrap();
            let leaf_visible = h.visible_rect(leaf).unwrap();
            assert_eq!(leaf_rect.3, 300.0, "leaf keeps its declared height at {width}");
            assert_eq!(
                viewport_rect.3, 300.0,
                "scene viewport must not collapse at {width}: {viewport_rect:?}"
            );
            assert_eq!(
                leaf_visible.3, 300.0,
                "full scene leaf should be vertically visible at {width}: {leaf_visible:?}"
            );
        }
    }
}
