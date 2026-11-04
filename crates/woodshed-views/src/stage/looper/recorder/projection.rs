use super::*;

/// Disposable view of ordered admitted segments. No Session/History conversion,
/// phrase replay or global clock is introduced by this projection.
pub fn projection(ui: &super::super::UiState) -> RecorderProjection<'_, RecorderKey> {
    let observed = &ui.recorder_observation;
    let context = context(ui);
    let rows = keys(&context);
    let selected = match context.guard.as_ref() {
        BackingGuard::Set(_) => rows.iter().find(
            |key| matches!(key.target, RecorderTarget::Card(id) if Some(id)==ui.loop_selected),
        ),
        BackingGuard::Saved(_) => rows.get(ui.song_edit_cursor),
        BackingGuard::Draft => None,
    }
    .or_else(|| rows.first());
    let capture_target = observed.capture_target.and_then(|index| rows.get(index));
    let accepted_capture_key = capture_key(ui, &context);
    let busy = observed.capture_busy;
    let output = context.admitted && observed.output_rate.is_some();
    let mut lanes = Vec::new();
    for (index, key) in rows.iter().enumerate() {
        let (name, owner, clock) = match context.guard.as_ref() {
            BackingGuard::Set(plan) => {
                let segment = &plan.segments[index];
                (
                    segment.label.clone(),
                    format!("Set {} · Card {}", segment.id.set.0, segment.id.card.0),
                    match segment.seconds {
                        Some(seconds) => format!(
                            "{seconds:.3} s · {} bpm click · {}/4",
                            segment.bpm, segment.beats
                        ),
                        None => format!(
                            "{} bpm · {}/4 × {}",
                            segment.bpm, segment.beats, segment.bars
                        ),
                    },
                )
            },
            BackingGuard::Saved(song) => {
                let bar = &song.bars[index];
                (
                    format!(
                        "Bar {} · {}",
                        index + 1,
                        if bar.is_silent() {
                            "silent"
                        } else {
                            &bar.chord_label
                        }
                    ),
                    "Saved backing".into(),
                    format!("{} bpm · {}/4 × {}", bar.bpm, bar.beats, bar.length),
                )
            },
            BackingGuard::Draft => unreachable!(),
        };
        let unavailable = match key.target {
            RecorderTarget::Card(id) => {
                ui.loop_captures.takes.iter().any(|take| take.id == id)
                    && observed
                        .buffers
                        .get(index)
                        .and_then(Option::as_ref)
                        .is_none()
            },
            _ => false,
        };
        let recording = if observed.recording_bar == Some(index) {
            RecordingState::Recording
        } else if busy && observed.recording_bar.is_none() && capture_target == Some(key) {
            RecordingState::Armed
        } else {
            RecordingState::Idle
        };
        let clearable = observed
            .buffers
            .get(index)
            .and_then(Option::as_ref)
            .is_some()
            || unavailable;
        lanes.push(LaneProjection {
            key: key.clone(),
            name: name.into(),
            owner: owner.into(),
            clock: clock.into(),
            color: [224, 166, 75],
            selected: selected == Some(key),
            recording,
            waveform: waveform(
                observed.buffers.get(index).and_then(Option::as_ref),
                unavailable,
            ),
            layers: Vec::new(),
            controls: LaneControls {
                select: Some(command(
                    key.clone(),
                    format!(
                        "Select {}",
                        match key.target {
                            RecorderTarget::Card(id) => format!("Card {}", id.card.0),
                            RecorderTarget::SavedBar(i) => format!("bar {}", i + 1),
                            _ => String::new(),
                        }
                    ),
                    true,
                )),
                clear: Some(command(
                    key.clone(),
                    "Clear captured mix",
                    !busy && clearable,
                )),
                ..Default::default()
            },
        });
    }
    // Inspection selection remains independent of an accepted Set capture.
    // Its epoch-qualified owner, never a newly selected row, supplies busy
    // transport focus. Lane recording indicators still use runtime truth above.
    let set_capture_busy = busy && matches!(context.source, LooperSource::Set(_));
    let accepted_set_capture = accepted_capture_key.as_ref().filter(|key| {
        context.admitted
            && key.capture_epoch.is_some()
            && matches!(key.target, RecorderTarget::Card(_))
    });
    let focus = if set_capture_busy {
        accepted_set_capture
    } else {
        selected
    };
    let focus_clock = focus
        .and_then(|key| lanes.iter().find(|lane| lane.key.target == key.target))
        .map(|lane| lane.clock.clone());
    let mut transport = TransportProjection::new(context.clone());
    transport.playing = ui.song_playing;
    transport.recording = if observed.recording_bar.is_some() {
        RecordingState::Recording
    } else if busy {
        RecordingState::Armed
    } else {
        RecordingState::Idle
    };
    transport.status = if set_capture_busy && accepted_set_capture.is_none() {
        "Capture target unavailable · stop all loops before recording again".into()
    } else if observed.capture_invalid {
        if busy {
            "Capture incomplete · take will be discarded".into()
        } else {
            "Capture incomplete · no new take saved".into()
        }
    } else if busy && observed.input_permission_pending {
        "Allow microphone access in macOS · recording has not started".into()
    } else if busy && observed.recording_bar.is_none() && observed.input_rate.is_none() {
        "Opening microphone · recording starts after input admission".into()
    } else if busy && observed.recording_bar.is_none() {
        "Capture armed / waiting for playback or boundary".into()
    } else {
        observed.status.clone().into()
    };
    transport.input_status = if busy && observed.input_permission_pending {
        "Waiting for microphone permission · Stop cancels pending Record".to_string()
    } else if busy && observed.recording_bar.is_none() && observed.input_rate.is_none() {
        "Input admission pending · recording has not started".to_string()
    } else {
        observed.input_rate.map_or_else(
            || "Input unopened · opens on Record".to_string(),
            |rate| format!("Input configuration: {rate} Hz"),
        )
    }
    .into();
    transport.record_note = focus
        .map_or_else(
            || {
                if set_capture_busy {
                    "Capture target unavailable".to_string()
                } else {
                    "Choose an admitted segment".to_string()
                }
            },
            |key| match key.target {
                RecorderTarget::Card(id) if set_capture_busy => format!(
                    "{} Card {}",
                    if observed.recording_bar.is_some() {
                        "Recording"
                    } else {
                        "Armed"
                    },
                    id.card.0,
                ),
                RecorderTarget::Card(id) => format!("Selected Card {}", id.card.0),
                RecorderTarget::SavedBar(i) => format!("Selected bar {}", i + 1),
                _ => String::new(),
            },
        )
        .into();
    if set_capture_busy {
        // Keep the unavailable caption visible without targeting selection.
        transport.record = Some(command(
            accepted_set_capture.unwrap_or(&context).clone(),
            "Record",
            false,
        ));
    } else if let Some(key) = capture_target.or(selected) {
        transport.record = Some(command(key.clone(), "Record", !busy && output));
    }
    if set_capture_busy {
        if let Some(key) = accepted_set_capture {
            transport.stop_capture = Some(command(key.clone(), "Stop recording / armed", true));
        }
    } else if let Some(key) = capture_target.filter(|_| busy) {
        transport.stop_capture = Some(command(
            accepted_capture_key.clone().unwrap_or_else(|| key.clone()),
            "Stop recording / armed",
            true,
        ));
    }
    transport.play = Some(command(
        context.clone(),
        if ui.song_playing {
            "Pause backing"
        } else {
            "Play backing"
        },
        output && !rows.is_empty(),
    ));
    transport.stop = Some(command(context.clone(), "Stop all loops", output));
    transport.rewind = Some(command(context.clone(), "Rewind", output && !busy));
    transport.meter_heading = if set_capture_busy {
        "Capture timing"
    } else {
        "Selected timing"
    }
    .into();
    transport.meter_label = focus_clock;
    transport.replace = Some(toggle(
        context.clone(),
        "Replace capture",
        !busy,
        ui.song_record_replace,
    ));
    match context.source {
        LooperSource::Set(_) => {
            transport.tempo = Some(TempoControl {
                target: context.clone(),
                label: "Requested inherited tempo".into(),
                enabled: !busy,
                bpm: ui.looper.inherited_bpm,
                step: 5.0,
            });
            transport.click = Some(toggle(context.clone(), "Click", !busy, ui.looper.click));
        },
        LooperSource::SavedSong => {
            if let Some(key) = selected {
                let bpm = match context.guard.as_ref() {
                    BackingGuard::Saved(song) => {
                        song.bars[ui.song_edit_cursor.min(song.bars.len() - 1)].bpm
                    },
                    _ => 120.0,
                };
                transport.tempo = Some(TempoControl {
                    target: key.clone(),
                    label: "Selected tempo".into(),
                    enabled: !busy,
                    bpm,
                    step: 5.0,
                });
            }
            transport.click = Some(toggle(context.clone(), "Click", !busy, ui.song.click));
        },
    }
    RecorderProjection {
        title: if context.admitted {
            "Ordered backing segments".into()
        } else {
            "Draft backing · audio not admitted".into()
        },
        hint: "Select a segment · captured mix stays with its owner".into(),
        lanes,
        transport,
        meters: None,
    }
}
