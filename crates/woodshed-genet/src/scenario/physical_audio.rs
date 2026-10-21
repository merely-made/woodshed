//! Scenario-only, observed physical-audio dwell. Script `wait` is a frame cap;
//! only this monotonic clock AND device-frame progress establish duration.
use super::{Ctx, Product, Shared};
use serde::Serialize;
use serde_json::json;
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use woodshed_views::stage::UiState;

#[derive(Clone, Copy, Debug, Serialize)]
enum Mode {
    Backing,
    Capture,
}
impl Mode {
    fn seconds(self) -> u64 {
        match self {
            Self::Backing => 5,
            Self::Capture => 15,
        }
    }
    fn stem(self) -> &'static str {
        match self {
            Self::Backing => "physical-backing-dwell",
            Self::Capture => "physical-capture-dwell",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct Observation {
    monotonic_ns: u64,
    unix_ms: u64,
    output_id: u64,
    output_frames: u64,
    output_rate: u32,
    input_rate: Option<u32>,
    input_admission_pending: bool,
    native_streams: usize,
    playing: bool,
    recording: bool,
    recording_bar: Option<usize>,
    cursor_bar: usize,
    capture_busy: bool,
    capture_target: Option<usize>,
    capture_epoch: Option<u64>,
    capture_invalid: bool,
}

struct State {
    mode: Mode,
    target: usize,
    first: Option<Observation>,
    baseline: Option<Observation>,
    last: Option<Observation>,
    observations: u64,
    admitted_input_rate: Option<u32>,
    result: Option<Result<(), String>>,
}
impl State {
    fn new(mode: Mode, target: usize) -> Self {
        Self {
            mode,
            target,
            first: None,
            baseline: None,
            last: None,
            observations: 0,
            admitted_input_rate: None,
            result: None,
        }
    }
    fn fail(&mut self, reason: impl Into<String>) {
        self.result = Some(Err(reason.into()));
    }

    fn observe(&mut self, now: Observation) -> bool {
        if self.result.is_some() {
            return false;
        }
        self.observations += 1;
        let check = (|| -> Result<bool, String> {
            if now.native_streams != 1 || now.output_rate == 0 || !now.playing {
                return Err(
                    "physical dwell requires one native output stream and active backing playback"
                        .into(),
                );
            }
            if now.capture_invalid {
                return Err("capture invalidated (including input-feed overflow/loss); physical dwell refused".into());
            }
            if let Some(first) = &self.first {
                if now.output_id != first.output_id || now.output_rate != first.output_rate {
                    return Err(
                        "output runtime or device rate changed during physical dwell".into(),
                    );
                }
            }
            if let Some(last) = &self.last {
                if now.monotonic_ns < last.monotonic_ns || now.output_frames < last.output_frames {
                    return Err("physical dwell clock or device frames moved backwards".into());
                }
            }
            if matches!(self.mode, Mode::Capture) {
                if self.baseline.is_none()
                    && now.monotonic_ns >= Duration::from_secs(30).as_nanos() as u64
                {
                    return Err(
                        "timed out awaiting observed active recording after Record/count-in".into(),
                    );
                }
                if !now.capture_busy
                    || now.capture_target != Some(self.target)
                    || now.capture_epoch.is_none()
                {
                    return Err("Record was refused, capture stopped, or selected input/target is not admitted".into());
                }
                if let Some(first) = &self.first {
                    if now.capture_epoch != first.capture_epoch {
                        return Err("capture execution changed during physical dwell".into());
                    }
                }
                if let Some(rate) = self.admitted_input_rate {
                    if now.input_rate != Some(rate) {
                        return Err(
                            "admitted input rate changed or disappeared during physical dwell"
                                .into(),
                        );
                    }
                } else {
                    match now.input_rate {
                        Some(rate) if rate > 0 => self.admitted_input_rate = Some(rate),
                        None if now.input_admission_pending
                            && self.baseline.is_none()
                            && !now.recording =>
                        {
                            return Ok(false); // explicit one-click reservation; opening time never counts
                        },
                        _ => {
                            return Err(
                                "input is unavailable without an explicit pending admission".into(),
                            );
                        },
                    }
                }
                if !now.recording {
                    if self.baseline.is_some() {
                        return Err(
                            "active recording stopped before both physical dwell bounds completed"
                                .into(),
                        );
                    }
                    return Ok(false); // armed/count-in time never counts as capture dwell
                }
                if now.recording_bar != Some(self.target) || now.cursor_bar != self.target {
                    return Err(
                        "active recording bar or output cursor left the accepted selected Card"
                            .into(),
                    );
                }
            }
            let Some(start) = &self.baseline else {
                return Ok(false);
            };
            let elapsed = now.monotonic_ns - start.monotonic_ns;
            let frames = now.output_frames - start.output_frames;
            let wall_ready = elapsed >= Duration::from_secs(self.mode.seconds()).as_nanos() as u64;
            let frames_ready = frames >= u64::from(start.output_rate) * self.mode.seconds();
            if elapsed >= Duration::from_secs(60).as_nanos() as u64 {
                return Err(
                    "timed out before both monotonic time and device-frame dwell bounds completed"
                        .into(),
                );
            }
            if wall_ready && frames_ready {
                return Ok(true);
            }
            Ok(false)
        })();
        if self.first.is_none() {
            self.first = Some(now.clone());
        }
        if check.is_ok()
            && self.baseline.is_none()
            && (matches!(self.mode, Mode::Backing) || now.recording)
        {
            self.baseline = Some(now.clone());
        }
        self.last = Some(now);
        match check {
            Ok(true) => self.result = Some(Ok(())),
            Err(reason) => self.fail(reason),
            Ok(false) => {},
        }
        self.result.is_none()
    }
}

pub(super) struct Dwell {
    started: Instant,
    requested_unix_ms: u64,
    session: Arc<()>,
    path: PathBuf,
    state: State,
}

fn unix_ms() -> Result<u64, String> {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_millis(),
    )
    .map_err(|e| e.to_string())
}

impl Dwell {
    fn read(&self, ui: &UiState, shared: &Shared) -> Result<Observation, String> {
        if !Arc::ptr_eq(&self.session, &ui.audio_session) {
            return Err("practice session changed during physical dwell".into());
        }
        let backend = shared
            .backend
            .as_ref()
            .ok_or("no physical output backend")?;
        let observed = backend.recorder_observation(None);
        let snapshot = backend
            .output_snapshot()
            .ok_or("no device-frame snapshot")?;
        Ok(Observation {
            monotonic_ns: u64::try_from(self.started.elapsed().as_nanos())
                .map_err(|e| e.to_string())?,
            unix_ms: unix_ms()?,
            output_id: backend
                .output_observation_id()
                .ok_or("no output observation identity")?,
            output_frames: snapshot.device_frames,
            output_rate: observed
                .output_rate
                .ok_or("physical output rate not admitted")?,
            input_rate: observed.input_rate,
            input_admission_pending: backend.input_admission_pending(),
            native_streams: backend.output_stream_count(),
            playing: snapshot.playing,
            recording: snapshot.recording,
            recording_bar: observed.recording_bar,
            cursor_bar: snapshot.cursor.bar_idx,
            capture_busy: observed.capture_busy,
            capture_target: observed.capture_target,
            capture_epoch: observed.capture_epoch,
            capture_invalid: snapshot.capture_invalid || observed.capture_invalid,
        })
    }
    pub(super) fn poll(&mut self, ui: &UiState, shared: &Shared) -> bool {
        if self.state.result.is_some() {
            return false;
        }
        match self.read(ui, shared) {
            Ok(observation) => self.state.observe(observation),
            Err(reason) => {
                self.state.fail(reason);
                false
            },
        }
    }
    fn write(&self) -> Result<(), String> {
        use std::io::Write;
        let elapsed_ns = self
            .state
            .baseline
            .as_ref()
            .zip(self.state.last.as_ref())
            .map(|(a, b)| b.monotonic_ns.saturating_sub(a.monotonic_ns));
        let frame_advance = self
            .state
            .baseline
            .as_ref()
            .zip(self.state.last.as_ref())
            .map(|(a, b)| b.output_frames.saturating_sub(a.output_frames));
        let success = matches!(self.state.result, Some(Ok(())));
        let error = self
            .state
            .result
            .as_ref()
            .and_then(|result| result.as_ref().err());
        let receipt = json!({
            "schema":"woodshed-physical-dwell/v1", "mode":self.state.mode,
            "requested_unix_ms":self.requested_unix_ms, "minimum_active_seconds":self.state.mode.seconds(),
            "minimum_output_frame_advance":self.state.baseline.as_ref().map(|s|u64::from(s.output_rate)*self.state.mode.seconds()),
            "first":self.state.first,"active_start":self.state.baseline,"end":self.state.last,
            "active_elapsed_ns":elapsed_ns,"output_frame_advance":frame_advance,
            "observations":self.state.observations,"success":success,"error":error,
            "overflow_evidence":"processor capture_invalid latch covers feed overflow and input loss; no raw counter exposed",
            "audibility":"native output progress; acoustic audibility requires human observation"
        });
        let bytes = serde_json::to_vec_pretty(&receipt).map_err(|e| e.to_string())?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.path)
            .map_err(|e| e.to_string())?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())
    }
}

pub(super) fn act(product: &mut Product, ctx: &mut Ctx<'_>, label: &str) -> Option<bool> {
    let mode = match label {
        "physical-backing-dwell-start" => Some(Mode::Backing),
        "physical-capture-dwell-start" => Some(Mode::Capture),
        "physical-dwell-assert" => None,
        _ => return None,
    };
    let result = (|| -> Result<(), String> {
        if let Some(mode) = mode {
            let mut slot = product.physical_dwell.borrow_mut();
            if slot.is_some() {
                return Err("assert the previous physical dwell before starting another".into());
            }
            let directory = PathBuf::from(
                std::env::var_os("WOODSHED_CAPTURE_DIR").ok_or("no isolated capture directory")?,
            );
            let ui = ctx.runner.state();
            let id = ui.loop_selected.ok_or("select a physical-trial Card")?;
            let target = ui
                .loop_admitted
                .as_ref()
                .and_then(|p| p.index_of(id))
                .ok_or("selected physical-trial Card is not admitted")?;
            let mut dwell = Dwell {
                started: Instant::now(),
                requested_unix_ms: unix_ms()?,
                session: ui.audio_session.clone(),
                path: directory.join(format!("{}.json", mode.stem())),
                state: State::new(mode, target),
            };
            dwell.poll(ui, &product.shared.borrow());
            if let Some(Err(reason)) = &dwell.state.result {
                let reason = reason.clone();
                dwell.write()?;
                return Err(reason);
            }
            *slot = Some(dwell);
        } else {
            let mut dwell = product
                .physical_dwell
                .borrow_mut()
                .take()
                .ok_or("no physical dwell was started")?;
            // Recheck health at assertion, not merely a previously latched pass.
            if matches!(dwell.state.result, Some(Ok(()))) {
                dwell.state.result = None;
            }
            dwell.poll(ctx.runner.state(), &product.shared.borrow());
            if dwell.state.result.is_none() {
                dwell.state.fail("scenario frame cap exhausted before both physical dwell bounds; a wait cap is not elapsed time");
            }
            dwell.write()?;
            dwell
                .state
                .result
                .as_ref()
                .expect("terminal physical dwell")
                .clone()?;
            product
                .shared
                .borrow_mut()
                .events
                .push(format!("physical-dwell-exact {}", dwell.state.mode.stem()));
        }
        Ok(())
    })();
    if let Err(reason) = &result {
        eprintln!("physical dwell refused: {reason}");
    }
    Some(result.is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(seconds: u64, frames: u64) -> Observation {
        Observation {
            monotonic_ns: seconds * 1_000_000_000,
            unix_ms: 1_800_000_000_000 + seconds * 1_000,
            output_id: 17,
            output_frames: frames,
            output_rate: 48_000,
            input_rate: Some(48_000),
            input_admission_pending: false,
            native_streams: 1,
            playing: true,
            recording: true,
            capture_busy: true,
            recording_bar: Some(1),
            cursor_bar: 1,
            capture_target: Some(1),
            capture_epoch: Some(3),
            capture_invalid: false,
        }
    }

    #[test]
    fn wall_and_device_frame_bounds_are_both_required() {
        let mut wall_only = State::new(Mode::Capture, 1);
        assert!(wall_only.observe(sample(0, 100)));
        assert!(wall_only.observe(sample(15, 100 + 48_000 * 14)));
        assert!(wall_only.result.is_none());
        assert!(!wall_only.observe(sample(16, 100 + 48_000 * 15)));
        assert!(matches!(wall_only.result, Some(Ok(()))));

        let mut frames_only = State::new(Mode::Capture, 1);
        assert!(frames_only.observe(sample(0, 100)));
        assert!(frames_only.observe(sample(1, 100 + 48_000 * 15)));
        assert!(frames_only.result.is_none());
        assert!(!frames_only.observe(sample(15, 100 + 48_000 * 15)));
        assert!(matches!(frames_only.result, Some(Ok(()))));
    }

    #[test]
    fn armed_and_count_in_seconds_never_count_as_active_capture() {
        let mut state = State::new(Mode::Capture, 1);
        let mut waiting = sample(0, 0);
        waiting.recording = false;
        assert!(state.observe(waiting.clone()));
        waiting.monotonic_ns = 20_000_000_000;
        waiting.output_frames = 48_000 * 20;
        assert!(state.observe(waiting));
        assert!(state.baseline.is_none());
        assert!(state.observe(sample(21, 48_000 * 21)));
        assert_eq!(
            state.baseline.as_ref().unwrap().monotonic_ns,
            21_000_000_000
        );
        assert!(state.observe(sample(35, 48_000 * 35)));
        assert!(!state.observe(sample(36, 48_000 * 36)));
        assert!(matches!(state.result, Some(Ok(()))));
    }

    #[test]
    fn explicit_input_opening_is_allowed_but_only_active_admitted_time_and_frames_count() {
        let mut state = State::new(Mode::Capture, 1);
        let opening = |seconds| {
            let mut now = sample(seconds, 48_000 * seconds);
            now.recording = false;
            now.recording_bar = None;
            now.input_rate = None;
            now.input_admission_pending = true;
            now
        };
        assert!(state.observe(opening(0)));
        assert!(state.observe(opening(8)));
        assert!(state.baseline.is_none());
        let mut armed = sample(9, 48_000 * 9);
        armed.recording = false;
        armed.recording_bar = None;
        assert!(state.observe(armed));
        assert!(state.baseline.is_none());
        assert!(state.observe(sample(10, 48_000 * 10)));
        assert!(
            state.observe(sample(24, 48_000 * 25)),
            "frames alone cannot pass"
        );
        assert!(!state.observe(sample(25, 48_000 * 25)));
        assert!(matches!(state.result, Some(Ok(()))));
        assert_eq!(
            state.baseline.as_ref().unwrap().monotonic_ns,
            10_000_000_000
        );
    }

    #[test]
    fn pending_input_timeout_missing_pending_and_lost_admitted_rate_are_failures() {
        for defect in 0..4 {
            let mut state = State::new(Mode::Capture, 1);
            let mut now = sample(0, 0);
            now.recording = false;
            now.recording_bar = None;
            now.input_rate = None;
            now.input_admission_pending = true;
            assert!(state.observe(now.clone()));
            match defect {
                0 => {
                    now.monotonic_ns = 30_000_000_000;
                },
                1 => {
                    now.input_admission_pending = false;
                },
                _ => {
                    now.input_rate = Some(48_000);
                    now.input_admission_pending = false;
                    assert!(state.observe(now.clone()));
                    now.input_rate = if defect == 2 { None } else { Some(44_100) };
                    now.input_admission_pending = true; // cannot excuse loss after admission
                },
            }
            assert!(!state.observe(now));
            assert!(matches!(state.result, Some(Err(_))));
        }
    }

    #[test]
    fn refused_stopped_overflowed_replaced_or_offline_capture_cannot_pass() {
        for defect in 0..11 {
            let mut state = State::new(Mode::Capture, 1);
            assert!(state.observe(sample(0, 1_000)));
            let mut bad = sample(15, 1_000 + 48_000 * 15);
            match defect {
                0 => bad.capture_invalid = true,
                1 => bad.input_rate = None,
                2 => bad.recording = false,
                3 => bad.capture_busy = false,
                4 => bad.capture_target = Some(0),
                5 => bad.capture_epoch = Some(4),
                6 => bad.output_id = 18,
                7 => bad.output_rate = 44_100,
                8 => bad.native_streams = 0,
                9 => bad.output_frames = 999,
                _ => bad.playing = false,
            }
            assert!(!state.observe(bad));
            assert!(matches!(state.result, Some(Err(_))), "defect {defect}");
        }
    }

    #[test]
    fn moved_recording_bar_or_cursor_fails_despite_unchanged_accepted_target() {
        for at_assert in [false, true] {
            for move_recording_bar in [false, true] {
                let mut state = State::new(Mode::Capture, 1);
                assert!(state.observe(sample(0, 100)));
                if at_assert {
                    assert!(!state.observe(sample(15, 720_100)));
                    assert!(matches!(state.result, Some(Ok(()))));
                    // Production assertion resamples after provisional success.
                    state.result = None;
                }
                let mut moved = sample(if at_assert { 16 } else { 1 }, 768_100);
                if move_recording_bar {
                    moved.recording_bar = Some(2);
                } else {
                    moved.cursor_bar = 2;
                }
                assert_eq!(moved.capture_target, Some(1));
                assert!(!state.observe(moved));
                assert!(matches!(state.result, Some(Err(_))));
            }
        }
    }

    #[test]
    fn waiting_or_device_progress_timeout_is_a_terminal_failure() {
        let mut waiting = State::new(Mode::Capture, 1);
        let mut observation = sample(30, 48_000 * 30);
        observation.recording = false;
        assert!(!waiting.observe(observation));
        assert!(matches!(waiting.result, Some(Err(_))));
        let mut stalled = State::new(Mode::Capture, 1);
        assert!(stalled.observe(sample(0, 100)));
        assert!(!stalled.observe(sample(60, 101)));
        assert!(matches!(stalled.result, Some(Err(_))));
    }

    #[test]
    fn backing_dwell_needs_five_real_seconds_and_assert_rechecks_capture_health() {
        let mut backing = State::new(Mode::Backing, 1);
        let mut start = sample(0, 100);
        start.recording = false;
        start.capture_busy = false;
        start.input_rate = None;
        assert!(backing.observe(start.clone()));
        start.monotonic_ns = 5_000_000_000;
        start.output_frames += 48_000 * 5;
        assert!(!backing.observe(start));
        assert!(matches!(backing.result, Some(Ok(()))));
        let mut capture = State::new(Mode::Capture, 1);
        capture.observe(sample(0, 100));
        capture.observe(sample(15, 720_100));
        assert!(matches!(capture.result, Some(Ok(()))));
        // The production assert clears only a provisional success and samples
        // fresh health before admitting the receipt.
        capture.result = None;
        let mut invalidated = sample(16, 768_100);
        invalidated.capture_invalid = true;
        assert!(!capture.observe(invalidated));
        assert!(matches!(capture.result, Some(Err(_))));
    }

    #[test]
    fn json_receipt_records_observed_active_interval_and_refuses_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("physical-capture-dwell.json");
        let mut state = State::new(Mode::Capture, 1);
        state.observe(sample(3, 10_000));
        state.observe(sample(18, 730_000));
        let dwell = Dwell {
            started: Instant::now(),
            requested_unix_ms: 1_800_000_000_000,
            session: Arc::new(()),
            path: path.clone(),
            state,
        };
        dwell.write().unwrap();
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(value["success"], true);
        assert_eq!(value["active_elapsed_ns"], 15_000_000_000_u64);
        assert_eq!(value["output_frame_advance"], 720_000);
        assert_eq!(value["active_start"]["recording"], true);
        assert_eq!(value["end"]["input_rate"], 48_000);
        assert_eq!(value["end"]["capture_invalid"], false);
        assert!(dwell.write().is_err());
    }
}
