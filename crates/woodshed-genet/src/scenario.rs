//! The scenario lane: woodshed drives itself.
//!
//! Woodshed had no self-drive hook, so every headed receipt was SendKeys plus a
//! desktop grab: synthetic input that loses the foreground race whenever the
//! machine is in use, and captures that can silently photograph the wrong
//! window. This lane replaces both. The generic half (parsing, the verb loop,
//! selector resolution, assertions) is [`taproot`]; what lives here is only
//! what is woodshed's: which surfaces it has, what it can be asked to observe,
//! its named commands, and its acceptance diagnostics. Mesquite owns captures and completion.
//!
//! Since the host extraction it no longer owns *routing*. A delivered point is
//! queued as a [`HostPointer`] and the host runs it through the same hit test,
//! capture, and dispatch a real mouse takes — so a receipt exercises the
//! shipping path rather than an app-local imitation of it.
//!
//! Two env vars turn it on:
//!
//! - `WOODSHED_SCENARIO` — path to a `.scn` file (grammar in `taproot`).
//! - `WOODSHED_CAPTURE_DIR` — where `capture <name>` writes `<name>.png` and,
//!   at the end, `scenario.done` whose first line is `RESULT ok` or
//!   `RESULT fail`.
//! - `WOODSHED_PRESENTATION_TIMEOUT_SECS` — optional acceptance-only startup
//!   allowance, bounded to 10–60 seconds, for bringing a background window
//!   forward. The default remains ten seconds; progress still requires frames.
//!
//! Captures are in-process readbacks of the same rasterized view the frame
//! presented, so they need no compositor, no foreground, and no ffmpeg, and
//! they cannot photograph another window by mistake.
//!
//! `WOODSHED_STATE` overrides the session file. A scenario run must set it:
//! without it the run would read, and then overwrite, the real practice
//! session.

use std::cell::RefCell;
use std::rc::Rc;

use cambium_genet_winit_host::HostPointer;
use taproot::{Automatable, AutomatableExt, ProbeSnapshot, ProbeSurface, Selector};
use woodshed_core::Lens;
use woodshed_views::stage::canvas_camera::{self, CanvasSurface};
use woodshed_views::stage::{
    UiState, set_graph_relation_choices, set_graph_snapshot, set_graph_swatch_from_snapshot,
};

use crate::shared::Shared;
use crate::sync::Ctx;

/// Product commands and observations; Mesquite owns the frame pump and captures.
pub struct Product {
    shared: Rc<RefCell<Shared>>,
    sheet: String,
    export_wait: Option<(String, std::time::Instant)>,
    take_playback_wait: Option<(usize, std::time::Instant, std::cell::Cell<bool>)>,
    scenario_cursor_wait: RefCell<Option<ScenarioCursorWait>>,
}

#[derive(Clone, Copy)]
struct ScenarioCursorWait {
    expected_cursor: usize,
    minimum: std::time::Duration,
    started: std::time::Instant,
}

pub fn from_env(shared: Rc<RefCell<Shared>>) -> Option<mesquite::Lane<Product>> {
    let config = mesquite::LaneConfig::from_env("WOODSHED")?;
    let sheet = shared.borrow().accessible_sheet();
    Some(
        mesquite::Lane::from_config(
            config,
            Product {
                shared,
                sheet,
                export_wait: None,
                take_playback_wait: None,
                scenario_cursor_wait: RefCell::new(None),
            },
            cambium_genet_winit_host::read_file,
        )
        .expect("load Woodshed scenario")
        .with_presentation_timeout(std::time::Duration::from_secs(
            std::env::var("WOODSHED_PRESENTATION_TIMEOUT_SECS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(10)
                .clamp(10, 60),
        )),
    )
}

impl mesquite::Product for Product {
    type State = UiState;
    type Logic = crate::sync::Logic;
    type View = woodshed_views::stage::UiChild;
    const KIND: &'static str = "woodshed";
    const SURFACE: &'static str = "woodshed";
    const LOG_PREFIX: &'static str = "woodshed-genet";
    fn sheet(&self) -> &str {
        &self.sheet
    }
    fn snapshot(&self, ctx: &Ctx<'_>, _: usize, _: f32) -> ProbeSnapshot {
        Snapshot {
            ctx,
            shared: &self.shared.borrow(),
        }
        .snapshot()
        .with_field("take-playback-observed", self.take_playback_wait.as_ref()
            .is_some_and(|(_, _, observed)| observed.get()).to_string())
    }
    fn drain_events(&mut self, _: &mut Ctx<'_>) -> Vec<String> {
        std::mem::take(&mut self.shared.borrow_mut().events)
    }
    fn busy(&self, ctx: &Ctx<'_>, capture_pending: bool) -> Option<bool> {
        let export_wait = self
            .export_wait
            .as_ref()
            .is_some_and(|(expected, started)| {
                started.elapsed().as_secs() < 180
                    && !ctx
                        .runner
                        .state()
                        .loop_export_notice
                        .as_deref()
                        .is_some_and(|notice| notice.contains(expected))
            });
        let take_wait = self
            .take_playback_wait
            .as_ref()
            .is_some_and(|(bar, started, observed)| {
                if !observed.get()
                    && self
                        .shared
                        .borrow()
                        .backend
                        .as_ref()
                        .and_then(|backend| backend.output_snapshot())
                        .is_some_and(|snapshot| {
                            snapshot.playing
                                && snapshot.cursor.bar_idx == *bar
                                && snapshot.cursor.sample_in_bar > 0
                        })
                {
                    observed.set(true);
                }
                !observed.get() && started.elapsed().as_secs() < 30
            });
        let scenario_cursor_wait = {
            let mut wait = self.scenario_cursor_wait.borrow_mut();
            match *wait {
                Some(wait)
                    if ctx.runner.state().set.cursor != wait.expected_cursor
                        || wait.started.elapsed() < wait.minimum =>
                {
                    true
                }
                Some(_) => {
                    *wait = None;
                    false
                }
                None => false,
            }
        };
        Some(
            take_wait || scenario_cursor_wait || export_wait
                || ctx.runner.state().loop_export_busy
                || capture_pending
                || self
                    .shared
                    .borrow()
                    .backend
                    .as_ref()
                    .is_some_and(|backend| backend.preview_busy()),
        )
    }
    fn act(&mut self, ctx: &mut Ctx<'_>, label: &str) -> bool {
        let scenario_wait = match label {
            "p5a-wait-for-manual" => Some((2, std::time::Duration::from_secs(1))),
            "p5a-wait-on-manual" => Some((2, std::time::Duration::from_secs(2))),
            "p5a-wait-on-retried-manual" => Some((4, std::time::Duration::from_secs(2))),
            _ => None,
        };
        if let Some((expected_cursor, minimum)) = scenario_wait {
            self.scenario_cursor_wait.replace(Some(ScenarioCursorWait {
                expected_cursor,
                minimum,
                started: std::time::Instant::now(),
            }));
            return true;
        }
        self.scenario_cursor_wait.replace(None);
        if label == "await-selected-take-playback" {
            let ui = ctx.runner.state();
            let Some(bar) = ui.loop_selected.and_then(|id| ui.loop_admitted.as_ref()?.index_of(id))
            else { return false; };
            self.take_playback_wait = Some((bar, std::time::Instant::now(), std::cell::Cell::new(false)));
            return true;
        }
        if let Some(expected) = match label {
            "wav-native-await-cancel" => Some("WAV export cancelled"),
            "wav-native-await-save" => Some("Exported "),
            "wav-native-await-collision" => Some("already exists"),
            "wav-native-await-replacement-cancel" => Some("WAV replacement cancelled"),
            _ => None,
        } {
            ctx.runner.update(|ui| ui.loop_export_notice = None);
            self.export_wait = Some((expected.into(), std::time::Instant::now()));
            return true;
        }
        if let Some(stem) = label.strip_prefix("fold-checkpoint-") {
            return crate::fold_receipt::write(ctx.runner.state(), &self.shared.borrow(), stem);
        }
        if label == "verify-mere-take-retention" {
            // Receipt-only observation of an explicitly isolated scenario profile.
            let verify = || -> Result<(), String> {
                let shared = self.shared.borrow();
                if !matches!(shared.seal, Some(woodshed_views::persona::PracticeSeal::Sealed { .. })) {
                    return Err("take receipt requires real sealed custody".into());
                }
                let json = shared.storage.as_ref().ok_or("no store")?
                    .load().ok_or("sealed session unreadable")?;
                let persisted = woodshed_core::storage::decode_session(&json)
                    .map_err(|e| e.to_string())?.session;
                let presentation: woodshed_views::stage::OverviewPresentation =
                    serde_json::from_str(persisted.overview_presentation_json.as_deref().ok_or("no presentation")?)
                    .map_err(|e| e.to_string())?;
                let ui = ctx.runner.state();
                if presentation.version != 2 || persisted.loop_captures != ui.loop_captures
                    || presentation.focus != ui.overview_focus {
                    return Err("sealed presentation or take manifest differs from the live view".into());
                }
                let positions: std::collections::BTreeMap<_, _> = presentation.positions.iter().cloned().collect();
                let Some(id @ woodshed_core::session_overview::OverviewNodeId::Artifact(
                    woodshed_core::session_overview::SessionArtifactId::LoopTake(_))) = ui.overview_focus.as_ref()
                else { return Err("focus is not a retained take".into()); };
                let live_positions = ui.overview_dynamics.positions();
                let live = live_positions
                    .get(id)
                    .or_else(|| ui.overview_positions.get(id));
                if !positions.get(id).zip(live).is_some_and(|(stored, live)| {
                    (stored.0 - live.0).abs() < 0.00001 && (stored.1 - live.1).abs() < 0.00001
                }) {
                    return Err("take placement did not round-trip".into());
                }
                let captures = shared.backend.as_ref().ok_or("no output backend")?.capture_snapshot();
                if captures.is_empty() { return Err("no hydrated PCM".into()); }
                for take in &captures {
                    let reference = persisted.loop_captures.takes.iter().find(|r| r.id == take.id)
                        .ok_or("hydrated take has no persisted reference")?;
                    let stored = shared.storage.as_ref().unwrap().try_read_capture(
                        persisted.loop_captures.namespace.as_deref().ok_or("no capture namespace")?, reference)?;
                    if stored.len() != take.buffer.len() || !stored.iter().zip(take.buffer.data.iter())
                        .all(|(stored, live)| stored.to_bits() == live.to_bits()) {
                        return Err("sealed PCM differs from hydrated PCM".into());
                    }
                    for (index, sample) in take.buffer.data.iter().enumerate() {
                        if sample.to_bits() != [0.000125f32, -0.00025, -0.0, 0.0005][index % 4].to_bits() {
                            return Err("deterministic PCM bits changed".into());
                        }
                    }
                }
                let directory = std::path::PathBuf::from(std::env::var_os("WOODSHED_CAPTURE_DIR").ok_or("no receipt directory")?);
                let report = serde_json::json!({
                    "sealed": true, "presentation": presentation,
                    "manifest": persisted.loop_captures,
                    "hydrated": captures.iter().map(|take| serde_json::json!({
                        "id": take.id, "frames": take.buffer.len(),
                        "sample_rate": take.buffer.sample_rate_hz,
                        "legacy_clock": take.legacy_clock,
                        "exact_fixture_bits": true, "exact_sealed_companion_bits": true,
                    })).collect::<Vec<_>>(),
                });
                use std::io::Write;
                let mut file = std::fs::OpenOptions::new().write(true).create_new(true)
                    .open(directory.join("take-retention.json")).map_err(|e| e.to_string())?;
                file.write_all(serde_json::to_string_pretty(&report).unwrap().as_bytes()).map_err(|e| e.to_string())?;
                Ok(())
            };
            return match verify() { Ok(()) => true, Err(error) => { eprintln!("Mere take receipt: {error}"); false } };
        }
        if let Some(stem) = label.strip_prefix("p5a-save-") {
            return p5a_write_card_receipt(ctx, &self.shared, stem, false);
        }
        if let Some(stem) = label.strip_prefix("p5a-verify-") {
            return p5a_write_card_receipt(ctx, &self.shared, stem, true);
        }
        if label == "catalog-audition-save-baseline" {
            return catalog_audition_receipt(ctx, &self.shared, CatalogAuditionReceipt::Baseline);
        }
        if label == "catalog-audition-check-one" {
            return catalog_audition_receipt(ctx, &self.shared, CatalogAuditionReceipt::OneAccepted);
        }
        if label == "catalog-audition-save-final" {
            return catalog_audition_receipt(ctx, &self.shared, CatalogAuditionReceipt::SaveFinal);
        }
        if label == "catalog-audition-verify-final" {
            return catalog_audition_receipt(ctx, &self.shared, CatalogAuditionReceipt::VerifyFinal);
        }
        if label == "wav-export-fixture" || label == "wav-export-backing-fixture" {
            let Some(directory) = std::env::var_os("WOODSHED_CAPTURE_DIR") else {
                return false;
            };
            let name = if label == "wav-export-fixture" {
                "loop-export.wav"
            } else {
                "backing-export.wav"
            };
            let path = std::path::PathBuf::from(directory).join(name);
            ctx.runner.update(|ui| {
                ui.loop_export_requests
                    .push(woodshed_views::stage::LoopExportRequest::ChooseDestination);
            });
            crate::wav_export::after_dispatch_with_chooser(
                &mut self.shared.borrow_mut(),
                ctx,
                |_| Some(path.clone()),
            );
            return true;
        }
        Probe {
            ctx,
            shared: &mut self.shared.borrow_mut(),
            clicks: &mut mesquite::Clicks::default(),
        }
        .act(label)
    }
    fn app_step_with_clicks(
        &mut self,
        ctx: &mut Ctx<'_>,
        _: mesquite::Checkpoints<'_>,
        clicks: &mut mesquite::Clicks,
        line: &str,
    ) -> Result<(), String> {
        self.scenario_cursor_wait.replace(None);
        Probe {
            ctx,
            shared: &mut self.shared.borrow_mut(),
            clicks,
        }
        .app_step(line)
    }
    fn receipt_lines(&self) -> Vec<String> {
        let shared = self.shared.borrow();
        if shared.drag_frame_metrics.samples > 0 {
            vec![shared.drag_frame_metrics.summary()]
        } else {
            vec![]
        }
    }
}

/// Save or compare the exact authored Card payload in an isolated sealed
/// scenario profile. This receipt never edits the session; the UI and normal
/// host persistence path remain the writers.
fn p5a_write_card_receipt(
    ctx: &mut Ctx<'_>,
    shared: &Rc<RefCell<Shared>>,
    stem: &str,
    verify: bool,
) -> bool {
    if stem.is_empty()
        || !stem
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        eprintln!("P5a receipt refused: invalid receipt stem");
        return false;
    }
    let Some(directory) = std::env::var_os("WOODSHED_CAPTURE_DIR") else {
        eprintln!("P5a receipt refused: no isolated receipt directory");
        return false;
    };
    let shared = shared.borrow();
    if !matches!(
        shared.seal.as_ref(),
        Some(woodshed_views::persona::PracticeSeal::Sealed { .. })
    ) {
        eprintln!("P5a receipt refused: profile is not sealed");
        return false;
    }
    let Some(json) = shared.storage.as_ref().and_then(|storage| storage.load()) else {
        eprintln!("P5a receipt refused: sealed session is unreadable");
        return false;
    };
    let persisted = match woodshed_core::storage::decode_session(&json) {
        Ok(decoded) => decoded.session,
        Err(error) => {
            eprintln!("P5a receipt refused: session decode failed: {error}");
            return false;
        }
    };
    let ui = ctx.runner.state();
    let current_cards = match serde_json::to_value(&ui.set.cards) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("P5a receipt refused: live Card encoding failed: {error}");
            return false;
        }
    };
    let persisted_cards = match serde_json::to_value(&persisted.set.cards) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("P5a receipt refused: persisted Card encoding failed: {error}");
            return false;
        }
    };
    if current_cards != persisted_cards {
        eprintln!("P5a receipt refused: live and sealed Card payloads differ");
        return false;
    }
    let selected = ui.set.cards.get(ui.set.cursor);
    let resolved_preview = selected.map(|card| {
        match ui
            .current_card_stage()
            .card_sounding_pitches_at_tempo(card, ui.transport.bpm)
        {
            Ok((pitches, duration, spacing)) => serde_json::json!({
                "pitch_bits": pitches.iter().map(|pitch| pitch.to_bits()).collect::<Vec<_>>(),
                "duration_bits": duration.to_bits(),
                "spacing_ms_bits": spacing.to_bits(),
            }),
            Err(error) => serde_json::json!({ "refused": error.to_string() }),
        }
    });
    let active_owner = ui.working_sets.active_id.0;
    let cursor = ui.set.cursor;
    let selected_card_id = selected.map(|card| card.id.0);
    let path = std::path::PathBuf::from(directory).join(format!("p5a-{stem}.json"));
    if verify {
        let expected = match std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        {
            Some(value) => value,
            None => {
                eprintln!("P5a receipt refused: expected checkpoint is missing or invalid");
                return false;
            }
        };
        if expected.get("cards") != Some(&current_cards)
            || expected.get("owner") != Some(&serde_json::json!(active_owner))
            || expected.get("cursor") != Some(&serde_json::json!(cursor))
            || expected.get("selected_card_id") != Some(&serde_json::json!(selected_card_id))
            || expected.get("resolved_preview") != Some(&serde_json::json!(resolved_preview))
        {
            eprintln!("P5a receipt refused: reopened authored Cards differ from checkpoint");
            return false;
        }
        return true;
    }
    let report = serde_json::json!({
        "sealed": true,
        "cards": current_cards,
        "persisted_cards_match": true,
        "owner": active_owner,
        "cursor": cursor,
        "selected_card_id": selected_card_id,
        "resolved_preview": resolved_preview,
        "output_runtime_id": shared.backend.as_ref()
            .and_then(|backend| backend.output_observation_id()),
        "preview_starts": shared.backend.as_ref()
            .and_then(|backend| backend.output_snapshot()).map(|snapshot| snapshot.preview_starts),
        "output_sample_rate": shared.backend.as_ref()
            .and_then(|backend| backend.output_sample_rate()),
    });
    use std::io::Write;
    let mut file = match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(file) => file,
        Err(error) => {
            eprintln!("P5a receipt refused: could not create checkpoint: {error}");
            return false;
        }
    };
    match serde_json::to_vec_pretty(&report)
        .map_err(|error| error.to_string())
        .and_then(|bytes| file.write_all(&bytes).map_err(|error| error.to_string()))
    {
        Ok(()) => true,
        Err(error) => {
            eprintln!("P5a receipt refused: checkpoint write failed: {error}");
            false
        }
    }
}

#[derive(Clone, Copy)]
enum CatalogAuditionReceipt {
    Baseline,
    OneAccepted,
    SaveFinal,
    VerifyFinal,
}

fn catalog_audition_events(
    history: &woodshed_core::history::PracticeHistory,
) -> Vec<woodshed_core::history::PracticeEvent> {
    history.recent(history.len())
}

fn catalog_audition_cards(ui: &UiState) -> Result<serde_json::Value, String> {
    serde_json::to_value(&ui.set.cards).map_err(|error| error.to_string())
}

fn valid_pattern_audition(
    event: &woodshed_core::history::PracticeEvent,
    subject: &str,
    from: &str,
    owner: woodshed_core::working_sets::WorkingSetId,
) -> bool {
    event.kind == woodshed_core::history::EngagementKind::Previewed
        && event.subject_id == subject
        && event.from_id.as_deref() == Some(from)
        && event.provenance.as_ref().is_some_and(|provenance| {
            provenance.working_set_id == Some(owner)
                && serde_json::to_string(&provenance.card_snapshot)
                    .is_ok_and(|snapshot| snapshot.contains("ScalePattern"))
        })
}

fn catalog_audition_receipt(
    ctx: &mut Ctx<'_>,
    shared: &Rc<RefCell<Shared>>,
    receipt: CatalogAuditionReceipt,
) -> bool {
    let fail = |reason: &str| {
        eprintln!("Catalog audition receipt refused: {reason}");
        false
    };
    let Some(directory) = std::env::var_os("WOODSHED_CAPTURE_DIR") else {
        return fail("no isolated receipt directory");
    };
    let path = std::path::PathBuf::from(directory);
    let Some(shared_storage) = shared.borrow().storage.as_ref().and_then(|storage| storage.load()) else {
        return fail("sealed session is unreadable");
    };
    let persisted = match woodshed_core::storage::decode_session(&shared_storage) {
        Ok(decoded) => decoded.session,
        Err(error) => return fail(&format!("session decode failed: {error}")),
    };
    let ui = ctx.runner.state();
    if !matches!(
        shared.borrow().seal.as_ref(),
        Some(woodshed_views::persona::PracticeSeal::Sealed { .. })
    ) {
        return fail("profile is not sealed");
    }
    let events = catalog_audition_events(&ui.practice_history);
    let cards = match catalog_audition_cards(ui) {
        Ok(cards) => cards,
        Err(error) => return fail(&format!("live Card encoding failed: {error}")),
    };
    let owner = ui.working_sets.active_id;
    let baseline_path = path.join("catalog-audition-baseline.json");
    match receipt {
        CatalogAuditionReceipt::Baseline => {
            let report = serde_json::json!({
                "owner": owner,
                "events": events,
                "cards": cards,
            });
            write_new_json(&baseline_path, &report).is_ok()
        },
        CatalogAuditionReceipt::OneAccepted => {
            let Some(baseline) = std::fs::read(&baseline_path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            else {
                return fail("baseline checkpoint is missing or invalid");
            };
            let Some(baseline_events) = baseline
                .get("events")
                .and_then(|value| serde_json::from_value::<Vec<woodshed_core::history::PracticeEvent>>(value.clone()).ok())
            else {
                return fail("baseline event history is invalid");
            };
            let Some(baseline_cards) = baseline.get("cards") else {
                return fail("baseline Card snapshot is missing");
            };
            if baseline.get("owner") != Some(&serde_json::json!(owner))
                || baseline_cards != &cards
                || events.len() != baseline_events.len() + 1
                || events.get(1..) != Some(baseline_events.as_slice())
                || !events.first().is_some_and(|event| {
                    valid_pattern_audition(event, "exercise:scale-thirds/v1:Major", "scale:Major", owner)
                })
            {
                return fail("refused audition changed authored Cards or history, or accepted event attribution differs");
            }
            true
        },
        CatalogAuditionReceipt::SaveFinal | CatalogAuditionReceipt::VerifyFinal => {
            let final_path = path.join("catalog-audition-final.json");
            let reopened_path = path.join("catalog-audition-reopened.json");
            let expected = if matches!(receipt, CatalogAuditionReceipt::VerifyFinal) {
                let Some(expected) = std::fs::read(&final_path)
                    .ok()
                    .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                else {
                    return fail("final checkpoint is missing or invalid");
                };
                Some(expected)
            } else {
                None
            };
            let baseline_events = if let Some(expected) = &expected {
                expected.get("baseline_events")
                    .and_then(|value| serde_json::from_value::<Vec<woodshed_core::history::PracticeEvent>>(value.clone()).ok())
            } else {
                std::fs::read(&baseline_path)
                    .ok()
                    .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                    .and_then(|baseline| baseline.get("events").cloned())
                    .and_then(|value| serde_json::from_value::<Vec<woodshed_core::history::PracticeEvent>>(value).ok())
            };
            let Some(baseline_events) = baseline_events else {
                return fail("baseline event history is missing or invalid");
            };
            if events.len() != baseline_events.len() + 2
                || events.get(2..) != Some(baseline_events.as_slice())
                || !events.first().is_some_and(|event| {
                    valid_pattern_audition(event, "exercise:scale-thirds/v1:Minor", "scale:Minor", owner)
                })
                || !events.get(1).is_some_and(|event| {
                    valid_pattern_audition(event, "exercise:scale-thirds/v1:Major", "scale:Major", owner)
                })
            {
                return fail("accepted-only pattern history or exact source attribution differs");
            }
            let persisted_events = catalog_audition_events(&persisted.practice_history);
            let persisted_cards = match serde_json::to_value(&persisted.set.cards) {
                Ok(cards) => cards,
                Err(error) => return fail(&format!("persisted Card encoding failed: {error}")),
            };
            if persisted_events != events || persisted_cards != cards {
                return fail("live and sealed accepted history or Card data differ");
            }
            let report = serde_json::json!({
                "owner": owner,
                "cards": cards,
                "events": events,
                "baseline_events": baseline_events,
                "persisted_events_match": true,
                "output_observation_id": shared.borrow().backend.as_ref().and_then(|backend| backend.output_observation_id()),
                "output_sample_rate": shared.borrow().backend.as_ref().and_then(|backend| backend.output_sample_rate()),
                "preview_starts": shared.borrow().backend.as_ref().and_then(|backend| backend.output_snapshot()).map(|snapshot| snapshot.preview_starts),
            });
            if matches!(receipt, CatalogAuditionReceipt::SaveFinal) {
                write_new_json(&final_path, &report).is_ok()
            } else {
                let expected = expected.expect("verify branch loads its checkpoint");
                if expected.get("owner") != Some(&serde_json::json!(owner))
                    || expected.get("cards") != Some(&cards)
                    || expected.get("events") != report.get("events")
                    || expected.get("baseline_events") != report.get("baseline_events")
                {
                    return fail("freshly reopened accepted history or source data differs");
                }
                let reopened = serde_json::json!({
                    "owner": owner,
                    "cards": cards,
                    "events": events,
                    "output_observation_id": shared.borrow().backend.as_ref().and_then(|backend| backend.output_observation_id()),
                    "output_sample_rate": shared.borrow().backend.as_ref().and_then(|backend| backend.output_sample_rate()),
                    "preview_starts": shared.borrow().backend.as_ref().and_then(|backend| backend.output_snapshot()).map(|snapshot| snapshot.preview_starts),
                    "matches_final_checkpoint": true,
                    "persisted_events_match": true,
                    "persisted_cards_match": true,
                });
                write_new_json(&reopened_path, &reopened).is_ok()
            }
        },
    }
}

fn write_new_json(path: &std::path::Path, value: &serde_json::Value) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(serde_json::to_string_pretty(value).unwrap().as_bytes())
}

pub fn drive(lane: &mut mesquite::Lane<Product>, ctx: &mut Ctx<'_>) {
    {
        let product = lane.product_mut();
        let mut shared = product.shared.borrow_mut();
        note_events(&mut shared, ctx.runner.state());
        product.sheet = shared.accessible_sheet();
    }
    lane.after_frame(ctx);
}

/// The state a scenario asserts against, sampled each time it may have changed.
/// Kept beside the events it produces so a transition is reported once, with
/// what it was and what it became.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Observed {
    pub cards: usize,
    pub cursor_id: u64,
    pub cursor_number: usize,
    pub relations: String,
    pub edges: usize,
}

impl Observed {
    fn read(ui: &UiState) -> Self {
        let graph = ui
            .set
            .graph()
            .with_relations(&ui.app_settings.stage.visible_relations());
        Self {
            cards: ui.set.cards.len(),
            cursor_id: ui.set.cursor_id().map(|id| id.0).unwrap_or_default(),
            cursor_number: if ui.set.cards.is_empty() {
                0
            } else {
                ui.set.cursor.min(ui.set.cards.len() - 1) + 1
            },
            relations: relation_summary(ui),
            edges: graph.edges.len(),
        }
    }
}

fn relation_summary(ui: &UiState) -> String {
    let visible = ui.app_settings.stage.visible_relations();
    if visible.is_empty() {
        return "none".to_string();
    }
    visible
        .iter()
        .map(|kind| kind.label())
        .collect::<Vec<_>>()
        .join(",")
}

/// Sample the observed state and emit an event for anything that moved. Events
/// are the app's own transitions, not the driver's intentions, so a scenario
/// asserting one is asserting that the app really did it.
pub fn note_events(shared: &mut Shared, ui: &UiState) {
    let now = Observed::read(ui);
    let before = std::mem::replace(&mut shared.observed, now.clone());
    if before == now {
        return;
    }
    if before.cards != now.cards {
        shared.events.push(format!("set-size {}", now.cards));
    }
    if before.cursor_id != now.cursor_id || before.cursor_number != now.cursor_number {
        shared.events.push(format!(
            "set-cursor id={} number={}",
            now.cursor_id, now.cursor_number
        ));
    }
    if before.relations != now.relations {
        shared
            .events
            .push(format!("relations-visible {}", now.relations));
    }
    if before.edges != now.edges {
        shared.events.push(format!("graph-edges {}", now.edges));
    }
}

/// The `Automatable` view of woodshed, borrowed for the duration of one tick.
///
/// The host owns the runner, so the application cannot hold a long-lived `&mut`
/// to it; it borrows the hook's context for exactly as long as the driver needs
/// and queues pointer delivery back through the host.
struct Probe<'a, 'c> {
    ctx: &'a mut Ctx<'c>,
    shared: &'a mut Shared,
    clicks: &'a mut mesquite::Clicks,
}

struct Snapshot<'a, 'c> {
    ctx: &'a Ctx<'c>,
    shared: &'a Shared,
}

impl Snapshot<'_, '_> {
    /// The `data-key` of the Set-graph node the canvas is currently painting
    /// as focused, or `"none"`. The canvas owns that emphasis, so the DOM is
    /// the authority for it.
    fn graph_focus_key(&self) -> String {
        use layout_dom_api::LayoutDom;
        let dom = self.ctx.runner.dom();
        let dom = dom.borrow();
        dom.all_with_class(dom.document(), "graph-canvas-swatch-node")
            .into_iter()
            .find(|node| {
                dom.attribute(
                    *node,
                    &layout_dom_api::Namespace::from(""),
                    &layout_dom_api::LocalName::from("class"),
                )
                .is_some_and(|class| class.split_whitespace().any(|token| token == "focused"))
            })
            .and_then(|node| {
                dom.attribute(
                    node,
                    &layout_dom_api::Namespace::from(""),
                    &layout_dom_api::LocalName::from("data-key"),
                )
                .map(str::to_owned)
            })
            .unwrap_or_else(|| "none".to_string())
    }

    fn graph_label_placements(&self) -> (usize, usize) {
        use layout_dom_api::LayoutDom;
        let dom = self.ctx.runner.dom();
        let dom = dom.borrow();
        dom.all_with_class(dom.document(), "graph-canvas-swatch-label")
            .into_iter()
            .fold((0, 0), |(above, side), node| {
                let placement = dom.attribute(
                    node,
                    &layout_dom_api::Namespace::from(""),
                    &layout_dom_api::LocalName::from("data-label-placement"),
                );
                match placement {
                    Some("above") => (above + 1, side),
                    Some("left" | "right") => (above, side + 1),
                    _ => (above, side),
                }
            })
    }

    fn graph_node_card_count(&self) -> usize {
        use layout_dom_api::LayoutDom;
        let dom = self.ctx.runner.dom();
        let dom = dom.borrow();
        dom.all_with_class(dom.document(), "set-graph-selected-card")
            .len()
    }

    fn pointer_capture_class(&self) -> String {
        use layout_dom_api::LayoutDom;
        let Some(node) = self.ctx.runner.pointer_capture() else {
            return "none".to_string();
        };
        let dom = self.ctx.runner.dom();
        let dom = dom.borrow();
        dom.attribute(
            node,
            &layout_dom_api::Namespace::from(""),
            &layout_dom_api::LocalName::from("class"),
        )
        .unwrap_or("unclassed")
        .to_string()
    }
}
impl Snapshot<'_, '_> {
    fn snapshot(&self) -> ProbeSnapshot {
        let ui = self.ctx.runner.state();
        let observed = Observed::read(ui);
        let overview = woodshed_views::stage::overview_snapshot(ui);
        let stage_snapshot = set_graph_snapshot(ui);
        let stage_swatch =
            set_graph_swatch_from_snapshot(&stage_snapshot, ui, ui.set_tray_expanded);
        let routed_lanes = stage_swatch
            .projected_relations()
            .iter()
            .filter(|(_, route)| route.len() == 4)
            .count();
        let (card_incident_relations, card_anchored_relations) = stage_swatch
            .selected
            .as_ref()
            .and_then(|selected| {
                let region = stage_swatch.projected_node_footprint(selected)?;
                let right = region.left + region.width;
                let bottom = region.top + region.height;
                let mut incident = 0;
                let mut anchored = 0;
                for (relation, route) in stage_swatch.projected_relations() {
                    let endpoint = if &relation.from == selected {
                        route.first().copied()
                    } else if &relation.to == selected {
                        route.last().copied()
                    } else {
                        None
                    };
                    let Some(point) = endpoint else {
                        continue;
                    };
                    incident += 1;
                    let on_vertical =
                        (point.0 - region.left).abs() < 0.01 || (point.0 - right).abs() < 0.01;
                    let on_horizontal =
                        (point.1 - region.top).abs() < 0.01 || (point.1 - bottom).abs() < 0.01;
                    if (on_vertical && point.1 >= region.top && point.1 <= bottom)
                        || (on_horizontal && point.0 >= region.left && point.0 <= right)
                    {
                        anchored += 1;
                    }
                }
                Some((incident, anchored))
            })
            .unwrap_or_default();
        let (labels_above, labels_side) = self.graph_label_placements();
        let relation_choices = set_graph_relation_choices(&stage_snapshot, ui);
        let cursor_label = ui
            .set
            .cursor_id()
            .and_then(|id| ui.set.card(id))
            .map(|card| card.label.clone())
            .unwrap_or_default();
        let selected_card = ui.set.cards.get(ui.set.cursor);
        let selected_timing = selected_card.and_then(|card| {
            woodshed_core::resolve_card_timing(&card.timing, ui.transport.bpm).ok()
        });
        let selected_preview = selected_card.and_then(|card| {
            ui.stage.card_sounding_pitches_at_tempo(card, ui.transport.bpm).ok()
        });
        let output_sample_rate = self.shared.backend.as_ref()
            .and_then(|backend| backend.output_sample_rate());
        let reading = ui.relationship_reading().ok().flatten();
        let appearance = ui.appearance.resolve(&ui.app_settings.appearance);
        let font_requests = ui.appearance.choice(&ui.app_settings.appearance).fonts;
        let requested_font = |role, name: &str| {
            font_requests
                .0
                .get(&role)
                .and_then(|stack| stack.first())
                .is_some_and(|family| family.name() == name)
        };
        let mut snap = ProbeSnapshot::default()
            .with_field(
                "appearance-device-free",
                (crate::appearance::device_free_receipt()
                    && self.shared.backend.is_none()
                    && ui.midi.input_ports.is_empty()
                    && ui.midi.output_ports.is_empty())
                .to_string(),
            )
            .with_field("appearance-theme", appearance.theme.id.clone())
            .with_field("appearance-mode", appearance.mode.as_key())
            .with_field("font-request-roles", font_requests.0.len().to_string())
            .with_field(
                "font-label-mono",
                requested_font(tabard::typography::FontRole::Label, "IBM Plex Mono").to_string(),
            )
            .with_field(
                "font-reading-serif",
                requested_font(tabard::typography::FontRole::Reading, "Source Serif 4").to_string(),
            )
            .with_field(
                "font-heading-serif",
                requested_font(tabard::typography::FontRole::Heading, "Source Serif 4").to_string(),
            )
            .with_field(
                "font-local-mono",
                ui.appearance
                    .available_fonts
                    .iter()
                    .any(|name| name == "IBM Plex Mono")
                    .to_string(),
            )
            .with_field(
                "font-local-serif",
                ui.appearance
                    .available_fonts
                    .iter()
                    .any(|name| name == "Source Serif 4")
                    .to_string(),
            )
            .with_field(
                "font-fallback-notice",
                (!woodshed_views::appearance::fonts::diagnostics(
                    &appearance.theme,
                    &font_requests,
                    &ui.appearance.available_fonts,
                )
                .is_empty())
                .to_string(),
            )
            .with_field(
                "appearance-authoring",
                ui.appearance_authoring_available.to_string(),
            )
            .with_field(
                "appearance-workshop",
                ui.appearance.workshop_open.to_string(),
            )
            .with_field(
                "appearance-dirty",
                ui.appearance.workshop.has_changes().to_string(),
            )
            .with_field(
                "appearance-name",
                ui.appearance.workshop.draft_theme().name.clone(),
            )
            .with_field("recipe-valid", reading.is_some().to_string())
            .with_field(
                "recipe-label",
                reading
                    .as_ref()
                    .map(|r| r.snapshot.recipe.definition.label.as_str())
                    .unwrap_or(""),
            )
            .with_field(
                "recipe-spacing",
                reading
                    .as_ref()
                    .map(|r| r.snapshot.recipe.definition.arrangement.spacing)
                    .unwrap_or(0)
                    .to_string(),
            )
            .with_field(
                "recipe-occurrence",
                reading
                    .as_ref()
                    .and_then(|r| r.snapshot.selected_occurrence.as_deref())
                    .unwrap_or(""),
            )
            .with_field(
                "recipe-relationship",
                reading
                    .as_ref()
                    .and_then(|r| r.snapshot.selected_relationship.as_deref())
                    .unwrap_or(""),
            )
            .with_field(
                "recipe-owner",
                reading
                    .as_ref()
                    .and_then(|r| r.source.as_ref())
                    .map(|s| s.owner.0)
                    .unwrap_or(0)
                    .to_string(),
            )
            .with_field(
                "pattern-sequential",
                (ui.set.cards.get(ui.set.cursor).is_some_and(|card| {
                    matches!(
                        card.material,
                        woodshedding::rehearsal::Material::ScalePattern { .. }
                    )
                }) && ui.preview_voicing().is_ok_and(|preview| preview.2 > 0.0))
                    .to_string(),
            )
            .with_field(
                "approach-cards",
                ui.set
                    .cards
                    .iter()
                    .filter(|card| {
                        matches!(
                            card.material,
                            woodshedding::rehearsal::Material::ChordApproach { .. }
                        )
                    })
                    .count()
                    .to_string(),
            )
            .with_field("retained-sets", ui.retained_sets.entries.len().to_string())
            .with_field(
                "working-sets",
                (ui.working_sets.inactive.len() + 1).to_string(),
            )
            .with_field("working-set-id", ui.working_sets.active_id.0.to_string())
            .with_field(
                "explorations",
                (ui.catalog_explorations.inactive.len() + 1).to_string(),
            )
            .with_field(
                "exploration-id",
                ui.catalog_explorations.active_id.0.to_string(),
            )
            .with_field("catalog-root", ui.stage.root_idx.to_string())
            .with_field("catalog-lens", format!("{:?}", ui.stage.lens))
            .with_field("catalog-tuning", ui.stage.tuning().name.clone())
            .with_field("catalog-search", ui.search.text())
            .with_field(
                "runner-set-id",
                ui.rehearsal_owner.map_or(0, |id| id.0).to_string(),
            )
            .with_field("runner-cursor", ui.rehearsal_set().cursor.to_string())
            .with_field(
                "runner-foreground",
                ui.is_current_set_rehearsing().to_string(),
            )
            .with_field("overview-folds", ui.overview_folds.records.len().to_string())
            .with_field("overview-fold-selection", ui.overview_folds.selected.len().to_string())
            .with_field("overview-visible-nodes", woodshed_views::stage::overview_swatch(ui).graph.nodes.len().to_string())
            .with_field("overview-zoom", format!("{:.2}", ui.overview_viewport.zoom))
            .with_field(
                "overview-background",
                ui.overview_background.len().to_string(),
            )
            .with_field("overview-atmosphere", ui.overview_atmosphere.kind.as_str())
            .with_field(
                "overview-atmosphere-seed",
                ui.overview_atmosphere.seed.to_string(),
            )
            .with_field(
                "overview-atmosphere-revision",
                ui.overview_ambient.revision().to_string(),
            )
            .with_field(
                "overview-movement-controls",
                ui.overview_movement_controls.to_string(),
            )
            .with_field("overview-motion", ui.overview_motion.to_string())
            .with_field(
                "overview-reduced-motion",
                ui.overview_reduced_motion.to_string(),
            )
            .with_field(
                "overview-pinned",
                ui.overview_roles
                    .values()
                    .filter(|role| role.as_str() == "pinned")
                    .count()
                    .to_string(),
            )
            .with_field(
                "preview-busy",
                self.shared
                    .backend
                    .as_ref()
                    .is_some_and(|backend| backend.preview_busy())
                    .to_string(),
            )
            .with_field(
                "preview-voices",
                self.shared
                    .backend
                    .as_ref()
                    .map_or(0, |backend| backend.preview_voice_count())
                    .to_string(),
            )
            .with_field(
                "output-streams",
                self.shared
                    .backend
                    .as_ref()
                    .map_or(0, |backend| backend.output_stream_count())
                    .to_string(),
            )
            .with_field(
                "output-device-frames",
                self.shared
                    .backend
                    .as_ref()
                    .and_then(|backend| backend.output_snapshot())
                    .map_or(0, |snapshot| snapshot.device_frames)
                    .to_string(),
            )
            .with_field(
                "output-processing-blocks",
                self.shared
                    .backend
                    .as_ref()
                    .and_then(|backend| backend.output_snapshot())
                    .map_or(0, |snapshot| snapshot.processing_blocks)
                    .to_string(),
            )
            .with_field(
                "output-capture-invalid",
                self.shared
                    .backend
                    .as_ref()
                    .and_then(|backend| backend.output_snapshot())
                    .is_some_and(|snapshot| snapshot.capture_invalid)
                    .to_string(),
            )
            .with_field(
                "output-click-playing",
                self.shared
                    .backend
                    .as_ref()
                    .and_then(|backend| backend.output_snapshot())
                    .is_some_and(|snapshot| snapshot.click_playing)
                    .to_string(),
            )
            .with_field(
                "capture-input-rate",
                self.shared
                    .backend
                    .as_ref()
                    .and_then(|backend| backend.capture_source_rate())
                    .map_or_else(|| "none".into(), |rate| rate.to_string()),
            )
            .with_field(
                "loop-capture-references",
                ui.loop_captures.takes.len().to_string(),
            )
            .with_field(
                "loop-capture-frames",
                ui.loop_captures
                    .takes
                    .iter()
                    .fold(0usize, |total, take| total.saturating_add(take.frames))
                    .to_string(),
            )
            .with_field(
                "loop-active-captures",
                ui.song_loop_bars
                    .iter()
                    .filter(|present| **present)
                    .count()
                    .to_string(),
            )
            .with_field(
                "recorder-rows",
                woodshed_views::stage::recorder_projection(ui)
                    .lanes
                    .len()
                    .to_string(),
            )
            .with_field(
                "recorder-waveforms",
                ui.recorder_observation
                    .buffers
                    .iter()
                    .flatten()
                    .filter(|buffer| !buffer.samples.is_empty())
                    .count()
                    .to_string(),
            )
            .with_field(
                "recorder-admitted",
                ui.recorder_observation.source.is_some().to_string(),
            )
            .with_field(
                "recorder-selected-card",
                ui.loop_selected.map_or(0, |id| id.card.0).to_string(),
            )
            .with_field(
                "recorder-capture-busy",
                ui.recorder_observation.capture_busy.to_string(),
            )
            .with_field(
                "recorder-inherited-bpm",
                ui.looper.inherited_bpm.to_string(),
            )
            .with_field("song-bars", ui.song.bars.len().to_string())
            .with_field("song-playing", ui.song_playing.to_string())
            .with_field("loop-export-busy", ui.loop_export_busy.to_string())
            .with_field(
                "loop-export-replace",
                ui.loop_export_replace.is_some().to_string(),
            )
            .with_field(
                "loop-export-notice",
                ui.loop_export_notice.clone().unwrap_or_default(),
            )
            .with_field("song-live-bar", ui.song_bar_live.to_string())
            .with_field(
                "song-first-bpm",
                ui.song.bars.first().map_or(0.0, |bar| bar.bpm).to_string(),
            )
            .with_field(
                "song-first-root",
                ui.song
                    .bars
                    .first()
                    .map_or(0, |bar| bar.root_pc)
                    .to_string(),
            )
            .with_field(
                "overview-context-recipes",
                ui.musical_context
                    .items()
                    .iter()
                    .filter(|item| item.captured_recipe.is_some())
                    .count()
                    .to_string(),
            )
            .with_field(
                "overview-recipe-source-present",
                ui.musical_context
                    .items()
                    .iter()
                    .filter_map(|item| item.captured_recipe.as_ref())
                    .any(|recipe| {
                        ui.set
                            .cards
                            .iter()
                            .any(|card| card.id == recipe.source_card)
                    })
                    .to_string(),
            )
            .with_field(
                "overview-recipe-bpm",
                ui.musical_context
                    .items()
                    .iter()
                    .find_map(|item| item.captured_recipe.as_ref())
                    .and_then(|recipe| recipe.card().ok())
                    .and_then(|card| card.timing.bpm)
                    .map_or("none".into(), |bpm| bpm.to_string()),
            )
            .with_field(
                "set-last-scale-pattern",
                ui.set
                    .cards
                    .last()
                    .is_some_and(|card| {
                        matches!(
                            card.material,
                            woodshedding::rehearsal::Material::ScalePattern {
                                pattern: woodshedding::rehearsal::ScalePattern::Thirds,
                                ..
                            }
                        ) && matches!(card.touch, woodshedding::rehearsal::Touch::Walk)
                    })
                    .to_string(),
            )
            .with_field(
                "set-last-arpeggio",
                ui.set
                    .cards
                    .last()
                    .is_some_and(|card| {
                        matches!(
                            card.touch,
                            woodshedding::rehearsal::Touch::Arpeggiate {
                                direction: woodshedding::rehearsal::ArpeggioDirection::Down,
                                inversion: 1
                            }
                        )
                    })
                    .to_string(),
            )
            .with_field(
                "overview-context-items",
                ui.musical_context.items().len().to_string(),
            )
            .with_field(
                "overview-context-containment",
                overview
                    .relations
                    .iter()
                    .filter(|relation| {
                        relation.kind
                            == woodshed_core::session_overview::OverviewRelationKind::ContainsTones
                    })
                    .count()
                    .to_string(),
            )
            .with_field(
                "overview-context-links",
                overview
                    .relations
                    .iter()
                    .filter(|relation| {
                        relation.kind
                            == woodshed_core::session_overview::OverviewRelationKind::ContextFor
                    })
                    .count()
                    .to_string(),
            )
            .with_field("overview-nodes", overview.nodes.len().to_string())
            .with_field(
                "overview-takes",
                overview
                    .nodes
                    .iter()
                    .filter(|node| {
                        matches!(
                            node.id,
                            woodshed_core::session_overview::OverviewNodeId::Artifact(
                                woodshed_core::session_overview::SessionArtifactId::LoopTake(_)
                            )
                        )
                    })
                    .count()
                    .to_string(),
            )
            .with_field(
                "overview-take-links",
                overview
                    .relations
                    .iter()
                    .filter(|relation| {
                        relation.kind
                            == woodshed_core::session_overview::OverviewRelationKind::CapturedFor
                    })
                    .count()
                    .to_string(),
            )
            .with_field(
                "loop-source-set",
                match ui.looper.source {
                    woodshed_core::loop_plan::LooperSource::Set(id) => id.0.to_string(),
                    woodshed_core::loop_plan::LooperSource::SavedSong => "saved".into(),
                },
            )
            .with_field(
                "loop-admitted-set",
                ui.loop_admitted
                    .as_ref()
                    .map_or("none".into(), |plan| plan.owner.0.to_string()),
            )
            .with_field(
                "recorder-selected-set",
                ui.loop_selected.map_or(0, |id| id.set.0).to_string(),
            )
            .with_field(
                "overview-views",
                overview
                    .nodes
                    .iter()
                    .filter(|node| {
                        matches!(
                            node.id,
                            woodshed_core::session_overview::OverviewNodeId::View(_)
                        )
                    })
                    .count()
                    .to_string(),
            )
            .with_field(
                "overview-processes",
                overview
                    .nodes
                    .iter()
                    .filter(|node| {
                        matches!(
                            node.id,
                            woodshed_core::session_overview::OverviewNodeId::Process(_)
                        )
                    })
                    .count()
                    .to_string(),
            )
            .with_field(
                "overview-active",
                (ui.workspace.active_panel()
                    == Some(woodshed_views::workspace::WorkspacePanel::Overview)
                    && ui.section == woodshed_core::storage::AppSection::Stage)
                    .to_string(),
            )
            .with_field(
                "working-saved-shared-ids",
                ui.retained_sets
                    .entries
                    .first()
                    .map_or(0, |saved| {
                        ui.set
                            .cards
                            .iter()
                            .filter(|card| {
                                saved
                                    .set
                                    .cards
                                    .iter()
                                    .any(|original| original.id == card.id)
                            })
                            .count()
                    })
                    .to_string(),
            )
            .with_field(
                "approach-sequential",
                (ui.set.cards.get(ui.set.cursor).is_some_and(|card| {
                    matches!(
                        card.material,
                        woodshedding::rehearsal::Material::ChordApproach { .. }
                    )
                }) && ui.preview_voicing().is_ok_and(|preview| preview.2 > 0.0))
                    .to_string(),
            )
            .with_field(
                "approach-descends",
                (ui.set.cards.get(ui.set.cursor).is_some_and(|card| {
                    matches!(
                        card.material,
                        woodshedding::rehearsal::Material::ChordApproach { .. }
                    )
                }) && ui
                    .preview_voicing()
                    .is_ok_and(|preview| preview.0.windows(2).any(|notes| notes[1] < notes[0])))
                .to_string(),
            )
            .with_field(
                "context-approaches",
                stage_snapshot
                    .nodes
                    .iter()
                    .filter(|node| {
                        node.kind == woodshed_core::stage_context::StageNodeKind::ChordApproach
                            && !node.foreground
                    })
                    .count()
                    .to_string(),
            )
            .with_field(
                "rehearsal-strings",
                ui.rehearsal_board_geometry().string_count.to_string(),
            )
            .with_field(
                "context-patterns",
                stage_snapshot
                    .nodes
                    .iter()
                    .filter(|node| {
                        node.kind == woodshed_core::stage_context::StageNodeKind::ScalePattern
                            && !node.foreground
                    })
                    .count()
                    .to_string(),
            )
            .with_field(
                "pattern-cards",
                ui.set
                    .cards
                    .iter()
                    .filter(|card| {
                        matches!(
                            card.material,
                            woodshedding::rehearsal::Material::ScalePattern { .. }
                        )
                    })
                    .count()
                    .to_string(),
            )
            .with_field(
                "pattern-descends",
                ui.set
                    .cards
                    .get(ui.set.cursor)
                    .is_some_and(|card| {
                        matches!(
                            card.material,
                            woodshedding::rehearsal::Material::ScalePattern { .. }
                        ) && ui
                            .preview_voicing()
                            .is_ok_and(|preview| preview.0.windows(2).any(|notes| notes[1] < notes[0]))
                    })
                    .to_string(),
            )
            .with_field("pattern-repeats", {
                let pitches = ui.preview_voicing().map(|preview| preview.0).unwrap_or_default();
                let unique = pitches
                    .iter()
                    .map(|pitch| pitch.to_bits())
                    .collect::<std::collections::BTreeSet<_>>();
                (ui.set.cards.get(ui.set.cursor).is_some_and(|card| {
                    matches!(
                        card.material,
                        woodshedding::rehearsal::Material::ScalePattern { .. }
                    )
                }) && unique.len() < pitches.len())
                .to_string()
            })
            .with_field("set-cards", observed.cards.to_string())
            .with_field("practice-saved", ui.practice_saved.to_string())
            .with_field(
                "preview-starts",
                self.shared
                    .backend
                    .as_ref()
                    .and_then(|backend| backend.output_snapshot())
                    .map_or(0, |snapshot| snapshot.preview_starts)
                    .to_string(),
            )
            .with_field(
                "practice-save-problem",
                ui.practice_save_notice.is_some().to_string(),
            )
            .with_field("rehearsal-running", ui.rehearsal_running.to_string())
            .with_field("rehearsal-pending", ui.rehearsal_pending.to_string())
            .with_field("card-preview-error", ui.preview_voicing().is_err().to_string())
            .with_field("card-timing-notice", ui.card_timing_notice.clone().unwrap_or_default())
            .with_field(
                "p5a-card-label",
                selected_card.map_or("".into(), |card| card.label.clone()),
            )
            .with_field(
                "p5a-card-bpm",
                selected_card
                    .and_then(|card| card.timing.bpm)
                    .map_or("none".into(), |bpm| bpm.to_string()),
            )
            .with_field(
                "p5a-card-hold",
                selected_card.map_or("none".into(), |card| match &card.timing.hold {
                    woodshedding::rehearsal::Hold::Manual => "manual".into(),
                    woodshedding::rehearsal::Hold::Bars(n) => format!("bars:{n}"),
                    woodshedding::rehearsal::Hold::Seconds(seconds) => {
                        format!("seconds:{}", seconds.to_bits())
                    }
                    woodshedding::rehearsal::Hold::Reps(n) => format!("reps:{n}"),
                }),
            )
            .with_field(
                "p5a-core-dwell-ms",
                selected_timing
                    .and_then(|timing| timing.dwell)
                    .map_or("none".into(), |dwell| dwell.as_millis().to_string()),
            )
            .with_field(
                "p5a-ordered-visits",
                selected_preview
                    .as_ref()
                    .map_or(0, |preview| preview.0.len())
                    .to_string(),
            )
            .with_field(
                "p5a-ordered-duration-ms",
                selected_preview.as_ref().map_or("unavailable".into(), |preview| {
                    (preview.1 * 1000.0).round().to_string()
                }),
            )
            .with_field(
                "p5a-ordered-spacing-ms",
                selected_preview
                    .as_ref()
                    .map_or("unavailable".into(), |preview| preview.2.round().to_string()),
            )
            .with_field(
                "p5a-card-timing-notice-empty",
                ui.card_timing_notice.is_none().to_string(),
            )
            .with_field(
                "p5a-output-sample-rate",
                output_sample_rate.map_or("none".into(), |rate| rate.to_string()),
            )
            .with_field("history-events", ui.practice_history.len().to_string())
            .with_field("audition-notice", ui.audition_notice.clone().unwrap_or_default())
            .with_field("audition-notice-empty", ui.audition_notice.is_none().to_string())
            .with_field("catalog-audition-previewed", {
                ui.practice_history
                    .recent(ui.practice_history.len())
                    .iter()
                    .filter(|event| {
                        event.kind == woodshed_core::history::EngagementKind::Previewed
                            && event.subject_id.starts_with("exercise:scale-thirds/v1:")
                    })
                    .count()
                    .to_string()
            })
            .with_field("catalog-audition-last-subject", {
                ui.practice_history
                    .recent(ui.practice_history.len())
                    .iter()
                    .find(|event| {
                        event.kind == woodshed_core::history::EngagementKind::Previewed
                            && event.subject_id.starts_with("exercise:scale-thirds/v1:")
                    })
                    .map_or("".into(), |event| event.subject_id.clone())
            })
            .with_field("catalog-audition-last-from", {
                ui.practice_history
                    .recent(ui.practice_history.len())
                    .iter()
                    .find(|event| {
                        event.kind == woodshed_core::history::EngagementKind::Previewed
                            && event.subject_id.starts_with("exercise:scale-thirds/v1:")
                    })
                    .and_then(|event| event.from_id.clone())
                    .unwrap_or_default()
            })
            .with_field("catalog-audition-output-rate", {
                output_sample_rate.map_or("none".into(), |rate| rate.to_string())
            })
            .with_field(
                "history-observed",
                ui.practice_history
                    .recent(1000)
                    .iter()
                    .filter(|event| event.provenance.is_some())
                    .count()
                    .to_string(),
            )
            .with_field(
                "arpeggio-cards",
                ui.set
                    .cards
                    .iter()
                    .filter(|card| {
                        matches!(
                            card.touch,
                            woodshedding::rehearsal::Touch::Arpeggiate { .. }
                        )
                    })
                    .count()
                    .to_string(),
            )
            .with_field(
                "scale-cards",
                ui.set
                    .cards
                    .iter()
                    .filter(|card| {
                        matches!(
                            card.material,
                            woodshedding::rehearsal::Material::Scale { .. }
                        )
                    })
                    .count()
                    .to_string(),
            )
            .with_field(
                "context-arpeggios",
                stage_snapshot
                    .nodes
                    .iter()
                    .filter(|node| {
                        node.kind == woodshed_core::stage_context::StageNodeKind::Arpeggio
                    })
                    .count()
                    .to_string(),
            )
            .with_field(
                "arpeggio-shape",
                ui.set
                    .cards
                    .iter()
                    .find(|card| {
                        matches!(
                            card.touch,
                            woodshedding::rehearsal::Touch::Arpeggiate { .. }
                        )
                    })
                    .and_then(|card| card.setting.voicing_idx)
                    .map_or_else(|| "none".into(), |index| index.to_string()),
            )
            .with_field("cursor-id", observed.cursor_id.to_string())
            .with_field("cursor-number", observed.cursor_number.to_string())
            .with_field("cursor-label", cursor_label.clone())
            .with_field("graph-nodes", ui.set.graph().nodes.len().to_string())
            .with_field("graph-total-nodes", stage_snapshot.nodes.len().to_string())
            .with_field(
                "graph-context-nodes",
                stage_snapshot
                    .nodes
                    .iter()
                    .filter(|node| !node.foreground)
                    .count()
                    .to_string(),
            )
            .with_field(
                "graph-reading",
                format!("{:?}", ui.app_settings.stage.set_graph_reading),
            )
            .with_field(
                "graph-context-focus",
                ui.context_focus
                    .as_ref()
                    .map(|focus| focus.wire_key())
                    .unwrap_or_default(),
            )
            .with_field("graph-edges", observed.edges.to_string())
            .with_field("graph-relations", stage_swatch.relations.len().to_string())
            .with_field(
                "graph-arrangement",
                ui.app_settings.stage.set_arrangement.label(),
            )
            .with_field("graph-routed-lanes", routed_lanes.to_string())
            .with_field("graph-labels-above", labels_above.to_string())
            .with_field("graph-labels-side", labels_side.to_string())
            .with_field("graph-width", stage_swatch.width.to_string())
            .with_field("graph-height", stage_swatch.height.to_string())
            .with_field("graph-node-cards", self.graph_node_card_count().to_string())
            .with_field(
                "graph-card-incident-relations",
                card_incident_relations.to_string(),
            )
            .with_field(
                "graph-card-anchored-relations",
                card_anchored_relations.to_string(),
            )
            .with_field("pointer-capture", self.pointer_capture_class())
            .with_field(
                "graph-visible-relations",
                relation_choices
                    .iter()
                    .filter(|relation| relation.visible)
                    .count()
                    .to_string(),
            )
            .with_field("graph-relation-choices", relation_choices.len().to_string())
            .with_field("graph-epoch", stage_snapshot.epoch().0.to_string())
            .with_field("graph-drag-active", ui.set_graph_drag_active.to_string())
            .with_field(
                "graph-selected-relation",
                ui.set_graph_relation.is_some().to_string(),
            )
            .with_field(
                "graph-moved-nodes",
                ui.set_graph_positions.len().to_string(),
            )
            .with_field(
                "view-only-dispatches",
                self.shared.view_only_dispatches.to_string(),
            )
            .with_field(
                "full-dispatch-syncs",
                self.shared.full_dispatch_syncs.to_string(),
            )
            .with_field(
                "drag-present-samples",
                self.shared.drag_frame_metrics.samples.to_string(),
            )
            .with_field(
                "drag-present-average-us",
                self.shared.drag_frame_metrics.average_us().to_string(),
            )
            .with_field(
                "drag-present-max-us",
                self.shared.drag_frame_metrics.max_us.to_string(),
            )
            .with_field(
                "drag-viewport-us",
                self.shared.drag_frame_metrics.viewport_us.to_string(),
            )
            .with_field(
                "drag-drive-us",
                self.shared.drag_frame_metrics.drive_us.to_string(),
            )
            .with_field(
                "drag-leaves-us",
                self.shared.drag_frame_metrics.leaves_us.to_string(),
            )
            .with_field(
                "drag-root-rebuilds",
                self.shared.drag_frame_metrics.root_rebuilds.to_string(),
            )
            .with_field(
                "drag-host-samples",
                self.shared.drag_frame_metrics.host_samples.to_string(),
            )
            .with_field(
                "drag-host-average-us",
                self.shared.drag_frame_metrics.host_average_us().to_string(),
            )
            .with_field(
                "drag-host-max-us",
                self.shared.drag_frame_metrics.host_max_us.to_string(),
            )
            .with_field(
                "drag-host-relayout-us",
                self.shared.drag_frame_metrics.host_relayout_us.to_string(),
            )
            .with_field(
                "drag-host-layout-update-us",
                self.shared
                    .drag_frame_metrics
                    .host_layout_update_us
                    .to_string(),
            )
            .with_field(
                "drag-host-layout-apply-us",
                self.shared
                    .drag_frame_metrics
                    .host_layout_apply_us
                    .to_string(),
            )
            .with_field(
                "drag-host-layout-mutations",
                self.shared
                    .drag_frame_metrics
                    .host_layout_mutations
                    .to_string(),
            )
            .with_field(
                "drag-host-layout-rebuilds",
                self.shared
                    .drag_frame_metrics
                    .host_layout_rebuilds
                    .to_string(),
            )
            .with_field(
                "drag-host-leaf-boxes-us",
                self.shared
                    .drag_frame_metrics
                    .host_leaf_boxes_us
                    .to_string(),
            )
            .with_field(
                "drag-host-leaf-render-us",
                self.shared
                    .drag_frame_metrics
                    .host_leaf_render_us
                    .to_string(),
            )
            .with_field(
                "drag-host-leaf-repaints",
                self.shared
                    .drag_frame_metrics
                    .host_leaf_repaints
                    .to_string(),
            )
            .with_field(
                "drag-host-fragments-us",
                self.shared.drag_frame_metrics.host_fragments_us.to_string(),
            )
            .with_field(
                "drag-host-emit-us",
                self.shared.drag_frame_metrics.host_emit_us.to_string(),
            )
            .with_field(
                "drag-host-raster-us",
                self.shared.drag_frame_metrics.host_raster_us.to_string(),
            )
            .with_field(
                "drag-host-acquire-us",
                self.shared.drag_frame_metrics.host_acquire_us.to_string(),
            )
            .with_field(
                "drag-host-clear-us",
                self.shared.drag_frame_metrics.host_clear_us.to_string(),
            )
            .with_field(
                "drag-host-compose-us",
                self.shared.drag_frame_metrics.host_compose_us.to_string(),
            )
            .with_field(
                "drag-host-present-us",
                self.shared.drag_frame_metrics.host_present_us.to_string(),
            )
            .with_field(
                "drag-host-a11y-us",
                self.shared.drag_frame_metrics.host_a11y_us.to_string(),
            )
            .with_field(
                "drag-raster-inner-us",
                self.shared.drag_frame_metrics.raster_inner_us.to_string(),
            )
            .with_field(
                "drag-raster-invalidate-us",
                self.shared
                    .drag_frame_metrics
                    .tile_invalidate_us
                    .to_string(),
            )
            .with_field(
                "drag-raster-rebuild-us",
                self.shared
                    .drag_frame_metrics
                    .dirty_tile_rebuild_us
                    .to_string(),
            )
            .with_field(
                "drag-raster-master-us",
                self.shared.drag_frame_metrics.master_compose_us.to_string(),
            )
            .with_field(
                "drag-raster-vello-us",
                self.shared.drag_frame_metrics.vello_render_us.to_string(),
            )
            .with_field(
                "drag-dirty-tiles",
                self.shared.drag_frame_metrics.dirty_tiles.to_string(),
            )
            .with_field(
                "drag-max-dirty-tiles",
                self.shared.drag_frame_metrics.max_dirty_tiles.to_string(),
            )
            .with_field("relations", observed.relations)
            .with_field("tray-expanded", ui.set_tray_expanded.to_string())
            .with_field("card-expanded", ui.set_graph_card_expanded.to_string())
            .with_field(
                "selected-card-board-visible",
                ui.selected_card_board_visible().to_string(),
            )
            // Graph focus emphasis belongs to the canvas component now, so it
            // is observed where it actually is — the focused node's rendered
            // `data-key` — rather than from a UiState field that would report
            // "none" forever.
            .with_field("graph-focus", self.graph_focus_key())
            .with_field("arrangement-producer", self.ctx.producers.contains(woodshed_views::stage::arrangement_editor::PRIMARY_KEY).to_string())
            .with_field("arrangement-related-producer", self.ctx.producers.contains(woodshed_views::stage::arrangement_editor::RELATED_KEY).to_string())
            .with_field("arrangement-saved",ui.set.progression_arrangement.is_some().to_string())
            .with_field("arrangement-dirty",ui.arrangement_editor.dirty().to_string())
            .with_field("arrangement-primary",ui.arrangement_editor.surfaces.borrow().primary.is_some().to_string())
            .with_field("arrangement-related",ui.arrangement_editor.surfaces.borrow().related.is_some().to_string())
            .with_field("arrangement-selection",ui.arrangement_editor.surfaces.borrow().primary.as_ref().and_then(|s|s.selected()).unwrap_or("").to_string())
            .with_field("arrangement-error",ui.arrangement_editor.surfaces.borrow().error.clone().unwrap_or_default())
            .with_field("section", ui.section.label().to_string())
            .with_field(
                "shape-index",
                ui.set
                    .cards
                    .get(ui.set.cursor)
                    .and_then(|card| card.setting.voicing_idx)
                    .map_or_else(|| "none".to_string(), |index| index.to_string()),
            )
            .with_field(
                "shape-dots",
                ui.set
                    .cards
                    .get(ui.set.cursor)
                    .map_or(0, |card| ui.stage.dots_for_card(card).len())
                    .to_string(),
            )
            .with_field("lens", format!("{:?}", ui.stage.lens))
            .with_field("material", ui.stage.material_name());
        let mut personal_ids = ui
            .personal_relations
            .as_ref()
            .into_iter()
            .flat_map(|source| source.retained_assertions())
            .filter_map(|record| serde_json::to_string(&record.id).ok())
            .collect::<Vec<_>>();
        personal_ids.sort();
        let retained_count = personal_ids.len();
        let held_count = ui
            .personal_relations
            .as_ref()
            .map_or(0, |source| source.held_assertions().count());
        let withdrawn_count = retained_count.saturating_sub(held_count);
        let personal_source_id = ui
            .personal_relations
            .as_ref()
            .map(|source| source.source_id().as_str().to_owned())
            .unwrap_or_else(|| "unavailable".into());
        let sealed_actor = ui.seal.as_ref().is_some_and(|seal| {
            matches!(
                seal,
                woodshed_views::persona::PracticeSeal::Sealed { persona, .. }
                    if !persona.trim().is_empty() && persona == persona.trim()
            )
        });
        let claims_pair = ui
            .personal_link_pair
            .as_ref()
            .map(|(from, to)| format!("{from} → {to}"))
            .unwrap_or_else(|| "none".into());
        let generated_snapshot = ui.personal_link_pair.as_ref().map_or_else(
            woodshed_core::mere::WoodshedMereSnapshot::default,
            |(from, _)| woodshed_views::stage::opened_claims_mere_snapshot(ui, from),
        );
        let mut generated_by_id =
            std::collections::BTreeMap::<String, &woodshed_core::mere::MaterialRelation>::new();
        if let Some((from, to)) = ui.personal_link_pair.as_ref() {
            for relation in generated_snapshot.relations.iter().filter(|relation| {
                relation.source.as_str() == from.as_str() && relation.target.as_str() == to.as_str()
            }) {
                generated_by_id
                    .entry(relation.claim_id.qualified_key())
                    .or_insert(relation);
            }
        }
        let generated_methods = generated_by_id
            .values()
            .map(|relation| relation.provenance.method.iri)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join(",");
        let generated_directions = generated_by_id
            .values()
            .map(|relation| {
                format!(
                    "{} → {}",
                    relation.provenance.original_from, relation.provenance.original_to
                )
            })
            .collect::<Vec<_>>()
            .join(";");
        let generated_evidence_count = generated_by_id
            .values()
            .map(|relation| relation.provenance.evidence.len())
            .sum::<usize>();
        snap = snap
            .with_field(
                "personal-source-status",
                if ui.personal_relations.is_some() {
                    "available"
                } else {
                    "unavailable"
                },
            )
            .with_field("personal-source-id", personal_source_id)
            .with_field(
                "personal-write-available",
                (ui.practice_saved && sealed_actor && ui.personal_relations.is_some()).to_string(),
            )
            .with_field("personal-retained-count", retained_count.to_string())
            .with_field("personal-held-count", held_count.to_string())
            .with_field("personal-withdrawn-count", withdrawn_count.to_string())
            .with_field("personal-retained-ids", personal_ids.join(","))
            .with_field("claims-pair", claims_pair)
            .with_field("claims-generated-count", generated_by_id.len().to_string())
            .with_field("claims-generated-methods", generated_methods)
            .with_field("claims-generated-directions", generated_directions)
            .with_field(
                "claims-generated-evidence-count",
                generated_evidence_count.to_string(),
            )
            .with_field(
                "claims-last-action",
                ui.personal_relation_notice.clone().unwrap_or_default(),
            );
        // The Related frontier, observed as the app computes it: the top
        // neighbor and how many distinct relations connect it. A scenario can
        // then assert that multiplicity survived ranking, rather than sniffing
        // for a substring in the DOM.
        let related = ui.stage.related_material_configured(
            &ui.practice_history,
            &ui.app_settings.stage.related,
            8,
        );
        if let Some(top) = related.first() {
            snap = snap
                .with_field("related-top", top.title.clone())
                .with_field("related-top-relations", top.relation_count().to_string())
                .with_field(
                    "related-top-kinds",
                    top.relations
                        .iter()
                        .map(|relation| relation.kind.label())
                        .collect::<Vec<_>>()
                        .join(","),
                )
                .with_field(
                    "related-top-authorities",
                    top.relations
                        .iter()
                        .map(|relation| format!("{:?}", relation.authority))
                        .collect::<Vec<_>>()
                        .join(","),
                );
        }
        snap = snap
            .with_field("related-count", related.len().to_string())
            .with_field(
                "related-multi-count",
                related
                    .iter()
                    .filter(|neighbor| neighbor.relation_count() > 1)
                    .count()
                    .to_string(),
            );
        // The richest pair on the frontier: the one carrying the most relations
        // at once. A single top row can honestly have one reason (an arpeggio
        // realizes its chord, and that is all it does), so multiplicity is
        // asserted where it actually lives.
        if let Some(richest) = related
            .iter()
            .max_by_key(|neighbor| (neighbor.relation_count(), neighbor.score))
        {
            snap = snap
                .with_field("related-richest", richest.title.clone())
                .with_field(
                    "related-richest-relations",
                    richest.relation_count().to_string(),
                )
                .with_field(
                    "related-richest-kinds",
                    richest
                        .relations
                        .iter()
                        .map(|relation| relation.kind.label())
                        .collect::<Vec<_>>()
                        .join(","),
                )
                .with_field(
                    "related-richest-authorities",
                    richest
                        .relations
                        .iter()
                        .map(|relation| format!("{:?}", relation.authority))
                        .collect::<Vec<_>>()
                        .join(","),
                );
        }
        // Graphshell stack lift P2: each graph surface's camera, and pictograph's
        // face measurement of hit targets against painted nodes.
        snap = snap
            .with_field("related-zoom", format!("{:.2}", ui.related_viewport.zoom))
            .with_field(
                "relationship-zoom",
                format!("{:.2}", ui.relationship_viewport.zoom),
            );
        for (surface, prefix) in [
            (CanvasSurface::Overview, "overview"),
            (CanvasSurface::Related, "related"),
            (CanvasSurface::Relationship, "relationship"),
        ] {
            let live = match surface {
                CanvasSurface::Overview => ui.overview_viewport,
                CanvasSurface::Related => ui.related_viewport,
                CanvasSurface::Relationship => ui.relationship_viewport,
            };
            let painted = self.shared.painted_cameras[surface as usize];
            snap = snap
                .with_field(format!("{prefix}-paint-zoom"), format!("{:.2}", painted.2))
                .with_field(
                    format!("{prefix}-paint-current"),
                    (painted == (live.pan.0, live.pan.1, live.zoom)).to_string(),
                );
            if let Some(faces) = canvas_camera::measure_surface_faces(ui, surface) {
                for (name, value) in faces.fields() {
                    snap = snap.with_field(format!("{prefix}-{name}"), value);
                }
            }
        }
        if !cursor_label.is_empty() {
            snap = snap.with_focus(cursor_label);
        }
        snap
    }
}

impl Probe<'_, '_> {
    fn claims_seed_facts_path() -> Option<std::path::PathBuf> {
        std::env::var_os("WOODSHED_CAPTURE_DIR")
            .map(std::path::PathBuf::from)
            .map(|directory| directory.join("claims-seed-identities.json"))
    }

    fn claims_identity_facts(ui: &UiState) -> Option<serde_json::Value> {
        let source = ui.personal_relations.as_ref()?;
        let mut ids = source
            .retained_assertions()
            .filter_map(|record| serde_json::to_string(&record.id).ok())
            .collect::<Vec<_>>();
        ids.sort();
        Some(serde_json::json!({
            "source_id": source.source_id().as_str(),
            "assertion_ids": ids,
        }))
    }

    fn stage_node_key(&self, index: usize) -> Option<String> {
        let ui = self.ctx.runner.state();
        let snapshot = set_graph_snapshot(ui);
        set_graph_swatch_from_snapshot(&snapshot, ui, ui.set_tray_expanded)
            .graph
            .nodes
            .get(index)
            .and_then(|node| node.key.clone())
    }

    fn stage_relation_id(&self, index: usize) -> Option<String> {
        let ui = self.ctx.runner.state();
        let snapshot = set_graph_snapshot(ui);
        set_graph_swatch_from_snapshot(&snapshot, ui, ui.set_tray_expanded)
            .relations
            .get(index)
            .map(|relation| relation.id.clone())
    }
}

impl Automatable for Probe<'_, '_> {
    fn click_target(&mut self, selector: &Selector) -> Option<bool> {
        Some(self.clicks.click(self.ctx, selector, |_, _, r| {
            (r[0] + r[2] * 0.5, r[1] + r[3] * 0.5)
        }))
    }

    fn selector_target(&self, selector: &Selector) -> taproot::SelectorTarget {
        let candidates = {
            let dom = self.ctx.runner.dom();
            let dom = dom.borrow();
            taproot::matching(&dom, selector)
        };
        let (width, height) = self.ctx.logical_size;
        for node in candidates {
            let Some((x, y, w, h)) = self.ctx.painted_rect(node) else {
                continue;
            };
            if ![x, y, w, h].iter().all(|value| value.is_finite()) || w <= 0.0 || h <= 0.0 {
                continue;
            }
            let point = (x + w / 2.0, y + h / 2.0);
            if point.0 >= 0.0 && point.1 >= 0.0 && point.0 < width && point.1 < height {
                return taproot::SelectorTarget::Hit(taproot::Hit {
                    surface: "woodshed",
                    point,
                });
            }
        }
        // Participating hosts own both hits and misses. Never re-layout after
        // a missing, hidden or offscreen retained target.
        taproot::SelectorTarget::Miss
    }

    fn with_surfaces<R>(&self, f: impl FnOnce(&[ProbeSurface<'_>]) -> R) -> R {
        let dom = self.ctx.runner.dom();
        let dom_ref = dom.borrow();
        let (w, h) = self.ctx.logical_size;
        f(&[ProbeSurface {
            name: "woodshed",
            dom: &dom_ref,
            // One runner covers the window, and the probe resolves in the same
            // logical coordinates the layout and the cursor use.
            rect: [0.0, 0.0, w, h],
            sheet: &self.shared.accessible_sheet(),
        }])
    }

    fn snapshot(&self) -> ProbeSnapshot {
        Snapshot {
            ctx: self.ctx,
            shared: self.shared,
        }
        .snapshot()
    }

    fn drain_events(&mut self) -> Vec<String> {
        std::mem::take(&mut self.shared.events)
    }

    fn act(&mut self, label: &str) -> bool {
        if label == "claims-remember-set" {
            let Ok(fingerprint) = serde_json::to_string(&self.ctx.runner.state().set) else {
                return false;
            };
            self.shared.scenario_claims_set_before_audition = Some(fingerprint);
            self.shared
                .events
                .push("claims-set-baseline-recorded".into());
            return true;
        }
        if label == "claims-assert-set-unchanged" {
            let Ok(current) = serde_json::to_string(&self.ctx.runner.state().set) else {
                return false;
            };
            let unchanged = self
                .shared
                .scenario_claims_set_before_audition
                .as_deref()
                .is_some_and(|before| before == current);
            self.shared
                .events
                .push(format!("claims-set-unchanged {unchanged}"));
            return true;
        }
        if label == "claims-save-seed-identities" {
            let Some(path) = Self::claims_seed_facts_path() else {
                return false;
            };
            let Some(facts) = Self::claims_identity_facts(self.ctx.runner.state()) else {
                return false;
            };
            let Ok(encoded) = serde_json::to_vec_pretty(&facts) else {
                return false;
            };
            if std::fs::write(path, encoded).is_err() {
                return false;
            }
            self.shared
                .events
                .push("claims-seed-identities-saved".into());
            return true;
        }
        if label == "claims-verify-reopen-identities" {
            let Some(path) = Self::claims_seed_facts_path() else {
                return false;
            };
            let Ok(seed_bytes) = std::fs::read(path) else {
                return false;
            };
            let Ok(seed) = serde_json::from_slice::<serde_json::Value>(&seed_bytes) else {
                return false;
            };
            let Some(current) = Self::claims_identity_facts(self.ctx.runner.state()) else {
                return false;
            };
            self.shared.events.push(format!(
                "claims-reopen-identities-match {}",
                seed == current
            ));
            return true;
        }
        if label == "seed-loop-capture" || label == "seed-selected-loop-capture" {
            use woodshed_core::audio::AudioBackend;
            let Some(plan) = self.ctx.runner.state().loop_admitted.clone() else {
                return false;
            };
            let selected = self.ctx.runner.state().loop_selected;
            let segment = if label == "seed-selected-loop-capture" {
                selected.and_then(|id| plan.segments.iter().find(|segment| segment.id == id))
            } else {
                plan.segments.first()
            };
            let Some(segment) = segment else { return false; };
            let Some(backend) = self.shared.backend.as_mut() else {
                return false;
            };
            let Some(rate) = backend.output_sample_rate() else {
                return false;
            };
            if backend.loop_capture_busy() {
                return false;
            }
            // Only the self-drive fixture supplies these exact samples. The
            // native flow qualifies storage/replay without requesting a mic.
            let frames = woodshed_audio::Bar {
                bpm: segment.bpm,
                time_signature: woodshed_audio::TimeSignature::new(segment.beats, 4),
                length: segment.bars,
                ..Default::default()
            }
            .duration_samples(rate as f32) as usize;
            let samples = (0..frames)
                .map(|frame| [0.000125, -0.00025, -0.0, 0.0005][frame % 4])
                .collect();
            let mut takes = backend.capture_snapshot();
            takes.retain(|take| take.id != segment.id);
            takes.push(crate::audio::CaptureSnapshot {
                legacy_clock: false,
                id: segment.id,
                bpm: segment.bpm,
                beats: segment.beats,
                bars: segment.bars,
                buffer: woodshed_audio::SampleBuffer::new(samples, rate),
            });
            backend.clear_loop_session();
            if let Err(error) = backend
                .restore_loop_captures(takes)
                .and_then(|_| backend.set_loop_plan(&plan))
            {
                eprintln!("capture fixture refused: {error}");
                return false;
            }
            crate::sync::after_dispatch(self.shared, self.ctx);
            return true;
        }
        if label == "sample-output-clock" {
            let Some(backend) = self.shared.backend.as_ref() else {
                return false;
            };
            let Some(snapshot) = backend.output_snapshot() else {
                return false;
            };
            let observation = format!(
                "output-clock streams={} frames={} blocks={} click={} playing={} capture-invalid={} bar={} in-bar={}",
                backend.output_stream_count(),
                snapshot.device_frames,
                snapshot.processing_blocks,
                snapshot.click_playing,
                snapshot.playing,
                snapshot.capture_invalid,
                snapshot.cursor.bar_idx,
                snapshot.cursor.sample_in_bar,
            );
            eprintln!("{observation}");
            self.shared.events.push(observation);
            return true;
        }
        if label == "reset-dispatch-sync-counts" {
            self.shared.view_only_dispatches = 0;
            self.shared.full_dispatch_syncs = 0;
            self.shared.drag_frame_metrics.reset();
            self.shared.scenario_drag_origin = None;
            return true;
        }
        let mut known = true;
        self.ctx.runner.update(|ui| match label {
            "song-cache-example" => {
                ui.stop_rehearsal();
                ui.song = woodshed_core::song::SongDoc {
                    name: "Chord cache qualification".into(),
                    click: false,
                    bars: [0, 5, 7, 0]
                        .into_iter()
                        .enumerate()
                        .map(|(i, root)| {
                            let mut bar = woodshed_core::song::SongBar {
                                root_pc: root,
                                bpm: 240.0,
                                label: format!("Bar {}", i + 1),
                                ..Default::default()
                            };
                            bar.revoice();
                            bar
                        })
                        .collect(),
                    ..Default::default()
                };
                ui.song_playing = false;
                ui.song_bar_live = 0;
                ui.song_edit_cursor = 0;
                ui.select_app_section(woodshed_core::storage::AppSection::Looper);
            },
            "tone-relationships-example" => {
                use woodshed_core::harmony::KeyedCatalogRef;
                use woodshedding::pitch::PitchClass;
                ui.stop_rehearsal();
                ui.set = Default::default();
                ui.working_sets = Default::default();
                ui.relationship_reading_json = None;
                for (label, formula, root) in [
                    ("Cmaj7", "chord:Major 7", 0),
                    ("Am7", "chord:Minor 7", 9),
                    ("C Major scale", "scale:Major", 0),
                ] {
                    let mut card = KeyedCatalogRef {
                        formula_id: formula.into(),
                        root: PitchClass::new(root),
                    }
                    .to_card()
                    .unwrap();
                    card.label = label.into();
                    ui.set.push(card);
                }
                ui.activate_workspace_panel(woodshed_views::workspace::WorkspacePanel::Overview);
            },
            "relationship-example" => {
                use woodshed_core::harmony::KeyedCatalogRef;
                use woodshedding::pitch::PitchClass;
                ui.stop_rehearsal();
                ui.set = Default::default();
                ui.working_sets = Default::default();
                ui.relationship_reading_json = None;
                for (label, formula, root) in [
                    ("C Major", "Major", 0),
                    ("C Major again", "Major", 0),
                    ("A Minor", "Minor", 9),
                ] {
                    let mut card = KeyedCatalogRef {
                        formula_id: format!("chord:{formula}"),
                        root: PitchClass::new(root),
                    }
                    .to_card()
                    .unwrap();
                    card.label = label.into();
                    ui.set.push(card);
                }
                ui.activate_workspace_panel(woodshed_views::workspace::WorkspacePanel::Overview);
            },
            "stage-current" => ui.stage_current(None),
            "connected-practice-example" | "connected-scale-setup-example" => {
                use woodshedding::rehearsal::{Hold, Set, Timing};
                ui.set = Set::default();
                ui.practice_history = Default::default();
                ui.stage.set_lens(Lens::Chords);
                for (root, name) in [(3, "Major 7"), (0, "Minor 7")] {
                    ui.stage.set_root(root);
                    ui.root_dd.selected = root;
                    let index = ui
                        .stage
                        .chords()
                        .iter()
                        .position(|formula| formula.name == name)
                        .expect("fixture chord exists");
                    ui.stage.select_chord(index);
                    ui.stage_current(None);
                }
                ui.set.cursor = 0;
                ui.step_card_shape(1);
                for card in &mut ui.set.cards {
                    card.timing = Timing {
                        bpm: Some(120.0),
                        hold: Hold::Seconds(2.0),
                    };
                }
                if label == "connected-scale-setup-example" {
                    for card in &mut ui.set.cards {
                        card.setting.instrument = "Ukulele".into();
                        card.setting.tuning = Some("Standard (high-G)".into());
                        card.setting.capo = Some(2);
                        card.setting.fret_window =
                            Some(woodshedding::rehearsal::FretWindow { start: 2, span: 4 });
                    }
                    ui.step_card_shape(1);
                }
                ui.set_graph_card_expanded = true;
                ui.set_graph_reading(woodshed_core::settings::StageGraphReading::CircleOfFifths);
            },
            "chord-approach-example" | "mere-example" | "mere-arpeggio-example" => {
                use woodshedding::rehearsal::{FretWindow, Hold, Set, Timing};
                ui.stop_rehearsal();
                ui.set = Set::default();
                ui.practice_history = Default::default();
                ui.stage.set_lens(Lens::Chords);
                for (root, name) in [(0, "Minor 7"), (3, "Major 7")] {
                    ui.stage.set_root(root);
                    ui.root_dd.selected = root;
                    let index = ui
                        .stage
                        .chords()
                        .iter()
                        .position(|formula| formula.name == name)
                        .expect("fixture chord exists");
                    ui.stage.select_chord(index);
                    ui.stage_current(None);
                }
                ui.set.cards[0].setting.fret_window = Some(FretWindow { start: 2, span: 6 });
                ui.set.cursor = 0;
                ui.step_card_shape(1);
                let target = &mut ui.set.cards[1];
                target.setting.instrument = "Ukulele".into();
                target.setting.tuning = Some("Standard (high-G)".into());
                target.setting.capo = Some(2);
                target.setting.fret_window = Some(FretWindow { start: 2, span: 12 });
                ui.set.cursor = 1;
                ui.step_card_shape(1);
                for card in &mut ui.set.cards {
                    card.timing = Timing {
                        bpm: None,
                        hold: Hold::Bars(1),
                    };
                }
                ui.transport.bpm = 80.0;
                ui.set.cursor = 0;
                ui.set_graph_card_expanded = true;
                ui.set_graph_reading(woodshed_core::settings::StageGraphReading::CircleOfFifths);
                if label == "mere-example" || label == "mere-arpeggio-example" {
                    ui.retained_sets = Default::default();
                    ui.working_sets = Default::default();
                    ui.catalog_explorations = Default::default();
                    for card in &mut ui.set.cards {
                        card.timing.hold = Hold::Manual;
                    }
                }
                if label == "mere-arpeggio-example" {
                    let source = ui.set.cards[0].id;
                    assert!(ui.inspect_chord_arpeggio(source));
                    let id = ui.stage_chord_arpeggio().expect("fixture arpeggio");
                    let recipe = ui.set.cards.iter_mut().find(|card| card.id == id).unwrap();
                    recipe.touch = woodshedding::rehearsal::Touch::Arpeggiate {
                        direction: woodshedding::rehearsal::ArpeggioDirection::Down,
                        inversion: 1,
                    };
                    ui.set.select_id(id);
                }
            },
            "mere-scale-pattern-example" => {
                use woodshed_core::harmony::KeyedCatalogRef;
                use woodshedding::pitch::PitchClass;
                use woodshedding::rehearsal::{FretWindow, Hold, ScalePattern};
                ui.stop_rehearsal();
                ui.set = Default::default();
                ui.practice_history = Default::default();
                ui.retained_sets = Default::default();
                ui.working_sets = Default::default();
                ui.catalog_explorations = Default::default();
                for (name, root) in [("Major", 0), ("Minor", 9)] {
                    let mut card = KeyedCatalogRef {
                        formula_id: format!("scale:{name}"),
                        root: PitchClass::new(root),
                    }
                    .to_card()
                    .expect("fixture scale");
                    card.setting.fret_window = Some(FretWindow { start: 2, span: 6 });
                    card.timing.hold = Hold::Manual;
                    card.timing.bpm = None;
                    ui.set.push(card);
                }
                ui.set.cursor = 0;
                ui.transport.bpm = 92.0;
                let source = ui.set.cards[0].id;
                assert!(ui.inspect_scale_pattern(source, ScalePattern::Thirds));
                let id = ui.stage_scale_pattern().expect("fixture scale pattern");
                ui.set.select_id(id);
            },
            "arrangement-editor-example" => crate::arrangement::seed_example(ui),
            "catalog-audition-admission-example" => {
                use woodshed_core::harmony::KeyedCatalogRef;
                use woodshedding::pitch::PitchClass;
                use woodshedding::rehearsal::{FretWindow, Hold, Set, Timing};

                ui.stop_rehearsal();
                ui.set = Set::default();
                ui.stage.set_lens(Lens::Scales);
                for (label, name, root, bpm, hold) in [
                    ("Accepted source", "Major", 0, 120.0, Hold::Bars(1)),
                    (
                        "Output refusal source",
                        "Minor",
                        9,
                        400.0,
                        Hold::Seconds(1e10),
                    ),
                ] {
                    let mut card = KeyedCatalogRef {
                        formula_id: format!("scale:{name}"),
                        root: PitchClass::new(root),
                    }
                    .to_card()
                    .expect("fixture scale exists");
                    card.label = label.into();
                    card.setting.fret_window = Some(FretWindow { start: 0, span: 8 });
                    card.timing = Timing {
                        bpm: Some(bpm),
                        hold,
                    };
                    ui.set.push(card);
                }
                ui.set.cursor = 0;
                ui.set_tray_expanded = true;
                ui.set_graph_card_expanded = true;
                ui.activate_workspace_panel(woodshed_views::workspace::WorkspacePanel::Practice);
                ui.select_app_section(woodshed_core::storage::AppSection::Stage);
            },
            "p5a-timing-admission-example" => {
                use woodshed_core::harmony::KeyedCatalogRef;
                use woodshedding::pitch::PitchClass;
                use woodshedding::rehearsal::{Hold, Set, Timing, Touch};

                ui.stop_rehearsal();
                ui.set = Set::default();
                ui.practice_history = Default::default();
                ui.retained_sets = Default::default();
                ui.working_sets = Default::default();
                ui.catalog_explorations = Default::default();
                ui.stage.set_lens(Lens::Scales);
                for (label, root, bpm, hold) in [
                    ("Ordered 20 BPM", 0, 20.0, Hold::Bars(1)),
                    ("Ordered 400 BPM", 7, 400.0, Hold::Bars(1)),
                    ("Manual wait", 2, 120.0, Hold::Manual),
                    ("Core duration refusal", 4, 120.0, Hold::Seconds(1e30)),
                    ("Output PCM refusal", 9, 400.0, Hold::Seconds(1e10)),
                ] {
                    let mut card = KeyedCatalogRef {
                        formula_id: "scale:Major".into(),
                        root: PitchClass::new(root),
                    }
                    .to_card()
                    .expect("fixture scale exists");
                    card.label = label.into();
                    card.touch = Touch::Walk;
                    // These cards model an imported catalog fixture. Subsequent
                    // timing changes in the scenario are made through the UI.
                    card.timing = Timing { bpm: Some(bpm), hold };
                    ui.set.push(card);
                }
                ui.set.cursor = 0;
                ui.set_tray_expanded = true;
                ui.set_graph_card_expanded = true;
                ui.activate_workspace_panel(woodshed_views::workspace::WorkspacePanel::Practice);
                ui.select_app_section(woodshed_core::storage::AppSection::Stage);
            },
            "mere-arpeggio-remove-source" | "mere-scale-pattern-remove-source" => {
                let id = ui
                    .musical_context
                    .items()
                    .iter()
                    .find_map(|item| item.captured_recipe.as_ref())
                    .expect("kept fixture recipe")
                    .source_card;
                let index = ui
                    .set
                    .cards
                    .iter()
                    .position(|card| card.id == id)
                    .expect("source present");
                ui.set.remove(index);
                ui.stage.set_root(5);
                ui.root_dd.selected = 5;
                ui.transport.bpm = 140.0;
            },
            "mere-context-chord-focus" => {
                ui.focus_context_catalog(woodshed_core::harmony::KeyedCatalogRef {
                    formula_id: "chord:Major 7".into(),
                    root: woodshedding::pitch::PitchClass::new(0),
                });
            },
            "mere-context-scale-focus" => {
                ui.focus_context_catalog(woodshed_core::harmony::KeyedCatalogRef {
                    formula_id: "scale:Major".into(),
                    root: woodshedding::pitch::PitchClass::new(0),
                });
            },
            "shape-comparison-example" => {
                ui.stage.set_root(3);
                ui.root_dd.selected = 3;
                ui.stage.set_lens(Lens::Chords);
                if let Some(index) = ui
                    .stage
                    .chords()
                    .iter()
                    .position(|chord| chord.name == "Major")
                {
                    ui.stage.select_chord(index);
                }
                ui.stage_current(None);
                ui.step_card_shape(1);
                let index = ui.set.cursor;
                ui.set.duplicate(index);
                ui.set.cursor = index + 1;
                ui.step_card_shape(1);
                ui.select_app_section(woodshed_core::storage::AppSection::Rehearsal);
            },
            "stage-context-example" => {
                ui.stage.set_root(3); // C in the A-first root picker.
                ui.root_dd.selected = 3;
                ui.stage.set_lens(Lens::Chords);
                if let Some(major) = ui
                    .stage
                    .chords()
                    .iter()
                    .position(|chord| chord.name == "Major")
                {
                    ui.stage.select_chord(major);
                    ui.stage_current(None);
                }
                ui.set_tray_expanded = true;
            },
            "stage-related-pair" => {
                ui.stage.set_lens(Lens::Chords);
                if let Some(major) = ui
                    .stage
                    .chords()
                    .iter()
                    .position(|chord| chord.name == "Major")
                {
                    ui.stage.select_chord(major);
                    ui.stage_current(None);
                }
                if let Some(major_seven) = ui
                    .stage
                    .chords()
                    .iter()
                    .position(|chord| chord.name == "Major 7")
                {
                    ui.stage.select_chord(major_seven);
                    ui.stage_current(None);
                }
                ui.set_tray_expanded = true;
            },
            "claims-open-major-major7" => {
                ui.personal_link_pair = Some(("chord:Major".into(), "chord:Major 7".into()));
                ui.personal_link_history = false;
                ui.personal_link_layout = None;
                ui.personal_relation_notice = None;
                ui.activate_workspace_panel(woodshed_views::workspace::WorkspacePanel::Related);
            },
            "claims-note-reason-one" => {
                ui.personal_relation_note = cambium::TextInput::new("Independent reason one");
            },
            "claims-note-reason-two" => {
                ui.personal_relation_note = cambium::TextInput::new("Independent reason two");
            },
            "hide-first-stage-relation" => {
                let snapshot = set_graph_snapshot(ui);
                if let Some(key) = set_graph_relation_choices(&snapshot, ui)
                    .first()
                    .map(|relation| relation.identity.clone())
                {
                    ui.toggle_set_graph_relation(&snapshot, key);
                }
            },
            "hide-all-stage-relations" => {
                let snapshot = set_graph_snapshot(ui);
                ui.hide_all_set_graph_relations(&snapshot);
            },
            "show-all-stage-relations" => ui.show_all_set_graph_relations(),
            "expand-tray" => ui.set_tray_expanded = true,
            "collapse-tray" => ui.set_tray_expanded = false,
            "duplicate-selected" => {
                let cursor = ui.set.cursor;
                ui.set.duplicate(cursor);
            },
            "move-selected-down" => {
                let cursor = ui.set.cursor;
                ui.set.move_card(cursor, 1);
            },
            "move-selected-up" => {
                let cursor = ui.set.cursor;
                ui.set.move_card(cursor, -1);
            },
            "remove-selected" => {
                let cursor = ui.set.cursor;
                ui.set.remove(cursor);
            },
            _ => known = false,
        });
        if known {
            crate::sync::after_dispatch(self.shared, self.ctx);
            note_events(self.shared, self.ctx.runner.state());
        }
        known
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
        // Woodshed has no fetch or actor round-trip in this lane: a step's
        // effect is in the state by the time the next frame lays out. Reporting
        // quiet is honest here, and keeps `wait` from burning its cap.
        Some(false)
    }
}

impl Probe<'_, '_> {
    /// Stage-canvas receipt verbs resolve epoch-qualified identities from the
    /// live snapshot, then use the ordinary host pointer lifecycle. This keeps
    /// them distinct from the Related graph, which shares the same CSS classes.
    fn app_step(&mut self, line: &str) -> Result<(), String> {
        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("checkpoint-arrangement") => {
                use std::io::Write;
                let name = parts.next().ok_or("checkpoint-arrangement wants a name")?;
                if parts.next().is_some()
                    || name.is_empty()
                    || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                {
                    return Err("simple checkpoint name required".into());
                }
                let directory = std::env::var_os("WOODSHED_CAPTURE_DIR")
                    .ok_or("isolated capture directory required")?;
                let ui = self.ctx.runner.state();
                if !matches!(self.shared.seal, Some(woodshed_views::persona::PracticeSeal::Sealed { .. })) {
                    return Err("arrangement checkpoint requires sealed custody".into());
                }
                let stored = self.shared.storage.as_ref().ok_or("no store")?
                    .load().ok_or("unreadable sealed session")?;
                let persisted = woodshed_core::storage::decode_session(&stored)
                    .map_err(|e| e.to_string())?.session;
                let surfaces = ui.arrangement_editor.surfaces.borrow();
                let value = serde_json::json!({
                    "live": ui.to_persisted(),
                    "stored": persisted,
                    "draft_dirty": ui.arrangement_editor.dirty(),
                    "document": surfaces.document,
                    "selection": surfaces.primary.as_ref().and_then(|s| s.selected()),
                    "error": surfaces.error,
                });
                let path = std::path::PathBuf::from(directory).join(format!("{name}.json"));
                std::fs::OpenOptions::new().write(true).create_new(true).open(path)
                    .map_err(|e| e.to_string())?
                    .write_all(&serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())
            },
            Some("checkpoint-p4f") => {
                let name = parts.next().ok_or("checkpoint-p4f wants a name")?;
                if parts.next().is_some()
                    || name.is_empty()
                    || !name
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                {
                    return Err("checkpoint-p4f requires one simple filename stem".into());
                }
                let directory = std::env::var_os("WOODSHED_CAPTURE_DIR")
                    .ok_or("checkpoint-p4f requires an isolated capture directory")?;
                let ui = self.ctx.runner.state();
                let realizations: Vec<_> = ui.set.cards.iter().map(|card| {
                    let (pitches, duration, strum) = ui.stage.card_sounding_pitches_at_tempo(card, ui.transport.bpm).map_err(|error| error.to_string())?;
                    Ok(serde_json::json!({"card":card.id, "pitch_bits":pitches.iter().map(|pitch| pitch.to_bits()).collect::<Vec<_>>(), "duration_bits":duration.to_bits(), "strum_bits":strum.to_bits()}))
                }).collect::<Result<Vec<_>, String>>()?;
                let value = serde_json::json!({
                    "working_owner":ui.working_sets.active_id,
                    "cards":ui.set.cards,
                    "cursor":ui.set.cursor,
                    "realizations":realizations,
                    "stage_projection":ui.app_settings.stage,
                    "workspace":ui.workspace.to_snapshot_json().map_err(|error| format!("workspace checkpoint: {error}"))?,
                    "personal_relations":ui.to_persisted().personal_relations,
                    "practice_saved":ui.practice_saved,
                    "save_problem":ui.practice_save_notice,
                    "preview_voices":self.shared.backend.as_ref().map(|backend| backend.preview_voice_count()),
                    "preview_starts":self.shared.backend.as_ref().and_then(|backend| backend.output_snapshot()).map(|snapshot|snapshot.preview_starts),
                    "output_observation_id":self.shared.backend.as_ref().and_then(|backend| backend.output_observation_id()),
                });
                let path = std::path::PathBuf::from(directory).join(format!("{name}.json"));
                std::fs::write(
                    path,
                    serde_json::to_vec_pretty(&value).map_err(|error| error.to_string())?,
                )
                .map_err(|error| error.to_string())
            }
            Some("ui-zoom") => {
                let zoom: f32 = parts
                    .next()
                    .ok_or("ui-zoom wants a scale")?
                    .parse()
                    .map_err(|_| "invalid UI zoom")?;
                if parts.next().is_some() || !zoom.is_finite() || !(0.5..=2.0).contains(&zoom) {
                    return Err("ui-zoom requires a scale from 0.5 to 2".into());
                }
                *self.ctx.set_ui_zoom = Some(zoom);
                Ok(())
            },
            Some("pointer-click") => {
                // For receipts measured from a presented frame when the probe's
                // independently computed selector layout disagrees with the host.
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
                if parts.next().is_some()
                    || !x.is_finite()
                    || !y.is_finite()
                    || x < 0.0
                    || y < 0.0
                    || x >= w
                    || y >= h
                {
                    return Err("pointer-click requires an in-window logical point".into());
                }
                self.moved(x, y);
                self.press(x, y);
                self.release(x, y);
                Ok(())
            },
            Some("open-stage-arrangement") => {
                if parts.next().is_some() {
                    return Err("open-stage-arrangement takes no arguments".to_string());
                }
                let selector = Selector::role("combobox").containing("Set arrangement");
                self.click(&selector)
                    .then_some(())
                    .ok_or_else(|| "open-stage-arrangement missed the picker".to_string())
            },
            Some("choose-stage-arrangement") => {
                let label = parts
                    .next()
                    .ok_or("choose-stage-arrangement wants one arrangement label")?;
                if parts.next().is_some() {
                    return Err("choose-stage-arrangement takes one arrangement label".to_string());
                }
                let selector = Selector::role("option").containing(label);
                self.click(&selector)
                    .then_some(())
                    .ok_or_else(|| format!("choose-stage-arrangement missed {label}"))
            },
            Some("click-context-node") => {
                let target = parts.collect::<Vec<_>>().join(" ");
                let node_key = {
                    let ui = self.ctx.runner.state();
                    let snapshot = set_graph_snapshot(ui);
                    let swatch =
                        set_graph_swatch_from_snapshot(&snapshot, ui, ui.set_tray_expanded);
                    swatch.graph.nodes.iter().find_map(|node| {
                        let subject = snapshot.node_of(node.id.instance)?;
                        (!subject.foreground && subject.id.wire_key() == target)
                            .then(|| node.key.clone())
                            .flatten()
                    })
                }
                .ok_or_else(|| format!("no context node {target}"))?;
                let selector =
                    Selector::class("graph-canvas-swatch-node").with_attr("data-key", node_key);
                self.click(&selector)
                    .then_some(())
                    .ok_or_else(|| format!("click-context-node missed {target}"))
            },
            Some("click-stage-node") => {
                let index: usize = parts
                    .next()
                    .and_then(|value| value.parse().ok())
                    .ok_or("click-stage-node wants a node index")?;
                if parts.next().is_some() {
                    return Err("click-stage-node takes one index".to_string());
                }
                let node_key = self
                    .stage_node_key(index)
                    .ok_or_else(|| format!("no Stage node at index {index}"))?;
                let selector =
                    Selector::class("graph-canvas-swatch-node").with_attr("data-key", node_key);
                self.click(&selector)
                    .then_some(())
                    .ok_or_else(|| format!("click-stage-node missed index {index}"))
            },
            Some("click-stage-relation") => {
                let index: usize = parts
                    .next()
                    .and_then(|value| value.parse().ok())
                    .ok_or("click-stage-relation wants a relation index")?;
                if parts.next().is_some() {
                    return Err("click-stage-relation takes one index".to_string());
                }
                let relation_id = self
                    .stage_relation_id(index)
                    .ok_or_else(|| format!("no Stage relation at index {index}"))?;
                let selector = Selector::class("graph-canvas-swatch-relation")
                    .with_attr("data-relation-id", relation_id);
                self.click(&selector)
                    .then_some(())
                    .ok_or_else(|| format!("click-stage-relation missed index {index}"))
            },
            Some("drag-stage-node") => {
                let index: usize = parts
                    .next()
                    .and_then(|value| value.parse().ok())
                    .ok_or("drag-stage-node wants a node index")?;
                let dx: f32 = parts
                    .next()
                    .and_then(|value| value.parse().ok())
                    .ok_or("drag-stage-node wants an x delta")?;
                let dy: f32 = parts
                    .next()
                    .and_then(|value| value.parse().ok())
                    .ok_or("drag-stage-node wants a y delta")?;
                if parts.next().is_some() {
                    return Err("drag-stage-node takes exactly index, dx, and dy".to_string());
                }
                let node_key = self
                    .stage_node_key(index)
                    .ok_or_else(|| format!("no Stage node at index {index}"))?;
                let selector =
                    Selector::class("graph-canvas-swatch-node").with_attr("data-key", node_key);
                let hit = self
                    .resolve(&selector)
                    .ok_or_else(|| format!("drag-stage-node missed index {index}"))?;
                let start = hit.point;
                let end = (start.0 + dx, start.1 + dy);
                self.press(start.0, start.1);
                self.moved((start.0 + end.0) * 0.5, (start.1 + end.1) * 0.5);
                self.moved(end.0, end.1);
                self.release(end.0, end.1);
                Ok(())
            },
            Some("drag-stage-graph-resize") => {
                let dx: f32 = parts
                    .next()
                    .and_then(|value| value.parse().ok())
                    .ok_or("drag-stage-graph-resize wants an x delta")?;
                let dy: f32 = parts
                    .next()
                    .and_then(|value| value.parse().ok())
                    .ok_or("drag-stage-graph-resize wants a y delta")?;
                if parts.next().is_some() {
                    return Err("drag-stage-graph-resize takes exactly dx and dy".to_string());
                }
                let selector = Selector::class("resize-handle");
                let hit = self
                    .resolve(&selector)
                    .ok_or("drag-stage-graph-resize missed the handle")?;
                let start = hit.point;
                let end = (start.0 + dx, start.1 + dy);
                self.press(start.0, start.1);
                self.moved((start.0 + end.0) * 0.5, (start.1 + end.1) * 0.5);
                self.moved(end.0, end.1);
                self.release(end.0, end.1);
                Ok(())
            },
            Some("press-stage-graph-resize") => {
                if parts.next().is_some() {
                    return Err("press-stage-graph-resize takes no arguments".to_string());
                }
                let selector = Selector::class("resize-handle");
                let hit = self
                    .resolve(&selector)
                    .ok_or("press-stage-graph-resize missed the handle")?;
                self.shared.scenario_drag_origin = Some(hit.point);
                self.press(hit.point.0, hit.point.1);
                Ok(())
            },
            Some("move-stage-graph-resize") => {
                let dx: f32 = parts
                    .next()
                    .and_then(|value| value.parse().ok())
                    .ok_or("move-stage-graph-resize wants an x delta")?;
                let dy: f32 = parts
                    .next()
                    .and_then(|value| value.parse().ok())
                    .ok_or("move-stage-graph-resize wants a y delta")?;
                if parts.next().is_some() {
                    return Err("move-stage-graph-resize takes x and y deltas".to_string());
                }
                let origin = self
                    .shared
                    .scenario_drag_origin
                    .ok_or("move-stage-graph-resize has no pressed handle")?;
                self.moved(origin.0 + dx, origin.1 + dy);
                Ok(())
            },
            Some("release-stage-graph-resize") => {
                let dx: f32 = parts
                    .next()
                    .and_then(|value| value.parse().ok())
                    .ok_or("release-stage-graph-resize wants an x delta")?;
                let dy: f32 = parts
                    .next()
                    .and_then(|value| value.parse().ok())
                    .ok_or("release-stage-graph-resize wants a y delta")?;
                if parts.next().is_some() {
                    return Err("release-stage-graph-resize takes x and y deltas".to_string());
                }
                let origin = self
                    .shared
                    .scenario_drag_origin
                    .take()
                    .ok_or("release-stage-graph-resize has no pressed handle")?;
                self.release(origin.0 + dx, origin.1 + dy);
                Ok(())
            },
            Some("press-stage-node") => {
                let index: usize = parts
                    .next()
                    .and_then(|value| value.parse().ok())
                    .ok_or("press-stage-node wants a node index")?;
                if parts.next().is_some() {
                    return Err("press-stage-node takes one index".to_string());
                }
                let node_key = self
                    .stage_node_key(index)
                    .ok_or_else(|| format!("no Stage node at index {index}"))?;
                let selector =
                    Selector::class("graph-canvas-swatch-node").with_attr("data-key", node_key);
                let hit = self
                    .resolve(&selector)
                    .ok_or_else(|| format!("press-stage-node missed index {index}"))?;
                self.shared.scenario_drag_origin = Some(hit.point);
                self.press(hit.point.0, hit.point.1);
                Ok(())
            },
            Some("move-stage-node") => {
                let dx: f32 = parts
                    .next()
                    .and_then(|value| value.parse().ok())
                    .ok_or("move-stage-node wants an x delta")?;
                let dy: f32 = parts
                    .next()
                    .and_then(|value| value.parse().ok())
                    .ok_or("move-stage-node wants a y delta")?;
                if parts.next().is_some() {
                    return Err("move-stage-node takes x and y deltas".to_string());
                }
                let origin = self
                    .shared
                    .scenario_drag_origin
                    .ok_or("move-stage-node has no pressed node")?;
                self.moved(origin.0 + dx, origin.1 + dy);
                Ok(())
            },
            Some("release-stage-node") => {
                let dx: f32 = parts
                    .next()
                    .and_then(|value| value.parse().ok())
                    .ok_or("release-stage-node wants an x delta")?;
                let dy: f32 = parts
                    .next()
                    .and_then(|value| value.parse().ok())
                    .ok_or("release-stage-node wants a y delta")?;
                if parts.next().is_some() {
                    return Err("release-stage-node takes x and y deltas".to_string());
                }
                let origin = self
                    .shared
                    .scenario_drag_origin
                    .take()
                    .ok_or("release-stage-node has no pressed node")?;
                self.release(origin.0 + dx, origin.1 + dy);
                Ok(())
            },
            _ => Err(format!("unknown verb: {line}")),
        }
    }
}
