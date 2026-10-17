//! Bounded mixed backing acceptance using real host admission and sealed storage.
use super::{Ctx, Shared};
use serde_json::{Value, json};
use std::{cell::RefCell, io::Write, path::PathBuf, rc::Rc};
use woodshed_core::{
    Lens,
    loop_plan::{LoopPlan, LooperConfig, LooperSource},
};
use woodshed_views::stage::UiState;
use woodshedding::rehearsal::{Hold, Set, Timing, Touch};

pub(super) fn act(ctx: &mut Ctx<'_>, shared: &Rc<RefCell<Shared>>, label: &str) -> Option<bool> {
    if label == "p7a-mixed-set-example" {
        ctx.runner.update(|ui| {
            ui.stop_rehearsal();
            ui.set = Set::default();
            ui.working_sets = Default::default();
            for (lens, name, bpm, bars) in [
                (Lens::Chords, "Loop chord", 240.0, 1),
                (Lens::Scales, "Loop scale", 180.0, 2),
                (Lens::Arpeggios, "Loop arpeggio", 300.0, 1),
            ] {
                ui.stage.set_lens(lens);
                let mut card = ui
                    .stage
                    .card_from_lens()
                    .expect("built-in fixture material");
                card.label = name.into();
                card.timing = Timing {
                    bpm: Some(bpm),
                    hold: Hold::Bars(bars),
                };
                if lens == Lens::Scales {
                    card.touch = Touch::Walk;
                }
                ui.set.push(card);
            }
            ui.looper = LooperConfig {
                source: LooperSource::Set(ui.working_sets.active_id),
                beats: 3,
                click: false,
                ..Default::default()
            };
            ui.set_tray_expanded = true;
            ui.select_app_section(woodshed_core::storage::AppSection::Looper);
        });
        crate::sync::after_dispatch(&mut shared.borrow_mut(), ctx);
        return Some(true);
    }
    if label == "p7a-manual-refusal" || label == "p7a-recover-bars" {
        ctx.runner.update(|ui| {
            let card = ui
                .set
                .cards
                .iter_mut()
                .find(|card| card.label == "Loop scale")
                .unwrap();
            card.timing.hold = if label == "p7a-manual-refusal" {
                Hold::Manual
            } else {
                Hold::Bars(2)
            };
        });
        crate::sync::after_dispatch(&mut shared.borrow_mut(), ctx);
        return Some(true);
    }
    if label == "p7a-reorder" {
        ctx.runner.update(|ui| ui.set.cards.swap(0, 1));
        crate::sync::after_dispatch(&mut shared.borrow_mut(), ctx);
        return Some(true);
    }
    if let Some(name) = label.strip_prefix("p7a-checkpoint ") {
        return Some(checkpoint(
            ctx.runner.state(),
            &mut shared.borrow_mut(),
            name,
        ));
    }
    if label == "p7a-verify-reopen" {
        return Some(verify_reopen(ctx.runner.state(), &mut shared.borrow_mut()));
    }
    None
}

fn path(name: &str) -> Result<PathBuf, String> {
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err("invalid receipt name".into());
    }
    Ok(
        PathBuf::from(std::env::var_os("WOODSHED_CAPTURE_DIR").ok_or("no receipt directory")?)
            .join(format!("p7a-{name}.json")),
    )
}
fn plan_value(plan: &LoopPlan) -> Value {
    json!({"owner":plan.owner,"name":plan.name,"one_shot":plan.one_shot,"click":plan.click,
        "segments":plan.segments.iter().map(|s| json!({"id":s.id,"label":s.label,
            "bpm":s.bpm,"beats":s.beats,"bars":s.bars,"pitches_hz":s.pitches_hz,
            "phrase":s.phrase.as_ref().map(|p|p.events.iter().map(|e|json!({
                "beat":e.beat,"pitches_hz":e.pitches_hz,"strum_seconds":e.strum_seconds
            })).collect::<Vec<_>>())})).collect::<Vec<_>>()})
}
fn report(ui: &UiState, shared: &mut Shared) -> Result<Value, String> {
    if !matches!(
        shared.seal,
        Some(woodshed_views::persona::PracticeSeal::Sealed { .. })
    ) {
        return Err("real sealed fixture required".into());
    }
    let storage = shared.storage.as_ref().ok_or("no store")?;
    let stored =
        woodshed_core::storage::decode_session(&storage.load().ok_or("sealed session unreadable")?)
            .map_err(|e| e.to_string())?
            .session;
    if !ui.practice_saved
        || ui.practice_save_notice.is_some()
        || serde_json::to_value(&ui.set).unwrap() != serde_json::to_value(&stored.set).unwrap()
        || ui.looper != stored.looper
        || ui.song != stored.song
        || ui.loop_captures != stored.loop_captures
        || ui.working_sets.active_id != stored.working_sets.active_id
    {
        return Err("live Set/owner/Looper/saved backing/captures differ from sealed state".into());
    }
    let backend = shared.backend.as_mut().ok_or("no output backend")?;
    let (song, rate) = backend.export_backing()?;
    let backend_plan = backend
        .recorder_observation(None)
        .plan
        .ok_or("no backend Set plan")?;
    if ui.loop_admitted.as_ref() != Some(&backend_plan)
        || song.bars.len() != backend_plan.segments.len()
    {
        return Err("UI admission differs from the actual backend Set plan".into());
    }
    for (bar, segment) in song.bars.iter().zip(&backend_plan.segments) {
        let phrase = segment
            .phrase
            .as_ref()
            .map(|p| woodshed_audio::BackingPhrase {
                events: p
                    .events
                    .iter()
                    .map(|e| woodshed_audio::BackingEvent {
                        beat: e.beat,
                        pitches_hz: e.pitches_hz.clone(),
                        strum_seconds: e.strum_seconds,
                    })
                    .collect(),
            });
        if bar.bpm != segment.bpm
            || bar.time_signature.numerator != segment.beats
            || bar.time_signature.denominator != 4
            || bar.length != segment.bars
            || bar.backing_phrase != phrase
            || bar
                .chord_ref
                .as_ref()
                .map(|c| c.pitches_hz.as_slice())
                .unwrap_or(&[])
                != segment.pitches_hz
        {
            return Err("actual output backing differs from its admitted Set segment".into());
        }
    }
    let snapshot = backend.output_snapshot().ok_or("no output snapshot")?;
    let captures = backend.capture_snapshot();
    let input_unopened = backend.capture_source_rate().is_none();
    let selected_has_capture = ui
        .loop_selected
        .is_some_and(|id| captures.iter().any(|take| take.id == id));
    let selected_index = ui.loop_selected.and_then(|id| backend_plan.index_of(id));

    let mut take_reports = Vec::new();
    for take in captures {
        let reference = stored
            .loop_captures
            .takes
            .iter()
            .find(|r| r.id == take.id)
            .ok_or("take has no sealed reference")?;
        let samples = storage.try_read_capture(
            stored
                .loop_captures
                .namespace
                .as_deref()
                .ok_or("no capture namespace")?,
            reference,
        )?;
        if samples.len() != take.buffer.len()
            || !samples
                .iter()
                .zip(take.buffer.data.iter())
                .all(|(a, b)| a.to_bits() == b.to_bits())
        {
            return Err("sealed take PCM bits differ from hydrated capture".into());
        }
        let bits: Vec<u8> = samples
            .iter()
            .flat_map(|s| s.to_bits().to_le_bytes())
            .collect();
        take_reports.push(json!({"id":take.id,"frames":samples.len(),"rate":take.buffer.sample_rate_hz,
            "bpm":take.bpm,"beats":take.beats,"bars":take.bars,"legacy_clock":take.legacy_clock,
            "pcm_bits_digest":woodshed_core::loop_capture::pcm_digest(&bits),"sealed_pcm_exact":true}));
    }
    // This is a read-only reference render of the actual host-admitted snapshot.
    // The scenario's Export WAV action separately exercises the normal host worker.
    let mut wav = std::io::Cursor::new(Vec::new());
    let export = woodshed_audio::offline::export_song_wav_to(&song, rate, &mut wav)
        .map_err(|e| e.to_string())?;
    let requested = ui.requested_loop_plan();
    Ok(
        json!({"set":ui.set,"owner":ui.working_sets.active_id,"looper":ui.looper,"saved_backing":ui.song,
        "manifest":ui.loop_captures,"captures":take_reports,"sealed_exact":true,
        "backend_admitted":plan_value(&backend_plan),"selected":ui.loop_selected,
        "selected_index":selected_index,"selected_has_capture":selected_has_capture,
        "requested_ready":requested.is_ok(),"admitted":ui.loop_admitted.as_ref().map(plan_value),
        "requested":requested.as_ref().ok().map(plan_value),"playing":snapshot.playing,
        "bar":snapshot.cursor.bar_idx,"sample_in_bar":snapshot.cursor.sample_in_bar,
        "output_frames":snapshot.device_frames,"rate":rate,"streams":backend.output_stream_count(),
        "input_unopened":input_unopened,"wav_frames":export.frames,
        "wav_digest":woodshed_core::loop_capture::pcm_digest(wav.get_ref()),
        "phrase_segments":song.bars.iter().filter(|b|b.backing_phrase.is_some()).count()}),
    )
}
fn write(name: &str, report: &Value) -> Result<(), String> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path(name)?)
        .map_err(|e| e.to_string())?;
    file.write_all(serde_json::to_string_pretty(report).unwrap().as_bytes())
        .map_err(|e| e.to_string())
}
fn checkpoint(ui: &UiState, shared: &mut Shared, name: &str) -> bool {
    let result = (|| -> Result<(), String> {
        let report = report(ui, shared)?;
        if name == "refused" {
            if report["requested_ready"] != false || report["phrase_segments"] != 2 {
                return Err("readiness refusal must retain the admitted mixed backing".into());
            }
        } else if report["requested_ready"] != true
            || report["phrase_segments"] != 2
            || report["admitted"] != report["requested"]
        {
            return Err("all three current Cards must be admitted with two phrase segments".into());
        }
        if name == "reordered"
            && (report["selected_index"] != 0 || report["selected_has_capture"] != true)
        {
            return Err(
                "reordered selected Card must own the same captured take in row zero".into(),
            );
        }
        if report["captures"]
            .as_array()
            .unwrap()
            .iter()
            .any(|take| take["id"]["card"] != 2 || take["id"]["set"] != 1)
        {
            return Err("the fixture take must remain owned by Scale Card 2 in Set 1".into());
        }
        if name == "final"
            && (report["playing"] != false || report["captures"].as_array().unwrap().len() != 1)
        {
            return Err("final mixed backing must be stopped with one sealed capture".into());
        }
        write(name, &report)
    })();
    if let Err(error) = &result {
        eprintln!("P7a checkpoint {name}: {error}");
    }
    result.is_ok()
}
fn verify_reopen(ui: &UiState, shared: &mut Shared) -> bool {
    let result = (|| -> Result<(), String> {
        let expected: Value =
            serde_json::from_slice(&std::fs::read(path("final")?).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        let actual = report(ui, shared)?;
        for key in [
            "set",
            "owner",
            "looper",
            "saved_backing",
            "manifest",
            "captures",
            "admitted",
            "backend_admitted",
            "requested",
            "wav_frames",
            "wav_digest",
        ] {
            if expected[key] != actual[key] {
                return Err(format!("reopened {key} differs"));
            }
        }
        if actual["playing"] != false || actual["input_unopened"] != true {
            return Err("reopened output must be stopped and input unopened".into());
        }
        write("reopened", &actual)
    })();
    if let Err(error) = &result {
        eprintln!("P7a reopen: {error}");
    }
    result.is_ok()
}
