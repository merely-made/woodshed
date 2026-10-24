//! Read-only material summary for a Card occurrence opened from navigation.

use super::UiChild;
use cambium::{el, text};
use woodshedding::rehearsal::{Card, CardId, Material};
use woodshed_core::harmony::KeyedCatalogRef;
use woodshed_core::working_sets::WorkingSetId;

pub(super) fn view(owner: WorkingSetId, owner_name: &str, card: &Card) -> UiChild {
    let material = KeyedCatalogRef::from_card(card)
        .and_then(|reference| reference.label())
        .unwrap_or_else(|| match &card.material {
            Material::Riff { name } => format!("Riff · {name}"),
            Material::Path { positions, .. } => format!("Path · {} notes", positions.len()),
            other => other.tag().to_owned(),
        });
    let label = if card.label.trim().is_empty() {
        "Untitled Card"
    } else {
        &card.label
    };

    Box::new(
        el(
            "section",
            (
                el("span", text(format!("Card context · {owner_name}")))
                    .attr("class", "stage-card-context-owner"),
                el("strong", text(label.to_owned())).attr("class", "stage-card-context-title"),
                el("span", text(material)).attr("class", "stage-card-context-material"),
            ),
        )
        .attr("class", "stage-card-context")
        .attr("role", "region")
        .attr(
            "aria-label",
            format!("Card context: {label}, in {owner_name}"),
        )
        .attr("data-working-set-id", owner.0.to_string())
        .attr("data-card-id", card.id.0.to_string())
        .attr(
            "style",
            "display:flex;align-items:baseline;gap:8px 14px;flex-wrap:wrap;padding:5px 12px;margin:0 0 8px;border-left:3px solid currentColor;",
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{
        UiState, UiChild, WorkspacePanel, card_context_view, contextual_navigation::WoodshedScopePlace,
    };
    use cambium::el;
    use cambium_genet_winit_host::Harness;
    use taproot::Selector;
    use woodshed_core::Lens;
    use woodshed_core::working_sets::WorkingSetId;

    fn context(ui: &UiState) -> UiChild {
        card_context_view(ui).unwrap_or_else(|| Box::new(el("div", ())))
    }

    fn assert_context(
        host: &mut Harness<UiState, fn(&UiState) -> UiChild, UiChild>,
        owner: WorkingSetId,
        card_id: CardId,
        owner_name: &str,
        authored_label: &str,
        material_label: &str,
    ) {
        host.layout_at(760.0, 640.0);
        assert!(host
            .resolve(
                &Selector::role("region")
                    .with_attr("aria-label", format!("Card context: {authored_label}, in {owner_name}"))
                    .with_attr("data-working-set-id", owner.0.to_string())
                    .with_attr("data-card-id", card_id.0.to_string())
            )
            .is_some());
        assert!(host
            .resolve(&Selector::class("stage-card-context"))
            .is_some());
        assert!(host
            .resolve(&Selector::class("stage-card-context-title").containing(authored_label))
            .is_some());
        assert!(host
            .resolve(&Selector::class("stage-card-context-owner").containing(owner_name))
            .is_some());
        assert!(host
            .resolve(&Selector::class("stage-card-context-material").containing(material_label))
            .is_some());
        assert!(host.resolve(&Selector::role("button")).is_none());
        assert!(host.resolve(&Selector::role("region")).is_some());
    }

    #[test]
    fn owner_qualified_card_context_stays_readonly_and_separate_from_catalog() {
        let mut ui = UiState::new();
        ui.stage.set_lens(Lens::Chords);
        ui.stage_current(None);
        let card_id = ui.set.cards[0].id;
        ui.set.cards[0].label = "First authored chord".into();
        let first_owner = ui.working_sets.active_id;
        let first_material = KeyedCatalogRef::from_card(&ui.set.cards[0])
            .and_then(|reference| reference.label())
            .unwrap();

        // The independent Set owner can retain the same local Card ID and a
        // different authored label. The address must resolve through owner.
        let mut second_set = ui.set.clone();
        second_set.cards[0].label = "Second authored chord".into();
        let second_owner = ui.create_working_set_from("Second owner", second_set);
        assert_eq!(
            ui.working_sets.get(second_owner, &ui.set).unwrap().cards[0].id,
            card_id
        );
        ui.stage.set_lens(Lens::Arpeggios);

        assert!(ui.visit_scope(
            WoodshedScopePlace::CardOccurrence {
                owner: first_owner,
                card: card_id,
            },
            None,
        ));
        let mut host = Harness::new("", ui, context as fn(&UiState) -> UiChild);
        let before_first_render = serde_json::to_value(host.state().to_persisted()).unwrap();
        assert_context(
            &mut host,
            first_owner,
            card_id,
            "Working Set 1",
            "First authored chord",
            &first_material,
        );
        assert_eq!(
            serde_json::to_value(host.state().to_persisted()).unwrap(),
            before_first_render,
            "rendering the occurrence summary leaves persisted product state unchanged"
        );
        assert_eq!(host.state().stage.lens, Lens::Arpeggios);

        host.update(|ui| {
            assert!(ui.visit_scope(
                WoodshedScopePlace::CardOccurrence {
                    owner: second_owner,
                    card: card_id,
                },
                None,
            ));
        });
        let before_second_render = serde_json::to_value(host.state().to_persisted()).unwrap();
        assert_context(
            &mut host,
            second_owner,
            card_id,
            "Second owner",
            "Second authored chord",
            &first_material,
        );
        assert_eq!(
            serde_json::to_value(host.state().to_persisted()).unwrap(),
            before_second_render,
            "rendering a second owner with the same Card ID also leaves state unchanged"
        );
        assert_eq!(host.state().stage.lens, Lens::Arpeggios);
        assert_eq!(host.state().workspace.active_panel(), Some(WorkspacePanel::Practice));
        assert_eq!(host.state().set.cursor_id(), Some(card_id));
    }
}
