//! The scenario lane: Redshank drives itself.
//!
//! The same shape Woodshed's `woodshed-genet/src/scenario.rs` proved. The
//! generic half — parsing, the verb loop, selector resolution, assertions —
//! is [`taproot`]; what lives here is only what is Redshank's: its one
//! surface, its named-command vocabulary, the typed [`Observed`] sample it
//! emits events from, and how a presented frame becomes a PNG.
//!
//! Three env vars turn it on:
//!
//! - `REDSHANK_SCENARIO` — path to a `.scn` file (grammar in `taproot`).
//! - `REDSHANK_CAPTURE_DIR` — where `capture <name>` writes `<name>.png` and,
//!   at the end, `scenario.done` whose first line is `RESULT ok` or
//!   `RESULT fail`, followed by the run log.
//! - `REDSHANK_DATA_DIR` (already honoured by `main`) — the seeded fixture
//!   store. A scenario run must set it, or it would read and then overwrite
//!   the real listener state.
//!
//! Captures are in-process readbacks of the same rasterized view the frame
//! presented: no compositor, no foreground race, and no chance of
//! photographing another window.
//!
//! Widths come from the host, not the scenario — `HostOptions::size_env` is
//! pointed at `REDSHANK_WIDTH` / `REDSHANK_HEIGHT`, so one `.scn` is captured
//! at every artboard width by the driver script.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use cambium_genet_winit_host::{AppCtx, Frame, HostPointer, Key, KeyPress, NamedKey, read_frame};
use redshank_model::{ItemId, ThemeMode, ThemeSeed};
use redshank_surfaces::{
    CompactCommand, FullView, Layout, Mode, NotesFilter, RedshankSurfaceState, Scene, Seed,
    SurfaceTab, TransportState,
};
use taproot::{Automatable, Driveable, ProbeSnapshot, ProbeSurface, Progress, Scenario};

/// Self-contained aliases: the lane does not borrow `main`'s, so a rename over
/// there cannot silently change what this file is driving.
type Logic = fn(&RedshankSurfaceState) -> FullView;
type Ctx<'a> = AppCtx<'a, RedshankSurfaceState, Logic, FullView>;

// ---------------------------------------------------------------------------
// The lane
// ---------------------------------------------------------------------------

/// A running scenario plus everything the run needs that the host does not own:
/// where receipts go, the event stream, and the last observed sample.
pub struct ScenarioLane {
    /// Moved out for the duration of a tick so the driver can borrow the rest.
    scenario: Option<Scenario>,
    capture_dir: Option<PathBuf>,
    /// Set once the sentinel has been written, so it is written exactly once.
    finished: bool,
    events: Vec<String>,
    observed: Observed,
}

impl ScenarioLane {
    /// Build the lane from the environment, or `None` for an ordinary run.
    pub fn from_env() -> Option<Self> {
        let path = std::env::var("REDSHANK_SCENARIO").ok()?;
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) => {
                eprintln!("[redshank-scenario] '{path}' unreadable: {e}");
                return None;
            },
        };
        let scenario = match Scenario::parse(&text) {
            Ok(scenario) => scenario,
            Err(e) => {
                eprintln!("[redshank-scenario] '{path}' rejected: {e}");
                return None;
            },
        };
        let capture_dir = std::env::var("REDSHANK_CAPTURE_DIR").ok().map(|dir| {
            let dir = PathBuf::from(dir);
            let _ = std::fs::create_dir_all(&dir);
            dir
        });
        eprintln!("[redshank-scenario] armed: {path}");
        Some(Self {
            scenario: Some(scenario),
            capture_dir,
            finished: false,
            events: Vec::new(),
            observed: Observed::default(),
        })
    }

    /// Pump the scenario one step, after a presented frame. Called from the
    /// host's `after_frame` hook, so every assertion reads a state that was
    /// actually rendered.
    pub fn drive(&mut self, ctx: &mut Ctx<'_>) {
        write_pending_capture();
        self.note_events(ctx.runner.state());
        let Some(mut scenario) = self.scenario.take() else {
            return;
        };
        let progress = {
            let mut probe = Probe { ctx, lane: self };
            scenario.tick(&mut probe)
        };
        // Hold the sentinel until every armed capture has actually been
        // written, or the receipt would claim a screenshot that does not exist.
        if progress == Progress::Done && PENDING.with(|p| p.borrow().is_none()) {
            self.write_outcome(scenario.finish());
            *ctx.close = true;
        }
        self.scenario = Some(scenario);
        // A scenario run must keep frames coming: every step is pumped by one,
        // and an idle app would stall the run rather than finish it.
        if let Some(window) = ctx.window {
            window.request_redraw();
        }
    }

    /// Whether a scenario is still running, for the host's keep-polling answer.
    pub fn active(&self) -> bool {
        !self.finished
    }

    /// Sample the observed state and emit an event for anything that moved.
    /// Events are the app's own transitions, not the driver's intentions, so a
    /// scenario asserting one is asserting that the app really did it.
    fn note_events(&mut self, state: &RedshankSurfaceState) {
        let now = Observed::read(state);
        let before = std::mem::replace(&mut self.observed, now.clone());
        self.events.extend(before.diff(&now));
    }

    /// Write the `scenario.done` sentinel the driver script waits on.
    fn write_outcome(&mut self, outcome: taproot::Outcome) {
        if self.finished {
            return;
        }
        self.finished = true;
        let result = if outcome.ok {
            "RESULT ok"
        } else {
            "RESULT fail"
        };
        let body = std::iter::once(result.to_string())
            .chain(outcome.log.iter().cloned())
            .collect::<Vec<_>>()
            .join("\n");
        eprintln!("[redshank-scenario] {result}");
        for line in &outcome.log {
            eprintln!("[redshank-scenario]   {line}");
        }
        if let Some(dir) = &self.capture_dir {
            let _ = std::fs::write(dir.join("scenario.done"), body);
        }
    }
}

// ---------------------------------------------------------------------------
// The typed observation
// ---------------------------------------------------------------------------

/// The state a scenario asserts against, sampled each time it may have changed.
/// Kept beside the events it produces so a transition is reported once, with
/// what it was and what it became.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Observed {
    pub tab: &'static str,
    /// Owned: `TransportState::Unavailable` carries its own message.
    pub transport: String,
    pub position_ms: u64,
    pub recording: bool,
    pub queue: usize,
    pub notes: usize,
    pub layout: &'static str,
    pub scene: &'static str,
    pub seed: &'static str,
    pub mode: &'static str,
}

impl Observed {
    pub fn read(state: &RedshankSurfaceState) -> Self {
        Self {
            tab: tab_name(state.active_tab),
            transport: state.compact.transport.micro_word().to_owned(),
            position_ms: state
                .compact
                .now_playing
                .as_ref()
                .map_or(0, |now| now.position_ms),
            recording: state.compact.recording.is_some(),
            queue: state.queue.len(),
            notes: state.notes.len(),
            layout: layout_name(state.layout),
            scene: state.scene.label(),
            seed: seed_name(state.seed),
            mode: mode_name(state.mode),
        }
    }

    /// The events `self -> next` produces. Position is bucketed to whole
    /// seconds: a playing item moves it every frame, and a per-millisecond
    /// event would drown the stream an `assert event` greps.
    pub fn diff(&self, next: &Self) -> Vec<String> {
        let mut out = Vec::new();
        if self.tab != next.tab {
            out.push(format!("tab {}", next.tab));
        }
        if self.transport != next.transport {
            out.push(format!("transport {}", next.transport));
        }
        if self.position_ms / 1_000 != next.position_ms / 1_000 {
            out.push(format!("position {}", next.position_ms / 1_000));
        }
        if self.recording != next.recording {
            out.push(format!("recording {}", next.recording));
        }
        if self.queue != next.queue {
            out.push(format!("queue-size {}", next.queue));
        }
        if self.notes != next.notes {
            out.push(format!("note-count {}", next.notes));
        }
        if self.layout != next.layout {
            out.push(format!("layout {}", next.layout));
        }
        if self.scene != next.scene {
            out.push(format!("scene {}", next.scene));
        }
        if self.seed != next.seed {
            out.push(format!("seed {}", next.seed));
        }
        if self.mode != next.mode {
            out.push(format!("mode {}", next.mode));
        }
        out
    }
}

fn tab_name(tab: SurfaceTab) -> &'static str {
    tab.label()
}

fn layout_name(layout: Layout) -> &'static str {
    match layout {
        Layout::Dock => "dock",
        Layout::Rail => "rail",
    }
}

fn seed_name(seed: Seed) -> &'static str {
    match seed {
        Seed::Wetland => "wetland",
        Seed::BrandShell => "brand",
    }
}

fn mode_name(mode: Mode) -> &'static str {
    match mode {
        Mode::Dark => "dark",
        Mode::Light => "light",
        Mode::HcDark => "hc-dark",
        Mode::HcLight => "hc-light",
    }
}

// ---------------------------------------------------------------------------
// The named-command vocabulary
// ---------------------------------------------------------------------------

/// What one `act <label>` resolves to before it touches state.
///
/// Most labels are a plain [`CompactCommand`]; the rest need something the
/// parser cannot see — the settings to carry, or the skip distance the user
/// configured — so they stay symbolic until [`apply`].
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum Named {
    Command(CompactCommand),
    /// Skip by the listener's own configured distance, not a literal.
    SkipBack,
    SkipForward,
    Seed(ThemeSeed),
    Mode(ThemeMode),
}

/// Parse one scenario command label. `None` is an unknown label, which the
/// generic loop fails loudly on rather than silently no-opping.
pub fn parse_named(label: &str) -> Option<Named> {
    let label = label.trim();
    if let Some(rest) = label.strip_prefix("tab:") {
        let tab = match rest {
            "listen" => SurfaceTab::Listen,
            "library" => SurfaceTab::Library,
            "notes" => SurfaceTab::Notes,
            "mere" => SurfaceTab::Mere,
            "settings" => SurfaceTab::Settings,
            _ => return None,
        };
        return Some(Named::Command(CompactCommand::SelectTab(tab)));
    }
    if let Some(rest) = label.strip_prefix("scene:") {
        let scene = match rest {
            "chain" => Scene::Chain,
            "orrery" => Scene::Orrery,
            "trail" => Scene::Trail,
            _ => return None,
        };
        return Some(Named::Command(CompactCommand::SelectScene(scene)));
    }
    if let Some(rest) = label.strip_prefix("layout:") {
        let layout = match rest {
            "dock" => Layout::Dock,
            "rail" => Layout::Rail,
            _ => return None,
        };
        return Some(Named::Command(CompactCommand::SetLayout(layout)));
    }
    if let Some(rest) = label.strip_prefix("notes:") {
        let filter = match rest {
            "this" => NotesFilter::ThisEpisode,
            "all" => NotesFilter::AllNotes,
            _ => return None,
        };
        return Some(Named::Command(CompactCommand::SetNotesFilter(filter)));
    }
    if let Some(rest) = label.strip_prefix("seed:") {
        return match rest {
            "wetland" => Some(Named::Seed(ThemeSeed::Wetland)),
            "brand" => Some(Named::Seed(ThemeSeed::BrandShell)),
            _ => None,
        };
    }
    if let Some(rest) = label.strip_prefix("mode:") {
        return match rest {
            "dark" => Some(Named::Mode(ThemeMode::Dark)),
            "light" => Some(Named::Mode(ThemeMode::Light)),
            "hc-dark" => Some(Named::Mode(ThemeMode::HcDark)),
            "hc-light" => Some(Named::Mode(ThemeMode::HcLight)),
            _ => None,
        };
    }
    if let Some(rest) = label.strip_prefix("seek:") {
        return rest
            .trim()
            .parse::<u64>()
            .ok()
            .map(|ms| Named::Command(CompactCommand::Seek(ms)));
    }
    if let Some(rest) = label.strip_prefix("select:") {
        let rest = rest.trim();
        if rest.is_empty() {
            return None;
        }
        return Some(Named::Command(CompactCommand::SelectItem(ItemId(
            rest.to_owned(),
        ))));
    }
    if let Some(rest) = label.strip_prefix("feed:") {
        let rest = rest.trim();
        // A bare `feed:` clears the selection, which is how the roster (rather
        // than one feed's episode list) is put back on screen.
        let choice = (!rest.is_empty()).then(|| rest.to_owned());
        return Some(Named::Command(CompactCommand::SelectFeed(choice)));
    }
    let command = match label {
        "play" => CompactCommand::Play,
        "pause" => CompactCommand::Pause,
        "replay" => CompactCommand::Replay,
        "skip-back" => return Some(Named::SkipBack),
        "skip-forward" => return Some(Named::SkipForward),
        "text-note" => CompactCommand::AddTextNote,
        "text-note-begin" => CompactCommand::BeginTextNote,
        "text-note-cancel" => CompactCommand::CancelTextNote,
        "voice-begin" => CompactCommand::BeginVoiceNote,
        "voice-finish" => CompactCommand::FinishVoiceNote,
        "voice-cancel" => CompactCommand::CancelVoiceNote,
        "export-notes" => CompactCommand::ExportAnnotations,
        _ => return None,
    };
    Some(Named::Command(command))
}

/// Apply a resolved label to surface state. Everything goes out as a real
/// `CompactCommand`, so a scenario exercises the shipping dispatch rather than
/// a lane-local imitation of it.
pub fn apply(state: &mut RedshankSurfaceState, named: Named) {
    match named {
        Named::Command(command) => state.request(command),
        Named::SkipBack => {
            let ms = state.compact.skip_backward_ms;
            state.request(CompactCommand::SkipBackward(ms));
        },
        Named::SkipForward => {
            let ms = state.compact.skip_forward_ms;
            state.request(CompactCommand::SkipForward(ms));
        },
        // Seed and mode live in two places — the persisted settings and the
        // scope class the sheet is selected by. Set the presentation field so
        // the very next frame is right, and send the settings command so the
        // host persists it the way the Settings tab would.
        Named::Seed(seed) => {
            state.settings.seed = seed;
            state.seed = match seed {
                ThemeSeed::Wetland => Seed::Wetland,
                ThemeSeed::BrandShell => Seed::BrandShell,
            };
            let settings = state.settings.clone();
            state.request(CompactCommand::UpdateSettings(settings));
        },
        Named::Mode(mode) => {
            state.settings.mode = mode;
            state.mode = match mode {
                ThemeMode::Dark => Mode::Dark,
                ThemeMode::Light => Mode::Light,
                ThemeMode::HcDark => Mode::HcDark,
                ThemeMode::HcLight => Mode::HcLight,
            };
            let settings = state.settings.clone();
            state.request(CompactCommand::UpdateSettings(settings));
        },
    }
}

// ---------------------------------------------------------------------------
// Capture
// ---------------------------------------------------------------------------

/// A capture armed for the next presented frame: its path and the readback slot.
type PendingCapture = (PathBuf, Rc<RefCell<Option<Frame>>>);

thread_local! {
    /// The capture armed for the next presented frame: where it goes and the
    /// readback the host will drop into it. A thread-local because the capture
    /// closure outlives the tick that armed it, and the application is
    /// single-threaded on the event loop.
    static PENDING: RefCell<Option<PendingCapture>> =
        const { RefCell::new(None) };
}

/// Encode whatever readback the last armed capture produced.
fn write_pending_capture() {
    let taken = PENDING.with(|p| p.borrow_mut().take());
    let Some((path, sink)) = taken else { return };
    let frame = sink.borrow_mut().take();
    match frame {
        Some(frame) => {
            if !write_png(&frame, &path) {
                eprintln!("[redshank-scenario] capture failed: {}", path.display());
            }
        },
        // Not presented yet: put it back and try again next frame.
        None => PENDING.with(|p| *p.borrow_mut() = Some((path, sink))),
    }
}

/// Write a read-back frame as a PNG. The same pixels the frame presented, so
/// the receipt is the frame.
fn write_png(frame: &Frame, path: &Path) -> bool {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let Ok(file) = std::fs::File::create(path) else {
        return false;
    };
    use image::ImageEncoder;
    image::codecs::png::PngEncoder::new(file)
        .write_image(
            &frame.rgba,
            frame.width,
            frame.height,
            image::ExtendedColorType::Rgba8,
        )
        .is_ok()
}

// ---------------------------------------------------------------------------
// The probe
// ---------------------------------------------------------------------------

/// The `Automatable` view of Redshank, borrowed for one tick.
///
/// The host owns the runner, so the application cannot hold a long-lived `&mut`
/// to it; the probe borrows the hook's context for exactly as long as the
/// driver needs and queues pointer delivery back through the host, which runs
/// it through the same hit test, capture, and dispatch a real mouse takes.
struct Probe<'a, 'c> {
    ctx: &'a mut Ctx<'c>,
    lane: &'a mut ScenarioLane,
}

impl Automatable for Probe<'_, '_> {
    fn with_surfaces<R>(&self, f: impl FnOnce(&[ProbeSurface<'_>]) -> R) -> R {
        let dom = self.ctx.runner.dom();
        let dom_ref = dom.borrow();
        let (w, h) = self.ctx.logical_size;
        let sheet = redshank_surfaces::sheet();
        f(&[ProbeSurface {
            name: "redshank",
            dom: &dom_ref,
            // One runner covers the window, and the probe resolves in the same
            // logical coordinates the layout and the cursor use.
            rect: [0.0, 0.0, w, h],
            sheet: &sheet,
        }])
    }

    fn snapshot(&self) -> ProbeSnapshot {
        let state = self.ctx.runner.state();
        let observed = Observed::read(state);
        let now = state.compact.now_playing.as_ref();
        let mut snap = ProbeSnapshot::default()
            .with_field("tab", observed.tab)
            .with_field("transport", observed.transport.clone())
            .with_field("position-ms", observed.position_ms.to_string())
            .with_field("recording", observed.recording.to_string())
            .with_field("queue-size", observed.queue.to_string())
            .with_field("note-count", observed.notes.to_string())
            .with_field("layout", observed.layout)
            .with_field("scene", observed.scene)
            .with_field("seed", observed.seed)
            .with_field("mode", observed.mode)
            .with_field("item-count", state.items.len().to_string())
            .with_field("feed-count", state.feeds.len().to_string())
            .with_field("session-count", state.sessions.len().to_string())
            .with_field("rate-percent", state.compact.rate_percent.to_string())
            .with_field("volume-percent", state.compact.volume_percent.to_string())
            .with_field(
                "voice-available",
                state.compact.voice_capture_available.to_string(),
            )
            .with_field("notes-filter", notes_filter_name(state.notes_filter))
            .with_field(
                "selected-feed",
                state.selected_feed.clone().unwrap_or_default(),
            )
            .with_field("notice", state.notice.clone().unwrap_or_default())
            .with_field("text-capture", state.text_capture.is_some().to_string());
        if let Some(now) = now {
            snap = snap
                .with_field("item-id", now.item_id.0.clone())
                .with_field("item-title", now.title.clone())
                .with_field(
                    "duration-ms",
                    now.duration_ms.unwrap_or_default().to_string(),
                )
                .with_field("buffered-percent", now.buffered_percent.to_string())
                .with_field("marker-count", now.markers.len().to_string())
                .with_focus(now.title.clone());
        }
        if let TransportState::Unavailable(message) = &state.compact.transport {
            snap = snap.with_field("unavailable", message.clone());
        }
        snap
    }

    fn drain_events(&mut self) -> Vec<String> {
        std::mem::take(&mut self.lane.events)
    }

    fn act(&mut self, label: &str) -> bool {
        let Some(named) = parse_named(label) else {
            return false;
        };
        self.ctx.runner.update(|state| apply(state, named));
        self.lane.note_events(self.ctx.runner.state());
        true
    }

    fn press(&mut self, x: f32, y: f32) {
        // Routed by the host: the same hit test, capture, and dispatch a real
        // pointer takes. Delivered once this hook returns, and observed by the
        // next frame's tick — which is where the scenario asserts anyway.
        self.ctx.pointer.push(HostPointer::Press(x, y));
    }

    fn moved(&mut self, x: f32, y: f32) {
        self.ctx.pointer.push(HostPointer::Moved(x, y));
    }

    fn release(&mut self, x: f32, y: f32) {
        self.ctx.pointer.push(HostPointer::Release(x, y));
    }

    fn busy(&mut self) -> Option<bool> {
        // The one in-flight condition this lane can see from surface state: a
        // remote item whose bytes have not arrived. Fetch, cache, and
        // persistence live on `Desktop`, which the lane deliberately does not
        // hold — so `wait` is honest about buffering and nothing else.
        Some(matches!(
            self.ctx.runner.state().compact.transport,
            TransportState::Buffering
        ))
    }
}

fn notes_filter_name(filter: NotesFilter) -> &'static str {
    match filter {
        NotesFilter::ThisEpisode => "this-episode",
        NotesFilter::AllNotes => "all-notes",
    }
}

impl Driveable for Probe<'_, '_> {
    fn app_step(&mut self, line: &str) -> Result<(), String> {
        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("ui-zoom") => {
                let zoom: f32 = parts
                    .next()
                    .ok_or("ui-zoom wants a scale")?
                    .parse()
                    .map_err(|_| "invalid UI zoom")?;
                if parts.next().is_some() || !zoom.is_finite() || !(0.5..=4.0).contains(&zoom) {
                    return Err("ui-zoom requires a scale from 0.5 to 4".into());
                }
                *self.ctx.set_ui_zoom = Some(zoom);
                Ok(())
            },
            Some("pointer-click") => {
                // For receipts measured from a presented frame, when the
                // probe's independently computed selector layout disagrees
                // with the host's.
                let x: f32 = parts
                    .next()
                    .ok_or("pointer-click wants x y")?
                    .parse()
                    .map_err(|_| "invalid pointer x")?;
                let y: f32 = parts
                    .next()
                    .ok_or("pointer-click wants x y")?
                    .parse()
                    .map_err(|_| "invalid pointer y")?;
                let (w, h) = self.ctx.logical_size;
                if parts.next().is_some() || !(0.0..w).contains(&x) || !(0.0..h).contains(&y) {
                    return Err("pointer-click requires an in-window logical point".into());
                }
                self.moved(x, y);
                self.press(x, y);
                self.release(x, y);
                Ok(())
            },
            // The shipping keyboard path, not a lane-local imitation: the same
            // `key_intercept` the host calls for a real press. There is no
            // synthetic key delivery through `AppCtx` at this host rev, so this
            // is as close to the real thing as the seam allows — it exercises
            // the shortcut table but not focus routing or text entry.
            Some("key") => {
                let name = parts.next().ok_or("key wants a key name")?;
                if parts.next().is_some() {
                    return Err("key takes one key name".into());
                }
                let key = parse_key(name).ok_or_else(|| format!("unknown key '{name}'"))?;
                crate::key_intercept(self.ctx.runner, &KeyPress::new(key));
                self.lane.note_events(self.ctx.runner.state());
                Ok(())
            },
            _ => Err(format!("unknown verb: {line}")),
        }
    }

    fn capture(&mut self, name: &str) -> bool {
        let Some(path) = self
            .lane
            .capture_dir
            .as_ref()
            .map(|dir| dir.join(format!("{name}.png")))
        else {
            // No capture dir: the run is an assertion-only receipt. Say so
            // rather than claiming a screenshot that does not exist.
            eprintln!("[redshank-scenario] capture '{name}' skipped: no REDSHANK_CAPTURE_DIR");
            return true;
        };
        let sink = Rc::new(RefCell::new(None::<Frame>));
        let out = sink.clone();
        *self.ctx.capture = Some(Box::new(move |surface, view, w, h| {
            *out.borrow_mut() = read_frame(surface, view, w, h);
        }));
        PENDING.with(|p| *p.borrow_mut() = Some((path, sink)));
        if let Some(window) = self.ctx.window {
            window.request_redraw();
        }
        true
    }
}

/// The key names a `key` verb accepts: the shortcuts the dock documents, plus
/// any single character.
fn parse_key(name: &str) -> Option<Key> {
    Some(match name {
        "space" => Key::Named(NamedKey::Space),
        "left" => Key::Named(NamedKey::ArrowLeft),
        "right" => Key::Named(NamedKey::ArrowRight),
        "enter" => Key::Named(NamedKey::Enter),
        "escape" => Key::Named(NamedKey::Escape),
        other if other.chars().count() == 1 => Key::Character(other.to_owned()),
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Tests: the pure halves, which do not need a window.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_command_vocabulary_parses_every_documented_label() {
        assert_eq!(
            parse_named("play"),
            Some(Named::Command(CompactCommand::Play))
        );
        assert_eq!(parse_named("skip-back"), Some(Named::SkipBack));
        assert_eq!(parse_named("skip-forward"), Some(Named::SkipForward));
        assert_eq!(
            parse_named("tab:settings"),
            Some(Named::Command(CompactCommand::SelectTab(
                SurfaceTab::Settings
            )))
        );
        assert_eq!(
            parse_named("scene:orrery"),
            Some(Named::Command(CompactCommand::SelectScene(Scene::Orrery)))
        );
        assert_eq!(
            parse_named("layout:rail"),
            Some(Named::Command(CompactCommand::SetLayout(Layout::Rail)))
        );
        assert_eq!(
            parse_named("seed:brand"),
            Some(Named::Seed(ThemeSeed::BrandShell))
        );
        assert_eq!(
            parse_named("mode:hc-light"),
            Some(Named::Mode(ThemeMode::HcLight))
        );
        assert_eq!(
            parse_named("seek:1240"),
            Some(Named::Command(CompactCommand::Seek(1_240)))
        );
        assert_eq!(
            parse_named("select:local-one"),
            Some(Named::Command(CompactCommand::SelectItem(ItemId(
                "local-one".into()
            ))))
        );
        assert_eq!(
            parse_named("feed:http://127.0.0.1:8765/feed.xml"),
            Some(Named::Command(CompactCommand::SelectFeed(Some(
                "http://127.0.0.1:8765/feed.xml".into()
            ))))
        );
        // A bare `feed:` puts the roster back, rather than selecting "".
        assert_eq!(
            parse_named("feed:"),
            Some(Named::Command(CompactCommand::SelectFeed(None)))
        );
    }

    #[test]
    fn an_unknown_label_is_a_miss_the_loop_can_fail_on() {
        assert_eq!(parse_named("fly"), None);
        assert_eq!(parse_named("tab:nowhere"), None);
        assert_eq!(parse_named("seek:soon"), None);
        assert_eq!(parse_named("select:"), None);
    }

    #[test]
    fn applying_a_label_emits_a_real_command() {
        let mut state = RedshankSurfaceState::default();
        apply(&mut state, parse_named("play").unwrap());
        apply(&mut state, parse_named("skip-back").unwrap());
        let commands: Vec<_> = state.drain_commands().collect();
        assert_eq!(
            commands,
            vec![CompactCommand::Play, CompactCommand::SkipBackward(15_000),]
        );
    }

    #[test]
    fn a_seed_change_moves_the_scope_class_and_persists() {
        let mut state = RedshankSurfaceState::default();
        assert_eq!(state.scope_class(), "t-redshank");
        apply(&mut state, parse_named("seed:brand").unwrap());
        assert_eq!(state.scope_class(), "t-dark");
        apply(&mut state, parse_named("mode:light").unwrap());
        assert_eq!(state.scope_class(), "t-light");
        let commands: Vec<_> = state.drain_commands().collect();
        assert_eq!(commands.len(), 2, "each carries the settings to persist");
        assert!(matches!(
            commands[1],
            CompactCommand::UpdateSettings(ref s)
                if s.seed == ThemeSeed::BrandShell && s.mode == ThemeMode::Light
        ));
    }

    #[test]
    fn observed_reports_one_event_per_transition() {
        let before = Observed::default();
        let mut state = RedshankSurfaceState::default();
        state.active_tab = SurfaceTab::Notes;
        state.layout = Layout::Rail;
        let now = Observed::read(&state);
        let events = before.diff(&now);
        assert!(events.contains(&"tab Notes".to_string()), "{events:?}");
        assert!(events.contains(&"layout rail".to_string()), "{events:?}");
        // A sample that did not move emits nothing at all.
        assert!(now.diff(&now).is_empty());
    }

    #[test]
    fn position_events_are_bucketed_to_seconds() {
        let a = Observed {
            position_ms: 1_100,
            ..Observed::default()
        };
        let b = Observed {
            position_ms: 1_400,
            ..a.clone()
        };
        let c = Observed {
            position_ms: 2_050,
            ..a.clone()
        };
        assert!(a.diff(&b).is_empty(), "same second, no event");
        assert_eq!(a.diff(&c), vec!["position 2".to_string()]);
    }

    #[test]
    fn the_key_verb_knows_the_documented_shortcuts() {
        assert!(matches!(
            parse_key("space"),
            Some(Key::Named(NamedKey::Space))
        ));
        assert!(matches!(parse_key("n"), Some(Key::Character(_))));
        assert!(parse_key("hyperspace").is_none());
    }
}
