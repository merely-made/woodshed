use super::*;
use cambium_genet_winit_host::Harness;
use dramatis::roster::RosterEntry;
use personae::ProfileId;
use taproot::Selector;
use winit::keyboard::NamedKey;
use woodshed_views::stage::{UiChild, UiState};

fn entry(id: &str, slots: usize, chosen: bool) -> RosterEntry {
    RosterEntry {
        id: ProfileId(id.into()),
        display_name: id.into(),
        slot_count: slots,
        chosen,
    }
}

fn roster_of(entries: Vec<RosterEntry>) -> Roster {
    let chosen = entries
        .iter()
        .find(|entry| entry.chosen)
        .map(|entry| entry.id.clone())
        .unwrap_or(ProfileId("default".into()));
    Roster {
        entries,
        chosen,
        description: "held by djinn".into(),
    }
}

#[test]
fn two_personas_and_no_choice_is_the_one_case_worth_asking_about() {
    let roster = startup_question(Some(roster_of(vec![
        entry("alt", 0, false),
        entry("work", 2, false),
    ])))
    .expect("the gate must open");
    assert_eq!(roster.entries.len(), 2);
    assert!(
        !roster.description.is_empty(),
        "djinn says what protects it"
    );
}

#[test]
fn a_sole_persona_is_not_a_question() {
    assert!(startup_question(Some(roster_of(vec![entry("only", 1, false)]))).is_none());
}

#[test]
fn a_chosen_persona_is_not_asked_again() {
    assert!(
        startup_question(Some(roster_of(vec![
            entry("alt", 0, true),
            entry("work", 2, false),
        ])))
        .is_none()
    );
}

#[test]
fn djinn_absent_is_stepped_over_rather_than_asked_about() {
    // Pending is not a question: the store opens pending and says so.
    assert!(startup_question(None).is_none());
}

/// A harness over the real product root, so what is asserted is the DOM the
/// shipping build lays out — not a view fn called in isolation.
fn gated_harness(roster: Roster) -> Harness<UiState, crate::sync::Logic, UiChild> {
    let mut ui = UiState::new();
    ui.persona = Some(PersonaPick::new(roster));
    let logic: crate::sync::Logic = Box::new(woodshed_views::stage::stage_root);
    // The shipping Escape policy, not a copy of it. Everything else is
    // inert: this harness is about the gate, not the audio seam.
    let hooks = cambium_genet_winit_host::HostHooks {
        key_intercept: Box::new(crate::escape_policy),
        ..cambium_genet_winit_host::inert_hooks()
    };
    let mut harness = Harness::with_hooks(
        cambium_genet_winit_host::Init {
            state: ui,
            logic,
            sheet: woodshed_views::theme::slate_stage_css(),
            fonts: Vec::new(),
            images: Vec::new(),
        },
        hooks,
    );
    harness.layout_at(1_100.0, 664.0);
    harness
}

fn two_persona_roster() -> Roster {
    roster_of(vec![entry("alt", 0, false), entry("work", 2, false)])
}

#[test]
fn the_gate_is_reachable_by_a_driver() {
    // The standing requirement for any new surface: taproot must be
    // able to find it through identity the DOM carries.
    let harness = gated_harness(two_persona_roster());
    assert!(
        harness
            .resolve(&Selector::class("command-picker"))
            .is_some(),
        "the picker itself must resolve"
    );
    assert!(
        harness
            .resolve(&Selector::role("dialog").containing("Choose a persona"))
            .is_some(),
        "the gate announces itself as a dialog with a label"
    );
    assert!(
        harness
            .resolve(&Selector::class("command-item").with_attr("data-key", "work"))
            .is_some(),
        "each persona is addressable by its id, not by the name it shows"
    );
}

#[test]
fn two_personas_sharing_a_name_are_still_told_apart() {
    // Why the rows carry a key at all. Display names are the user's and
    // need not be unique; the id is what `settle` opens the store on, so it
    // is what a driver has to be able to aim at.
    let twins = Roster {
        entries: vec![
            RosterEntry {
                id: ProfileId("work-laptop".into()),
                display_name: "Work".into(),
                slot_count: 2,
                chosen: false,
            },
            RosterEntry {
                id: ProfileId("work-studio".into()),
                display_name: "Work".into(),
                slot_count: 1,
                chosen: false,
            },
        ],
        chosen: ProfileId("work-laptop".into()),
        description: "test vault".into(),
    };

    fn chose(harness: &Harness<UiState, crate::sync::Logic, UiChild>) -> Option<PickerEvent> {
        harness
            .state()
            .persona
            .as_ref()
            .and_then(|pick| pick.outcome.clone())
    }

    // The route this replaces. Both twins answer to the same label, and a
    // driver gets whichever one comes first: the other is unreachable.
    let mut by_label = gated_harness(twins.clone());
    assert!(by_label.click_on(&Selector::class("command-label").containing("Work")));
    assert_eq!(
        chose(&by_label),
        Some(PickerEvent::Chose(ProfileId("work-laptop".into()))),
        "by name, the second twin cannot be reached at all"
    );

    // By key, each one resolves on its own.
    let mut by_key = gated_harness(twins);
    assert!(by_key.click_on(&Selector::class("command-item").with_attr("data-key", "work-studio")));
    assert_eq!(
        chose(&by_key),
        Some(PickerEvent::Chose(ProfileId("work-studio".into()))),
        "and it is the one that was asked for, not its namesake"
    );
}

#[test]
fn the_gate_stands_in_front_of_the_product_rather_than_over_it() {
    let harness = gated_harness(two_persona_roster());
    assert!(
        harness.resolve(&Selector::class("pills")).is_none(),
        "no product navigation while the session behind it is unread"
    );
}

#[test]
fn clicking_a_persona_records_the_choice_the_host_acts_on() {
    let mut harness = gated_harness(two_persona_roster());
    assert!(
        harness.click_on(&Selector::class("command-item").with_attr("data-key", "work")),
        "the row must be clickable where the driver found it"
    );
    let pick = harness
        .state()
        .persona
        .as_ref()
        .expect("the gate is still up");
    assert_eq!(
        pick.outcome,
        Some(PickerEvent::Chose(ProfileId("work".into()))),
        "the answer is recorded by id, for `settle` to open the store on"
    );
}

#[test]
fn asking_for_a_new_persona_keeps_the_gate_up_and_says_why() {
    let mut harness = gated_harness(two_persona_roster());
    // The create row's key is the picker's sentinel, which carries a NUL so
    // no profile id can collide with it. Matched on the readable tail.
    assert!(
        harness.click_on(
            &Selector::class("command-item").with_attr("data-key", "persona-picker:create")
        )
    );
    let pick = harness.state().persona.as_ref().expect("the gate stays up");
    assert!(pick.outcome.is_none(), "nothing for the host to settle");
    assert!(
        harness.resolve(&Selector::role("status")).is_some(),
        "the notice is on screen, not only in the state"
    );
}

#[test]
fn the_gate_takes_the_keyboard_without_a_tab() {
    // A startup gate is the whole window, so the picker asks for the caret
    // as it appears. Asserted as behaviour rather than as "which node has
    // focus": what matters is that the arrows and Enter do something on the
    // first press, with no Tab in front of them.
    let mut harness = gated_harness(two_persona_roster());
    assert!(
        harness.focus().is_some(),
        "the picker took the caret unasked"
    );

    // Rows are sorted by id, so selection starts on `alt` and one step down
    // is `work`.
    harness.key_named(NamedKey::ArrowDown);
    harness.key_named(NamedKey::Enter);
    harness.relayout();

    let pick = harness
        .state()
        .persona
        .as_ref()
        .expect("the host clears it");
    assert_eq!(
        pick.outcome,
        Some(PickerEvent::Chose(ProfileId("work".into()))),
        "arrow then Enter chose the second persona, cold"
    );
}

#[test]
fn escape_dismisses_the_gate_so_practice_is_never_blocked() {
    // "Picking nobody must not block practice." Escape has to reach the
    // picker's own key handler, which means the gate has to be focusable
    // and reachable by the host's Tab traversal from a cold start.
    let mut harness = gated_harness(two_persona_roster());
    harness.key_named(NamedKey::Escape);
    harness.relayout();
    let pick = harness
        .state()
        .persona
        .as_ref()
        .expect("the host clears it, not the view");
    assert_eq!(
        pick.outcome,
        Some(PickerEvent::Dismissed),
        "the first Escape declines, through the window-wide policy"
    );
}

/// The switch gate is raised by a Settings row, not by the startup rule,
/// so it must offer every persona even when the startup gate stays down.
#[test]
fn the_settings_row_asks_even_when_the_startup_rule_would_not() {
    let roster = roster_of(vec![entry("alt", 0, true), entry("work", 2, false)]);
    assert!(
        startup_question(Some(roster.clone())).is_none(),
        "a chosen persona is not a startup question"
    );
    assert_eq!(
        PersonaPick::switch(roster).roster.entries.len(),
        2,
        "both personas are offered to switch to"
    );
}

/// The hazard P2 introduces that P1 could not have: at startup the state
/// behind the gate is empty, but a switch happens over a loaded session,
/// and `restore` returns early when the incoming store holds nothing.
/// Without the reset, the next save would write the outgoing persona's
/// practice into the incoming persona's store.
#[test]
fn switching_into_an_unused_persona_does_not_carry_the_last_one_in() {
    use woodshed_views::stage::UiState;

    let mut ui = UiState::new();
    ui.song.name = "the previous persona's song".into();

    // An empty store stands in for a persona who has never practised;
    // `restore` returns early on it, leaving the state exactly as found.
    let unused: woodshed_core::storage::SessionStore<crate::storage::HostBackend> =
        woodshed_core::storage::SessionStore::new(Box::new(muniment::MemoryBackend::default()));

    // Restoring without the reset keeps the outgoing persona's practice.
    crate::session::restore(&unused, &mut ui);
    assert_eq!(
        ui.song.name, "the previous persona's song",
        "the hazard is real: an empty store restores nothing over what is there"
    );

    // What `settle` does for a switch, in the order it does it.
    ui = UiState::new();
    crate::session::restore(&unused, &mut ui);
    assert_ne!(
        ui.song.name, "the previous persona's song",
        "a fresh persona opens on its own empty practice"
    );
}

#[test]
fn declining_is_not_a_dead_end_the_settings_switch_starts_saving() {
    // The coupling between P1's decline and P2's switch, which nothing else
    // covers: `settle` resets the whole state for a switch, and the reset
    // is what turns saving back on. A declined session must be able to
    // adopt a persona from Settings rather than needing a restart.
    let mut ui = UiState::new();
    woodshed_views::persona::practise_unsaved(&mut ui);
    assert!(!ui.practice_saved);

    // What `settle` does on the switch path, before it opens the store.
    ui = UiState::new();
    assert!(
        ui.practice_saved,
        "a session that has just adopted a persona saves again"
    );
}

#[test]
fn the_unsaved_notice_is_on_screen_for_a_declined_session() {
    let mut harness = gated_harness(two_persona_roster());
    assert!(
        harness.resolve(&Selector::class("unsaved")).is_none(),
        "nothing to say while the gate is still up"
    );
    harness.update(woodshed_views::persona::practise_unsaved);
    assert!(
        harness.resolve(&Selector::class("pills")).is_some(),
        "the product is reachable without a persona"
    );
    assert!(
        harness
            .resolve(&Selector::role("status").containing("not being saved"))
            .is_some(),
        "and it says, for the rest of the session, that nothing is kept"
    );
}

#[test]
fn a_pending_store_reads_but_never_writes() {
    // D12: with djinn absent the store is pending; what it would write is
    // dropped rather than saved in the clear.
    let seal = woodshed_views::persona::PracticeSeal::Pending {
        reason: "djinn is not running".into(),
    };
    assert!(seal.summary().contains("not saved"));
}

#[path = "tests/installation.rs"]
mod installation;
