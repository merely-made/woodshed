//! The desktop [`AudioBackend`]: one root-owned output runtime behind
//! the core's seam (genet-host plan W0.1). The web host implements the
//! same trait over Web Audio / AudioWorklet.

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
    mpsc,
};
use std::time::Duration;

use woodshed_audio::output_runtime::{OutputError, OutputRuntime};
use woodshed_audio::{
    Bar, CalibrationOutcome, CalibrationSession, ChordRef, InputEngine, InputEngineBuilder,
    LooperCaptureHandle, OnsetAnalyzer, OnsetHandle, SequencerPattern, Song, Sound, Step,
    Subdivision, TimeSignature, Track, TunerHandle,
};
use woodshed_core::audio::{AudioBackend, CalibrationStatus, TransportState, TunerReading};
use woodshed_core::loop_plan::{LoopPlan, LoopSegmentId};
use woodshed_core::song::SongDoc;

mod loop_projection;
pub(crate) use loop_projection::CaptureSnapshot;
use loop_projection::LoopCaptures;

/// Context/resource/status servicing continues when the UI has no animation.
/// Worker completions wake immediately; this bounded heartbeat also observes
/// native output failures while fully stopped. It never opens a device.
struct OutputHeartbeat {
    interval_ms: Arc<AtomicU64>,
    control: mpsc::Sender<Option<u64>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl OutputHeartbeat {
    fn start(wake: Arc<dyn Fn() + Send + Sync>) -> Self {
        let interval_ms = Arc::new(AtomicU64::new(1000));
        let (control, commands) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("woodshed-output-service".into())
            .spawn(move || {
                let mut interval = 1000;
                loop {
                    match commands.recv_timeout(Duration::from_millis(interval)) {
                        Err(mpsc::RecvTimeoutError::Timeout) => wake(),
                        Ok(Some(next)) => interval = next,
                        Ok(None) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
            })
            .expect("start output service heartbeat");
        Self {
            interval_ms,
            control,
            thread: Some(thread),
        }
    }

    fn active(&self, active: bool) {
        let interval = if active { 50 } else { 1000 };
        if self.interval_ms.swap(interval, Ordering::AcqRel) != interval {
            let _ = self.control.send(Some(interval));
        }
    }
}

impl Drop for OutputHeartbeat {
    fn drop(&mut self) {
        let _ = self.control.send(None);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// A 4/4 quarter-note click at `bpm`, downbeat accented.
fn click_pattern(bpm: f32) -> SequencerPattern {
    let mut steps = vec![Step::Active { accent: true }];
    steps.extend(std::iter::repeat_n(Step::Active { accent: false }, 3));
    SequencerPattern {
        bpm,
        time_signature: TimeSignature::default(),
        subdivision: Subdivision::QUARTER,
        tracks: vec![Track {
            name: "click".to_string(),
            steps,
            sound: Sound::click(),
            muted: false,
        }],
    }
}

/// One input stream owns all three analyzers. The test-only stream variant
/// lets route tests exercise initialization without opening a device.
enum InputStream {
    Native {
        _engine: InputEngine,
    },
    #[cfg(test)]
    Test,
}

struct InputSession {
    _stream: InputStream,
    tuner: TunerHandle,
    onset: OnsetHandle,
    capture: LooperCaptureHandle,
}

impl InputSession {
    fn open() -> Result<Self, String> {
        Self::open_with(|builder| {
            builder
                .build()
                .map(|engine| InputStream::Native { _engine: engine })
                .map_err(|e| format!("audio input: {e}"))
        })
    }

    fn open_with(
        open_stream: impl FnOnce(InputEngineBuilder) -> Result<InputStream, String>,
    ) -> Result<Self, String> {
        let onset_analyzer = OnsetAnalyzer::new();
        let onset = onset_analyzer.handle();
        let (builder, capture) = InputEngineBuilder::new()
            .with_analyzer(onset_analyzer)
            .with_looper_capture();
        let (builder, tuner) = builder.with_pitch();
        tuner.set_enabled(false);
        Ok(Self {
            _stream: open_stream(builder)?,
            tuner,
            onset,
            capture,
        })
    }
}

#[cfg(test)]
type InputFactory = Box<dyn FnMut() -> Result<InputSession, String>>;

pub struct CpalBackend {
    /// One context, device stream and sample clock serve all output lanes.
    output: Option<OutputRuntime>,
    native_output: bool,
    heartbeat: Option<OutputHeartbeat>,
    last_metronome: Option<TransportState>,
    song_transport_requested: bool,
    /// Opened only when an input feature is requested; retained across
    /// disabling features so all analyzers keep sharing one cpal stream.
    input: Option<InputSession>,
    /// Desired tuner state, including a failed enable request. Repeated
    /// synchronization must not reopen the microphone after a failure.
    tuner_requested: bool,
    capture_attached: bool,
    /// Render-time target accepted by arm_capture, retained until returns drain.
    recorder_capture_target: Option<usize>,
    recorder_capture_epoch: u64,
    session_reset_pending: bool,
    #[cfg(test)]
    input_factory: Option<InputFactory>,
    /// The latency-calibration session driver.
    calib: CalibrationSession,
    /// Accepted input→output round-trip latency compensation (ms).
    latency_ms: Option<f32>,
    captures: LoopCaptures,
    /// Output failures survive an input retry.
    initial_error: Option<String>,
    error: Option<String>,
}

/// Convert the core's neutral song document into the audio model.
fn to_song(doc: &SongDoc) -> Song {
    let mut song = Song::new();
    song.name = doc.name.clone();
    song.one_shot = doc.one_shot;
    song.click_enabled = doc.click;
    song.bars = doc
        .bars
        .iter()
        .map(|b| Bar {
            legacy_clock: false,
            bpm: b.bpm,
            time_signature: TimeSignature::new(b.beats, 4),
            subdivision: Subdivision::QUARTER,
            chord_ref: (b.root_freq_hz > 0.0).then(|| ChordRef {
                formula_name: b.formula_name.clone(),
                root_freq_hz: b.root_freq_hz,
                pitches_hz: b.pitches_hz.clone(),
                label: b.chord_label.clone(),
            }),
            audio_buffer: None,
            label: b.label.clone(),
            length: b.length,
        })
        .collect();
    if song.bars.is_empty() {
        song.bars.push(Bar::default());
    }
    song
}

impl CpalBackend {
    pub(crate) fn output_sample_rate(&self) -> Option<u32> {
        self.output.as_ref().map(OutputRuntime::sample_rate)
    }

    /// Read admitted backing and shared PCM without opening input, admitting a
    /// document, or invoking export (which deliberately refuses busy capture).
    pub(crate) fn recorder_observation(
        &self,
        saved_song: Option<&SongDoc>,
    ) -> woodshed_views::stage::RecorderObservation {
        use woodshed_views::stage::{RecorderBuffer, RecorderObservation};
        if self.session_reset_pending {
            return RecorderObservation::default();
        }
        let output_rate = self
            .output
            .as_ref()
            .filter(|_| !self.native_output || self.output_stream_count() != 0)
            .map(OutputRuntime::sample_rate);
        let (snapshot, capture_busy) = self
            .output
            .as_ref()
            .map(OutputRuntime::capture_observation)
            .unwrap_or_default();
        let plan = self.captures.plan.clone();
        let source = plan
            .as_ref()
            .map(|plan| woodshed_core::loop_plan::LooperSource::Set(plan.owner))
            .or_else(|| saved_song.map(|_| woodshed_core::loop_plan::LooperSource::SavedSong));
        let buffers = self
            .output
            .as_ref()
            .map(|output| {
                output
                    .song()
                    .bars
                    .into_iter()
                    .map(|bar| {
                        bar.audio_buffer.map(|buffer| RecorderBuffer {
                            samples: buffer.data,
                            sample_rate: buffer.sample_rate_hz,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        RecorderObservation {
            source,
            plan,
            saved_song: saved_song.cloned(),
            buffers,
            capture_target: self.recorder_capture_target.filter(|_| capture_busy),
            capture_epoch: capture_busy.then_some(self.recorder_capture_epoch),
            capture_busy,
            capture_armed: snapshot.capture_armed,
            capture_invalid: snapshot.capture_invalid,
            recording_bar: snapshot.recording.then_some(snapshot.cursor.bar_idx),
            output_rate,
            input_rate: self.capture_source_rate(),
            status: self.error().map(str::to_owned).unwrap_or_else(|| {
                if output_rate.is_none() {
                    "Audio output is unavailable.".into()
                } else if self.input.is_none() {
                    "One shared output · input opens on Record".into()
                } else {
                    "One shared output · input configuration opened".into()
                }
            }),
        }
    }

    /// Runtime execution of a frozen saved-bar request; positional targets do
    /// not pass through the mutable UI editor cursor.
    pub(crate) fn recorder_arm_saved(&mut self, index: usize) -> Result<(), String> {
        if self.loop_capture_busy() {
            return Err("Capture is already armed or active.".into());
        }
        self.check_set_capture_quota()?;
        self.ensure_capture_input()?;
        if let Some(input) = &self.input {
            input.capture.set_enabled(true);
        }
        if let Err(error) = self.with_output(|output| output.arm_capture(index, false)) {
            if let Some(input) = &self.input {
                input.capture.set_enabled(false);
            }
            return Err(error);
        }
        self.recorder_capture_epoch = self.recorder_capture_epoch.wrapping_add(1);
        self.recorder_capture_target = Some(index);
        Ok(())
    }
    pub(crate) fn recorder_clear_saved(&mut self, index: usize) -> Result<(), String> {
        if self.loop_capture_busy() {
            return Err("Stop recording / armed capture before clearing a saved bar.".into());
        }
        self.with_output(|output| output.clear_recording(index))
    }
    pub(crate) fn recorder_stop_capture(&mut self, index: usize, epoch: u64) -> Result<(), String> {
        if !self.loop_capture_busy()
            || self.recorder_capture_target != Some(index)
            || self.recorder_capture_epoch != epoch
        {
            return Err("The accepted capture has changed. Use its current Stop control.".into());
        }
        self.with_output(OutputRuntime::stop_capture_now)
    }

    /// Immutable completed material from the accepted output document. No
    /// transport command, input stream or requested projection is realized.
    pub(crate) fn export_backing(&mut self) -> Result<(Song, u32), String> {
        self.service();
        if self.session_reset_pending {
            return Err("Captured loops are waiting for the current session to restore.".into());
        }
        if self.loop_capture_busy() {
            return Err("Stop recording / armed capture before exporting WAV.".into());
        }
        let output = self.output.as_ref().ok_or("Audio output is unavailable.")?;
        Ok((output.song(), output.sample_rate()))
    }

    /// The caller services completed output returns before exporting this
    /// cheap immutable snapshot. Inactive Sets retain their completed takes.
    pub(crate) fn capture_snapshot(&self) -> Vec<CaptureSnapshot> {
        if self.session_reset_pending {
            Vec::new()
        } else {
            self.captures.snapshot()
        }
    }

    /// Hydrate completed takes after the host has cleared a prior session's
    /// audio identity. Installing custody does not replace accepted output;
    /// normal guarded Set-plan admission resolves these buffers by owner/Card.
    pub(crate) fn restore_loop_captures(
        &mut self,
        takes: Vec<CaptureSnapshot>,
    ) -> Result<(), String> {
        self.finish_session_reset()?;
        if self.loop_capture_busy() {
            return Err(
                "Stop recording / armed capture before restoring stored Set captures.".into(),
            );
        }
        self.captures.restore(takes)
    }

    /// Explicit recovery can clear a stored take before its edited Card clock
    /// is admissible. Other loaded material and other owners remain intact.
    pub(crate) fn clear_stored_capture(&mut self, id: LoopSegmentId) -> Result<(), String> {
        if id.set.0 == 0 || id.card.0 == 0 {
            return Err("Stored capture clear requires a nonzero Set/Card identity.".into());
        }
        self.finish_session_reset()?;
        self.service();
        if self.loop_capture_busy() {
            return Err(
                "Stop recording / armed capture before clearing a stored Card loop.".into(),
            );
        }
        if let Some(index) = self
            .captures
            .plan
            .as_ref()
            .and_then(|plan| plan.index_of(id))
        {
            self.with_output(|output| output.clear_recording(index))?;
        }
        self.captures.clear(id);
        Ok(())
    }

    pub fn preview_busy(&self) -> bool {
        self.output
            .as_ref()
            .is_some_and(OutputRuntime::preview_busy)
    }

    pub fn preview_voice_count(&self) -> usize {
        self.output
            .as_ref()
            .map_or(0, |output| output.snapshot().preview_voices)
    }
    pub(crate) fn output_observation_id(&self) -> Option<u64> {
        self.output.as_ref().map(OutputRuntime::observation_id)
    }

    pub fn output_stream_count(&self) -> usize {
        self.output
            .as_ref()
            .map_or(0, OutputRuntime::native_stream_count)
    }

    pub fn output_snapshot(&self) -> Option<woodshed_audio::OutputSnapshot> {
        self.output.as_ref().map(OutputRuntime::snapshot)
    }

    pub fn capture_source_rate(&self) -> Option<u32> {
        self.input.as_ref().map(|input| input.capture.source_rate())
    }

    pub fn song_playing(&self) -> bool {
        (!self.native_output || self.output_stream_count() != 0)
            && self
                .output
                .as_ref()
                .is_some_and(|output| output.song().playing)
    }

    /// Acknowledging published status separates an actual new UI Play from
    /// stale true that ordinary dispatch still holds after a one-shot ends.
    pub fn observe_song_playing(&mut self) -> bool {
        let playing = self.song_playing();
        self.song_transport_requested = playing;
        playing
    }

    pub fn needs_service(&self) -> bool {
        (!self.native_output || self.output_stream_count() != 0)
            && self
                .output
                .as_ref()
                .is_some_and(OutputRuntime::needs_service)
    }

    /// Register both completion wakeups and the bounded idle context service.
    pub fn set_wake(&mut self, wake: Arc<dyn Fn() + Send + Sync>) {
        if let Some(output) = &mut self.output {
            output.set_wake(wake.clone());
            self.heartbeat = Some(OutputHeartbeat::start(wake));
        }
    }

    /// Context updates, capture retirement and stream failures are control
    /// work, including when every product transport is stopped.
    pub fn service(&mut self) {
        if let Some(output) = &mut self.output {
            let result = output.service();
            let problem = result.err().or_else(|| output.last_error().cloned());
            if let Some(error) = problem {
                self.output_failure(error);
            }
        }
        if let Some(output) = &self.output {
            self.captures.remember(&output.song());
        }
        // Disabling the producer before Stop reaches the processor could
        // invalidate a valid take in a still-running block. Await retirement.
        if !self.loop_capture_busy() {
            self.recorder_capture_target = None;
            if let Some(input) = &self.input {
                input.capture.set_enabled(false);
            }
        }
        if let Some(heartbeat) = &self.heartbeat {
            heartbeat.active(self.needs_service());
        }
    }

    fn output_failure(&mut self, error: OutputError) -> String {
        let message = format!("audio output: {error}");
        self.initial_error = Some(message.clone());
        self.error = Some(message.clone());
        message
    }

    /// Exact control-side admission for an already resolved Card request.
    /// Return this operation's result, not an unrelated persistent error.
    pub(crate) fn admit_card_preview(&mut self, preview: &woodshed_core::card_timing::CardPreview) -> Result<(), String> {
        let output = self.output.as_mut().ok_or_else(|| "Audio output is unavailable.".to_string())?;
        // The view checks unmarked availability before declaring silence.
        // Silence still requires the output to retire the preceding preview.
        if preview.0.is_empty() { return output.cancel_preview().map_err(|error| error.to_string()); }
        output.preview_card_pitches(&preview.0, preview.1, preview.2).map_err(|error| error.to_string())
    }

    fn with_output<T>(
        &mut self,
        action: impl FnOnce(&mut OutputRuntime) -> Result<T, OutputError>,
    ) -> Result<T, String> {
        let Some(output) = self.output.as_mut() else {
            return Err(self
                .initial_error
                .clone()
                .unwrap_or_else(|| "Audio output is unavailable.".into()));
        };
        let result = action(output);
        result.map_err(|error| self.output_failure(error))
    }

    /// Open one output stream at startup. The shared microphone stream opens
    /// on the first tuner, calibration, or recording request. Missing devices
    /// degrade and [`error`](AudioBackend::error) reports the failure.
    pub fn new() -> Self {
        Self::with_runtime(OutputRuntime::new())
    }

    fn with_runtime(runtime: Result<OutputRuntime, OutputError>) -> Self {
        let (output, error) = match runtime {
            Ok(output) => (Some(output), None),
            Err(error) => (None, Some(format!("audio output: {error}"))),
        };
        let native_output = output
            .as_ref()
            .is_some_and(|output| output.native_stream_count() != 0);
        Self {
            output,
            native_output,
            heartbeat: None,
            last_metronome: None,
            song_transport_requested: false,
            input: None,
            tuner_requested: false,
            capture_attached: false,
            recorder_capture_target: None,
            recorder_capture_epoch: 0,
            session_reset_pending: false,
            #[cfg(test)]
            input_factory: None,
            calib: CalibrationSession::new(),
            latency_ms: None,
            captures: LoopCaptures::default(),
            initial_error: error.clone(),
            error,
        }
    }

    #[cfg(test)]
    pub(crate) fn offline_for_test() -> Self {
        let mut backend = Self::with_runtime(OutputRuntime::offline(48_000));
        backend.input_factory = Some(Box::new(|| panic!("unexpected native input request")));
        backend
    }

    #[cfg(test)]
    pub(crate) fn process_for_test(&mut self, frames: usize) {
        self.service();
        self.output
            .as_mut()
            .unwrap()
            .render_offline(&mut vec![0.0; frames * 2])
            .unwrap();
        self.service();
    }

    /// Explicit input requests can retry an earlier failure. A successful
    /// session is reused for every feature for the backend's lifetime.
    fn ensure_input(&mut self) -> bool {
        if self.input.is_some() {
            return true;
        }
        #[cfg(test)]
        let result = match self.input_factory.as_mut() {
            Some(factory) => factory(),
            None => InputSession::open(),
        };
        #[cfg(not(test))]
        let result = InputSession::open();
        match result {
            Ok(input) => {
                input.tuner.set_enabled(self.tuner_requested);
                self.input = Some(input);
                self.error = self.initial_error.clone();
                true
            },
            Err(error) => {
                self.error = Some(match self.initial_error.as_ref() {
                    Some(initial) => format!("{initial}; {error}"),
                    None => error,
                });
                false
            },
        }
    }

    fn ensure_capture_input(&mut self) -> Result<(), String> {
        self.finish_session_reset()?;
        if self.output.is_none() {
            return Err(self
                .initial_error
                .clone()
                .unwrap_or_else(|| "Audio output is unavailable.".into()));
        }
        if !self.ensure_input() {
            return Err(self
                .error
                .clone()
                .unwrap_or_else(|| "Audio input is unavailable.".into()));
        }
        let input = self.input.as_ref().expect("input was opened");
        let source_rate = input.capture.source_rate();
        let output_rate = self.output.as_ref().expect("output exists").sample_rate();
        if source_rate == 0 || source_rate != output_rate {
            let message = format!(
                "Recording input rate {source_rate} Hz does not match output rate {output_rate} Hz. The loaded backing remains unchanged."
            );
            self.error = Some(message.clone());
            return Err(message);
        }
        if !self.capture_attached {
            let capture = input.capture.clone();
            self.with_output(|output| output.attach_capture_handle(&capture))?;
            self.capture_attached = true;
        }
        Ok(())
    }

    fn check_set_capture_quota(&self) -> Result<(), String> {
        if self.captures.plan.is_none() {
            return Ok(());
        }
        let output = self
            .output
            .as_ref()
            .ok_or("Set capture output is unavailable.")?;
        self.captures
            .check_capture_quota(&output.song(), output.sample_rate())
    }

    /// A reset that met output backpressure must finish before admitting
    /// incoming session-local ids. Old PCM cannot be inherited by those ids.
    fn finish_session_reset(&mut self) -> Result<(), String> {
        if self.session_reset_pending {
            if self.output.is_some() {
                self.with_output(OutputRuntime::clear_session)?;
            }
            self.captures = LoopCaptures::default();
            self.session_reset_pending = false;
        }
        Ok(())
    }

    /// End a calibration run: disable onset detection and restore the
    /// ordinary click lane, stopped. A refused restoration leaves the cache
    /// invalid so ordinary synchronization retries the full pattern/state.
    fn end_calibration(&mut self) {
        if let Some(o) = self.input.as_ref().map(|input| &input.onset) {
            o.set_enabled(false);
        }
        let _ = self
            .with_output(|output| output.set_click_pattern_playing(click_pattern(120.0), false));
        self.last_metronome = None;
    }
}

impl AudioBackend for CpalBackend {
    fn set_metronome(&mut self, transport: TransportState) {
        if self.last_metronome == Some(transport) {
            return;
        }
        let restore_pattern = self.last_metronome.is_none();
        if self
            .with_output(|output| {
                if restore_pattern {
                    output
                        .set_click_pattern_playing(click_pattern(transport.bpm), transport.playing)
                } else {
                    output.set_metronome(transport.bpm, transport.playing)
                }
            })
            .is_ok()
        {
            self.last_metronome = Some(transport);
        }
    }

    fn set_tuner_enabled(&mut self, enabled: bool) {
        let enabling = enabled && !self.tuner_requested;
        self.tuner_requested = enabled;
        if enabling && !self.ensure_input() {
            return;
        }
        if let Some(input) = self.input.as_ref() {
            if input.tuner.is_enabled() != enabled {
                input.tuner.set_enabled(enabled);
            }
        }
    }

    fn tuner_reading(&self) -> Option<TunerReading> {
        let tuner = &self.input.as_ref()?.tuner;
        if !tuner.is_enabled() {
            return None;
        }
        let snap = tuner.snapshot();
        let level = snap.input_level;
        snap.note.map(|n| TunerReading {
            note: n.name.to_string(),
            octave: n.octave,
            cents: n.cents_offset,
            in_tune: n.in_tune,
            level,
        })
    }

    fn set_song(&mut self, doc: &SongDoc) {
        let _ = self.try_set_song(doc);
    }

    fn try_set_song(&mut self, doc: &SongDoc) -> Result<(), String> {
        if doc
            .bars
            .iter()
            .any(|bar| !bar.bpm.is_finite() || bar.bpm <= 0.0 || bar.beats == 0 || bar.length == 0)
        {
            return Err("Saved backing contains an unsupported tempo, meter or hold. Its authored payload remains unchanged.".into());
        }
        self.finish_session_reset()?;
        self.service();
        if self.loop_capture_busy() {
            return Err("Stop recording / armed capture before updating the loaded backing. The currently loaded material remains active.".into());
        }
        let current = self
            .output
            .as_ref()
            .ok_or("Audio output is unavailable.")?
            .song();
        self.captures.remember(&current);
        let mut projected = to_song(doc);
        // Saved bars retain their index-based compatibility custody. No Set
        // capture is reassigned to that vector merely because source changed.
        for (bar, buffer) in projected.bars.iter_mut().zip(&self.captures.saved_song) {
            bar.audio_buffer = buffer.clone();
        }
        projected.playing = current.playing;
        self.with_output(|output| output.set_song(projected))?;
        self.captures.plan = None;
        if let Some(input) = &self.input {
            input.capture.set_enabled(false);
        }
        Ok(())
    }

    fn set_loop_plan(&mut self, plan: &LoopPlan) -> Result<(), String> {
        self.finish_session_reset()?;
        self.service();
        if self.loop_capture_busy() {
            return Err("Stop recording / armed capture before updating the loaded backing. The currently loaded Cards remain active.".into());
        }
        let current = self
            .output
            .as_ref()
            .ok_or("Set backing output is unavailable.")?
            .song();
        let mut projected = self.captures.prepare_replacement(&current, plan)?;
        projected.playing = current.playing;
        self.with_output(|output| output.set_song(projected))?;
        self.captures.plan = Some(plan.clone());
        Ok(())
    }

    fn clear_loop_session(&mut self) {
        self.recorder_capture_target = None;
        if let Some(input) = &self.input {
            input.capture.set_enabled(false);
            input.capture.clear();
        }
        self.session_reset_pending = true;
        let _ = self.finish_session_reset();
    }

    fn loop_arm_record(&mut self, id: LoopSegmentId) -> Result<(), String> {
        let index = self.captures.index_of(id)?;
        if self.loop_capture_busy() {
            return Err("Capture is already armed or active.".into());
        }
        self.check_set_capture_quota()?;
        self.ensure_capture_input()?;
        if let Some(input) = &self.input {
            input.capture.set_enabled(true);
        }
        if let Err(error) = self.with_output(|output| output.arm_capture(index, true)) {
            if let Some(input) = &self.input {
                input.capture.set_enabled(false);
            }
            return Err(error);
        }
        self.recorder_capture_epoch = self.recorder_capture_epoch.wrapping_add(1);
        self.recorder_capture_target = Some(index);
        Ok(())
    }

    fn loop_clear(&mut self, id: LoopSegmentId) -> Result<(), String> {
        let index = self.captures.index_of(id)?;
        if self.loop_capture_busy() {
            return Err("Stop recording / armed capture before clearing a Card loop.".into());
        }
        self.with_output(|output| output.clear_recording(index))?;
        self.captures.clear(id);
        Ok(())
    }

    fn loop_capture_busy(&self) -> bool {
        self.output
            .as_ref()
            .is_some_and(OutputRuntime::capture_busy)
    }

    fn loop_stop_record(&mut self) {
        let _ = self.with_output(OutputRuntime::stop_capture_now);
    }

    fn set_song_transport(&mut self, playing: bool) {
        if self.song_transport_requested != playing {
            let accepted = if playing {
                self.with_output(OutputRuntime::play)
            } else {
                self.with_output(OutputRuntime::stop)
            };
            if accepted.is_ok() {
                self.song_transport_requested = playing;
            }
        }
    }

    fn song_rewind(&mut self) {
        let _ = self.with_output(OutputRuntime::rewind);
    }

    fn song_bar(&self) -> Option<usize> {
        self.output
            .as_ref()
            .map(|output| output.snapshot().cursor.bar_idx)
    }

    fn cancel_preview(&mut self) {
        let _ = self.with_output(OutputRuntime::cancel_preview);
    }

    fn preview_pitches(&mut self, pitches_hz: &[f32], duration_secs: f32, strum_ms: f32) {
        let _ =
            self.with_output(|output| output.preview_pitches(pitches_hz, duration_secs, strum_ms));
    }

    fn preview_phrase(
        &mut self,
        before_hz: &[f32],
        after_hz: &[f32],
        chord_duration_secs: f32,
        gap_secs: f32,
        strum_ms: f32,
    ) {
        let _ = self.with_output(|output| {
            output.preview_phrase(before_hz, after_hz, chord_duration_secs, gap_secs, strum_ms)
        });
    }

    fn preview_note(&mut self, freq_hz: f32, duration_secs: f32) {
        let _ = self.with_output(|output| output.preview_note(freq_hz, duration_secs));
    }

    fn calibration_start(&mut self) {
        if !self.ensure_input() {
            return;
        }
        let Some(onset) = self.input.as_ref().map(|input| input.onset.clone()) else {
            return;
        };
        let Some(output) = self.output.as_mut() else {
            return;
        };
        let result = self.calib.start(output, &onset);
        if let Err(error) = result {
            self.output_failure(error);
            onset.set_enabled(false);
            // The atomic start retained prior output. Ordinary sync must
            // nevertheless restore its full request before caching it again.
            self.last_metronome = None;
        }
    }

    fn calibration_poll(&mut self) -> CalibrationStatus {
        let Some(onset) = self.input.as_ref().map(|input| input.onset.clone()) else {
            return CalibrationStatus::Unavailable;
        };
        let Some(output) = self.output.as_mut() else {
            return CalibrationStatus::Unavailable;
        };
        let result = self.calib.poll(output, &onset);
        let outcome = match result {
            Ok(outcome) => outcome,
            Err(error) => {
                self.output_failure(error);
                self.end_calibration();
                return CalibrationStatus::Unavailable;
            },
        };
        match outcome {
            CalibrationOutcome::InProgress {
                clicks_fired,
                total_clicks,
            } => CalibrationStatus::Running {
                clicks_fired,
                total: total_clicks,
            },
            CalibrationOutcome::Success {
                latency,
                matched_pairs,
                total_clicks,
            } => {
                self.end_calibration();
                CalibrationStatus::Success {
                    latency_ms: latency.as_secs_f32() * 1000.0,
                    matched: matched_pairs,
                    total: total_clicks,
                }
            },
            CalibrationOutcome::InsufficientPairs {
                matched_pairs,
                total_clicks,
            } => {
                self.end_calibration();
                CalibrationStatus::Insufficient {
                    matched: matched_pairs,
                    total: total_clicks,
                }
            },
            CalibrationOutcome::EngineUnavailable => CalibrationStatus::Unavailable,
        }
    }

    fn calibration_cancel(&mut self) {
        if let Some(output) = self.output.as_mut() {
            let result = self.calib.cancel(output);
            if let Err(error) = result {
                self.output_failure(error);
            }
        }
        self.end_calibration();
    }

    fn latency_ms(&self) -> Option<f32> {
        self.latency_ms
    }

    fn set_latency_ms(&mut self, ms: Option<f32>) {
        self.latency_ms = ms;
    }

    fn song_arm_record(&mut self, bar_idx: usize) {
        if let Err(error) = self.recorder_arm_saved(bar_idx) {
            self.error = Some(error);
        }
    }

    fn song_stop_record(&mut self) {
        self.loop_stop_record();
    }

    fn song_clear_loop(&mut self, bar_idx: usize) {
        let _ = self.with_output(|output| output.clear_recording(bar_idx));
    }

    fn song_set_record_replace(&mut self, replace: bool) {
        if self
            .output
            .as_ref()
            .is_some_and(|output| output.song().record_replace != replace)
        {
            let _ = self.with_output(|output| output.set_record_replace(replace));
        }
    }

    fn song_recording(&self) -> bool {
        self.output
            .as_ref()
            .map(|output| output.snapshot().recording)
            .unwrap_or(false)
    }

    fn song_loop_bars(&self) -> Vec<bool> {
        self.output
            .as_ref()
            .map(|output| {
                output
                    .song()
                    .bars
                    .iter()
                    .map(|bar| bar.audio_buffer.is_some())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    /// Production graph/control and analyzer construction with device opens
    /// replaced. Request routes still run; no native devices are needed.
    fn backend(factory: InputFactory) -> CpalBackend {
        let mut backend = CpalBackend::offline_for_test();
        backend.input_factory = Some(factory);
        backend
    }

    fn successful_input() -> Result<InputSession, String> {
        InputSession::open_with(|mut builder| {
            builder.prepare_analyzers(48_000);
            Ok(InputStream::Test)
        })
    }

    fn render(audio: &mut CpalBackend, frames: usize) {
        audio.process_for_test(frames);
    }

    #[test]
    fn admitted_card_silence_retires_the_previous_preview_and_requires_output() {
        let mut audio = CpalBackend::offline_for_test();
        audio.admit_card_preview(&(vec![440.0], 1.0, 0.0)).unwrap();
        let began = std::time::Instant::now();
        while audio.preview_voice_count() == 0 {
            assert!(began.elapsed() < std::time::Duration::from_secs(2));
            render(&mut audio, 128);
            std::thread::yield_now();
        }
        audio.admit_card_preview(&(Vec::new(), 0.0, 0.0)).unwrap();
        render(&mut audio, 128);
        assert_eq!(audio.preview_voice_count(), 0);
        assert!(!audio.preview_busy());
        audio.output = None;
        assert!(audio.admit_card_preview(&(Vec::new(), 0.0, 0.0)).is_err());
    }

    #[test]
    fn ordinary_synchronization_and_polls_do_not_open_microphone() {
        let mut audio = backend(Box::new(|| panic!("unexpected microphone open")));
        for _ in 0..10 {
            // These are the backend operations ordinary UI dispatch and frame
            // polling perform, including an appearance edit with tuner off.
            audio.set_metronome(TransportState::default());
            audio.set_tuner_enabled(false);
            audio.set_song(&SongDoc::default());
            audio.set_song_transport(false);
            audio.song_set_record_replace(false);
            audio.preview_note(440.0, 0.1);
            audio.preview_pitches(&[440.0], 0.1, 0.0);
            assert!(audio.tuner_reading().is_none());
            assert_eq!(audio.calibration_poll(), CalibrationStatus::Unavailable);
            audio.calibration_cancel();
            audio.song_stop_record();
            render(&mut audio, 256);
        }
        assert!(audio.input.is_none());
        assert!(audio.error().is_none());
    }

    #[test]
    fn each_input_feature_can_open_the_one_shared_session() {
        for first in 0..3 {
            let attempts = Rc::new(Cell::new(0));
            let counter = Rc::clone(&attempts);
            let mut audio = backend(Box::new(move || {
                counter.set(counter.get() + 1);
                successful_input()
            }));
            match first {
                0 => audio.set_tuner_enabled(true),
                1 => audio.calibration_start(),
                _ => audio.song_arm_record(0),
            }
            assert_eq!(attempts.get(), 1);
            let input = audio.input.as_ref().unwrap();
            let tuner = input.tuner.clone();
            let capture = input.capture.clone();
            audio.set_tuner_enabled(true);
            audio.calibration_start();
            audio.song_arm_record(0);
            assert_eq!(attempts.get(), 1);
            assert!(tuner.is_enabled());
            assert!(capture.is_enabled());
            assert_eq!(capture.source_rate(), 48_000);
            assert!(audio.capture_attached);
            assert!(
                audio
                    .input
                    .as_ref()
                    .unwrap()
                    .capture
                    .take_consumer()
                    .is_none()
            );
            audio.set_tuner_enabled(false);
            assert!(!tuner.is_enabled());
            assert!(capture.is_enabled());
            audio.song_stop_record();
            render(&mut audio, 32);
            assert!(!capture.is_enabled());
            audio.set_tuner_enabled(true);
            assert_eq!(attempts.get(), 1);
            assert!(tuner.is_enabled());
        }
    }

    #[test]
    fn failed_tuner_enable_is_visible_and_only_explicit_reenable_retries() {
        let attempts = Rc::new(Cell::new(0));
        let counter = Rc::clone(&attempts);
        let mut audio = backend(Box::new(move || {
            counter.set(counter.get() + 1);
            if counter.get() == 1 {
                Err("audio input: no microphone".into())
            } else {
                successful_input()
            }
        }));
        audio.set_tuner_enabled(true);
        assert_eq!(audio.error(), Some("audio input: no microphone"));
        assert!(audio.tuner_reading().is_none());
        for _ in 0..10 {
            audio.set_tuner_enabled(true);
            audio.set_metronome(TransportState::default());
        }
        assert_eq!(attempts.get(), 1);
        audio.set_tuner_enabled(false);
        assert_eq!(attempts.get(), 1);
        audio.set_tuner_enabled(true);
        assert_eq!(attempts.get(), 2);
        assert!(audio.input.as_ref().unwrap().tuner.is_enabled());
        assert!(audio.error().is_none());
    }

    #[test]
    fn explicit_calibration_and_recording_requests_retry_without_losing_output_error() {
        let attempts = Rc::new(Cell::new(0));
        let counter = Rc::clone(&attempts);
        let mut audio = backend(Box::new(move || {
            counter.set(counter.get() + 1);
            if counter.get() <= 2 {
                Err("audio input: unavailable".into())
            } else {
                successful_input()
            }
        }));
        audio.initial_error = Some("audio output: unavailable".into());
        audio.error = audio.initial_error.clone();
        audio.calibration_start();
        assert_eq!(attempts.get(), 1);
        assert_eq!(audio.calibration_poll(), CalibrationStatus::Unavailable);
        audio.song_arm_record(0);
        assert_eq!(attempts.get(), 2);
        assert!(audio.input.is_none());
        assert!(!audio.song_recording());
        assert_eq!(
            audio.error(),
            Some("audio output: unavailable; audio input: unavailable")
        );
        audio.song_arm_record(0);
        assert_eq!(attempts.get(), 3);
        assert!(audio.input.as_ref().unwrap().capture.is_enabled());
        assert_eq!(audio.error(), Some("audio output: unavailable"));
    }

    #[test]
    fn recording_retry_restores_pending_tuner_enable_on_same_session() {
        let attempts = Rc::new(Cell::new(0));
        let counter = Rc::clone(&attempts);
        let mut audio = backend(Box::new(move || {
            counter.set(counter.get() + 1);
            if counter.get() == 1 {
                Err("audio input: unavailable".into())
            } else {
                successful_input()
            }
        }));
        audio.set_tuner_enabled(true);
        assert!(audio.input.is_none());
        audio.song_arm_record(0);
        let input = audio.input.as_ref().unwrap();
        assert!(input.tuner.is_enabled());
        assert!(input.capture.is_enabled());
        audio.set_tuner_enabled(true);
        assert_eq!(attempts.get(), 2);
        assert!(audio.error().is_none());
    }

    fn owned_plan(owner: u64) -> LoopPlan {
        LoopPlan {
            owner: woodshed_core::working_sets::WorkingSetId(owner),
            name: "Owned backing".into(),
            one_shot: false,
            click: false,
            segments: vec![woodshed_core::loop_plan::LoopSegment {
                id: LoopSegmentId {
                    set: woodshed_core::working_sets::WorkingSetId(owner),
                    card: woodshedding::rehearsal::CardId(7),
                },
                label: "Authored Card".into(),
                bpm: 113.5,
                beats: 5,
                bars: 3,
                pitches_hz: vec![217.3, 281.6, 336.2],
            }],
        }
    }

    fn stored_take(owner: u64, rate: u32, value: f32) -> CaptureSnapshot {
        let plan = owned_plan(owner);
        let segment = &plan.segments[0];
        let bar = Bar {
            bpm: segment.bpm,
            time_signature: TimeSignature::new(segment.beats, 4),
            length: segment.bars,
            ..Bar::default()
        };
        CaptureSnapshot {
            legacy_clock: false,
            id: segment.id,
            bpm: segment.bpm,
            beats: segment.beats,
            bars: segment.bars,
            buffer: woodshed_audio::SampleBuffer::new(
                vec![value; bar.duration_samples(rate as f32) as usize],
                rate,
            ),
        }
    }

    #[test]
    fn wav_snapshot_retains_completed_pcm_and_refuses_armed_capture() {
        let mut audio = backend(Box::new(successful_input));
        let take = stored_take(1, 48_000, 0.25);
        audio.restore_loop_captures(vec![take.clone()]).unwrap();
        audio.set_loop_plan(&owned_plan(1)).unwrap();
        render(&mut audio, 32);
        let (frozen, rate) = audio.export_backing().unwrap();
        assert_eq!(rate, 48_000);
        assert!(Arc::ptr_eq(
            &frozen.bars[0].audio_buffer.as_ref().unwrap().data,
            &take.buffer.data
        ));
        assert!(audio.input.is_none());
        audio.clear_stored_capture(take.id).unwrap();
        assert_eq!(frozen.bars[0].audio_buffer.as_ref().unwrap().data[0], 0.25);
        assert!(
            audio.output.as_ref().unwrap().song().bars[0]
                .audio_buffer
                .is_none()
        );
        render(&mut audio, 32);
        audio.loop_arm_record(take.id).unwrap();
        assert!(audio.export_backing().unwrap_err().contains("armed"));
        assert_eq!(frozen.bars[0].audio_buffer.as_ref().unwrap().data[0], 0.25);
    }

    #[test]
    fn recorder_observation_uses_completed_pcm_without_input_or_export_and_freezes_arm_target() {
        let mut audio = backend(Box::new(|| successful_input()));
        let take = stored_take(1, 48000, 0.25);
        audio.restore_loop_captures(vec![take.clone()]).unwrap();
        let plan = owned_plan(1);
        audio.set_loop_plan(&plan).unwrap();
        render(&mut audio, 32);
        let first = audio.recorder_observation(None);
        assert_eq!(first.plan, Some(plan.clone()));
        assert!(Arc::ptr_eq(
            &first.buffers[0].as_ref().unwrap().samples,
            &take.buffer.data
        ));
        assert_eq!(first.input_rate, None);
        assert!(audio.input.is_none());
        assert_eq!(audio.recorder_observation(None), first);
        audio.loop_arm_record(plan.segments[0].id).unwrap();
        let armed = audio.recorder_observation(None);
        assert_eq!(armed.capture_target, Some(0));
        assert!(armed.capture_busy);
        assert!(armed.recording_bar.is_none());
        assert_eq!(armed.input_rate, Some(48000));
        assert!(audio.export_backing().is_err());
        // A failed or pending stop is not an invented idle observation.
        audio.loop_stop_record();
        let pending = audio.recorder_observation(None);
        assert!(pending.capture_busy);
        assert_eq!(pending.capture_target, Some(0));
    }

    #[test]
    fn frozen_saved_bar_queue_clears_distinct_pcm_and_refuses_replaced_source_or_session() {
        use woodshed_core::audio::{RecorderBacking, RecorderCommand, RecorderRequest};
        let mut audio = backend(Box::new(|| panic!("clear opened input")));
        let mut doc = SongDoc::default();
        doc.bars = vec![Default::default(), Default::default()];
        audio.try_set_song(&doc).unwrap();
        let mut accepted = audio.output.as_ref().unwrap().song();
        for (index, bar) in accepted.bars.iter_mut().enumerate() {
            bar.audio_buffer = Some(woodshed_audio::SampleBuffer::new(
                vec![0.1 + index as f32 * 0.1; bar.duration_samples(48000.0) as usize],
                48000,
            ));
        }
        audio.output.as_mut().unwrap().set_song(accepted).unwrap();
        render(&mut audio, 32);
        assert!(audio.song_loop_bars().iter().all(|value| *value));
        let session = Arc::new(());
        let mut persistence = crate::capture_persistence::CapturePersistence::default();
        let requests: Vec<_> = (0..2)
            .map(|index| RecorderRequest {
                session: session.clone(),
                backing: RecorderBacking::Saved(doc.clone()),
                command: RecorderCommand::ClearSavedBar(index),
            })
            .collect();
        for request in &requests {
            crate::sync::execute_recorder_request(
                request,
                &session,
                &mut audio,
                &mut persistence,
                Some(&doc),
                None,
            )
            .unwrap();
        }
        assert_eq!(audio.song_loop_bars(), [false, false]);
        assert!(audio.input.is_none());
        let mut changed = doc.clone();
        changed.bars.reverse();
        changed.bars[0].bpm = 90.0;
        assert!(
            crate::sync::execute_recorder_request(
                &requests[0],
                &session,
                &mut audio,
                &mut persistence,
                Some(&changed),
                None
            )
            .is_err()
        );
        assert!(
            crate::sync::execute_recorder_request(
                &requests[0],
                &Arc::new(()),
                &mut audio,
                &mut persistence,
                Some(&doc),
                None
            )
            .is_err()
        );
    }

    #[test]
    fn stale_stop_epoch_cannot_cancel_a_new_take_on_the_same_start_bar() {
        let mut audio = backend(Box::new(successful_input));
        let plan = owned_plan(1);
        audio.set_loop_plan(&plan).unwrap();
        render(&mut audio, 32);
        audio.loop_arm_record(plan.segments[0].id).unwrap();
        let first = audio.recorder_capture_epoch;
        assert!(
            audio
                .recorder_stop_capture(0, first.wrapping_sub(1))
                .is_err()
        );
        assert!(audio.loop_capture_busy());
        assert!(audio.recorder_stop_capture(0, first).is_ok());
        assert!(audio.loop_capture_busy());
        render(&mut audio, 32);
        assert!(!audio.loop_capture_busy());
        audio.loop_arm_record(plan.segments[0].id).unwrap();
        let second = audio.recorder_capture_epoch;
        assert_ne!(first, second);
        assert!(audio.recorder_stop_capture(0, first).is_err());
        assert!(audio.loop_capture_busy());
        assert_eq!(audio.recorder_observation(None).capture_epoch, Some(second));
        assert!(audio.recorder_stop_capture(0, second).is_ok());
    }

    #[test]
    fn two_queued_saved_record_requests_do_not_toggle_or_retarget_the_first_accepted_take() {
        use woodshed_core::audio::{RecorderBacking, RecorderCommand, RecorderRequest};
        let mut audio = backend(Box::new(successful_input));
        let mut doc = SongDoc::default();
        doc.bars = vec![Default::default(), Default::default()];
        audio.try_set_song(&doc).unwrap();
        render(&mut audio, 32);
        let session = Arc::new(());
        let mut persistence = crate::capture_persistence::CapturePersistence::default();
        let requests: Vec<_> = (0..2)
            .map(|index| RecorderRequest {
                session: session.clone(),
                backing: RecorderBacking::Saved(doc.clone()),
                command: RecorderCommand::RecordSavedBar(index),
            })
            .collect();
        crate::sync::execute_recorder_request(
            &requests[0],
            &session,
            &mut audio,
            &mut persistence,
            Some(&doc),
            None,
        )
        .unwrap();
        let epoch = audio.recorder_capture_epoch;
        assert!(
            crate::sync::execute_recorder_request(
                &requests[1],
                &session,
                &mut audio,
                &mut persistence,
                Some(&doc),
                None
            )
            .is_err()
        );
        assert!(audio.loop_capture_busy());
        assert_eq!(audio.recorder_capture_target, Some(0));
        assert_eq!(audio.recorder_capture_epoch, epoch);
    }

    fn quota_bank(count: u64, frames: usize, rate: u32) -> Vec<CaptureSnapshot> {
        let buffer = woodshed_audio::SampleBuffer::new(vec![0.25; frames], rate);
        (1..=count)
            .map(|card| CaptureSnapshot {
                legacy_clock: false,
                id: LoopSegmentId {
                    set: woodshed_core::working_sets::WorkingSetId(2),
                    card: woodshedding::rehearsal::CardId(card),
                },
                bpm: 60.0 * rate as f32 / frames as f32,
                beats: 1,
                bars: 1,
                buffer: buffer.clone(),
            })
            .collect()
    }

    #[test]
    fn set_arm_quota_refuses_count_and_frames_before_either_route_opens_input() {
        for frame_quota in [false, true] {
            for song_route in [false, true] {
                let attempts = Rc::new(Cell::new(0));
                let observed = attempts.clone();
                let mut audio = backend(Box::new(move || {
                    observed.set(observed.get() + 1);
                    successful_input()
                }));
                let takes = if frame_quota {
                    quota_bank(255, 65536, 48000)
                } else {
                    quota_bank(256, 500, 1000)
                };
                audio.restore_loop_captures(takes).unwrap();
                let plan = owned_plan(1);
                audio.set_loop_plan(&plan).unwrap();
                render(&mut audio, 32);
                let snapshots = audio.capture_snapshot();
                let before = audio.output_snapshot().unwrap();
                let error = if song_route {
                    audio.song_arm_record(0);
                    audio.error().unwrap().to_owned()
                } else {
                    audio.loop_arm_record(plan.segments[0].id).unwrap_err()
                };
                assert!(error.contains(if frame_quota { "16777216" } else { "256" }));
                assert_eq!(attempts.get(), 0);
                assert!(audio.input.is_none());
                assert!(!audio.capture_attached);
                assert!(!audio.loop_capture_busy());
                assert_eq!(audio.captures.plan, Some(plan));
                assert_eq!(audio.output_snapshot().unwrap(), before);
                let after = audio.capture_snapshot();
                assert_eq!(after.len(), snapshots.len());
                for (old, new) in snapshots.iter().zip(&after) {
                    assert_eq!(old.id, new.id);
                    assert!(Arc::ptr_eq(&old.buffer.data, &new.buffer.data));
                }
                assert!(
                    audio.output.as_ref().unwrap().song().bars[0]
                        .audio_buffer
                        .is_none()
                );
            }
        }
    }

    #[test]
    fn set_arm_can_replace_the_same_id_at_the_exact_stored_quota() {
        let attempts = Rc::new(Cell::new(0));
        let observed = attempts.clone();
        let mut audio = backend(Box::new(move || {
            observed.set(observed.get() + 1);
            successful_input()
        }));
        let mut takes = quota_bank(255, 65536, 48000);
        let mut plan = owned_plan(1);
        plan.segments[0].bpm = 60.0 * 48000.0 / 65536.0;
        plan.segments[0].beats = 1;
        plan.segments[0].bars = 1;
        let pcm = takes[0].buffer.clone();
        takes.push(CaptureSnapshot {
            legacy_clock: false,
            id: plan.segments[0].id,
            bpm: plan.segments[0].bpm,
            beats: 1,
            bars: 1,
            buffer: pcm.clone(),
        });
        audio.restore_loop_captures(takes).unwrap();
        audio.set_loop_plan(&plan).unwrap();
        render(&mut audio, 32);
        audio.loop_arm_record(plan.segments[0].id).unwrap();
        assert_eq!(attempts.get(), 1);
        assert!(audio.loop_capture_busy());
        assert_eq!(audio.capture_snapshot().len(), 256);
        let stored = audio
            .capture_snapshot()
            .into_iter()
            .find(|take| take.id == plan.segments[0].id)
            .unwrap();
        assert!(Arc::ptr_eq(&stored.buffer.data, &pcm.data));
        audio.loop_stop_record();
        render(&mut audio, 32);
    }

    #[test]
    fn restoring_set_custody_leaves_saved_output_and_compatibility_pcm_untouched() {
        let mut audio = backend(Box::new(|| panic!("capture hydration opened input")));
        let mut saved = Song::new();
        saved.name = "Saved backing".into();
        let legacy = woodshed_audio::SampleBuffer::new(
            vec![0.1; saved.bars[0].duration_samples(48000.0) as usize],
            48000,
        );
        saved.bars[0].audio_buffer = Some(legacy.clone());
        audio.output.as_mut().unwrap().set_song(saved).unwrap();
        render(&mut audio, 32);
        let incoming = stored_take(1, 48000, 0.6);
        audio.restore_loop_captures(vec![incoming.clone()]).unwrap();
        let snapshots = audio.capture_snapshot();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].id, incoming.id);
        assert!(Arc::ptr_eq(
            &snapshots[0].buffer.data,
            &incoming.buffer.data
        ));
        let current = audio.output.as_ref().unwrap().song();
        assert_eq!(current.name, "Saved backing");
        assert!(Arc::ptr_eq(
            &current.bars[0].audio_buffer.as_ref().unwrap().data,
            &legacy.data
        ));
        assert!(Arc::ptr_eq(
            &audio.captures.saved_song[0].as_ref().unwrap().data,
            &legacy.data
        ));
        audio.clear_stored_capture(incoming.id).unwrap();
        assert!(audio.capture_snapshot().is_empty());
        assert!(Arc::ptr_eq(
            &audio.output.as_ref().unwrap().song().bars[0]
                .audio_buffer
                .as_ref()
                .unwrap()
                .data,
            &legacy.data
        ));
        assert!(audio.input.is_none());
    }

    #[test]
    fn hydration_rejects_busy_capture_before_replacing_any_completed_take() {
        let mut audio = backend(Box::new(successful_input));
        let old = stored_take(1, 48000, 0.2);
        audio.restore_loop_captures(vec![old.clone()]).unwrap();
        let plan = owned_plan(1);
        audio.set_loop_plan(&plan).unwrap();
        render(&mut audio, 32);
        audio.loop_arm_record(old.id).unwrap();
        assert!(
            audio
                .restore_loop_captures(vec![stored_take(2, 48000, 0.8)])
                .unwrap_err()
                .contains("armed")
        );
        assert!(audio.clear_stored_capture(old.id).is_err());
        let snapshots = audio.capture_snapshot();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].id, old.id);
        assert!(Arc::ptr_eq(&snapshots[0].buffer.data, &old.buffer.data));
        assert_eq!(audio.captures.plan, Some(plan));
    }

    #[test]
    fn hydration_finishes_pending_identity_reset_before_installing_reused_ids() {
        let mut audio = backend(Box::new(|| panic!("hydration opened input")));
        let old = stored_take(1, 48000, 0.2);
        audio.restore_loop_captures(vec![old.clone()]).unwrap();
        let plan = owned_plan(1);
        audio.set_loop_plan(&plan).unwrap();
        render(&mut audio, 32);
        for _ in 0..64 {
            audio.output.as_mut().unwrap().set_output_gain(0.5).unwrap();
        }
        audio.clear_loop_session();
        assert!(audio.session_reset_pending);
        // Pending identity resets must not export the old session's takes.
        assert!(audio.capture_snapshot().is_empty());
        let incoming = stored_take(1, 48000, 0.7);
        assert!(audio.restore_loop_captures(vec![incoming.clone()]).is_err());
        render(&mut audio, 32);
        audio.restore_loop_captures(vec![incoming.clone()]).unwrap();
        assert!(!audio.session_reset_pending);
        assert!(audio.captures.plan.is_none());
        audio.set_loop_plan(&plan).unwrap();
        render(&mut audio, 32);
        let snapshots = audio.capture_snapshot();
        assert_eq!(snapshots.len(), 1);
        assert!(Arc::ptr_eq(
            &snapshots[0].buffer.data,
            &incoming.buffer.data
        ));
        assert!(!Arc::ptr_eq(&snapshots[0].buffer.data, &old.buffer.data));
        audio.clear_loop_session();
        render(&mut audio, 32);
        assert!(audio.capture_snapshot().is_empty());
    }

    #[test]
    fn hydrated_pcm_rate_is_preserved_then_refused_by_normal_output_admission() {
        let mut audio = backend(Box::new(|| panic!("hydration opened input")));
        let incoming = stored_take(1, 44100, 0.5);
        audio.restore_loop_captures(vec![incoming.clone()]).unwrap();
        let current = audio.output.as_ref().unwrap().song();
        assert!(
            audio
                .set_loop_plan(&owned_plan(1))
                .unwrap_err()
                .contains("rate")
        );
        assert!(audio.captures.plan.is_none());
        assert_eq!(audio.output.as_ref().unwrap().song().name, current.name);
        let snapshots = audio.capture_snapshot();
        assert_eq!(snapshots[0].buffer.sample_rate_hz, 44100);
        assert_eq!(snapshots[0].buffer.len(), incoming.buffer.len());
        assert!(Arc::ptr_eq(
            &snapshots[0].buffer.data,
            &incoming.buffer.data
        ));
    }

    #[test]
    fn explicit_unadmitted_clear_recovers_an_edited_clock_and_keeps_other_owners() {
        let mut audio = backend(Box::new(|| panic!("stored clear opened input")));
        let old = stored_take(1, 48000, 0.2);
        let inactive = stored_take(2, 48000, 0.6);
        audio
            .restore_loop_captures(vec![old.clone(), inactive.clone()])
            .unwrap();
        let mut edited = owned_plan(1);
        edited.segments[0].bpm = 90.0;
        assert!(
            audio
                .set_loop_plan(&edited)
                .unwrap_err()
                .contains("Clear the captured loop")
        );
        assert!(audio.captures.plan.is_none());
        let before = audio.output_snapshot().unwrap();
        audio.clear_stored_capture(old.id).unwrap();
        assert_eq!(audio.output_snapshot().unwrap(), before);
        let snapshots = audio.capture_snapshot();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].id, inactive.id);
        audio.set_loop_plan(&edited).unwrap();
        assert!(
            audio.output.as_ref().unwrap().song().bars[0]
                .audio_buffer
                .is_none()
        );
        audio
            .clear_stored_capture(LoopSegmentId {
                set: woodshed_core::working_sets::WorkingSetId(3),
                card: woodshedding::rehearsal::CardId(99),
            })
            .unwrap();
        assert_eq!(audio.capture_snapshot()[0].id, inactive.id);
    }

    #[test]
    fn clearing_an_admitted_stored_capture_updates_output_without_touching_inactive_sets() {
        let mut audio = backend(Box::new(|| panic!("stored clear opened input")));
        let loaded = stored_take(1, 48000, 0.2);
        let inactive = stored_take(2, 48000, 0.6);
        audio
            .restore_loop_captures(vec![loaded.clone(), inactive.clone()])
            .unwrap();
        let plan = owned_plan(1);
        audio.set_loop_plan(&plan).unwrap();
        render(&mut audio, 32);
        assert!(audio.song_loop_bars()[0]);
        audio.clear_stored_capture(loaded.id).unwrap();
        render(&mut audio, 32);
        assert!(!audio.song_loop_bars()[0]);
        assert_eq!(audio.captures.plan, Some(plan));
        let snapshots = audio.capture_snapshot();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].id, inactive.id);
        assert!(Arc::ptr_eq(
            &snapshots[0].buffer.data,
            &inactive.buffer.data
        ));
    }

    #[test]
    fn queued_capture_blocks_all_backing_replacements_before_the_processor_runs() {
        let mut audio = backend(Box::new(successful_input));
        let plan = owned_plan(1);
        audio.set_loop_plan(&plan).unwrap();
        render(&mut audio, 32);
        let accepted = audio.output.as_ref().unwrap().song();
        audio.loop_arm_record(plan.segments[0].id).unwrap();
        assert!(audio.loop_capture_busy());
        assert!(!audio.output_snapshot().unwrap().capture_armed);
        assert!(
            audio
                .try_set_song(&SongDoc::default())
                .unwrap_err()
                .contains("armed")
        );
        assert!(audio.set_loop_plan(&owned_plan(2)).is_err());
        assert_eq!(audio.captures.plan, Some(plan.clone()));
        let retained = audio.output.as_ref().unwrap().song();
        assert_eq!(retained.name, accepted.name);
        assert_eq!(retained.bars[0].chord_ref, accepted.bars[0].chord_ref);
        assert!(audio.input.as_ref().unwrap().capture.is_enabled());
        audio.loop_stop_record();
        assert!(audio.loop_capture_busy());
        render(&mut audio, 32);
        assert!(!audio.loop_capture_busy());
        audio.try_set_song(&SongDoc::default()).unwrap();
        assert!(audio.captures.plan.is_none());
    }

    #[test]
    fn recording_rate_mismatch_keeps_the_reader_available_and_current_owner_loaded() {
        let mut audio = backend(Box::new(|| {
            InputSession::open_with(|mut builder| {
                builder.prepare_analyzers(44_100);
                Ok(InputStream::Test)
            })
        }));
        let plan = owned_plan(1);
        audio.set_loop_plan(&plan).unwrap();
        let error = audio.loop_arm_record(plan.segments[0].id).unwrap_err();
        assert!(error.contains("44100") && error.contains("48000"));
        assert_eq!(audio.captures.plan, Some(plan));
        assert!(!audio.capture_attached && !audio.loop_capture_busy());
        let capture = &audio.input.as_ref().unwrap().capture;
        assert!(!capture.is_enabled());
        assert!(capture.take_consumer().is_some());
    }

    #[test]
    fn reset_backpressure_cannot_transfer_old_pcm_into_incoming_session_ids() {
        let mut audio = backend(Box::new(successful_input));
        let old = owned_plan(1);
        audio.set_loop_plan(&old).unwrap();
        let mut captured = audio.output.as_ref().unwrap().song();
        let frames = captured.bars[0].duration_samples(48_000.0) as usize;
        captured.bars[0].audio_buffer = Some(woodshed_audio::SampleBuffer::new(
            vec![0.25; frames],
            48_000,
        ));
        audio.output.as_mut().unwrap().set_song(captured).unwrap();
        render(&mut audio, 32);
        assert!(audio.song_loop_bars()[0]);
        for _ in 0..128 {
            if audio.output.as_mut().unwrap().set_output_gain(0.5).is_err() {
                break;
            }
        }
        audio.clear_loop_session();
        assert!(audio.session_reset_pending);
        assert!(audio.set_loop_plan(&old).is_err());
        render(&mut audio, 32);
        audio.set_loop_plan(&old).unwrap();
        assert!(!audio.session_reset_pending);
        assert!(
            audio
                .output
                .as_ref()
                .unwrap()
                .song()
                .bars
                .iter()
                .all(|bar| bar.audio_buffer.is_none())
        );
    }

    #[test]
    fn preview_completion_wakes_control_and_services_the_real_graph_without_input() {
        let mut audio = backend(Box::new(|| panic!("preview opened the microphone")));
        let (sent, received) = mpsc::channel();
        audio.set_wake(Arc::new(move || {
            let _ = sent.send(());
        }));
        audio.preview_pitches(&[217.3, 281.6, 336.2], 0.2, 0.0);
        received
            // Complete before the one-second idle heartbeat can mask a
            // missing worker notification.
            .recv_timeout(Duration::from_millis(750))
            .expect("preview completion did not wake control");
        audio.service();
        assert!(!audio.preview_busy());
        render(&mut audio, 32);
        let snapshot = audio.output_snapshot().unwrap();
        assert!(snapshot.device_frames >= 32 && snapshot.processing_blocks > 0);
        assert!(snapshot.preview_voices > 0);
        assert_eq!(audio.output_stream_count(), 0);
        assert!(audio.input.is_none());
    }

    #[test]
    fn failed_calibration_stop_invalidates_cache_and_retries_ordinary_click_atomically() {
        let mut audio = backend(Box::new(successful_input));
        let ordinary = TransportState {
            bpm: 137.0,
            playing: false,
        };
        audio.set_metronome(ordinary);
        render(&mut audio, 32);
        audio.calibration_start();
        render(&mut audio, 32);
        assert!(audio.output_snapshot().unwrap().click_playing);
        assert!(audio.input.as_ref().unwrap().onset.is_enabled());
        // Refuse both the session's terminal stop and the host restoration.
        // Neither may leave the old cached ordinary request suppressing retry.
        while audio.output.as_mut().unwrap().set_output_gain(0.5).is_ok() {}
        audio.calibration_cancel();
        assert!(audio.error().is_some());
        assert!(audio.last_metronome.is_none());
        assert!(!audio.input.as_ref().unwrap().onset.is_enabled());
        render(&mut audio, 32);
        assert!(audio.output_snapshot().unwrap().click_playing);
        audio.set_metronome(ordinary);
        render(&mut audio, 32);
        assert_eq!(audio.last_metronome, Some(ordinary));
        assert!(!audio.output_snapshot().unwrap().click_playing);
    }

    #[test]
    fn unsupported_saved_clock_is_refused_without_normalizing_the_authored_payload() {
        let mut audio = CpalBackend::offline_for_test();
        let mut doc = SongDoc::default();
        doc.bars.push(Default::default());
        doc.bars[0].beats = 0;
        doc.bars[0].length = 0;
        let payload = serde_json::to_value(&doc).unwrap();
        let projected = to_song(&doc);
        assert_eq!(projected.bars[0].time_signature.numerator, 0);
        assert_eq!(projected.bars[0].length, 0);
        assert!(audio.try_set_song(&doc).is_err());
        assert_eq!(serde_json::to_value(&doc).unwrap(), payload);
    }

    #[test]
    fn legacy_song_keeps_authored_frequencies_meter_tempo_and_lengths() {
        let mut doc = SongDoc::default();
        doc.bars.push(Default::default());
        doc.name = "authored".into();
        doc.one_shot = true;
        doc.click = false;
        doc.bars[0].pitches_hz = vec![261.111, 394.875, 522.333];
        doc.bars[0].root_freq_hz = 258.625;
        doc.bars[0].beats = 7;
        doc.bars[0].bpm = 83.75;
        doc.bars[0].length = 3;
        let song = to_song(&doc);
        assert_eq!(song.name, doc.name);
        assert!(song.one_shot);
        assert!(!song.click_enabled);
        let bar = &song.bars[0];
        let chord = bar.chord_ref.as_ref().unwrap();
        assert_eq!(chord.pitches_hz, doc.bars[0].pitches_hz);
        assert_eq!(chord.root_freq_hz, 258.625);
        assert_eq!(bar.time_signature, TimeSignature::new(7, 4));
        assert_eq!(bar.bpm, 83.75);
        assert_eq!(bar.length, 3);
        assert!((bar.duration_secs() - 7.0 * 3.0 * 60.0 / 83.75).abs() < 0.000002);
    }
}
