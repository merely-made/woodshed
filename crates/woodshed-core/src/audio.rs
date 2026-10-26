//! The audio seam (genet-host plan W0.1).
//!
//! The core describes what the app wants (transport state, tuner state) in
//! pure data; a host supplies an [`AudioBackend`] that realizes it — cpal
//! through `woodshed-audio` on desktop, Web Audio / AudioWorklet in the
//! browser. The core never touches an audio API, so the same state drives
//! both.

/// Shared user-controlled transport tempo limits used across Woodshed.
pub const TRANSPORT_MIN_BPM: f32 = 1.0;
pub const TRANSPORT_MAX_BPM: f32 = 400.0;

/// Host-observed stream/configuration availability, independent of user intent.
/// `Admitted` records accepted native resources, not physical audibility.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum AudioStreamStatus {
    #[default]
    Unavailable,
    Unopened,
    Requested,
    AwaitingPermission,
    Admitted,
    /// A prepared software output engine without a native device stream.
    Offline,
    Failed(String),
}

/// Transient host facts. Never persisted as persona preferences or inferred
/// from an enabled control. A host that has not reported defaults unavailable.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AudioAvailability {
    pub output: AudioStreamStatus,
    pub input: AudioStreamStatus,
    pub tuner_active: bool,
}

/// One tuner analysis snapshot, host-agnostic.
#[derive(Clone, Debug, PartialEq)]
pub struct TunerReading {
    /// Nearest 12-TET note name ("A", "C#").
    pub note: String,
    pub octave: i32,
    /// Cents from the nearest note, roughly [-50, +50].
    pub cents: f64,
    pub in_tune: bool,
    /// Input RMS level in [0, 1] — drives a level meter and the
    /// "listening but silent" presentation.
    pub level: f32,
}

/// What the metronome transport should be doing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransportState {
    pub bpm: f32,
    pub playing: bool,
}

impl Default for TransportState {
    fn default() -> Self {
        Self {
            bpm: 120.0,
            playing: false,
        }
    }
}

impl TransportState {
    pub fn nudge_bpm(&mut self, delta: f32) {
        self.bpm = (self.bpm + delta).clamp(TRANSPORT_MIN_BPM, TRANSPORT_MAX_BPM);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_nudges_respect_shared_one_to_four_hundred_bpm_range() {
        let mut transport = TransportState::default();
        transport.bpm = 30.0;
        transport.nudge_bpm(-100.0);
        assert_eq!(transport.bpm, TRANSPORT_MIN_BPM);

        transport.bpm = 300.0;
        transport.nudge_bpm(100.0);
        assert_eq!(transport.bpm, TRANSPORT_MAX_BPM);

        transport.bpm = TRANSPORT_MIN_BPM;
        transport.nudge_bpm(-1.0);
        assert_eq!(transport.bpm, TRANSPORT_MIN_BPM);
        transport.bpm = TRANSPORT_MAX_BPM;
        transport.nudge_bpm(1.0);
        assert_eq!(transport.bpm, TRANSPORT_MAX_BPM);
    }
}

/// Neutral snapshot of a round-trip latency calibration run, for the UI
/// (audio-depth slice 14). Latencies are in milliseconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CalibrationStatus {
    /// No run in progress.
    Idle,
    /// Playing clicks and listening for the player's taps.
    Running { clicks_fired: usize, total: usize },
    /// Finished — a measured round-trip latency to accept.
    Success {
        latency_ms: f32,
        matched: usize,
        total: usize,
    },
    /// Finished but too few hits matched; offer a retry.
    Insufficient { matched: usize, total: usize },
    /// The audio engines aren't available (no mic, etc.).
    Unavailable,
}

/// Desired tuner enablement and the latest reading a host polled.
/// Actual activation is reported separately through `AudioAvailability`.
#[derive(Clone, Debug, Default)]
pub struct TunerState {
    pub enabled: bool,
    /// Latest reading; `None` while disabled or below the level
    /// threshold. Hosts poll their backend and write this.
    pub reading: Option<TunerReading>,
}

/// Frozen musical backing for a queued recorder command. No Hocket project is
/// created; these are Woodshed's existing product snapshots.
#[derive(Clone, Debug, PartialEq)]
pub enum RecorderBacking {
    Set(crate::loop_plan::LoopPlan),
    Saved(crate::song::SongDoc),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecorderCommand {
    RecordCard(crate::loop_plan::LoopSegmentId),
    ClearCard(crate::loop_plan::LoopSegmentId),
    RecordSavedBar(usize),
    ClearSavedBar(usize),
    StopCapture {
        start_bar: usize,
        capture_epoch: u64,
    },
}
#[derive(Clone, Debug)]
pub struct RecorderRequest {
    pub session: std::sync::Arc<()>,
    pub backing: RecorderBacking,
    pub command: RecorderCommand,
}
impl PartialEq for RecorderRequest {
    fn eq(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.session, &other.session)
            && self.backing == other.backing
            && self.command == other.command
    }
}
impl RecorderRequest {
    /// Queue execution checks this AFTER any pending admission at dispatch tail.
    pub fn is_current(
        &self,
        session: &std::sync::Arc<()>,
        song: Option<&crate::song::SongDoc>,
        plan: Option<&crate::loop_plan::LoopPlan>,
    ) -> bool {
        std::sync::Arc::ptr_eq(&self.session, session)
            && match &self.backing {
                RecorderBacking::Set(expected) => plan == Some(expected) && song.is_none(),
                RecorderBacking::Saved(expected) => song == Some(expected) && plan.is_none(),
            }
    }
}

/// Frozen retained Card preview, qualified independently of later UI selection.
#[derive(Clone, Debug)]
pub struct CardPreviewRequest {
    pub session: std::sync::Arc<()>,
    pub owner: crate::working_sets::WorkingSetId,
    pub card_id: woodshedding::rehearsal::CardId,
    pub instruction: serde_json::Value,
    pub fallback_bpm_bits: u32,
    pub run_epoch: Option<u64>,
    pub preview: crate::card_timing::CardPreview,
}
impl PartialEq for CardPreviewRequest {
    fn eq(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.session, &other.session)
            && self.owner == other.owner
            && self.card_id == other.card_id
            && self.instruction == other.instruction
            && self.fallback_bpm_bits == other.fallback_bpm_bits
            && self.run_epoch == other.run_epoch
            && self.preview == other.preview
    }
}

/// Frozen event-time context for an audition observation. Admission does not
/// reconstruct this from the later focus or edited Set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreviewObservation {
    pub subject_id: String,
    pub from_id: Option<String>,
    pub provenance: Option<crate::history::ObservationProvenance>,
}

/// One explicit audition, retained through host admission. Separate presses
/// have separate tickets even when they request identical musical material.
#[derive(Clone, Debug)]
pub struct AuditionRequest {
    pub session: std::sync::Arc<()>,
    pub ticket: std::sync::Arc<()>,
    pub preview: crate::card_timing::CardPreview,
    pub observation: Option<PreviewObservation>,
}
impl PartialEq for AuditionRequest {
    fn eq(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.session, &other.session)
            && std::sync::Arc::ptr_eq(&self.ticket, &other.ticket)
            && self.preview == other.preview
            && self.observation == other.observation
    }
}

/// One thing the view layer asks the audio host to do, once.
///
/// These are commands, not settings: continuous state (metronome transport,
/// tuner enabled, record-replace) is realized idempotently from the app state
/// every sync, while these happen exactly once each time they are requested.
///
/// They are queued rather than flagged. A boolean flag per request coalesces
/// two presses in one frame into one action, loses the order the user pressed
/// things in, and has to be cleared correctly at every consumption site. A
/// queue keeps every request, in order, drained in one place.
///
/// Recorder commands additionally freeze their session, admitted backing and
/// target until execution; UI selection or a new admission cannot redirect a
/// queued command. Recording/calibration results remain host-polled.
#[derive(Clone, Debug, PartialEq)]
pub enum AudioRequest {
    Audition(AuditionRequest),
    CardPreview(CardPreviewRequest),
    /// The reusable surface freezes source/epoch and positional targets until execution.
    Recorder(RecorderRequest),
    /// Rewind the song transport to its start.
    SongRewind,
    /// Cancel pending preview work and sounding preview envelopes.
    CancelPreview,
    /// Voice the current lens or rehearsal card.
    PreviewVoicing,
    /// Audition a resolved selection. Carrying its pitches keeps later focus
    /// or section changes from redirecting a queued context audition.
    PreviewPitches {
        pitches: Vec<f32>,
        duration_s: f32,
        strum_s: f32,
    },
    /// Play a source-owned relation audition as two ordered chord groups.
    /// Both groups are frozen when the request is created, so later focus or
    /// Stage changes cannot redirect either side of the comparison.
    PreviewPhrase {
        before: Vec<f32>,
        after: Vec<f32>,
        chord_duration_s: f32,
        gap_s: f32,
        strum_s: f32,
    },
    /// Play one note at this frequency (Hz).
    PreviewNote(f32),
    /// Begin latency calibration.
    CalibrationStart,
    /// Abandon a calibration run.
    CalibrationCancel,
    /// Accept a finished calibration's measured latency.
    CalibrationAccept,
    /// Arm or stop song-mode recording at the edit cursor.
    SongRecordToggle,
    /// Clear the recorded loop at the edit cursor.
    SongClearLoop,
    /// Record on an explicitly owned Card occurrence, never a view position.
    LoopRecordToggle(crate::loop_plan::LoopSegmentId),
    /// Clear only this Card's captured material.
    LoopClear(crate::loop_plan::LoopSegmentId),
}

/// The host-supplied audio realization. Implementations should be
/// tolerant of missing devices: construct in a degraded state and report
/// through [`error`](Self::error) rather than failing the app.
pub trait AudioBackend {
    /// Realize the metronome transport (idempotent; called after every
    /// input dispatch).
    fn set_metronome(&mut self, transport: TransportState);
    /// Start/stop the tuner analysis pipeline (idempotent).
    fn set_tuner_enabled(&mut self, enabled: bool);
    /// Latest tuner reading, `None` when disabled or no signal.
    fn tuner_reading(&self) -> Option<TunerReading>;
    /// Load the song lane (replaces the current song; the transport
    /// keeps its playing state).
    fn set_song(&mut self, doc: &crate::song::SongDoc);
    /// Admit saved backing without discarding an armed or active capture.
    /// Hosts with fallible output admission override this adapter and leave
    /// the previous backing loaded when they return an error.
    fn try_set_song(&mut self, doc: &crate::song::SongDoc) -> Result<(), String> {
        if self.loop_capture_busy() {
            return Err(
                "Stop recording / armed capture before updating the loaded backing.".into(),
            );
        }
        self.set_song(doc);
        Ok(())
    }
    /// Admit a derived Set projection atomically, or explain why the current
    /// projection must remain loaded. A missing implementation is visible.
    fn set_loop_plan(&mut self, _plan: &crate::loop_plan::LoopPlan) -> Result<(), String> {
        Err("This host does not support Set backing playback.".into())
    }
    /// Forget transient capture custody when the practice session changes.
    fn clear_loop_session(&mut self) {}
    fn loop_arm_record(&mut self, _id: crate::loop_plan::LoopSegmentId) -> Result<(), String> {
        Err("This host does not support Card recording.".into())
    }
    fn loop_clear(&mut self, _id: crate::loop_plan::LoopSegmentId) -> Result<(), String> {
        Err("This host does not support Card recording.".into())
    }
    /// Includes an armed, boundary-delayed capture, not just active input.
    fn loop_capture_busy(&self) -> bool {
        self.song_recording()
    }
    fn loop_stop_record(&mut self) {
        self.song_stop_record();
    }
    /// Start/stop song playback (idempotent).
    fn set_song_transport(&mut self, playing: bool);
    /// Snap the song cursor back to the top.
    fn song_rewind(&mut self);
    /// The bar block under the playback cursor (for timeline follow).
    fn song_bar(&self) -> Option<usize>;
    /// Cancel queued/in-flight previews and their sounding envelopes.
    fn cancel_preview(&mut self) {}
    /// Voice a set of chord / scale tones on demand — the "hear it"
    /// preview. The desktop queues previews while song playback is stopped.
    /// `strum_ms` staggers note
    /// onsets: 0 = block chord, ~18 = a gentle strum, larger = an
    /// arpeggiated cascade (a scale run). Default no-op so a backend
    /// that can't voice previews stays silent rather than being forced
    /// to implement it.
    fn preview_pitches(&mut self, _pitches_hz: &[f32], _duration_secs: f32, _strum_ms: f32) {}
    /// Play two frozen chord groups in order. The default stays silent for
    /// backends that do not support phrase audition; concatenating both groups
    /// into one chord would change the relation's before/after meaning.
    fn preview_phrase(
        &mut self,
        _before_hz: &[f32],
        _after_hz: &[f32],
        _chord_duration_secs: f32,
        _gap_secs: f32,
        _strum_ms: f32,
    ) {
    }
    /// Voice a single pitched note on demand — the arpeggio / exercise
    /// step-through sonification. Default no-op (see
    /// [`preview_pitches`](Self::preview_pitches)).
    fn preview_note(&mut self, _freq_hz: f32, _duration_secs: f32) {}
    /// Start a round-trip latency calibration run: play a lead of clicks
    /// the player taps along to; the onset detector times the hits.
    /// No-op default: a backend without input calibration stays idle.
    fn calibration_start(&mut self) {}
    /// Poll a running calibration; returns a neutral status snapshot.
    fn calibration_poll(&mut self) -> CalibrationStatus {
        CalibrationStatus::Idle
    }
    /// Cancel a running calibration and restore the metronome.
    fn calibration_cancel(&mut self) {}
    /// The active input→output latency compensation (ms), if calibrated.
    fn latency_ms(&self) -> Option<f32> {
        None
    }
    /// Set or clear the active latency compensation (ms) — the user
    /// accepting a calibration result, or clearing it.
    fn set_latency_ms(&mut self, _ms: Option<f32>) {}
    /// Arm song-mode loop recording for the bar at `bar_idx` — capture
    /// begins when playback next reaches that bar. No-op default.
    fn song_arm_record(&mut self, _bar_idx: usize) {}
    /// Stop song-mode loop recording.
    fn song_stop_record(&mut self) {}
    /// Clear (erase) the recorded loop on the bar at `bar_idx`.
    fn song_clear_loop(&mut self, _bar_idx: usize) {}
    /// Recording mode: `true` = replace the bar's audio each pass,
    /// `false` = overdub (sum) onto it.
    fn song_set_record_replace(&mut self, _replace: bool) {}
    /// Whether the song engine is currently capturing input into a bar.
    fn song_recording(&self) -> bool {
        false
    }
    /// Per-bar flags: does each bar hold a recorded loop? Empty when the
    /// backend has no song engine.
    fn song_loop_bars(&self) -> Vec<bool> {
        Vec::new()
    }
    /// A device/stream failure to surface in the UI, if any.
    fn error(&self) -> Option<&str>;
}
