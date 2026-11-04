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

use djinn::door::app_admission::AppId;
use djinn::door::custody_client::BlockingCustodyClient;
use dramatis::roster::Roster;
use persona_picker::PickerEvent;
use woodshed_views::persona::{PendingSwitch, PersonaPick, PickPurpose, SwitchDecision};

use crate::shared::Shared;
use crate::storage::{CUSTODY_APP, OpenedStore, open_store_as, open_store_at};
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
    let mut already_chosen = false;
    ctx.runner.update(|ui| {
        requested = std::mem::take(&mut ui.persona_switch_requested);
        if let Some(pick) = ui.persona.as_mut() {
            answer = pick.outcome.take();
            purpose = pick.purpose;
            already_chosen = matches!(answer.as_ref(), Some(PickerEvent::Chose(id))
                if pick.roster.entries.iter().any(|entry| entry.chosen && &entry.id == id));
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
        // A chosen, installed persona is a selection-only action. A different
        // family choice still needs custody even when this process holds the
        // selected store. Canonical identity, rather than display name, binds it.
        if already_chosen
            && shared.store_binding.as_ref().is_some_and(|binding| {
                shared.data_root.as_deref() == Some(binding.root())
                    && binding.matches_current(&profile).unwrap_or(false)
            })
            && shared.storage.is_some()
            && matches!(shared.seal.as_ref(),
                Some(woodshed_views::persona::PracticeSeal::Sealed { persona, .. })
                    if persona == &profile.0)
        {
            ctx.runner.update(|ui| ui.persona = None);
            return;
        }
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
    let root = shared.data_root.clone();
    resume_switch_with(shared, ctx, |profile| match &root {
        Some(root) => open_store_at(root, Some(profile)),
        None => open_store_as(Some(profile)),
    });
}

fn resume_switch_with(
    shared: &mut Shared,
    ctx: &mut Ctx<'_>,
    mut open: impl FnMut(&personae::ProfileId) -> OpenedStore,
) {
    let mut chosen = None;
    ctx.runner
        .update(|ui| chosen = preserve_pending_switch(shared, ui));
    if let Some(profile) = chosen {
        let mut opened = open(&profile);
        // A successful custody connection can still answer as another profile
        // if the family's choice changed while it was reopening. The selected
        // identity, not that convention, authorizes retiring outgoing practice.
        opened.seal = match opened.seal {
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
        install_store(shared, ctx, opened, PickPurpose::Switch);
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
    let opened = match shared.data_root.as_deref() {
        Some(root) => open_store_at(root, chosen),
        None => open_store_as(chosen),
    };
    install_store(shared, ctx, opened, purpose);
}

fn install_store(
    shared: &mut Shared,
    ctx: &mut Ctx<'_>,
    opened: OpenedStore,
    purpose: PickPurpose,
) {
    let OpenedStore {
        storage,
        seal,
        binding,
    } = opened;
    let candidate = if purpose == PickPurpose::Switch || binding.is_some() {
        let admitted = (|| {
            if let woodshed_views::persona::PracticeSeal::Pending { reason } = &seal {
                return Err(reason.clone());
            }
            let candidate = crate::session::admit(&storage)?;
            let binding = binding
                .as_ref()
                .ok_or("Incoming store has no admitted owner.")?;
            let profile = shared_pending_profile(ctx)
                .or_else(|| match &seal {
                    woodshed_views::persona::PracticeSeal::Sealed { persona, .. } => {
                        Some(personae::ProfileId(persona.clone()))
                    },
                    _ => None,
                })
                .ok_or("The persona switch is no longer pending.")?;
            binding.choose_profile(&profile)?;
            Ok(candidate)
        })();
        match admitted {
            Ok(candidate) => Some(candidate),
            Err(reason) => {
                ctx.runner.update(|ui| {
                    if let Some(pick) = &mut ui.persona {
                        pick.notice = Some(format!("Practice could not open: {reason}. Choose another persona or continue unsaved."));
                    }
                    if let Some(pending) = &mut ui.pending_persona_switch {
                        pending.decision = None;
                        pending.waiting = false;
                        pending.notice = format!("Incoming practice could not open ({reason}). Outgoing practice remains active. Retry or cancel the switch.");
                    }
                });
                return;
            },
        }
    } else {
        None
    };
    let mut candidate = candidate;
    shared.wav_export.revoke_session();
    ctx.runner.update(|ui| {
        if let Some(incoming) = candidate.take() {
            install_persona_state(ui, incoming);
        } else {
            crate::session::restore(&storage, ui);
        }
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
    shared.store_binding = binding;
}

/// Replace persona-owned practice and application settings while retaining the
/// process-wide MIDI routing, authored library and its current editor model. Reloading that
/// library here would introduce profile/environment I/O during a session reset.
fn shared_pending_profile(ctx: &Ctx<'_>) -> Option<personae::ProfileId> {
    ctx.runner
        .state()
        .pending_persona_switch
        .as_ref()
        .map(|pending| pending.profile.clone())
}

fn reset_for_persona(ui: &mut woodshed_views::stage::UiState) {
    install_persona_state(ui, woodshed_views::stage::UiState::new());
}

fn install_persona_state(
    ui: &mut woodshed_views::stage::UiState,
    incoming: woodshed_views::stage::UiState,
) {
    let practice_clock_managed = ui.practice_clock_managed;
    let event_clock = ui.event_clock;
    let audio_availability = ui.audio_availability.clone();
    let audio_error = ui.audio_error.clone();
    let appearance = std::mem::take(&mut ui.appearance);
    // Port inventory and routing belong to the running host, not a persona.
    let midi = std::mem::take(&mut ui.midi);
    let authoring_available = ui.appearance_authoring_available;
    let library_notice = if authoring_available {
        None
    } else {
        ui.appearance_notice.take()
    };
    *ui = incoming;
    ui.practice_clock_managed = practice_clock_managed;
    ui.event_clock = event_clock;
    ui.audio_availability = audio_availability;
    ui.audio_error = audio_error;
    ui.appearance = appearance;
    // Definitions and the shared editor survive, while cached paint, queued
    // application choices and font drafts belong to the outgoing persona.
    ui.appearance.reset_active();
    ui.appearance.reset_font_editor(&ui.app_settings.appearance);
    ui.midi = midi;
    ui.appearance_authoring_available = authoring_available;
    ui.appearance_notice = library_notice;
}

/// Seed the gate onto a fresh [`UiState`], if one is pending.
pub fn seed(shared: &mut Shared, ui: &mut woodshed_views::stage::UiState) {
    ui.persona = shared.pending_roster.take().map(PersonaPick::new);
}

#[cfg(test)]
#[path = "persona/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "persona_preservation_tests.rs"]
mod preservation_tests;
