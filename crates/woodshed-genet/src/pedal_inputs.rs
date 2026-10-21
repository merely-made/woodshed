//! Keyboard/controller admission to the existing pedal dispatcher and writer.
use crate::{
    shared::Shared,
    sync::{Ctx, Logic},
};
use cambium_genet_winit_host::{Key, KeyPress, NamedKey, Runner};
use layout_dom_api::{LayoutDom as _, LocalName, Namespace};
use woodshed_core::midi::{MidiBackend as _, PedalAction, PedalShortcut};
use woodshed_views::stage::{UiChild, UiState};

fn shortcut(press: &KeyPress) -> Option<PedalShortcut> {
    // Preserve native editing, window and application command chords.
    if press.modifiers.ctrl || press.modifiers.meta {
        return None;
    }
    let key = match &press.key {
        Key::Character(text)
            if text.len() == 1 && text.bytes().all(|b| b.is_ascii_alphanumeric()) =>
        {
            text.to_ascii_lowercase()
        },
        Key::Named(key) => match key {
            NamedKey::ArrowLeft => "ArrowLeft",
            NamedKey::ArrowRight => "ArrowRight",
            NamedKey::ArrowUp => "ArrowUp",
            NamedKey::ArrowDown => "ArrowDown",
            NamedKey::PageUp => "PageUp",
            NamedKey::PageDown => "PageDown",
            NamedKey::Home => "Home",
            NamedKey::End => "End",
            _ => return None,
        }
        .into(),
        _ => return None,
    };
    Some(PedalShortcut {
        key,
        shift: press.modifiers.shift,
        alt: press.modifiers.alt,
    })
}
fn editing(runner: &Runner<UiState, Logic, UiChild>) -> bool {
    let handle = runner.dom();
    let dom = handle.borrow();
    let mut node = runner.focus();
    while let Some(current) = node {
        let attr =
            |name: &str| dom.attribute(current, &Namespace::from(""), &LocalName::from(name));
        if matches!(
            attr("role"),
            Some("textbox" | "combobox" | "menu" | "menuitem")
        ) || attr("contenteditable").is_some_and(|v| v != "false")
            || dom
                .element_name(current)
                .is_some_and(|name| matches!(name.local.as_ref(), "input" | "textarea" | "select"))
        {
            return true;
        }
        node = dom.parent(current);
    }
    false
}
pub(crate) fn keyboard(runner: &mut Runner<UiState, Logic, UiChild>, press: &KeyPress) -> bool {
    if runner.state().persona.is_some() || runner.state().practice_close_pending {
        return false;
    }
    if editing(runner) {
        if runner.state().midi.learn_key.is_some() {
            runner.update(|ui| {
                ui.midi.learn_key = None;
                ui.midi.input_notice =
                    Some("Keyboard learning cancelled while editing text".into());
            });
        }
        return false;
    }
    if let Some(action) = runner.state().midi.learn_key {
        if matches!(press.key, Key::Named(NamedKey::Escape)) {
            runner.update(|ui| {
                ui.midi.disarm_inputs();
                ui.midi.input_notice = Some("Learning cancelled".into());
            });
            return true;
        }
        if press.modifiers.ctrl || press.modifiers.meta {
            return false;
        }
        if press.repeat {
            return true;
        }
        let Some(key) = shortcut(press) else {
            runner.update(|ui| {
                ui.midi.input_notice = Some(
                    "Choose a letter/digit, arrow, Page Up/Down, Home or End; Shift/Alt optional"
                        .into(),
                )
            });
            return true;
        };
        runner.update(|ui| {
            if PedalAction::ALL.into_iter().any(|other| {
                other != action
                    && ui
                        .app_settings
                        .audio_midi
                        .midi_pedal
                        .inputs
                        .get(other)
                        .keyboard
                        .as_ref()
                        == Some(&key)
            }) {
                ui.midi.input_notice =
                    Some("That shortcut belongs to the other pedal action; choose another".into());
                return;
            }
            ui.midi.disarm_inputs();
            ui.midi.input_notice = Some(format!(
                "{} shortcut: {}. Inputs remain disarmed.",
                action.label(),
                key.label()
            ));
            ui.app_settings
                .audio_midi
                .midi_pedal
                .inputs
                .get_mut(action)
                .keyboard = Some(key);
        });
        return true;
    }
    if !runner.state().midi.inputs_armed {
        return false;
    }
    let Some(key) = shortcut(press) else {
        return false;
    };
    match runner
        .state()
        .app_settings
        .audio_midi
        .midi_pedal
        .inputs
        .keyboard_action(&key)
    {
        Ok(Some(action)) => {
            let target = runner.state().midi.pedal_target;
            woodshed_audio::midi::diagnostics::record(
                "keyboard-input",
                None,
                target.map(|t| t.revision),
                target.and_then(|t| t.generation),
                &[],
                || {
                    format!(
                        "key={}; repeat={}; action={action:?}",
                        key.label(),
                        press.repeat
                    )
                },
            );
            // Fresh key-down only. The shared native host retains OS repeat
            // metadata; releasing the key permits the next fresh press.
            if !press.repeat {
                runner.update(|ui| ui.midi.request_from_input(action));
            }
            true
        },
        Err(error) => {
            runner.update(|ui| {
                ui.midi.disarm_inputs();
                ui.midi.input_notice = Some(error.into());
            });
            true
        },
        Ok(None) => false,
    }
}

/// Publish arming/configuration before persistence or rendering can block the
/// UI. Only the existing callback-side input owner interprets live MIDI edges.
pub(crate) fn sync(shared: &mut Shared, ui: &UiState) {
    let selected = |ports: &[String], index: usize| {
        index
            .checked_sub(1)
            .and_then(|index| ports.get(index))
            .cloned()
    };
    let armed = ui.midi.inputs_armed
        && ui.persona.is_none()
        && !ui.practice_close_pending
        && selected(&ui.midi.input_ports, ui.midi.input_dd.selected) == ui.midi.connected_in
        && selected(&ui.midi.output_ports, ui.midi.output_dd.selected) == ui.midi.connected_out;
    shared.midi.configure_pedal_inputs(
        &ui.app_settings.audio_midi.midi_pedal,
        armed,
        ui.midi.pedal_target,
    );
}

/// Establish the learning cursor at the click, before persistence/rendering.
/// Flushing on the next frame would also discard the user's first new press.
pub(crate) fn after_dispatch(shared: &mut Shared, ctx: &mut Ctx<'_>) {
    sync(shared, ctx.runner.state());
    if ctx.runner.state().midi.learn_controller_flush {
        let batch = shared.midi.pedal_input_events();
        ctx.runner.update(|ui| {
            ui.midi.learn_controller_flush = false;
            if batch.invalidated {
                ui.midi.disarm_inputs();
                ui.midi.input_notice = Some(batch.notice.unwrap_or_else(|| {
                    "Controller connection changed; start learning again".into()
                }));
            }
        });
        sync(shared, ctx.runner.state());
    }
}

/// Drain preparation/display history, including while unarmed. Only the
/// callback owns live controller actions; this path can never replay history.
pub(crate) fn poll(shared: &mut Shared, ctx: &mut Ctx<'_>) -> bool {
    sync(shared, ctx.runner.state());
    let batch = shared.midi.pedal_input_events();
    // Keep consuming releases while disarmed too. Otherwise the host can go
    // idle after Learn and leave a controller's release in old display history.
    let active = shared.midi.connected_input().is_some();
    if batch.events.is_empty()
        && !batch.invalidated
        && batch.notice.is_none()
        && !ctx.runner.state().midi.learn_controller_flush
    {
        return active;
    }
    let mut changed = false;
    ctx.runner.update(|ui| {
        if batch.invalidated {
            ui.midi.disarm_inputs();
            ui.midi.input_notice = Some(
                batch
                    .notice
                    .unwrap_or_else(|| "Controller connection changed; inputs disarmed".into()),
            );
            return;
        }
        if std::mem::take(&mut ui.midi.learn_controller_flush) {
            return;
        }
        if let Some(notice) = batch.notice {
            ui.midi.input_notice = Some(notice);
        }
        if ui.persona.is_some() || ui.practice_close_pending {
            ui.midi.disarm_inputs();
            return;
        }
        for event in batch.events {
            if let Some(action) = ui.midi.learn_controller {
                if event.pressed() {
                    if PedalAction::ALL.into_iter().any(|other| {
                        other != action
                            && ui
                                .app_settings
                                .audio_midi
                                .midi_pedal
                                .inputs
                                .get(other)
                                .controller
                                == Some(event.binding)
                    }) {
                        ui.midi.input_notice = Some(
                            "That MIDI control belongs to the other action; choose another".into(),
                        );
                        continue;
                    }
                    ui.app_settings
                        .audio_midi
                        .midi_pedal
                        .inputs
                        .get_mut(action)
                        .controller = Some(event.binding);
                    ui.midi.disarm_inputs();
                    ui.midi.input_notice = Some(format!(
                        "{} controller: {}. Release it, then arm inputs.",
                        action.label(),
                        event.binding.label()
                    ));
                    changed = true;
                }
                continue;
            }
        }
    });
    // The existing dispatch tail owns settings persistence. A controller-learn
    // completion can arrive in a frame without a keyboard/pointer dispatch.
    if changed {
        crate::sync::after_dispatch(shared, ctx);
    }
    sync(shared, ctx.runner.state());
    active
}

#[cfg(test)]
mod tests {
    use super::*;
    use cambium_genet_winit_host::{Harness, HostHooks, HostOptions, Init};
    use woodshed_core::midi::PedalTarget;
    fn host() -> Harness<UiState, Logic, UiChild> {
        let mut ui = UiState::new();
        ui.section = woodshed_core::storage::AppSection::Settings;
        ui.app_settings.page = woodshed_core::settings::SettingsPage::AudioMidi;
        ui.midi.pedal_target = Some(PedalTarget {
            revision: 1,
            generation: Some(7),
        });
        ui.midi.inputs_armed = true;
        ui.app_settings
            .audio_midi
            .midi_pedal
            .inputs
            .effect_next
            .keyboard = Some(PedalShortcut {
            key: "n".into(),
            alt: false,
            shift: false,
        });
        let mut host = Harness::with_command_init(
            move |commands| {
                let commands = commands.clone();
                Init {
                    sheet: crate::appearance::stylesheet(&ui),
                    state: ui,
                    logic: Box::new(move |ui: &UiState| crate::desktop_root(ui, &commands))
                        as Logic,
                    fonts: vec![],
                    images: vec![],
                }
            },
            HostHooks {
                key_intercept: Box::new(crate::escape_policy),
                ..cambium_genet_winit_host::inert_hooks()
            },
            HostOptions::default(),
        );
        host.layout_at(1180.0, 1400.0);
        host
    }
    #[test]
    fn held_shortcut_ignores_repeat_and_fresh_press_keeps_same_target() {
        let mut host = host();
        host.key_char("n");
        assert_eq!(
            host.state().midi.pedal_request,
            Some(PedalAction::EffectNext)
        );
        assert_eq!(
            host.state().midi.pedal_request_target,
            Some(PedalTarget {
                revision: 1,
                generation: Some(7)
            })
        );
        host.update(|ui| {
            ui.midi.pedal_request = None;
            ui.midi.pedal_request_target = None;
        });
        let mut held = KeyPress::character("n");
        held.repeat = true;
        for _ in 0..64 {
            host.press_key(&held);
            assert!(host.state().midi.pedal_request.is_none());
        }
        held.repeat = false;
        host.press_key(&held);
        assert_eq!(
            host.state().midi.pedal_request,
            Some(PedalAction::EffectNext)
        );
        host.update(|ui| ui.midi.disarm_inputs());
        assert!(host.state().midi.pedal_request.is_none());
        host.press_key(&held);
        assert!(host.state().midi.pedal_request.is_none());
    }
    #[test]
    fn shortcut_learning_captures_without_dispatch_and_rejects_duplicates() {
        let mut host = host();
        host.update(|ui| {
            ui.midi.disarm_inputs();
            ui.midi.learn_key = Some(PedalAction::EffectPrevious);
        });
        host.key_char("n");
        assert!(host.state().midi.learn_key.is_some());
        assert!(host.state().midi.pedal_request.is_none());
        host.key_char("p");
        assert_eq!(
            host.state()
                .app_settings
                .audio_midi
                .midi_pedal
                .inputs
                .effect_previous
                .keyboard
                .as_ref()
                .unwrap()
                .key,
            "p"
        );
        assert!(host.state().midi.learn_key.is_none());
        assert!(!host.state().midi.inputs_armed);
        assert!(host.state().midi.pedal_request.is_none());
    }
    #[test]
    fn command_chords_do_not_become_pedal_shortcuts() {
        let mut press = KeyPress::character("n");
        press.modifiers.ctrl = true;
        assert!(shortcut(&press).is_none());
        press.modifiers.ctrl = false;
        press.modifiers.meta = true;
        assert!(shortcut(&press).is_none());
        assert!(shortcut(&KeyPress::named(NamedKey::Enter)).is_none());
        assert!(shortcut(&KeyPress::named(NamedKey::Tab)).is_none());
    }

    #[test]
    fn actual_search_focus_prevents_shortcuts_and_cancels_learning() {
        let mut host = host();
        assert!(host.click_on(&taproot::Selector::role("textbox")));
        assert!(host.focus().is_some());
        host.key_char("n");
        assert!(host.state().midi.pedal_request.is_none());
        host.update(|ui| {
            ui.midi.disarm_inputs();
            ui.midi.learn_key = Some(PedalAction::EffectNext);
        });
        host.key_char("p");
        assert!(host.state().midi.learn_key.is_none());
        assert!(host.state().midi.pedal_request.is_none());
        assert_eq!(
            host.state()
                .app_settings
                .audio_midi
                .midi_pedal
                .inputs
                .effect_next
                .keyboard
                .as_ref()
                .unwrap()
                .key,
            "n"
        );
    }

    #[test]
    fn actual_mapping_controls_learn_arm_clear_and_binding_edits_disarm() {
        use taproot::Selector;
        let mut host = host();
        assert!(host.click_on(
            &Selector::class("t-btn").with_attr("aria-label", "Effect Previous learn keyboard")
        ));
        assert!(!host.state().midi.inputs_armed);
        host.key_char("p");
        assert!(host.state().midi.pedal_request.is_none());
        // Preparation alone cannot arm a disabled output binding.
        assert!(host.click_on(&Selector::class("t-btn").containing("Arm pedal inputs")));
        assert!(!host.state().midi.inputs_armed);
        host.update(|ui| ui.app_settings.audio_midi.midi_pedal.effect_next.enabled = true);
        assert!(host.click_on(&Selector::class("t-btn").containing("Arm pedal inputs")));
        assert!(host.state().midi.inputs_armed);
        host.key_char("n");
        assert_eq!(
            host.state().midi.pedal_request,
            Some(PedalAction::EffectNext)
        );
        // Clearing one input also cancels any pending input gesture.
        assert!(host.click_on(
            &Selector::class("t-btn").with_attr("aria-label", "Effect Previous clear keyboard")
        ));
        assert!(!host.state().midi.inputs_armed);
        assert!(host.state().midi.pedal_request.is_none());
        assert!(
            host.state()
                .app_settings
                .audio_midi
                .midi_pedal
                .inputs
                .effect_previous
                .keyboard
                .is_none()
        );
        assert!(host.click_on(&Selector::class("t-btn").containing("Arm pedal inputs")));
        assert!(host.state().midi.inputs_armed);
        assert!(host.click_on(
            &Selector::class("neck-step").with_attr("aria-label", "Effect Next Number increase")
        ));
        assert!(!host.state().midi.inputs_armed);
        assert_eq!(
            host.state()
                .app_settings
                .audio_midi
                .midi_pedal
                .effect_next
                .number,
            1
        );
    }
}
