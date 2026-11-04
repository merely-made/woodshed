//! The tail of every input dispatch: push the state through the seams it owns.
//!
//! Dropdown picks land in the core, the audio backend is told the new
//! transport, the MIDI ports are connected or dropped, window-chrome requests
//! are honoured, the session is persisted, and a theme or accessibility change
//! asks the host for a new sheet.

use cambium_genet_winit_host::AppCtx;
use woodshed_core::audio::{AudioBackend, AudioRequest, CalibrationStatus};
use woodshed_core::loop_plan::{LoopPlan, LooperSource};
use woodshed_core::song::SongDoc;
use woodshed_views::stage::{UiChild, UiState};

use crate::shared::Shared;

/// The runner shape this application uses, spelled once.
/// A boxed closure rather than a `fn` pointer: the desktop root captures the
/// host's window-verb handle, so the caption buttons can invoke it without a
/// flag on the shared (browser-portable) `UiState`.
pub type Logic = Box<dyn FnMut(&UiState) -> UiChild>;
/// The host context a woodshed hook receives.
pub type Ctx<'a> = AppCtx<'a, UiState, Logic, UiChild>;

/// Everything woodshed does after an input dispatch.
pub fn after_dispatch(shared: &mut Shared, ctx: &mut Ctx<'_>) {
    crate::pedal_inputs::after_dispatch(shared, ctx);
    // First, so that everything below sees the session the chosen persona's
    // key opened rather than the empty one that stood in for it.
    crate::persona::after_dispatch(shared, ctx);
    crate::appearance::after_dispatch(ctx);
    // Graph motion changes only transient projection state. Pointer Down and
    // Move leave this set; Up clears it before this hook and therefore runs the
    // ordinary tail once. Skipping here avoids three retained-root rebuilds,
    // two serializations, audio/MIDI polling, and two synchronous store writes
    // for every sampled position while preserving the existing commit path at
    // gesture release.
    if ctx.runner.state().set_graph_drag_active {
        shared.view_only_dispatches = shared.view_only_dispatches.saturating_add(1);
        return;
    }
    shared.full_dispatch_syncs = shared.full_dispatch_syncs.saturating_add(1);
    crate::pack_io::after_dispatch(shared, ctx);
    push_backend(shared, ctx);
    crate::wav_export::after_dispatch(shared, ctx);
    crate::music_export::after_dispatch(shared, ctx);
    // Window verbs used to be drained here from four `UiState` flags. The
    // host owns them now: the caption buttons call `WindowCommands` directly
    // and the title bar drags from its `--app-region` declaration.
    midi_devices(shared, ctx);
    crate::midi_diagnostics::after_dispatch(shared, ctx);
}

/// Apply personal assertion intents to a replayed candidate, persist the
/// candidate session, and only then publish its new journal in live UI state.
/// If persistence acknowledgment is uncertain, the current graph stays
/// unchanged and this session's storage capability is revoked until reopen.
fn commit_personal_relation_intents(
    ui: &mut UiState,
    storage: &mut Option<woodshed_core::storage::SessionStore<crate::storage::HostBackend>>,
) {
    use woodshed_core::personal_relations::{
        AssertionBasis, Author, EdgeId, PersonalRelationSource, Relation, RelationClass,
    };
    use woodshed_views::stage::PersonalRelationIntent;

    if !ui.practice_saved {
        ui.personal_relation_intents.clear();
        return;
    }
    let intents = std::mem::take(&mut ui.personal_relation_intents);
    for intent in intents {
        let Some(store) = storage.as_ref() else {
            ui.personal_relation_notice =
                Some("Personal relation changes are unavailable without a saved session.".into());
            continue;
        };
        let Some(actor) = (match ui.seal.as_ref() {
            Some(woodshed_views::persona::PracticeSeal::Sealed { persona, .. })
                if ui.practice_saved && !persona.trim().is_empty() && persona == persona.trim() =>
            {
                Some(Author::new(persona.clone()))
            },
            _ => None,
        }) else {
            ui.personal_relation_notice = Some(
                "Personal relation changes require a saved session with a sealed persona.".into(),
            );
            continue;
        };
        let Some(current) = ui.personal_relations.as_ref() else {
            ui.personal_relation_notice =
                Some("The personal assertion source is unavailable; no change was saved.".into());
            continue;
        };
        let expected_source_id = match &intent {
            PersonalRelationIntent::Assert {
                expected_source_id, ..
            }
            | PersonalRelationIntent::Confirm {
                expected_source_id, ..
            }
            | PersonalRelationIntent::Withdraw {
                expected_source_id, ..
            } => expected_source_id,
        };
        if current.source_id() != expected_source_id {
            ui.personal_relation_notice = Some(
                "This relation action belongs to a different saved source; reopen the current source before acting.".into(),
            );
            continue;
        }
        let mut candidate = PersonalRelationSource::restore(current.persisted());
        let result = match intent {
            PersonalRelationIntent::Assert {
                expected_source_id: _,
                expected_revision,
                from,
                to,
                note,
                at_ms,
            } => candidate
                .assert_relation(
                    &actor,
                    expected_revision,
                    &from,
                    &to,
                    Relation::new(RelationClass::app("woodshed", 3)).with_label("related"),
                    at_ms,
                    AssertionBasis {
                        note,
                        ..Default::default()
                    },
                )
                .map(|id| Some(id)),
            PersonalRelationIntent::Confirm {
                expected_source_id: _,
                expected_revision,
                id,
                from,
                to,
                at_ms,
            } => serde_json::from_str::<EdgeId>(&id)
                .map_err(|error| {
                    woodshed_core::personal_relations::PersonalRelationError::Commit(
                        error.to_string(),
                    )
                })
                .and_then(|id| {
                    candidate
                        .confirm_exact(&actor, expected_revision, id, &from, &to, at_ms)
                        .map(|()| None)
                }),
            PersonalRelationIntent::Withdraw {
                expected_source_id: _,
                expected_revision,
                id,
                from,
                to,
                at_ms,
            } => serde_json::from_str::<EdgeId>(&id)
                .map_err(|error| {
                    woodshed_core::personal_relations::PersonalRelationError::Commit(
                        error.to_string(),
                    )
                })
                .and_then(|id| {
                    candidate
                        .withdraw_exact(&actor, expected_revision, id, &from, &to, at_ms)
                        .map(|()| None)
                }),
        };
        let result = match result {
            Ok(result) => result,
            Err(error) => {
                ui.personal_relation_notice = Some(error.to_string());
                continue;
            },
        };
        let mut persisted = ui.to_persisted();
        persisted.personal_relations = Some(candidate.persisted());
        let encoded = match serde_json::to_string(&persisted) {
            Ok(encoded) => encoded,
            Err(error) => {
                ui.personal_relation_notice = Some(format!(
                    "Personal relation change could not be encoded; nothing changed: {error}"
                ));
                continue;
            },
        };
        if let Err(error) = store.try_save(&encoded) {
            // A backend may have accepted the put before readback failed. Keep
            // the live journal unchanged and close this storage capability so
            // later autosaves cannot overwrite an uncertain candidate.
            *storage = None;
            ui.practice_saved = false;
            ui.personal_relation_notice = Some(format!(
                "Session write was not confirmed. This session is now read-only; reopen or switch persona to reconcile stored state. Error: {error}"
            ));
            return;
        }
        ui.personal_relations = Some(candidate);
        ui.personal_relation_notice = Some(match result {
            Some(id) => format!("Personal assertion saved: {id:?}"),
            None => "Personal assertion lifecycle change saved.".into(),
        });
    }
}

/// Execute after backing admission using the frozen source/epoch, not the UI
/// selection at queue drain. Arm commands never toggle somebody else's take.
pub(crate) fn execute_recorder_request(
    request: &woodshed_core::audio::RecorderRequest,
    session: &std::sync::Arc<()>,
    backend: &mut crate::audio::CpalBackend,
    persistence: &mut crate::capture_persistence::CapturePersistence,
    song: Option<&SongDoc>,
    plan: Option<&LoopPlan>,
) -> Result<bool, String> {
    use woodshed_core::audio::{RecorderBacking, RecorderCommand};
    if !request.is_current(session, song, plan) {
        return Err(
            "This queued recorder source or session has changed. Use its current controls.".into(),
        );
    }
    match (&request.backing, &request.command) {
        (RecorderBacking::Set(plan), RecorderCommand::RecordCard(id)) => {
            if plan.index_of(*id).is_none() {
                return Err("That Card is not in the admitted backing.".into());
            }
            persistence.permit_record(
                plan,
                backend
                    .output_sample_rate()
                    .ok_or("Audio output is unavailable.")?,
                backend.capture_snapshot(),
            )?;
            backend.loop_arm_record(*id)?;
            Ok(false)
        },
        (RecorderBacking::Set(plan), RecorderCommand::ClearCard(id)) => {
            if plan.index_of(*id).is_none() {
                return Err("That Card is not in the admitted backing.".into());
            }
            backend.clear_stored_capture(*id)?;
            persistence.forget(*id);
            Ok(true)
        },
        (RecorderBacking::Saved(song), RecorderCommand::RecordSavedBar(index))
            if *index < song.bars.len() =>
        {
            backend.recorder_arm_saved(*index)?;
            Ok(false)
        },
        (RecorderBacking::Saved(song), RecorderCommand::ClearSavedBar(index))
            if *index < song.bars.len() =>
        {
            backend.recorder_clear_saved(*index)?;
            Ok(true)
        },
        (
            _,
            RecorderCommand::StopCapture {
                start_bar,
                capture_epoch,
            },
        ) => {
            backend.recorder_stop_capture(*start_bar, *capture_epoch)?;
            Ok(true)
        },
        _ => Err("The queued recorder target does not match its admitted backing.".into()),
    }
}

/// Connect / disconnect per the dropdowns, and re-scan the port lists on
/// request.
fn midi_devices(shared: &mut Shared, ctx: &mut Ctx<'_>) {
    shared
        .midi
        .set_pedal_bindings(&ctx.runner.state().app_settings.audio_midi.midi_pedal);
    ctx.runner
        .update(|ui| sync_midi_devices(ui, &mut shared.midi));
    crate::pedal_inputs::sync(shared, ctx.runner.state());
}

/// Reconcile requested routing through the one backend, then observe completion.
fn sync_midi_devices(ui: &mut UiState, midi: &mut impl woodshed_core::midi::MidiBackend) {
    if std::mem::take(&mut ui.midi.refresh_requested) {
        ui.midi
            .refresh_ports(midi.input_ports(), midi.output_ports());
    }
    let input = ui.midi.selected_input().map(str::to_string);
    midi.connect_input(input.as_deref());
    let output = ui.midi.selected_output().map(str::to_string);
    midi.connect_output(output.as_deref());
    if let Some(action) = ui.midi.pedal_request.take() {
        let binding = ui.app_settings.audio_midi.midi_pedal.get(action).clone();
        ui.midi.pedal_report = Some(if let Some(target) = ui.midi.pedal_request_target.take() {
            midi.trigger_pedal_for_target(action, binding, target)
        } else {
            midi.trigger_pedal(action, binding)
        });
    }
    poll_midi_state(ui, midi);
}

/// Read-only completion projection shared by ordinary frames and dispatch.
pub(crate) fn poll_midi_state(
    ui: &mut UiState,
    midi: &mut impl woodshed_core::midi::MidiBackend,
) -> bool {
    midi.poll();
    ui.midi.connected_in = midi.connected_input().map(str::to_string);
    ui.midi.connected_out = midi.connected_output().map(str::to_string);
    ui.midi.error = midi.error().map(str::to_string);
    ui.midi.output_notice = midi.output_status().map(str::to_string);
    ui.midi.output_ready = midi.output_ready();
    let target = midi.pedal_target();
    if ui.midi.pedal_target != target {
        ui.midi.disarm_inputs();
        ui.midi.input_notice = Some("Output connection changed; pedal inputs disarmed".into());
    }
    ui.midi.pedal_target = target;
    ui.midi.pedal_report = midi.pedal_report();
    midi.needs_service()
}

/// Publish the admitted snapshot to the view. A refused edit must not make the
/// timeline or capture controls describe a plan the engine never loaded.
fn admit_playback(
    ui: &mut UiState,
    backend: &mut impl AudioBackend,
    last_song: &mut Option<SongDoc>,
    last_loop_plan: &mut Option<LoopPlan>,
) {
    let loaded_source = last_loop_plan
        .as_ref()
        .map_or(LooperSource::SavedSong, |plan| {
            LooperSource::Set(plan.owner)
        });
    if ui.looper.source != loaded_source && backend.loop_capture_busy() {
        ui.looper.source = loaded_source;
        ui.loop_admitted = last_loop_plan.clone();
        ui.loop_notice = Some("Stop recording / armed capture before changing backing source. The loaded backing remains active.".into());
        return;
    }
    match ui.looper.source {
        LooperSource::SavedSong => {
            if last_loop_plan.is_some() || last_song.as_ref() != Some(&ui.song) {
                match backend.try_set_song(&ui.song) {
                    Ok(()) => {
                        *last_song = Some(ui.song.clone());
                        *last_loop_plan = None;
                        ui.loop_admitted = None;
                        ui.loop_notice = None;
                    },
                    Err(reason) => {
                        ui.looper.source = loaded_source;
                        ui.loop_admitted = last_loop_plan.clone();
                        ui.loop_notice = Some(reason);
                        if last_song.is_none() && last_loop_plan.is_none() {
                            ui.song_playing = false;
                        }
                    },
                }
            }
        },
        LooperSource::Set(_) => {
            match ui.requested_loop_plan() {
                Ok(plan) => {
                    if last_loop_plan.as_ref() != Some(&plan) {
                        match backend.set_loop_plan(&plan) {
                            Ok(()) => {
                                *last_loop_plan = Some(plan);
                                *last_song = None;
                                ui.loop_notice = None;
                                ui.song_bar_live = 0;
                            },
                            Err(reason) => ui.loop_notice = Some(reason),
                        }
                    }
                },
                Err(_) => ui.loop_notice = Some("The selected Set is not ready. The loaded backing remains unchanged; open a Card below to resolve its readiness.".into()),
            }
            ui.loop_admitted = last_loop_plan.clone();
            if let Some(plan) = last_loop_plan.as_ref() {
                if ui
                    .loop_selected
                    .is_none_or(|id| plan.index_of(id).is_none())
                {
                    ui.loop_selected = plan.segments.first().map(|segment| segment.id);
                }
            }
            if last_loop_plan.is_none() && last_song.is_none() {
                ui.song_playing = false;
            }
        },
    }
}

#[cfg(test)]
#[path = "sync/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "sync/midi_selection_tests.rs"]
mod midi_selection_tests;

#[path = "sync/backend.rs"]
mod backend;

use backend::push_backend;
