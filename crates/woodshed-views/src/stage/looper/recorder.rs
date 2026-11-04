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
#[path = "recorder/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "recorder/authoring_regressions.rs"]
mod authoring_regressions;

#[cfg(test)]
#[path = "recorder/key_and_queue_regressions.rs"]
mod key_and_queue_regressions;

#[path = "recorder/projection.rs"]
mod projection;

pub use projection::projection;
