//! Read-only recorder presentation over Woodshed's admitted backing.
//! This is transient observation, never a second persisted recorder project.

use std::sync::Arc;
use woodshed_core::loop_plan::{LoopPlan, LooperSource};
use woodshed_core::song::SongDoc;

#[derive(Clone, Debug)]
pub struct RecorderBuffer {
    pub samples: Arc<Vec<f32>>,
    pub sample_rate: u32,
}
impl PartialEq for RecorderBuffer {
    fn eq(&self, other: &Self) -> bool {
        self.sample_rate == other.sample_rate && Arc::ptr_eq(&self.samples, &other.samples)
    }
}

/// The desktop copies only admitted source metadata and immutable PCM handles.
/// An unopened input is not unavailable: Record requests it lazily. Input rate
/// is opened configuration, not a certification of microphone signal/health.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RecorderObservation {
    pub source: Option<LooperSource>,
    pub plan: Option<LoopPlan>,
    pub saved_song: Option<SongDoc>,
    pub buffers: Vec<Option<RecorderBuffer>>,
    pub capture_target: Option<usize>,
    pub capture_epoch: Option<u64>,
    pub capture_busy: bool,
    pub capture_armed: bool,
    pub capture_invalid: bool,
    /// The active cursor, distinct from the originally armed start target.
    pub recording_bar: Option<usize>,
    pub output_rate: Option<u32>,
    pub input_rate: Option<u32>,
    /// The OS consent dialog is pending; native input has not been opened.
    pub input_permission_pending: bool,
    pub status: String,
}

use std::hash::{Hash, Hasher};
use woodshed_core::loop_plan::LoopSegmentId;

/// Saved backing retains its existing position semantics, qualified by the
/// exact admitted document. Set occurrences carry both owning identifiers.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RecorderTarget {
    Context,
    Card(LoopSegmentId),
    SavedBar(usize),
}

#[derive(Clone, Debug, PartialEq)]
enum BackingGuard {
    Set(LoopPlan),
    Saved(SongDoc),
    Draft,
}

/// Keeps the render-time session allocation alive. Reopening even the same
/// saved IDs cannot reuse this identity while an old view/action still exists.
#[derive(Clone, Debug)]
pub struct RecorderKey {
    session: Arc<()>,
    source: LooperSource,
    admitted: bool,
    capture_epoch: Option<u64>,
    guard: Arc<BackingGuard>,
    guard_hash: u64,
    pub target: RecorderTarget,
}
impl PartialEq for RecorderKey {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.session, &other.session)
            && self.source == other.source
            && self.admitted == other.admitted
            && self.capture_epoch == other.capture_epoch
            && self.target == other.target
            && self.guard_hash == other.guard_hash
            && self.guard == other.guard
    }
}
impl Eq for RecorderKey {}
impl Hash for RecorderKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (Arc::as_ptr(&self.session) as usize).hash(state);
        match self.source {
            LooperSource::SavedSong => 0u8.hash(state),
            LooperSource::Set(id) => {
                1u8.hash(state);
                id.0.hash(state);
            },
        }
        self.capture_epoch.hash(state);
        self.admitted.hash(state);
        self.guard_hash.hash(state);
        self.target.hash(state);
    }
}

pub const RECORDER_MOUNT: hocket_views::RecorderMount = hocket_views::RecorderMount::new(0x575348);

fn context(ui: &super::UiState) -> RecorderKey {
    let observation = &ui.recorder_observation;
    let source = observation.source.unwrap_or(ui.looper.source);
    let admitted = observation.source.is_some();
    let guard = match source {
        LooperSource::Set(owner) => observation
            .plan
            .as_ref()
            .filter(|plan| plan.owner == owner)
            .or_else(|| {
                if admitted {
                    None
                } else {
                    ui.loop_admitted.as_ref().filter(|plan| plan.owner == owner)
                }
            })
            .cloned()
            .or_else(|| (!admitted).then(|| ui.requested_loop_plan().ok()).flatten())
            .map(BackingGuard::Set)
            .unwrap_or(BackingGuard::Draft),
        LooperSource::SavedSong => BackingGuard::Saved(
            observation
                .saved_song
                .clone()
                .unwrap_or_else(|| ui.song.clone()),
        ),
    };
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    // A process-local rendering signature, never persisted or used as custody.
    format!("{guard:?}").hash(&mut hash);
    RecorderKey {
        session: ui.audio_session.clone(),
        source,
        admitted,
        capture_epoch: None,
        guard: Arc::new(guard),
        guard_hash: hash.finish(),
        target: RecorderTarget::Context,
    }
}

fn row_key(context: &RecorderKey, target: RecorderTarget) -> RecorderKey {
    RecorderKey {
        target,
        ..context.clone()
    }
}

fn keys(context: &RecorderKey) -> Vec<RecorderKey> {
    match context.guard.as_ref() {
        BackingGuard::Set(plan) => plan
            .segments
            .iter()
            .map(|segment| row_key(context, RecorderTarget::Card(segment.id)))
            .collect(),
        BackingGuard::Saved(song) => (0..song.bars.len())
            .map(|index| row_key(context, RecorderTarget::SavedBar(index)))
            .collect(),
        BackingGuard::Draft => Vec::new(),
    }
}

fn capture_key(ui: &super::UiState, context: &RecorderKey) -> Option<RecorderKey> {
    let index = ui.recorder_observation.capture_target?;
    let mut key = keys(context).get(index)?.clone();
    key.capture_epoch = ui.recorder_observation.capture_epoch;
    Some(key)
}
fn key_is_current(ui: &super::UiState, key: &RecorderKey) -> bool {
    let current = context(ui);
    if key.capture_epoch.is_some() {
        return capture_key(ui, &current).as_ref() == Some(key);
    }
    let expected = row_key(&current, key.target.clone());
    *key == expected && (key.target == RecorderTarget::Context || keys(&current).contains(key))
}

use hocket_views::{
    CommandControl, LaneControls, LaneProjection, RecorderProjection, RecordingState, TempoControl,
    ToggleControl, TransportProjection, WaveformProjection, WaveformSource,
};

fn command(
    target: RecorderKey,
    label: impl Into<std::borrow::Cow<'static, str>>,
    enabled: bool,
) -> CommandControl<'static, RecorderKey> {
    CommandControl {
        target,
        label: label.into(),
        enabled,
    }
}
fn toggle(
    target: RecorderKey,
    label: &'static str,
    enabled: bool,
    active: bool,
) -> ToggleControl<'static, RecorderKey> {
    ToggleControl {
        target,
        label: label.into(),
        enabled,
        active,
    }
}

fn waveform(buffer: Option<&RecorderBuffer>, unavailable: bool) -> WaveformProjection<'_> {
    match buffer {
        Some(buffer) => WaveformProjection::Data {
            revision: Arc::as_ptr(&buffer.samples) as usize as u64
                ^ u64::from(buffer.sample_rate).rotate_left(32),
            source: WaveformSource::Pcm(&buffer.samples),
        },
        None if unavailable => {
            WaveformProjection::Unavailable("Stored loop unavailable · clear or restore it".into())
        },
        None => WaveformProjection::Empty("No captured mix · select and Record".into()),
    }
}

/// Disposable view of ordered admitted segments. No Session/History conversion,
/// phrase replay or global clock is introduced by this projection.
pub fn projection(ui: &super::UiState) -> RecorderProjection<'_, RecorderKey> {
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
                        Some(seconds) => format!("{seconds:.3} s · {} bpm click · {}/4", segment.bpm, segment.beats),
                        None => format!("{} bpm · {}/4 × {}", segment.bpm, segment.beats, segment.bars),
                    },
                )
            },
            BackingGuard::Saved(song) => {
                let bar = &song.bars[index];
                (
                    format!(
                        "Bar {} · {}",
                        index + 1,
                        if bar.chord_label.is_empty() {
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
        "Capture input fault · take will be rejected on retirement".into()
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
    } else { observed
        .input_rate
        .map_or_else(
            || "Input unopened · opens on Record".to_string(),
            |rate| format!("Input configuration: {rate} Hz"),
        ) }.into();
    transport.record_note = focus
        .map_or_else(
            || if set_capture_busy {
                "Capture target unavailable".to_string()
            } else {
                "Choose an admitted segment".to_string()
            },
            |key| match key.target {
                RecorderTarget::Card(id) if set_capture_busy => format!(
                    "{} Card {}",
                    if observed.recording_bar.is_some() { "Recording" } else { "Armed" },
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
            accepted_set_capture.unwrap_or(&context).clone(), "Record", false,
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
    transport.meter_heading = if set_capture_busy { "Capture timing" } else { "Selected timing" }.into();
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

use hocket_recorder::host_policy::{HostAction, HostCapabilities, HostInput, HostPolicy};
use woodshed_core::audio::AudioRequest;

fn policy(ui: &super::UiState) -> HostPolicy<RecorderKey> {
    let context = context(ui);
    let rows = keys(&context);
    let observed = &ui.recorder_observation;
    let clearable=rows.iter().enumerate().filter(|(index,key)| {
        observed.buffers.get(*index).and_then(Option::as_ref).is_some()
            || matches!(key.target,RecorderTarget::Card(id) if ui.loop_captures.takes.iter().any(|take|take.id==id))
    }).map(|(_,key)|key.clone()).collect();
    let unavailable_media=rows.iter().enumerate().filter(|(index,key)| {
        observed.buffers.get(*index).and_then(Option::as_ref).is_none()
            && matches!(key.target,RecorderTarget::Card(id) if ui.loop_captures.takes.iter().any(|take|take.id==id))
    }).map(|(_,key)|key.clone()).collect();
    let selected = match context.guard.as_ref() {
        BackingGuard::Set(_) => rows.iter().find(
            |key| matches!(key.target,RecorderTarget::Card(id) if Some(id)==ui.loop_selected),
        ),
        BackingGuard::Saved(_) => rows.get(ui.song_edit_cursor),
        _ => None,
    }
    .or_else(|| rows.first())
    .cloned();
    HostPolicy {
        capture_target: capture_key(ui, &context),
        capture_busy: observed.capture_busy || ui.song_recording,
        context: context.clone(),
        recordable: rows.clone(),
        rows,
        clearable,
        layers: Vec::new(),
        selected,
        output_available: observed.output_rate.is_some(),
        input: HostInput::Lazy,
        backing_refusal: (!context.admitted
            || matches!(context.guard.as_ref(), BackingGuard::Draft))
        .then(|| "Load backing before using recorder runtime controls.".into()),
        unavailable_media,
        tempo: match context.guard.as_ref() {
            BackingGuard::Saved(song) => song
                .bars
                .get(ui.song_edit_cursor)
                .map_or(120.0, |bar| bar.bpm),
            _ => ui.looper.inherited_bpm,
        },
        capabilities: HostCapabilities {
            record: true,
            clear: true,
            stop_capture: true,
            play: true,
            stop: true,
            rewind: true,
            tempo: true,
            click: true,
            replace: true,
            ..Default::default()
        },
    }
}

fn desired_matches(ui: &super::UiState, key: &RecorderKey) -> bool {
    if key.source != ui.looper.source {
        return false;
    }
    match key.guard.as_ref() {
        BackingGuard::Set(plan) => ui
            .requested_loop_plan()
            .as_ref()
            .is_ok_and(|requested| requested == plan),
        BackingGuard::Saved(song) => ui.song == *song,
        BackingGuard::Draft => !key.admitted,
    }
}

/// Shared host policy validates the frozen target; Woodshed routes the effect
/// to its existing owner/config/audio command seam. Accepted state is only
/// reflected by the runtime observation, never invented here.
pub fn dispatch(ui: &mut super::UiState, action: HostAction<RecorderKey>) {
    let key = match &action {
        HostAction::NudgeTempo { target, .. } | HostAction::SetReplace { target, .. } => target,
        HostAction::Select(key)
        | HostAction::Record(key)
        | HostAction::Clear(key)
        | HostAction::StopCapture(key)
        | HostAction::Play(key)
        | HostAction::Stop(key)
        | HostAction::Rewind(key)
        | HostAction::ToggleClick(key)
        | HostAction::ToggleMasterClock(key)
        | HostAction::Add(key)
        | HostAction::Mute(key)
        | HostAction::Solo(key)
        | HostAction::LayerMute(key) => key,
    };
    if !key_is_current(ui, key) {
        ui.loop_notice =
            Some("This recorder source or target has changed. Use its current controls.".into());
        return;
    }
    // Admission runs at dispatch tail. Don't let that new admission redirect
    // a command frozen to the old document or a positional saved bar.
    let needs_desired = matches!(
        &action,
        HostAction::Record(_)
            | HostAction::Play(_)
            | HostAction::Rewind(_)
            | HostAction::ToggleClick(_)
            | HostAction::SetReplace { .. }
    ) || matches!(key.target, RecorderTarget::SavedBar(_))
        && matches!(
            &action,
            HostAction::Select(_) | HostAction::Clear(_) | HostAction::NudgeTempo { .. }
        );
    if key.source != ui.looper.source && matches!(&action, HostAction::NudgeTempo { .. })
        || needs_desired && !desired_matches(ui, key)
    {
        ui.loop_notice=Some("The requested backing differs from this rendered target. Resolve its pending edit before using this control.".into());
        return;
    }
    let effect = match policy(ui).evaluate(&action) {
        Ok(effect) => effect.into_action(),
        Err(reason) => {
            ui.loop_notice = Some(reason.to_string());
            return;
        },
    };
    ui.loop_notice = None;
    match effect {
        HostAction::Select(key) => match key.target {
            RecorderTarget::Card(id) => ui.loop_selected = Some(id),
            RecorderTarget::SavedBar(index) => ui.song_edit_cursor = index,
            _ => {},
        },
        HostAction::Record(key) => {
            let command = match key.target {
                RecorderTarget::Card(id) => woodshed_core::audio::RecorderCommand::RecordCard(id),
                RecorderTarget::SavedBar(index) => {
                    woodshed_core::audio::RecorderCommand::RecordSavedBar(index)
                },
                _ => return,
            };
            queue(ui, &key, command);
        },
        HostAction::StopCapture(key) => {
            let Some(start_bar) = ui.recorder_observation.capture_target else {
                return;
            };
            let Some(capture_epoch) = key.capture_epoch else {
                return;
            };
            queue(
                ui,
                &key,
                woodshed_core::audio::RecorderCommand::StopCapture {
                    start_bar,
                    capture_epoch,
                },
            );
        },
        HostAction::Clear(key) => {
            let command = match key.target {
                RecorderTarget::Card(id) => woodshed_core::audio::RecorderCommand::ClearCard(id),
                RecorderTarget::SavedBar(index) => {
                    woodshed_core::audio::RecorderCommand::ClearSavedBar(index)
                },
                _ => return,
            };
            queue(ui, &key, command);
        },
        HostAction::Play(_) => ui.song_playing = !ui.song_playing,
        HostAction::Stop(_) => {
            ui.song_playing = false;
            if ui.song_recording {
                if let Some(target) = policy(ui).capture_target {
                    dispatch(ui, HostAction::StopCapture(target));
                }
            }
        },
        HostAction::Rewind(_) => ui.request(AudioRequest::SongRewind),
        HostAction::NudgeTempo { target, delta } => match target.target {
            RecorderTarget::SavedBar(index) => ui.song.bars[index].nudge_bpm(delta),
            RecorderTarget::Context => {
                ui.looper.inherited_bpm = (ui.looper.inherited_bpm + delta).max(1.0)
            },
            _ => {},
        },
        HostAction::ToggleClick(key) => match key.source {
            LooperSource::Set(_) => ui.looper.click = !ui.looper.click,
            LooperSource::SavedSong => ui.song.click = !ui.song.click,
        },
        HostAction::SetReplace { replace, .. } => ui.song_record_replace = replace,
        _ => unreachable!("unsupported controls were refused by shared policy"),
    }
}

fn queue(
    ui: &mut super::UiState,
    key: &RecorderKey,
    command: woodshed_core::audio::RecorderCommand,
) {
    let backing = match key.guard.as_ref() {
        BackingGuard::Set(plan) => woodshed_core::audio::RecorderBacking::Set(plan.clone()),
        BackingGuard::Saved(song) => woodshed_core::audio::RecorderBacking::Saved(song.clone()),
        BackingGuard::Draft => return,
    };
    ui.request(AudioRequest::Recorder(
        woodshed_core::audio::RecorderRequest {
            session: key.session.clone(),
            backing,
            command,
        },
    ));
}

pub fn surface(ui: &super::UiState) -> super::UiChild {
    let display = projection(ui);
    hocket_views::projected_surface(
        &display,
        RECORDER_MOUNT,
        if (ui.viewport_width > 0.0 && ui.viewport_width < 700.0)
            || (ui.viewport_h > 0.0 && ui.viewport_h < 800.0)
        {
            hocket_views::RecorderLayout::Compact
        } else {
            hocket_views::RecorderLayout::Full
        },
        dispatch,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use woodshed_core::Lens;
    use woodshedding::rehearsal::Hold;
    fn ready() -> super::super::UiState {
        let mut ui = super::super::UiState::new();
        ui.stage.set_lens(Lens::Chords);
        for (bpm, bars) in [(90.0, 1), (150.0, 2)] {
            let mut card = ui.stage.card_from_lens().unwrap();
            card.timing.bpm = Some(bpm);
            card.timing.hold = Hold::Bars(bars);
            ui.set.push(card);
        }
        ui.looper.beats = 7;
        ui.loop_working_set(ui.working_sets.active_id);
        let plan = ui.requested_loop_plan().unwrap();
        ui.loop_selected = Some(plan.segments[0].id);
        ui.loop_admitted = Some(plan.clone());
        ui.recorder_observation = RecorderObservation {
            source: Some(LooperSource::Set(plan.owner)),
            plan: Some(plan),
            buffers: vec![
                Some(RecorderBuffer {
                    samples: Arc::new(vec![0.25, -0.5, 0.75]),
                    sample_rate: 48000,
                }),
                None,
            ],
            output_rate: Some(48000),
            status: "input opens on Record".into(),
            ..Default::default()
        };
        ui
    }
    #[test]
    fn seconds_lane_discloses_absolute_duration_and_stale_duration_command_refuses() {
        let mut ui = ready();
        ui.set.cards[0].timing.hold = Hold::Seconds(1.125);
        let plan = ui.requested_loop_plan().unwrap();
        ui.recorder_observation.plan = Some(plan.clone());
        ui.loop_admitted = Some(plan);
        let projection = projection(&ui);
        assert!(projection.lanes[0].clock.contains("1.125 s"));
        assert!(!projection.lanes[0].clock.contains("× 0"));
        assert_eq!(projection.transport.meter_heading, "Selected timing");
        let stale = projection.transport.play.as_ref().unwrap().target.clone();
        drop(projection);
        ui.set.cards[0].timing.hold = Hold::Seconds(2.0);
        let changed = ui.requested_loop_plan().unwrap();
        ui.recorder_observation.plan = Some(changed.clone());
        ui.loop_admitted = Some(changed);
        assert_ne!(stale, context(&ui));
        let playing = ui.song_playing;
        ui.audio_requests.clear();
        dispatch(&mut ui, HostAction::Play(stale));
        assert_eq!(ui.song_playing, playing);
        assert!(ui.audio_requests.is_empty());
        assert!(ui.loop_notice.as_deref().unwrap().contains("changed"));
    }

    #[test]
    fn heterogeneous_projection_uses_actual_pcm_and_render_target_without_authoring_a_second_session()
     {
        let mut ui = ready();
        let display = projection(&ui);
        assert_eq!(display.lanes[0].clock, "90 bpm · 7/4 × 1");
        assert_eq!(display.lanes[1].clock, "150 bpm · 7/4 × 2");
        assert!(display.meters.is_none());
        assert!(display.transport.add.is_none());
        assert!(display.lanes[0].controls.mute.is_none());
        let target = display.lanes[0].key.clone();
        let other = display.lanes[1].key.clone();
        drop(display);
        let session = ui.to_persisted();
        dispatch(&mut ui, HostAction::Select(other));
        dispatch(&mut ui, HostAction::Record(target.clone()));
        assert!(
            matches!(&ui.audio_requests[..],[AudioRequest::Recorder(request)] if request.command==woodshed_core::audio::RecorderCommand::RecordCard(match target.target{RecorderTarget::Card(id)=>id,_=>unreachable!()}))
        );
        assert!(!ui.song_recording);
        assert_eq!(
            serde_json::to_value(&ui.to_persisted().set).unwrap(),
            serde_json::to_value(&session.set).unwrap()
        );
    }
    #[test]
    fn stale_session_source_reorder_and_saved_bar_keys_refuse_queued_actions() {
        let mut ui = ready();
        let old = projection(&ui).lanes[0].key.clone();
        ui.audio_session = Arc::new(());
        dispatch(&mut ui, HostAction::Record(old));
        assert!(ui.audio_requests.is_empty());
        let old = projection(&ui).lanes[0].key.clone();
        ui.recorder_observation
            .plan
            .as_mut()
            .unwrap()
            .segments
            .reverse();
        dispatch(&mut ui, HostAction::Clear(old));
        assert!(ui.audio_requests.is_empty());
        let mut ui = super::super::UiState::new();
        ui.song.bars.push(Default::default());
        ui.recorder_observation.source = Some(LooperSource::SavedSong);
        ui.recorder_observation.saved_song = Some(ui.song.clone());
        ui.recorder_observation.output_rate = Some(48000);
        let old = projection(&ui).lanes[0].key.clone();
        ui.song.bars[0].cycle_root();
        dispatch(&mut ui, HostAction::Record(old));
        assert!(ui.audio_requests.is_empty());
    }
    #[test]
    fn scoped_capture_focus_survives_selection_and_returns_to_selection_when_idle() {
        let mut ui = ready();
        ui.recorder_observation.capture_busy = true;
        ui.recorder_observation.capture_target = Some(0);
        ui.recorder_observation.capture_epoch = Some(1);
        ui.song_recording = true;
        let other = projection(&ui).lanes[1].key.clone();
        dispatch(&mut ui, HostAction::Select(other));
        let selected = ui.loop_selected;
        let armed = projection(&ui);
        assert_eq!(armed.transport.record_note, "Armed Card 1");
        drop(armed);
        ui.recorder_observation.input_permission_pending = true;
        let consent = projection(&ui);
        assert!(consent.transport.status.contains("Allow microphone access in macOS"));
        assert!(consent.transport.input_status.contains("Stop cancels pending Record"));
        drop(consent);
        ui.recorder_observation.input_permission_pending = false;
        let armed = projection(&ui);
        assert!(armed.transport.status.contains("Opening microphone"));
        assert!(armed.transport.input_status.contains("recording has not started"));
        assert_eq!(armed.transport.meter_heading, "Capture timing");
        assert_eq!(armed.transport.meter_label.as_deref(), Some("90 bpm · 7/4 × 1"));
        assert!(armed.lanes[1].selected && !armed.lanes[0].selected);
        assert_eq!(armed.lanes[0].recording, RecordingState::Armed);
        drop(armed);
        ui.recorder_observation.recording_bar = Some(0);
        ui.recorder_observation.input_rate = Some(48_000);
        let display = projection(&ui);
        assert_eq!(display.lanes[0].recording, RecordingState::Recording);
        assert_eq!(display.lanes[1].recording, RecordingState::Idle);
        assert_eq!(display.transport.record_note, "Recording Card 1");
        assert_eq!(display.transport.meter_label.as_deref(), Some("90 bpm · 7/4 × 1"));
        let record = display.transport.record.as_ref().unwrap();
        assert!(!record.enabled);
        assert_eq!(record.target.capture_epoch, Some(1));
        let stop = display
            .transport
            .stop_capture
            .as_ref()
            .unwrap()
            .target
            .clone();
        drop(display);
        dispatch(&mut ui, HostAction::StopCapture(stop.clone()));
        assert!(
            matches!(&ui.audio_requests[..],[AudioRequest::Recorder(request)] if request.command==woodshed_core::audio::RecorderCommand::StopCapture{start_bar:0,capture_epoch:1})
        );
        assert!(ui.song_recording);
        let key = projection(&ui).lanes[0].key.clone();
        dispatch(&mut ui, HostAction::Clear(key));
        assert_eq!(ui.audio_requests.len(), 1);
        assert!(
            ui.loop_notice
                .as_deref()
                .unwrap()
                .contains("Stop recording")
        );
        ui.recorder_observation.capture_busy = false;
        ui.recorder_observation.capture_target = None;
        ui.recorder_observation.capture_epoch = None;
        ui.recorder_observation.recording_bar = None;
        ui.song_recording = false;
        let idle = projection(&ui);
        assert_eq!(ui.loop_selected, selected);
        assert_eq!(idle.transport.record_note, "Selected Card 2");
        assert_eq!(idle.transport.meter_heading, "Selected timing");
        assert_eq!(idle.transport.meter_label.as_deref(), Some("150 bpm · 7/4 × 2"));
        assert!(idle.transport.record.as_ref().unwrap().enabled);
        assert!(idle.transport.stop_capture.is_none());
    }
    #[test]
    fn busy_set_capture_without_accepted_mapping_never_falls_back_to_selection() {
        for (target, epoch) in [(None, Some(1)), (Some(99), Some(1)), (Some(0), None)] {
            let mut ui = ready();
            ui.loop_selected = Some(ui.recorder_observation.plan.as_ref().unwrap().segments[1].id);
            ui.recorder_observation.capture_busy = true;
            ui.recorder_observation.capture_target = target;
            ui.recorder_observation.capture_epoch = epoch;
            let display = projection(&ui);
            assert_eq!(display.transport.record_note, "Capture target unavailable");
            assert!(display.transport.status.contains("Capture target unavailable"));
            assert_eq!(display.transport.meter_heading, "Capture timing");
            assert!(display.transport.meter_label.is_none());
            let record = display.transport.record.as_ref().unwrap();
            assert!(!record.enabled);
            assert_eq!(record.target.target, RecorderTarget::Context);
            assert!(display.transport.stop_capture.is_none());
            assert!(display.transport.stop.as_ref().unwrap().enabled);
            assert!(display.lanes[1].selected);
        }
    }
    #[test]
    fn recording_lane_reports_runtime_destination_even_if_it_disagrees_with_set_target() {
        let mut ui = ready();
        ui.recorder_observation.capture_busy = true;
        ui.recorder_observation.capture_target = Some(0);
        ui.recorder_observation.capture_epoch = Some(1);
        ui.recorder_observation.recording_bar = Some(1);
        let display = projection(&ui);
        assert_eq!(display.lanes[0].recording, RecordingState::Idle);
        assert_eq!(display.lanes[1].recording, RecordingState::Recording);
        assert_eq!(display.transport.record_note, "Recording Card 1");
    }
    #[test]
    fn saved_song_capture_keeps_moving_destination_and_selection_contract() {
        let mut ui = super::super::UiState::new();
        ui.song.bars = vec![Default::default(), Default::default()];
        ui.looper.source = LooperSource::SavedSong;
        ui.song_edit_cursor = 1;
        ui.recorder_observation = RecorderObservation {
            source: Some(LooperSource::SavedSong),
            saved_song: Some(ui.song.clone()),
            capture_busy: true,
            capture_target: Some(0),
            capture_epoch: Some(1),
            recording_bar: Some(1),
            output_rate: Some(48000),
            ..Default::default()
        };
        let display = projection(&ui);
        assert_eq!(display.lanes[0].recording, RecordingState::Idle);
        assert_eq!(display.lanes[1].recording, RecordingState::Recording);
        assert!(display.lanes[1].selected);
        assert_eq!(display.transport.record_note, "Selected bar 2");
        assert_eq!(display.transport.meter_heading, "Selected timing");
        assert_eq!(display.transport.meter_label.as_ref(), Some(&display.lanes[1].clock));
        let stop = &display.transport.stop_capture.as_ref().unwrap().target;
        assert_eq!(stop.target, RecorderTarget::SavedBar(0));
        assert_eq!(stop.capture_epoch, Some(1));
    }
    #[test]
    fn mounted_component_routes_real_select_record_and_clear_controls() {
        use cambium_genet_winit_host::Harness;
        use taproot::Selector;
        let ui = ready();
        let target = ui.recorder_observation.plan.as_ref().unwrap().segments[0].id;
        let logic: Box<dyn FnMut(&super::super::UiState) -> super::super::UiChild> =
            Box::new(surface);
        let mut h = Harness::new(crate::theme::slate_stage_css(), ui, logic);
        h.layout_at(1100.0, 600.0);
        assert!(h.click_on(&Selector::class("hocket-recorder-arm")));
        assert_eq!(h.state().loop_selected, Some(target));
        assert!(h.click_on(&Selector::class("hocket-recorder-record")));
        assert!(
            matches!(&h.state().audio_requests[..],[AudioRequest::Recorder(request)] if request.command==woodshed_core::audio::RecorderCommand::RecordCard(target))
        );
        assert!(h.click_on(&Selector::class("hocket-recorder-clear")));
        assert!(
            matches!(h.state().audio_requests.last(),Some(AudioRequest::Recorder(request)) if request.command==woodshed_core::audio::RecorderCommand::ClearCard(target))
        );
    }
}

#[cfg(test)]
mod authoring_regressions {
    use super::*;
    #[test]
    fn no_backend_keeps_set_and_saved_bar_clock_authoring_without_runtime_admission() {
        let mut ui = super::super::UiState::new();
        ui.looper.source = LooperSource::Set(ui.working_sets.active_id);
        let key = projection(&ui).transport.tempo.unwrap().target;
        dispatch(
            &mut ui,
            HostAction::NudgeTempo {
                target: key,
                delta: 5.0,
            },
        );
        assert_eq!(ui.looper.inherited_bpm, 125.0);
        assert!(ui.audio_requests.is_empty());
        assert!(!ui.song_playing);
        ui.looper.source = LooperSource::SavedSong;
        ui.song.bars.push(Default::default());
        ui.song.bars.push(Default::default());
        let key = projection(&ui).transport.tempo.unwrap().target;
        dispatch(
            &mut ui,
            HostAction::NudgeTempo {
                target: key,
                delta: 5.0,
            },
        );
        assert_eq!(ui.song.bars[0].bpm, 125.0);
        assert_eq!(ui.song.bars[1].bpm, 120.0);
        assert!(ui.audio_requests.is_empty());
    }
    #[test]
    fn old_transport_controls_cannot_be_redirected_by_valid_pending_backing_edits() {
        let mut ui = super::super::UiState::new();
        ui.song.bars.push(Default::default());
        ui.recorder_observation.source = Some(LooperSource::SavedSong);
        ui.recorder_observation.saved_song = Some(ui.song.clone());
        ui.recorder_observation.output_rate = Some(48000);
        let context = projection(&ui).transport.context;
        ui.song.bars[0].cycle_root();
        for action in [
            HostAction::Play(context.clone()),
            HostAction::Rewind(context.clone()),
            HostAction::ToggleClick(context.clone()),
            HostAction::SetReplace {
                target: context.clone(),
                replace: true,
            },
        ] {
            dispatch(&mut ui, action);
            assert!(!ui.song_playing);
            assert!(ui.audio_requests.is_empty());
            assert!(ui.loop_notice.is_some());
        }
        dispatch(&mut ui, HostAction::Stop(context));
        assert!(ui.loop_notice.is_none());
    }
}

#[cfg(test)]
mod key_and_queue_regressions {
    use super::*;
    #[test]
    fn signed_zero_saved_guards_use_consistent_bit_qualified_key_equality_and_hash() {
        let mut ui = super::super::UiState::new();
        ui.song.bars.push(Default::default());
        ui.recorder_observation.source = Some(LooperSource::SavedSong);
        ui.recorder_observation.saved_song = Some(ui.song.clone());
        let plus = context(&ui);
        ui.recorder_observation.saved_song.as_mut().unwrap().bars[0].root_freq_hz = -0.0;
        let minus = context(&ui);
        let hash = |key: &RecorderKey| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            key.hash(&mut h);
            h.finish()
        };
        assert_ne!(plus, minus);
        assert_ne!(hash(&plus), hash(&minus));
        let rebuilt = context(&ui);
        assert_eq!(minus, rebuilt);
        assert_eq!(hash(&minus), hash(&rebuilt));
    }
    #[test]
    fn two_queued_saved_bar_controls_keep_distinct_frozen_targets() {
        let mut ui = super::super::UiState::new();
        ui.song.bars = vec![Default::default(), Default::default()];
        ui.recorder_observation.source = Some(LooperSource::SavedSong);
        ui.recorder_observation.saved_song = Some(ui.song.clone());
        ui.recorder_observation.output_rate = Some(48000);
        ui.recorder_observation.buffers = vec![
            Some(RecorderBuffer {
                samples: Arc::new(vec![0.1]),
                sample_rate: 48000
            });
            2
        ];
        let keys: Vec<_> = projection(&ui)
            .lanes
            .iter()
            .map(|lane| lane.key.clone())
            .collect();
        dispatch(&mut ui, HostAction::Clear(keys[0].clone()));
        dispatch(&mut ui, HostAction::Clear(keys[1].clone()));
        assert!(
            matches!(&ui.audio_requests[..],[AudioRequest::Recorder(a),AudioRequest::Recorder(b)] if a.command==woodshed_core::audio::RecorderCommand::ClearSavedBar(0) && b.command==woodshed_core::audio::RecorderCommand::ClearSavedBar(1))
        );
    }
}
