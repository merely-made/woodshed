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
                taproot::matching(dom, &Selector::class("arrangement-error")).len(),
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
    fn native_arrangement_layout_keeps_preview_in_wide_and_narrow_viewports() {
        for width in [1180.0, 420.0] {
            let mut ui = UiState::new();
            seed_example(&mut ui);
            ui.set_viewport_width(width);
            ui.activate_workspace_panel(WorkspacePanel::Arrangement);
            ui.start_arrangement();
            ui.place_arrangement_cards_in_order();
            let sheet = crate::appearance::stylesheet(&ui);
            let logic: crate::sync::Logic = Box::new(woodshed_views::stage::stage_root);
            let mut h = cambium_genet_winit_host::Harness::new(sheet, ui, logic);
            h.layout_at(width, 1100.0);
            let dom = h.runner().dom();
            let dom = dom.borrow();
            let nodes = taproot::matching(&dom, &Selector::class("arrangement-preview"));
            assert_eq!(nodes.len(), 1);
            let (_, _, w, _) = h.painted_rect(nodes[0]).unwrap();
            assert!(w <= width, "preview {w} exceeds viewport {width}");
        }
    }
}
