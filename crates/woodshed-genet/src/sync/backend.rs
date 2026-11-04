use super::*;

/// Sync dropdown state into the core, push the audio state through the backend
/// seam, capture the skin settings, and persist.
pub(super) fn push_backend(shared: &mut Shared, ctx: &mut Ctx<'_>) {
    let mut sheet = None;
    let mut persisted_settings: Option<String> = None;
    let availability = shared
        .backend
        .as_ref()
        .map(crate::audio::CpalBackend::availability)
        .unwrap_or_default();
    if ctx.runner.state().audio_availability != availability {
        ctx.runner.update(|ui| {
            ui.audio_availability = availability.clone();
            if !ui.audio_availability.tuner_active {
                ui.tuner.reading = None;
            }
        });
    }

    if !crate::capture_persistence::session_current(shared, ctx.runner.state()) {
        shared.wav_export.revoke_session();
    }

    // Set/Card ids are session-local. Every reset/open, including reopening
    // the same persona, releases PCM custody before admitting incoming ids.
    let restore_notice = match crate::capture_persistence::begin_session(shared, ctx.runner.state())
    {
        Ok(notice) => notice,
        Err(error) => {
            ctx.runner.update(|ui| {
                ui.loop_capture_notice =
                    Some(format!("Captured loops are waiting to restore: {error}"))
            });
            return;
        },
    };
    if let Some(backend) = shared.backend.as_mut() {
        // Retire capture/preview work before any source admission or command.
        backend.service();
    }
    ctx.runner.update(|ui| {
        if let Some(notice) = &restore_notice {
            ui.loop_capture_notice = notice.clone();
        }
        let backend = shared.backend.as_mut();
        let last_song = &mut shared.last_song;
        let last_loop_plan = &mut shared.last_loop_plan;
        let capture_persistence = &mut shared.capture_persistence;
        ui.sync();
        commit_personal_relation_intents(ui, &mut shared.storage);
        let storage = shared.storage.as_ref();
        let appearance_attempted = if storage.is_some() {
            woodshed_views::appearance::commit_selection(ui, |settings| {
                let store = storage.ok_or("Choose a persona before saving appearance.")?;
                let json = serde_json::to_string(settings).map_err(|error| error.to_string())?;
                store
                    .try_save_settings(&json)
                    .map_err(|error| error.to_string())
            })
        } else {
            false
        };
        if let Some(backend) = backend {
            backend.sync_practice_clock(ui);
            // Calibration owns the shared runtime's click lane during a run.
            if !ui.calib_active {
                if let Err(error) = backend.sync_pack(ui.audio_pack.as_ref(), ui.transport) {
                    ui.pack_notice = error;
                }
                backend.set_metronome(ui.transport);
            }
            backend.set_tuner_enabled(ui.tuner.enabled);
            admit_playback(ui, backend, last_song, last_loop_plan);
            backend.set_song_transport(ui.song_playing);
            // Every one-shot command the views queued this frame, realized in
            // the order the user asked for it. Draining here is the only place
            // a request is consumed, so none can be left set or cleared twice.
            let mut capture_changed = false;
            for request in std::mem::take(&mut ui.audio_requests) {
                match request {
                    AudioRequest::Audition(request) => {
                        crate::drive::service_audition_request(ui, backend, &request);
                    },
                    AudioRequest::CardPreview(request) => {
                        if ui.card_preview_is_current(&request) {
                            let outcome = backend.admit_card_preview(&request.preview);
                            ui.finish_card_preview_admission(&request, outcome);
                        }
                    },
                    AudioRequest::Recorder(request) => {
                        match execute_recorder_request(
                            &request,
                            &ui.audio_session,
                            backend,
                            capture_persistence,
                            last_song.as_ref(),
                            last_loop_plan.as_ref(),
                        ) {
                            Ok(changed) => capture_changed |= changed,
                            Err(reason) => ui.loop_notice = Some(reason),
                        }
                    },
                    AudioRequest::SongRewind => backend.song_rewind(),
                    AudioRequest::CancelPreview => backend.cancel_preview(),
                    AudioRequest::PreviewVoicing => match ui.preview_voicing() {
                        Ok(preview) => {
                            if let Err(error) = backend.admit_card_preview(&preview) {
                                ui.card_timing_notice = Some(format!("Queued preview: {error}"));
                            }
                        },
                        Err(error) => ui.card_timing_notice = Some(error.to_string()),
                    },
                    AudioRequest::PreviewPitches {
                        pitches,
                        duration_s,
                        strum_s,
                    } => {
                        if !pitches.is_empty() {
                            if let Err(error) =
                                backend.admit_card_preview(&(pitches, duration_s, strum_s))
                            {
                                ui.card_timing_notice = Some(format!("Queued preview: {error}"));
                            }
                        }
                    },
                    AudioRequest::PreviewPhrase {
                        before,
                        after,
                        chord_duration_s,
                        gap_s,
                        strum_s,
                    } => {
                        if !before.is_empty() && !after.is_empty() {
                            shared.events.push(format!(
                                "audio-preview-phrase ordered before={} after={}",
                                before.len(),
                                after.len()
                            ));
                            backend.preview_phrase(
                                &before,
                                &after,
                                chord_duration_s,
                                gap_s,
                                strum_s,
                            );
                        }
                    },
                    AudioRequest::PreviewNote(freq) => backend.preview_note(freq, 0.9),
                    AudioRequest::CalibrationStart => {
                        backend.calibration_start();
                        ui.calib_status = backend.calibration_poll();
                        ui.calib_active =
                            matches!(ui.calib_status, CalibrationStatus::Running { .. });
                    },
                    AudioRequest::CalibrationCancel => {
                        backend.calibration_cancel();
                        ui.calib_active = false;
                        ui.calib_status = CalibrationStatus::Idle;
                    },
                    AudioRequest::CalibrationAccept => {
                        if let CalibrationStatus::Success { latency_ms, .. } = ui.calib_status {
                            backend.set_latency_ms(Some(latency_ms));
                        }
                        ui.calib_status = CalibrationStatus::Idle;
                    },
                    AudioRequest::SongRecordToggle
                        if ui.looper.source == LooperSource::SavedSong
                            && last_loop_plan.is_none()
                            && last_song.is_some() =>
                    {
                        if ui.song_recording {
                            backend.loop_stop_record();
                        } else {
                            backend.song_arm_record(ui.song_edit_cursor);
                        }
                    },
                    AudioRequest::SongClearLoop
                        if ui.looper.source == LooperSource::SavedSong
                            && last_loop_plan.is_none()
                            && last_song.is_some() =>
                    {
                        backend.song_clear_loop(ui.song_edit_cursor)
                    },
                    AudioRequest::SongRecordToggle | AudioRequest::SongClearLoop => {
                        ui.loop_notice = Some(
                            "Load saved backing before recording or clearing a saved bar.".into(),
                        );
                    },
                    AudioRequest::LoopRecordToggle(id) => {
                        let result = if last_loop_plan
                            .as_ref()
                            .is_none_or(|plan| plan.index_of(id).is_none())
                        {
                            Err("That Card is not in the currently loaded backing.".into())
                        } else if backend.loop_capture_busy() {
                            backend.loop_stop_record();
                            capture_changed = true;
                            Ok(())
                        } else {
                            let plan = last_loop_plan.as_ref().expect("capture owner checked");
                            backend
                                .output_sample_rate()
                                .ok_or_else(|| "Audio output is unavailable.".to_string())
                                .and_then(|rate| {
                                    capture_persistence.permit_record(
                                        plan,
                                        rate,
                                        backend.capture_snapshot(),
                                    )
                                })
                                .and_then(|_| backend.loop_arm_record(id))
                        };
                        if let Err(reason) = result {
                            ui.loop_notice = Some(reason);
                        }
                    },
                    AudioRequest::LoopClear(id) => {
                        let visible = ui.loop_captures.takes.iter().any(|take| take.id == id)
                            || ui.loop_retained_ids.contains(&id)
                            || ui
                                .loop_admitted
                                .as_ref()
                                .is_some_and(|plan| plan.index_of(id).is_some())
                            || ui
                                .requested_loop_plan()
                                .ok()
                                .is_some_and(|plan| plan.index_of(id).is_some());
                        if !visible {
                            ui.loop_notice =
                                Some("Choose the owning Set before clearing a Card loop.".into());
                        } else {
                            match backend.clear_stored_capture(id) {
                                Ok(()) => {
                                    capture_persistence.forget(id);
                                    capture_changed = true;
                                    ui.loop_notice = None;
                                },
                                Err(reason) => ui.loop_notice = Some(reason),
                            }
                        }
                    },
                }
            }
            if capture_changed {
                admit_playback(ui, backend, last_song, last_loop_plan);
                backend.set_song_transport(ui.song_playing);
            }
            ui.latency_ms = backend.latency_ms();
            backend.song_set_record_replace(ui.song_record_replace);
            // Source controls must include delayed legacy capture as well.
            ui.song_recording = backend.loop_capture_busy();
            ui.song_loop_bars = backend.song_loop_bars();
            ui.audio_error = backend.error().map(str::to_owned);
            ui.audio_availability = backend.availability();
            if !ui.audio_availability.tuner_active {
                ui.tuner.reading = None;
            }
            ui.recorder_observation = backend.recorder_observation(last_song.as_ref());
        } else {
            ui.audio_availability = Default::default();
            ui.tuner.reading = None;
            let mut retained_requests = Vec::new();
            for request in std::mem::take(&mut ui.audio_requests) {
                match request {
                    AudioRequest::Audition(request) => {
                        ui.finish_audition_admission(
                            &request,
                            Err("Audio output is unavailable.".into()),
                        );
                    },
                    AudioRequest::LoopClear(id)
                        if ui.loop_captures.takes.iter().any(|take| take.id == id) =>
                    {
                        capture_persistence.forget(id);
                        ui.loop_notice = None;
                    },
                    other => retained_requests.push(other),
                }
            }
            ui.audio_requests = retained_requests;
        }
        sheet = Some(crate::appearance::stylesheet(ui));
        if !appearance_attempted {
            persisted_settings = serde_json::to_string(&ui.app_settings).ok();
        }
        crate::capture_persistence::persist(shared, ui);
    });

    if let Some(sheet) = sheet.and_then(|sheet| shared.reskin_sheet(sheet)) {
        // The host takes it from here: a new sheet forces a full relayout.
        *ctx.set_sheet = Some(sheet);
    }
    // No store while the persona gate is up, and nothing worth saving either:
    // what would be written is the empty state standing in for a session that
    // has not been read.
    let Some(storage) = shared.storage.as_ref() else {
        return;
    };
    if let Some(json) = persisted_settings {
        storage.save_settings(&json);
        ctx.runner
            .update(|ui| crate::session::refresh_save_status(storage, ui));
    }
}
