//! The text seam: which field has the caret.
//!
//! The host owns the caret, the selection overlay, IME, drag selection, and
//! visual (layout-aware) caret movement — but it cannot know where an
//! application keeps its text. This is woodshed's half of that seam: recognize
//! the focused `div[role=textbox]` by the class of its wrapper, and hand back borrows of
//! the matching [`TextInput`].

use cambium_genet_winit_host::{FocusedTextSlot, Runner};
use layout_dom_api::{LayoutDom as _, LocalName, Namespace};
use woodshed_views::stage::{UiChild, UiState};

use crate::sync::Logic;

/// Woodshed's editable fields.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    Search,
    CardRename,
    RelationshipLabel,
}

/// Which field the focused node is, if it is one.
pub fn focused_text(runner: &Runner<UiState, Logic, UiChild>) -> Option<FocusedTextSlot<UiState>> {
    let node = runner.focus()?;
    if runner.state().appearance.workshop_open {
        let dom = runner.dom();
        let dom = dom.borrow();
        if let Some(field) =
            dom.attribute(node, &Namespace::from(""), &LocalName::from("data-field"))
        {
            let field = field.to_string();
            runner.state().appearance.workshop.text_field(&field)?;
            let field_mut = field.clone();
            return Some(FocusedTextSlot {
                node,
                get: Box::new(move |ui| {
                    ui.appearance
                        .workshop
                        .text_field(&field)
                        .expect("mounted workshop field")
                }),
                get_mut: Box::new(move |ui| {
                    ui.appearance
                        .workshop
                        .text_field_mut(&field_mut)
                        .expect("mounted workshop field")
                }),
            });
        }
    }
    let field = {
        let dom = runner.dom();
        let dom = dom.borrow();
        // Cambium's single-line fields are `div[role=textbox]` since mere r44.
        let attr = |name: &str| dom.attribute(node, &Namespace::from(""), &LocalName::from(name));
        if attr("role") != Some("textbox") || attr("aria-multiline") == Some("true") {
            return None;
        }
        let parent = dom.parent(node)?;
        match dom.attribute(parent, &Namespace::from(""), &LocalName::from("class"))? {
            "search-wrap" => Field::Search,
            "card-rename" => Field::CardRename,
            "relationship-label" => Field::RelationshipLabel,
            _ => return None,
        }
    };
    Some(match field {
        Field::Search => FocusedTextSlot {
            node,
            get: Box::new(|ui: &UiState| &ui.search),
            get_mut: Box::new(|ui: &mut UiState| &mut ui.search),
        },
        Field::RelationshipLabel => FocusedTextSlot {
            node,
            get: Box::new(|ui: &UiState| &ui.relationship_label),
            get_mut: Box::new(|ui: &mut UiState| &mut ui.relationship_label),
        },
        Field::CardRename => FocusedTextSlot {
            node,
            get: Box::new(|ui: &UiState| &ui.card_rename),
            get_mut: Box::new(|ui: &mut UiState| &mut ui.card_rename),
        },
    })
}
