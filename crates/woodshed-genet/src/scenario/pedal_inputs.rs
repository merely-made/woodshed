//! Acceptance-only observations and KeyPress injection at the production hook.
//! This does not claim OS keyboard delivery or synthesize pedal/writer requests.
use crate::{shared::Shared, sync::Ctx};
use cambium_genet_winit_host::KeyPress;
use std::{cell::RefCell, rc::Rc};
use taproot::ProbeSnapshot;
use woodshed_views::stage::UiState;

pub(super) fn observation(ui: &UiState) -> serde_json::Value {
    serde_json::json!({
        "connected":ui.midi.connected_in,
        "selected_index":ui.midi.input_dd.selected,
        "armed":ui.midi.inputs_armed,
        "learn_key":ui.midi.learn_key,
        "learn_controller":ui.midi.learn_controller,
        "notice":ui.midi.input_notice,
        "target":ui.midi.pedal_target,
        "pending":ui.midi.pedal_request,
        "pending_target":ui.midi.pedal_request_target,
    })
}

pub(super) fn snapshot(mut snapshot: ProbeSnapshot, ui: &UiState) -> ProbeSnapshot {
    for (key, value) in observation(ui).as_object().unwrap() {
        snapshot = snapshot.with_field(
            format!("pedal-input-{key}"),
            value
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| value.to_string()),
        );
    }
    snapshot
}

pub(super) fn act(ctx: &mut Ctx<'_>, shared: &Rc<RefCell<Shared>>, label: &str) -> Option<bool> {
    let rest = label.strip_prefix("pedal-input-key ")?;
    // Small explicit vocabulary; the production interceptor interprets keys,
    // modifiers and repeat state. No action/binding lookup is repeated here.
    let mut parts = rest.split_whitespace();
    let text = parts.next()?;
    if text.len() != 1 || !text.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return Some(false);
    }
    let mut press = KeyPress::character(text);
    for option in parts {
        match option {
            "repeat" => press.repeat = true,
            "shift" => press.modifiers.shift = true,
            "alt" => press.modifiers.alt = true,
            "ctrl" => press.modifiers.ctrl = true,
            "meta" => press.modifiers.meta = true,
            _ => return Some(false),
        }
    }
    if crate::escape_policy(ctx.runner, &press) {
        // This is the same consumed-key tail the native host invokes. Mesquite
        // text injection bypasses that hook and would not test shortcuts.
        crate::sync::after_dispatch(&mut shared.borrow_mut(), ctx);
    }
    Some(true)
}
