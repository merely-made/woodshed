//! Whether to ask which persona is practising, and what to do with the answer.
//!
//! The view half is `woodshed_views::persona`. This half owns the two acts a
//! view must not perform: deciding, before the window exists, that the question
//! needs asking at all; and asking djinn to switch to the answer before
//! reopening the practice store under it.
//!
//! **djinn holds the vault** (dramatis DR-C). The roster comes from its
//! custody route and a switch is djinn's act, remembered for the whole family;
//! Woodshed opens no vault of its own.
//!
//! **Nobody is asked who has already answered.** djinn speaks as a persona
//! whenever it is unlocked, so the startup question is left for the one case
//! its roster can still present: several personas, none of them chosen. djinn
//! absent or Locked is not a question either: the store opens pending (D12).

use dramatis::roster::Roster;
use graphshell_endpoint::door::app_admission::AppId;
use graphshell_endpoint::door::custody_client::BlockingCustodyClient;
use persona_picker::PickerEvent;
use woodshed_views::persona::{PendingSwitch, PersonaPick, PickPurpose, SwitchDecision};

use crate::shared::Shared;
use crate::storage::{CUSTODY_APP, open_store_as};
use crate::sync::Ctx;

/// The roster to ask about, or `None` when djinn already decides or cannot
/// be asked.
pub fn pending_roster() -> Option<Roster> {
    startup_question(roster_now().ok())
}

/// The startup gate's rule over a roster djinn answered: several personas and
/// none chosen.
pub fn startup_question(roster: Option<Roster>) -> Option<Roster> {
    let roster = roster?;
    (roster.entries.len() > 1 && !roster.entries.iter().any(|entry| entry.chosen)).then_some(roster)
}

/// The whole roster, from djinn, whatever the startup rule would decide (P2).
/// Answered while Locked too; an error when djinn is absent.
///
/// Asked on a thread of its own: the blocking client owns a runtime, which
/// must not start inside one.
pub fn roster_now() -> Result<Roster, String> {
    std::thread::spawn(|| {
        BlockingCustodyClient::open(AppId::new(CUSTODY_APP))
            .and_then(|mut client| client.roster())
            .map_err(|error| error.to_string())
    })
    .join()
    .unwrap_or_else(|_| Err("the roster request panicked".into()))
}

/// Act on a gate the user has answered, if they have, and raise one if the
/// Settings row asked for it.
///
/// Runs at the head of the dispatch tail so the rest of it (the audio seam, the
/// skin, persistence) sees the restored session in the same beat the choice
/// lands, rather than one frame later.
pub fn after_dispatch(shared: &mut Shared, ctx: &mut Ctx<'_>) {
    if ctx.runner.state().pending_persona_switch.is_some() {
        resume_switch(shared, ctx);
        return;
    }
    // Most dispatches have nothing to do with the persona gate. Avoid a
    // `Runner::update` in that common case: even an empty update rebuilds the
    // retained root, which used to add a full rebuild to every graph-drag
    // sample before the storage/audio tail ran.
    let has_work = {
        let ui = ctx.runner.state();
        ui.persona_switch_requested
            || ui
                .persona
                .as_ref()
                .is_some_and(|pick| pick.outcome.is_some())
    };
    if !has_work {
        return;
    }

    let mut answer = None;
    let mut purpose = PickPurpose::Startup;
    let mut requested = false;
    ctx.runner.update(|ui| {
        requested = std::mem::take(&mut ui.persona_switch_requested);
        if let Some(pick) = ui.persona.as_mut() {
            answer = pick.outcome.take();
            purpose = pick.purpose;
        }
    });
    if requested {
        raise_switch(ctx);
        return;
    }
    let Some(answer) = answer else {
        return;
    };
    let chosen = match answer {
        PickerEvent::Chose(id) => Some(id),
        PickerEvent::Dismissed => match purpose {
            // Escape at startup: practise with no persona at all.
            PickPurpose::Startup => {
                decline(ctx);
                return;
            },
            // Escape on a switch changes nothing: the store that is open stays
            // open, and re-settling on the convention here would quietly move
            // the user off the persona they are already practising as.
            PickPurpose::Switch => {
                ctx.runner.update(|ui| ui.persona = None);
                return;
            },
        },
        // The view answers this one itself and keeps the gate open (P3 wires
        // the create flow), so it never reaches here.
        PickerEvent::CreateRequested => return,
    };
    if purpose == PickPurpose::Switch {
        let Some(profile) = chosen else {
            return;
        };
        ctx.runner.update(|ui| {
            ui.persona = None;
            ui.practice_close_pending = false;
            ui.practice_discard_close = false;
            ui.pending_persona_switch = Some(PendingSwitch {
                profile: profile.clone(),
                decision: Some(SwitchDecision::Retry),
                waiting: false,
                notice: "Saving outgoing practice before switching…".into(),
            });
        });
        resume_switch(shared, ctx);
    } else {
        settle(shared, ctx, chosen.as_ref(), purpose);
    }
}

/// Retry only an explicit decision or a pending worker completion. The outgoing
/// store, audio epoch and authored state remain installed until this succeeds.
pub(crate) fn resume_switch(shared: &mut Shared, ctx: &mut Ctx<'_>) {
    let Some(pending) = ctx.runner.state().pending_persona_switch.as_ref() else {
        return;
    };
    if pending.decision.is_none() && !pending.waiting {
        return;
    }
    resume_switch_with(shared, ctx, |profile| open_store_as(Some(profile)));
}

fn resume_switch_with(
    shared: &mut Shared,
    ctx: &mut Ctx<'_>,
    mut open: impl FnMut(
        &personae::ProfileId,
    ) -> (
        woodshed_core::storage::SessionStore<crate::storage::HostBackend>,
        woodshed_views::persona::PracticeSeal,
    ),
) {
    let mut chosen = None;
    ctx.runner
        .update(|ui| chosen = preserve_pending_switch(shared, ui));
    if let Some(profile) = chosen {
        let (storage, seal) = open(&profile);
        // A successful custody connection can still answer as another profile
        // if the family's choice changed while it was reopening. The selected
        // identity, not that convention, authorizes retiring outgoing practice.
        let seal = match seal {
            woodshed_views::persona::PracticeSeal::Sealed { ref persona, .. }
                if persona != &profile.0 =>
            {
                woodshed_views::persona::PracticeSeal::Pending {
                    reason: format!(
                        "djinn opened persona {persona:?} instead of the selected persona {:?}",
                        profile.0
                    ),
                }
            },
            seal => seal,
        };
        install_store(shared, ctx, storage, seal, PickPurpose::Switch);
    }
}

/// Returning the chosen identity permits an incoming open attempt, not an
/// outgoing reset. Keep the target until that attempt establishes a seal.
fn preserve_pending_switch(
    shared: &mut Shared,
    ui: &mut woodshed_views::stage::UiState,
) -> Option<personae::ProfileId> {
    let pending = ui.pending_persona_switch.as_mut()?;
    if pending.decision.is_none() && !pending.waiting {
        return None;
    }
    let decision = pending.decision.take();
    let profile = pending.profile.clone();
    if decision == Some(SwitchDecision::Cancel) {
        ui.pending_persona_switch = None;
        return None;
    }
    let mut outcome = crate::capture_persistence::Preservation::Ready;
    if decision != Some(SwitchDecision::Discard) {
        outcome = crate::capture_persistence::preserve_request(shared, ui);
        if outcome == crate::capture_persistence::Preservation::Ready
            && shared.wav_export.is_finishing()
        {
            outcome = crate::capture_persistence::Preservation::Preparing;
        }
    }
    match outcome {
        crate::capture_persistence::Preservation::Ready => {
            if let Some(pending) = &mut ui.pending_persona_switch {
                pending.waiting = false;
            }
            Some(profile)
        },
        crate::capture_persistence::Preservation::Preparing => {
            if let Some(pending) = &mut ui.pending_persona_switch {
                pending.waiting = true;
                pending.notice = "Finishing outgoing media work before switching…".into();
            }
            None
        },
        crate::capture_persistence::Preservation::Blocked(reason) => {
            if let Some(pending) = &mut ui.pending_persona_switch {
                pending.waiting = false;
                pending.notice = reason;
            }
            None
        },
    }
}

/// Put the switch gate up, or say why it cannot go up.
///
/// A vault that will not open is reported on the gate itself rather than
/// swallowed: the row was a deliberate act, and a control that answers a click
/// with nothing reads as broken.
fn raise_switch(ctx: &mut Ctx<'_>) {
    let pick = match roster_now() {
        Ok(roster) => PersonaPick::switch(roster),
        Err(error) => {
            eprintln!("[woodshed] cannot read the persona roster: {error}");
            PersonaPick::switch(Roster {
                entries: Vec::new(),
                chosen: personae::ProfileId(String::new()),
                description: "djinn did not answer".into(),
            })
            .with_notice(format!(
                "djinn would not answer ({error}). Practice continues \
                 as the current persona."
            ))
        },
    };
    // Taken rather than moved: the runner's callback is `FnMut`, and the pick
    // is not `Copy`.
    let mut pick = Some(pick);
    ctx.runner.update(move |ui| ui.persona = pick.take());
}

/// Practise with no persona: no store, nothing saved, and the nav row says so.
///
/// The alternative was to open on the convention, which is what this did until
/// the shape was thought through. On the only vault that reaches this screen —
/// several personas, none chosen — the convention resolves to `default` and
/// *mints it*, so declining would have added a third identity beside the user's
/// two and sealed their practice to one they never picked. Opening nothing is
/// both the honest reading of "no thanks" and the only one that leaves the
/// vault as it was found.
///
/// The cost is stated where it lands rather than buried: `Shared.storage` stays
/// `None`, every save in the dispatch tail is skipped, and `practice_saved`
/// turns the nav-row notice on for the rest of the session.
fn decline(ctx: &mut Ctx<'_>) {
    eprintln!("[woodshed] no persona chosen; this session is not saved");
    ctx.runner.update(woodshed_views::persona::practise_unsaved);
}

/// Open the store on the settled persona, restore the session into it, and take
/// the gate down.
fn settle(
    shared: &mut Shared,
    ctx: &mut Ctx<'_>,
    chosen: Option<&personae::ProfileId>,
    purpose: PickPurpose,
) {
    // djinn switches to the choice and remembers it for the family; a switch
    // it refuses leaves the store pending rather than on somebody else.
    let (storage, seal) = open_store_as(chosen);
    install_store(shared, ctx, storage, seal, purpose);
}

fn install_store(
    shared: &mut Shared,
    ctx: &mut Ctx<'_>,
    storage: woodshed_core::storage::SessionStore<crate::storage::HostBackend>,
    seal: woodshed_views::persona::PracticeSeal,
    purpose: PickPurpose,
) {
    if purpose == PickPurpose::Switch {
        if let woodshed_views::persona::PracticeSeal::Pending { reason } = &seal {
            ctx.runner.update(|ui| {
                if let Some(pending) = &mut ui.pending_persona_switch {
                    pending.decision = None;
                    pending.waiting = false;
                    pending.notice = format!(
                        "Incoming practice could not open ({reason}). Outgoing practice remains \
                         active. Retry, cancel the switch, or explicitly discard outgoing changes \
                         and retry opening the selected persona."
                    );
                }
            });
            return;
        }
    }
    shared.wav_export.revoke_session();
    ctx.runner.update(|ui| {
        if purpose == PickPurpose::Switch {
            // The whole session goes, not just the parts the incoming persona
            // happens to have stored. `restore` returns early on a store with
            // no session, so anything left standing would be the OUTGOING
            // persona's practice — and the next frame's save would write it
            // into this persona's store. Host-fed fields (the MIDI port lists,
            // latency) refill on the next dispatch.
            reset_for_persona(ui);
        }
        crate::session::restore(&storage, ui);
        ui.persona = None;
        // After the reset above, so a switch does not wipe the seal it just
        // established. Cloned rather than moved: the callback is `FnMut`.
        ui.practice_saved = !matches!(seal, woodshed_views::persona::PracticeSeal::Pending { .. });
        ui.seal = Some(seal.clone());
        crate::session::refresh_save_status(&storage, ui);
    });
    ctx.runner.update(|ui| {
        crate::session::ensure_personal_relation_source(&storage, ui);
    });
    // Both from the one value, so `Shared` and the view cannot disagree about
    // who is practising.
    shared.seal = Some(seal);
    shared.storage = Some(storage);
}

/// Replace persona-owned practice and application settings while retaining the
/// process-wide MIDI routing, authored library and its current editor model. Reloading that
/// library here would introduce profile/environment I/O during a session reset.
fn reset_for_persona(ui: &mut woodshed_views::stage::UiState) {
    let practice_clock_managed = ui.practice_clock_managed;
    let event_clock = ui.event_clock;
    let appearance = std::mem::take(&mut ui.appearance);
    // Port inventory and routing belong to the running host, not a persona.
    let midi = std::mem::take(&mut ui.midi);
    let authoring_available = ui.appearance_authoring_available;
    let library_notice = if authoring_available {
        None
    } else {
        ui.appearance_notice.take()
    };
    *ui = woodshed_views::stage::UiState::new();
    ui.practice_clock_managed = practice_clock_managed;
    ui.event_clock = event_clock;
    ui.appearance = appearance;
    ui.midi = midi;
    ui.appearance_authoring_available = authoring_available;
    ui.appearance_notice = library_notice;
}

/// Seed the gate onto a fresh [`UiState`], if one is pending.
pub fn seed(shared: &mut Shared, ui: &mut woodshed_views::stage::UiState) {
    ui.persona = shared.pending_roster.take().map(PersonaPick::new);
}

#[cfg(test)]
mod tests {
    use super::*;
    use cambium_genet_winit_host::Harness;
    use dramatis::roster::RosterEntry;
    use personae::ProfileId;
    use taproot::Selector;
    use winit::keyboard::NamedKey;
    use woodshed_views::stage::{UiChild, UiState};

    #[test]
    fn persona_reset_preserves_process_midi_routing_and_inventory() {
        let mut ui = UiState::new();
        ui.midi.input_ports = vec!["keyboard".into()];
        ui.midi.output_ports = vec!["synth".into()];
        ui.midi.input_dd.selected = 1;
        ui.midi.output_dd.selected = 1;
        ui.midi.clock_out = true;
        ui.midi.connected_out = Some("synth".into());
        ui.midi.error = Some("pending diagnostic".into());
        reset_for_persona(&mut ui);
        assert_eq!(ui.midi.input_ports, ["keyboard"]);
        assert_eq!(ui.midi.output_ports, ["synth"]);
        assert_eq!(ui.midi.input_dd.selected, 1);
        assert_eq!(ui.midi.output_dd.selected, 1);
        assert!(ui.midi.clock_out);
        assert_eq!(ui.midi.connected_out.as_deref(), Some("synth"));
        assert_eq!(ui.midi.error.as_deref(), Some("pending diagnostic"));
    }

    #[test]
    fn persona_reset_keeps_the_host_clock_binding_and_clears_run_position() {
        let mut ui = UiState::new();
        ui.enable_practice_clock();
        ui.event_clock = Some(|| Some(789));
        ui.stage_clock_contacts = vec![(0, 1)];
        ui.stage_clock_event = Some(3);
        reset_for_persona(&mut ui);
        assert!(ui.practice_clock_managed);
        ui.refresh_event_time();
        assert_eq!(ui.now_ms, Some(789));
        assert!(ui.stage_clock_contacts.is_empty());
        assert_eq!(ui.stage_clock_event, None);
        assert!(!ui.rehearsal_running);
        assert!(ui.rehearsal_clock_program.is_none());
    }

    #[test]
    fn switching_persona_retains_the_global_authored_library_but_restores_incoming_selection() {
        use woodshed_core::{settings::AppSettings, storage::SessionStore};
        use woodshed_views::{appearance::AppearanceState, theme::ThemeMode};

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("themes.json");
        let mut appearance = AppearanceState::load(&path).unwrap();
        let mut incoming = AppSettings::default();
        incoming.appearance.theme = "Ember".into();
        appearance.begin_edit(&incoming.appearance).unwrap();
        appearance.workshop.save();
        appearance.apply_workshop(&mut incoming.appearance).unwrap();
        let authored_id = appearance.workshop.draft_theme().id.clone();
        appearance.workshop_open = false;
        let reader = appearance.workshop.reader_preview();
        assert!(path.exists(), "the authored definition was really saved");

        let mut ui = UiState::new();
        ui.appearance = appearance;
        ui.appearance_authoring_available = true;
        ui.appearance_notice = Some("Outgoing application selection applied.".into());
        ui.appearance_close_app = true;
        ui.set_theme(ThemeMode::Parchment);
        ui.app_settings.accessibility.reduce_motion = true;

        reset_for_persona(&mut ui);
        assert_eq!(
            ui.app_settings,
            AppSettings::default(),
            "outgoing persona settings must not survive"
        );
        assert!(!ui.appearance_close_app);
        assert!(ui.appearance_notice.is_none());
        assert!(ui.appearance_authoring_available);
        assert_eq!(ui.appearance.workshop.library_path(), Some(path.as_path()));
        assert!(
            std::rc::Rc::ptr_eq(&reader, &ui.appearance.workshop.reader_preview()),
            "retain the actual shared editor rather than reloading it"
        );

        // Use the real settings-only session decoder against a memory store;
        // this never reads the process environment or a user's profile/vault.
        let backend: crate::storage::HostBackend = Box::<muniment::MemoryBackend>::default();
        let storage = SessionStore::new(backend);
        storage.save_settings(&serde_json::to_string(&incoming).unwrap());
        crate::session::restore(&storage, &mut ui);
        assert_eq!(ui.app_settings.appearance, incoming.appearance);
        let resolved = ui.appearance.resolve(&ui.app_settings.appearance);
        assert_eq!(resolved.theme.id, authored_id);
        assert!(resolved.fallback_reason.is_none());
        assert!(ui.appearance_authoring_available);
    }

    #[test]
    fn switching_persona_retains_the_global_library_load_error() {
        let mut ui = UiState::new();
        ui.appearance_authoring_available = false;
        ui.appearance_notice = Some("Could not open theme library: corrupt library.".into());
        ui.set_theme(woodshed_views::theme::ThemeMode::Ember);
        let reader = ui.appearance.workshop.reader_preview();
        reset_for_persona(&mut ui);
        assert_eq!(
            ui.app_settings,
            woodshed_core::settings::AppSettings::default()
        );
        assert!(!ui.appearance_authoring_available);
        assert_eq!(
            ui.appearance_notice.as_deref(),
            Some("Could not open theme library: corrupt library.")
        );
        assert!(std::rc::Rc::ptr_eq(
            &reader,
            &ui.appearance.workshop.reader_preview()
        ));
    }

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
        assert!(
            by_key.click_on(&Selector::class("command-item").with_attr("data-key", "work-studio"))
        );
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
        assert!(harness.click_on(
            &Selector::class("command-item").with_attr("data-key", "persona-picker:create")
        ));
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
}

#[cfg(test)]
#[path = "persona_preservation_tests.rs"]
mod preservation_tests;
