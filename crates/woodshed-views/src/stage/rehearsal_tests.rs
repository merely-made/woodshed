use super::*;
use crate::stage::{PracticeClockProjection, PracticeClockStatus};
use cambium_genet_winit_host::{Harness, KeyPress, NamedKey};
use taproot::Selector;
use woodshedding::rehearsal::{Hold, LoopMode};

type RehearsalView = fn(&UiState) -> UiChild;
type RehearsalHarness = Harness<UiState, RehearsalView, UiChild>;

fn three_card_set() -> UiState {
    let mut ui = UiState::new();
    ui.stage_current(None);
    let card = ui.set.cards[0].clone();
    ui.set.push(card.clone());
    ui.set.push(card);
    ui
}

fn mount(ui: UiState) -> RehearsalHarness {
    Harness::new(
        crate::theme::slate_stage_css(),
        ui,
        screen as fn(&UiState) -> UiChild,
    )
}

fn focus_class(host: &mut RehearsalHarness, class: &str) {
    for _ in 0..256 {
        let node = host.with_dom(|dom| taproot::matching_classes(dom, &Selector::class(class))[0]);
        if host.focus() == Some(node) {
            return;
        }
        host.press_key(&KeyPress::named(NamedKey::Tab));
    }
    panic!("could not move keyboard focus to .{class}");
}

fn activate_class(host: &mut RehearsalHarness, class: &str, key: NamedKey) {
    focus_class(host, class);
    host.press_key(&KeyPress::named(key));
}

#[test]
fn transport_buttons_are_keyboard_operable_for_run_pause_resume_seek_loop_and_stop() {
    let mut ui = three_card_set();
    ui.set.cursor = 1;
    ui.enable_practice_clock();
    let mut host = mount(ui);
    host.layout_at(900.0, 700.0);

    activate_class(&mut host, "rehearsal-run", NamedKey::Space);
    assert!(host.state().rehearsal_running);
    activate_class(&mut host, "rehearsal-run", NamedKey::Space);
    assert!(host.state().rehearsal_paused);
    activate_class(&mut host, "rehearsal-run", NamedKey::Enter);
    assert!(!host.state().rehearsal_paused);

    activate_class(&mut host, "rehearsal-prev", NamedKey::Enter);
    assert_eq!(
        host.state().practice_clock_commands.clone(),
        [super::super::PracticeClockCommand::Seek(0)]
    );
    host.update(|ui| ui.practice_clock_commands.clear());
    activate_class(&mut host, "rehearsal-next", NamedKey::Space);
    assert_eq!(
        host.state().practice_clock_commands.clone(),
        [super::super::PracticeClockCommand::Advance]
    );

    activate_class(&mut host, "rehearsal-loop-mode", NamedKey::Enter);
    assert_eq!(host.state().set.loop_mode, LoopMode::All);
    activate_class(&mut host, "rehearsal-loop-mode", NamedKey::Enter);
    assert_eq!(host.state().set.loop_mode, LoopMode::Card);
    activate_class(&mut host, "rehearsal-loop-mode", NamedKey::Enter);
    assert_eq!(host.state().set.loop_mode, LoopMode::Off);

    activate_class(&mut host, "rehearsal-stop", NamedKey::Space);
    assert!(!host.state().rehearsal_running);
}

#[test]
fn rename_field_space_and_arrows_do_not_control_rehearsal_or_move_the_card() {
    let mut host = mount(three_card_set());
    host.layout_at(900.0, 700.0);
    assert!(host.click_on(&Selector::role("textbox")));
    host.key_char(" ");
    host.press_key(&KeyPress::named(NamedKey::ArrowLeft));
    host.press_key(&KeyPress::named(NamedKey::ArrowRight));
    assert!(!host.state().rehearsal_running);
    assert_eq!(host.state().set.cursor, 0);
}

#[test]
fn filmstrip_selection_queues_seek_during_managed_run() {
    let mut ui = three_card_set();
    ui.enable_practice_clock();
    ui.toggle_rehearsal();
    let mut host = mount(ui);
    host.layout_at(900.0, 700.0);
    let second = host.with_dom(|dom| taproot::matching_classes(dom, &Selector::class("film-card"))[1]);
    let (x, y, w, h) = host.painted_rect(second).unwrap();
    host.click_at(x + w / 2.0, y + h / 2.0);
    assert_eq!(
        host.state().set.cursor,
        0,
        "the UI waits for the clock seek observation"
    );
    assert_eq!(
        host.state().practice_clock_commands.clone(),
        [super::super::PracticeClockCommand::Seek(1)]
    );
}

#[test]
fn filmstrip_selection_uses_tab_and_enter_without_changing_the_cursor_ahead_of_audio() {
    let mut ui = three_card_set();
    ui.enable_practice_clock();
    ui.toggle_rehearsal();
    let mut host = mount(ui);
    host.layout_at(900.0, 700.0);
    focus_class(&mut host, "film-card");
    host.press_key(&KeyPress::named(NamedKey::Tab));
    host.press_key(&KeyPress::named(NamedKey::Enter));
    assert_eq!(host.state().set.cursor, 0);
    assert_eq!(
        host.state().practice_clock_commands,
        [super::super::PracticeClockCommand::Seek(1)]
    );
}

fn clocked_hold(hold: Hold, status: PracticeClockStatus) -> UiState {
    let mut ui = UiState::new();
    ui.stage_current(None);
    ui.set.cards[0].timing.hold = hold;
    ui.enable_practice_clock();
    ui.toggle_rehearsal();
    let card = &ui.set.cards[0];
    ui.practice_clock_projection = Some(PracticeClockProjection {
        epoch: ui.rehearsal_epoch,
        owner: ui.working_sets.active_id,
        card_id: card.id,
        card_index: 0,
        event_ordinal: None,
        contacts: Vec::new(),
        device_frame: 0,
        active_frames: 96_000,
        sample_rate: 48_000,
        beat_ticks: 1_920,
        ticks_per_beat: 960,
        card_frames: 96_000,
        status,
    });
    ui
}

#[test]
fn progress_uses_matching_projection_and_lowered_hold_units() {
    let beats = clocked_hold(Hold::Bars(2), PracticeClockStatus::Playing);
    let text = progress_text(&beats, &beats.set.cards[0]).unwrap();
    assert_eq!(text.0, "2.0 / 8.0 beats");
    assert!((text.1 - 0.25).abs() < 0.001);
    let mut paused = clocked_hold(Hold::Bars(2), PracticeClockStatus::Playing);
    paused.practice_clock_projection.as_mut().unwrap().status = PracticeClockStatus::Paused;
    assert_eq!(progress_text(&paused, &paused.set.cards[0]).unwrap(), text);

    let seconds = clocked_hold(Hold::Seconds(8.0), PracticeClockStatus::Playing);
    let text = progress_text(&seconds, &seconds.set.cards[0]).unwrap();
    assert!(text.0.contains("2.0 / 8.0s"));
    assert!((text.1 - 0.25).abs() < 0.001);

    let manual = clocked_hold(Hold::Manual, PracticeClockStatus::WaitingManual);
    let text = progress_text(&manual, &manual.set.cards[0]).unwrap();
    assert_eq!(text.0, "Phrase complete · waiting for Next");
    assert_eq!(text.1, 1.0);
    let paused_manual = clocked_hold(Hold::Manual, PracticeClockStatus::Paused);
    assert!(
        progress_text(&paused_manual, &paused_manual.set.cards[0])
            .unwrap()
            .0
            .contains("Resume or Next")
    );

    let mut other_owner = clocked_hold(Hold::Bars(2), PracticeClockStatus::Playing);
    other_owner
        .practice_clock_projection
        .as_mut()
        .unwrap()
        .owner
        .0 += 1;
    assert!(progress_text(&other_owner, &other_owner.set.cards[0]).is_none());
}

#[test]
fn progress_and_context_fit_compact_rehearsal_layout() {
    let mut ui = clocked_hold(Hold::Seconds(8.0), PracticeClockStatus::Playing);
    ui.set.loop_mode = LoopMode::Card;
    let mut host = mount(ui);
    host.layout_at(420.0, 700.0);
    assert!(
        host.resolve(&Selector::class("rehearsal-context"))
            .is_some()
    );
    assert!(
        host.resolve(&Selector::class("rehearsal-progress-track"))
            .is_some()
    );
    assert!(
        host.resolve(&Selector::class("rehearsal-current"))
            .is_some()
    );
    let node =
        host.with_dom(|dom| taproot::matching_classes(dom, &Selector::class("rehearsal-context"))[0]);
    let (x, _, width, _) = host.painted_rect(node).unwrap();
    assert!(
        x >= 0.0 && x + width <= 420.1,
        "runner context must fit the compact viewport"
    );
}

#[test]
fn adjacent_context_matches_explicit_navigation_and_wraps_only_all() {
    assert_eq!(adjacent_card_indices(0, 3, LoopMode::Off), (None, Some(1)));
    assert_eq!(adjacent_card_indices(2, 3, LoopMode::Off), (Some(1), None));
    assert_eq!(
        adjacent_card_indices(0, 3, LoopMode::All),
        (Some(2), Some(1))
    );
    assert_eq!(
        adjacent_card_indices(2, 3, LoopMode::All),
        (Some(1), Some(0))
    );
    assert_eq!(adjacent_card_indices(2, 3, LoopMode::Card), (Some(1), None));
    assert_eq!(adjacent_card_indices(0, 3, LoopMode::Card), (None, Some(1)));
}
