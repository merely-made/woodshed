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
#[path = "physical_audio/tests.rs"]
mod tests;
