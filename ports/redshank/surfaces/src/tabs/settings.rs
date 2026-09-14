//! The settings tab body: four sections of label/control rows.
//! Lane S2 owns this file.

use super::{controls, rows};
use crate::{CompactCommand, FullView, RedshankSurfaceState, format_bytes};
use cambium::el;
use redshank_model::{
    CapturePlaybackBehavior, ListenerSettings, NotePrivacy, RefreshSchedule, ThemeMode, ThemeSeed,
};

/// One settings write.
fn write(settings: ListenerSettings) -> CompactCommand {
    CompactCommand::UpdateSettings(settings)
}

/// One settings write as a segment option's command list.
fn writes(settings: ListenerSettings) -> Vec<CompactCommand> {
    vec![write(settings)]
}

/// The Library feed header's `Auto-download` toggle, same field and command.
pub fn auto_download_toggle(state: &RedshankSurfaceState) -> FullView {
    let settings = &state.settings;
    controls::toggle(
        "Auto-download new episodes",
        settings.auto_download,
        write(ListenerSettings {
            auto_download: !settings.auto_download,
            ..settings.clone()
        }),
    )
}

fn section(word: &str, rows_out: Vec<FullView>) -> FullView {
    Box::new(
        el("section", (rows::micro(word.to_owned()), rows_out))
            .attr("class", "rs-settings-group")
            .attr("aria-label", word.to_owned()),
    )
}

const RATES: [(&str, u16); 4] = [("0.8×", 80), ("1.0×", 100), ("1.2×", 120), ("1.5×", 150)];

fn playback(state: &RedshankSurfaceState) -> FullView {
    let settings = &state.settings;
    let skip_row = |label: &str, back: bool| {
        let value = if back {
            settings.skip_backward_ms
        } else {
            settings.skip_forward_ms
        };
        let shift = |delta: i64| {
            let next = (value as i64 + delta).clamp(5_000, 300_000) as u64;
            let mut changed = settings.clone();
            if back {
                changed.skip_backward_ms = next;
            } else {
                changed.skip_forward_ms = next;
            }
            write(changed)
        };
        controls::row(
            label,
            None,
            controls::stepper(
                label,
                (value / 1_000) as i64,
                "s",
                shift(-5_000),
                shift(5_000),
            ),
        )
    };
    // Rate moves the backend and the store together: the dock reads the
    // effective rate, the store remembers the requested one.
    let rate_options = RATES
        .iter()
        .map(|(label, percent)| {
            (
                (*label).to_owned(),
                settings.playback_rate_percent == *percent,
                vec![
                    CompactCommand::SetRate(*percent),
                    write(ListenerSettings {
                        playback_rate_percent: *percent,
                        ..settings.clone()
                    }),
                ],
            )
        })
        .collect();
    section(
        "PLAYBACK",
        vec![
            skip_row("Skip back", true),
            skip_row("Skip forward", false),
            controls::row(
                "Rate",
                Some("pitch-preserving"),
                controls::segment("Rate", rate_options),
            ),
        ],
    )
}

fn capture(state: &RedshankSurfaceState) -> FullView {
    let settings = &state.settings;
    let offset = settings.reaction_offset_ms;
    let offset_command = |next: u64| {
        write(ListenerSettings {
            reaction_offset_ms: next,
            ..settings.clone()
        })
    };
    let behavior_options = [
        ("pause", CapturePlaybackBehavior::Pause),
        ("duck", CapturePlaybackBehavior::Duck),
        ("continue", CapturePlaybackBehavior::Continue),
    ]
    .iter()
    .map(|(label, behavior)| {
        (
            (*label).to_owned(),
            settings.capture_playback == *behavior,
            writes(ListenerSettings {
                capture_playback: *behavior,
                ..settings.clone()
            }),
        )
    })
    .collect();
    let privacy_options = [
        ("private", NotePrivacy::Private),
        ("shareable", NotePrivacy::Shareable),
    ]
    .iter()
    .map(|(label, privacy)| {
        (
            (*label).to_owned(),
            settings.note_privacy == *privacy,
            writes(ListenerSettings {
                note_privacy: *privacy,
                ..settings.clone()
            }),
        )
    })
    .collect();
    section(
        "CAPTURE",
        vec![
            controls::row(
                "Reaction offset",
                Some("anchor earlier than the press"),
                controls::stepper(
                    "Reaction offset",
                    (offset / 1_000) as i64,
                    "s",
                    offset_command(offset.saturating_sub(1_000)),
                    offset_command(offset.saturating_add(1_000).min(30_000)),
                ),
            ),
            controls::row(
                "While recording",
                None,
                controls::segment("While recording", behavior_options),
            ),
            controls::row(
                "Resume playback after capture",
                None,
                controls::toggle(
                    "Resume playback after capture",
                    settings.resume_after_capture,
                    write(ListenerSettings {
                        resume_after_capture: !settings.resume_after_capture,
                        ..settings.clone()
                    }),
                ),
            ),
            controls::row(
                "New notes are",
                None,
                controls::segment("New notes are", privacy_options),
            ),
            controls::row(
                "Microphone",
                None,
                controls::readout(
                    "Microphone",
                    state
                        .microphone_label
                        .clone()
                        .unwrap_or_else(|| "System default".into()),
                    "",
                ),
            ),
        ],
    )
}

fn storage(state: &RedshankSurfaceState) -> FullView {
    let settings = &state.settings;
    const STEP: u64 = 256 * 1024 * 1024;
    let budget = |up: bool| {
        let next = if up {
            settings.cache_budget_bytes.saturating_add(STEP)
        } else {
            settings.cache_budget_bytes.saturating_sub(STEP).max(STEP)
        };
        write(ListenerSettings {
            cache_budget_bytes: next,
            ..settings.clone()
        })
    };
    section(
        "STORAGE",
        vec![
            controls::row(
                "Offline cache budget",
                None,
                controls::stepper(
                    "Offline cache budget",
                    (settings.cache_budget_bytes / (1024 * 1024)) as i64,
                    "MiB",
                    budget(false),
                    budget(true),
                ),
            ),
            controls::row(
                "In use",
                None,
                controls::readout("In use", format_bytes(state.cache_used_bytes), ""),
            ),
            controls::row(
                "Automatic download of new episodes",
                None,
                controls::toggle(
                    "Automatic download of new episodes",
                    settings.auto_download,
                    write(ListenerSettings {
                        auto_download: !settings.auto_download,
                        ..settings.clone()
                    }),
                ),
            ),
            controls::row(
                "Reclaim least-recent downloads automatically",
                None,
                controls::toggle(
                    "Reclaim least-recent downloads automatically",
                    settings.auto_reclaim,
                    write(ListenerSettings {
                        auto_reclaim: !settings.auto_reclaim,
                        ..settings.clone()
                    }),
                ),
            ),
        ],
    )
}

/// The four role squares the seed derives.
fn swatches() -> FullView {
    const ROLES: [&str; 4] = [
        "--t-primary",
        "--t-secondary",
        "--t-tertiary",
        "--t-surface",
    ];
    let squares: Vec<FullView> = ROLES
        .iter()
        .map(|role| {
            Box::new(
                el("span", cambium::text(""))
                    .attr("class", "rs-settings-swatch")
                    .attr("style", format!("background:var({role})")),
            ) as FullView
        })
        .collect();
    Box::new(el("span", squares).attr("class", "rs-settings-swatches"))
}

fn appearance(state: &RedshankSurfaceState) -> FullView {
    let settings = &state.settings;
    let refresh_options = [
        ("manual", RefreshSchedule::Manual),
        ("hourly", RefreshSchedule::Hourly),
        ("daily", RefreshSchedule::Daily),
    ]
    .iter()
    .map(|(label, schedule)| {
        (
            (*label).to_owned(),
            settings.refresh_schedule == *schedule,
            writes(ListenerSettings {
                refresh_schedule: *schedule,
                ..settings.clone()
            }),
        )
    })
    .collect();
    let seed_options = [
        ("wetland", ThemeSeed::Wetland),
        ("brand shell", ThemeSeed::BrandShell),
    ]
    .iter()
    .map(|(label, seed)| {
        (
            (*label).to_owned(),
            settings.seed == *seed,
            writes(ListenerSettings {
                seed: *seed,
                ..settings.clone()
            }),
        )
    })
    .collect();
    let mode_options = [
        ("light", ThemeMode::Light),
        ("dark", ThemeMode::Dark),
        ("hc light", ThemeMode::HcLight),
        ("hc dark", ThemeMode::HcDark),
    ]
    .iter()
    .map(|(label, mode)| {
        (
            (*label).to_owned(),
            settings.mode == *mode,
            writes(ListenerSettings {
                mode: *mode,
                ..settings.clone()
            }),
        )
    })
    .collect();
    section(
        "FEEDS · APPEARANCE",
        vec![
            controls::row(
                "Refresh subscriptions",
                None,
                controls::segment("Refresh subscriptions", refresh_options),
            ),
            controls::row(
                "Seeds",
                Some("Tabard roles derive from these"),
                Box::new(
                    el(
                        "span",
                        (swatches(), controls::segment("Seed", seed_options)),
                    )
                    .attr("class", "rs-settings-seeds"),
                ),
            ),
            controls::row("Mode", None, controls::segment("Mode", mode_options)),
        ],
    )
}

pub fn panel(state: &RedshankSurfaceState) -> FullView {
    Box::new(
        el(
            "section",
            (
                playback(state),
                capture(state),
                storage(state),
                appearance(state),
            ),
        )
        .attr("class", "rs-panel rs-settings"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tabs::tests_support::{click, commands, markup, runner};

    fn settings_state() -> RedshankSurfaceState {
        RedshankSurfaceState {
            cache_used_bytes: 1_412 * 1024 * 1024,
            microphone_label: Some("System default · 48 kHz mono WAV".into()),
            ..Default::default()
        }
    }

    #[test]
    fn every_section_and_control_kind_is_present() {
        let markup = markup(settings_state(), panel);
        for word in ["PLAYBACK", "CAPTURE", "STORAGE", "FEEDS · APPEARANCE"] {
            assert!(markup.contains(word), "missing section {word}");
        }
        assert!(markup.contains("rs-stepper"));
        assert!(markup.contains("rs-segment-on"));
        assert!(markup.contains("role=\"switch\""));
        assert!(markup.contains("rs-readout"));
        assert!(markup.contains("1.4 GiB"));
        assert!(markup.contains("System default · 48 kHz mono WAV"));
        assert!(markup.contains("rs-settings-swatch"));
    }

    #[test]
    fn skip_steppers_emit_update_settings_with_the_changed_field() {
        let mut runner = runner(settings_state(), panel);
        click(&mut runner, "Increase Skip forward");
        click(&mut runner, "Decrease Skip back");
        let commands = commands(&mut runner);
        assert!(matches!(
            &commands[0],
            CompactCommand::UpdateSettings(settings) if settings.skip_forward_ms == 35_000
        ));
        assert!(matches!(
            &commands[1],
            CompactCommand::UpdateSettings(settings) if settings.skip_backward_ms == 10_000
        ));
    }

    #[test]
    fn cache_budget_stepper_moves_by_256_mib() {
        let mut runner = runner(settings_state(), panel);
        click(&mut runner, "Increase Offline cache budget");
        assert!(matches!(
            &commands(&mut runner)[0],
            CompactCommand::UpdateSettings(settings)
                if settings.cache_budget_bytes == 2304 * 1024 * 1024
        ));
    }

    #[test]
    fn reaction_offset_steps_by_one_second() {
        let mut runner = runner(settings_state(), panel);
        click(&mut runner, "Increase Reaction offset");
        assert!(matches!(
            &commands(&mut runner)[0],
            CompactCommand::UpdateSettings(settings) if settings.reaction_offset_ms == 1_000
        ));
    }

    #[test]
    fn capture_behaviour_and_privacy_segments_emit_their_fields() {
        let mut runner = runner(settings_state(), panel);
        click(&mut runner, "While recording: duck");
        click(&mut runner, "New notes are: shareable");
        let commands = commands(&mut runner);
        assert!(matches!(
            &commands[0],
            CompactCommand::UpdateSettings(settings)
                if settings.capture_playback == CapturePlaybackBehavior::Duck
        ));
        assert!(matches!(
            &commands[1],
            CompactCommand::UpdateSettings(settings)
                if settings.note_privacy == NotePrivacy::Shareable
        ));
    }

    #[test]
    fn toggles_flip_their_field() {
        let mut runner = runner(settings_state(), panel);
        click(&mut runner, "Automatic download of new episodes");
        click(&mut runner, "Reclaim least-recent downloads automatically");
        click(&mut runner, "Resume playback after capture");
        let commands = commands(&mut runner);
        assert!(matches!(
            &commands[0],
            CompactCommand::UpdateSettings(settings) if settings.auto_download
        ));
        assert!(matches!(
            &commands[1],
            CompactCommand::UpdateSettings(settings) if settings.auto_reclaim
        ));
        assert!(matches!(
            &commands[2],
            CompactCommand::UpdateSettings(settings) if !settings.resume_after_capture
        ));
    }

    #[test]
    fn rate_segment_sets_the_backend_and_the_store() {
        let mut runner = runner(settings_state(), panel);
        click(&mut runner, "Rate: 1.2×");
        let commands = commands(&mut runner);
        assert_eq!(commands[0], CompactCommand::SetRate(120));
        assert!(matches!(
            &commands[1],
            CompactCommand::UpdateSettings(settings) if settings.playback_rate_percent == 120
        ));
    }

    #[test]
    fn refresh_seed_and_mode_segments_emit_their_fields() {
        let mut runner = runner(settings_state(), panel);
        click(&mut runner, "Refresh subscriptions: daily");
        click(&mut runner, "Seed: brand shell");
        click(&mut runner, "Mode: hc dark");
        let commands = commands(&mut runner);
        assert!(matches!(
            &commands[0],
            CompactCommand::UpdateSettings(settings)
                if settings.refresh_schedule == RefreshSchedule::Daily
        ));
        assert!(matches!(
            &commands[1],
            CompactCommand::UpdateSettings(settings) if settings.seed == ThemeSeed::BrandShell
        ));
        assert!(matches!(
            &commands[2],
            CompactCommand::UpdateSettings(settings) if settings.mode == ThemeMode::HcDark
        ));
    }
}
