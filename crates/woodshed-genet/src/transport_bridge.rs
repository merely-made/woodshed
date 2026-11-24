//! Opt-in desktop transport mailbox for the Turnstone integration proof.
//! The controller publishes one complete command at a time and waits for its
//! acknowledgement before publishing the next. Only the desktop event loop
//! changes Woodshed transport or window state.

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::json;
use woodshed_core::audio::AudioBackend as _;
use woodshed_core::song::{SongBar, SongDoc};

use crate::{Shared, audio::CpalBackend, sync::Ctx};

const STATUS_INTERVAL: Duration = Duration::from_millis(50);
const QUIET_GAIN: f32 = 0.15;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Command {
    id: u64,
    action: String,
    value: Option<i64>,
}

enum Observation {
    Gain(f32),
    Playing(bool),
    Stepped(u64),
    WavPlaying(bool),
}

struct Pending {
    id: u64,
    observation: Observation,
    deadline: Instant,
}

enum AfterPublish {
    Quit,
    Release,
}

pub(crate) struct Bridge {
    directory: PathBuf,
    next_poll: Instant,
    last_command: Option<u64>,
    command_error: Option<String>,
    attached: bool,
    pending: Option<Pending>,
    after_publish: Option<AfterPublish>,
}

impl Bridge {
    pub(crate) fn from_env() -> Option<Self> {
        let directory = PathBuf::from(std::env::var_os("WOODSHED_TRANSPORT_DIR")?);
        if !directory.is_absolute() || !directory.is_dir() {
            eprintln!("[woodshed] WOODSHED_TRANSPORT_DIR must name an existing absolute directory");
            return None;
        }
        Some(Self {
            directory,
            next_poll: Instant::now(),
            last_command: None,
            command_error: None,
            attached: false,
            pending: None,
            after_publish: None,
        })
    }

    /// Return true once the acknowledged release ends mailbox ownership.
    pub(crate) fn poll(&mut self, shared: &mut Shared, ctx: &mut Ctx<'_>) -> bool {
        let now = Instant::now();
        if now < self.next_poll {
            return false;
        }
        self.next_poll = now + STATUS_INTERVAL;
        if let Some(backend) = shared.backend.as_mut() {
            backend.set_transport_bridge_active(true);
        }
        if self.after_publish.is_none() && self.pending.is_none() {
            match read_command(&self.directory.join("command.json")) {
                Ok(Some(command)) if self.last_command.is_none_or(|last| last < command.id) => {
                    self.command_error = None;
                    match self.apply(&command, shared, ctx) {
                        Ok(Some(observation)) => {
                            self.pending = Some(Pending {
                                id: command.id,
                                observation,
                                deadline: now + Duration::from_secs(5),
                            });
                            if let Some(backend) = shared.backend.as_mut() {
                                backend.service();
                            }
                        },
                        Ok(None) => self.last_command = Some(command.id),
                        Err(error) => {
                            self.command_error = Some(error);
                            self.last_command = Some(command.id);
                        },
                    }
                },
                Ok(_) => {},
                Err(error) => self.command_error = Some(error.to_string()),
            }
        }
        let backend = shared.backend.as_ref();
        let output = backend.and_then(CpalBackend::output_snapshot);
        if let Some(pending) = &self.pending {
            if backend.is_none_or(|backend| backend.output_stream_count() != 1) {
                self.command_error =
                    Some("Native audio output lost during transport change".into());
            }
            let applied = match pending.observation {
                Observation::WavPlaying(playing) => backend
                    .and_then(CpalBackend::listening_wav_observation)
                    .is_some_and(|wav| wav.playing == playing && wav.region_frames > 0),
                Observation::Gain(gain) => {
                    output.is_some_and(|output| output.output_gain_bits == gain.to_bits())
                },
                Observation::Playing(playing) => {
                    output.is_some_and(|output| output.playing == playing)
                },
                Observation::Stepped(revision) => {
                    output.is_some_and(|output| output.transport_revision != revision)
                },
            };
            if !applied && now >= pending.deadline && self.command_error.is_none() {
                self.command_error =
                    Some("Native audio did not confirm the transport change".into());
            }
            if applied || self.command_error.is_some() {
                self.last_command = Some(pending.id);
                self.pending = None;
            }
        }
        if self.publish(backend, output.unwrap_or_default()).is_ok() {
            match self.after_publish.take() {
                Some(AfterPublish::Quit) => *ctx.close = true,
                Some(AfterPublish::Release) => {
                    if let Some(backend) = shared.backend.as_mut() {
                        backend.set_transport_bridge_active(false);
                    }
                    return true;
                },
                None => {},
            }
        }
        false
    }

    fn apply(
        &mut self,
        command: &Command,
        shared: &mut Shared,
        ctx: &mut Ctx<'_>,
    ) -> Result<Option<Observation>, String> {
        match command.action.as_str() {
            "attach" => {
                ctx.window_commands.minimize();
                self.attached = true;
            },
            "detach" => {
                ctx.window_commands.show();
                self.attached = false;
            },
            "quiet" => {
                let backend = native_backend(shared)?;
                let quiet = backend
                    .output_snapshot()
                    .is_some_and(|output| output.output_gain_bits == QUIET_GAIN.to_bits());
                let gain = if quiet { 1.0 } else { QUIET_GAIN };
                backend.set_output_gain(gain)?;
                return Ok(Some(Observation::Gain(gain)));
            },
            "gain" => {
                let percent = command
                    .value
                    .filter(|value| (0..=100).contains(value))
                    .ok_or("Gain requires an integer value from 0 to 100")?;
                let gain = percent as f32 / 100.0;
                native_backend(shared)?.set_output_gain(gain)?;
                return Ok(Some(Observation::Gain(gain)));
            },
            "previous" | "next" if ctx.runner.state().set.wav_phrase.is_some() => {
                return Err("The marked WAV loop has no backing bars.".into());
            },
            "previous" | "next" => {
                let backend = native_backend(shared)?;
                let revision = backend
                    .output_snapshot()
                    .ok_or("Audio output unavailable")?
                    .transport_revision;
                backend.step_backing_bar(command.action == "next")?;
                return Ok(Some(Observation::Stepped(revision)));
            },
            "play" | "pause" if ctx.runner.state().set.wav_phrase.is_some() => {
                let playing = command.action == "play";
                crate::practice_wav::set_playing(shared, ctx, playing)?;
                return Ok(wav_playing_observation(shared, playing));
            },
            "play" | "pause" => {
                native_backend(shared)?;
                let playing = command.action == "play";
                set_playing(shared, ctx, playing)?;
                return Ok(Some(Observation::Playing(playing)));
            },
            "import-practice" => crate::practice_clip::import(&self.directory, shared, ctx)?,
            "inspect-practice" => crate::practice_clip::inspect(&self.directory, shared, ctx)?,
            "import-listening" => {
                crate::practice_clip::import_listening(&self.directory, shared, ctx)?
            },
            "inspect-listening" => {
                crate::practice_clip::inspect_listening(&self.directory, shared, ctx)?
            },
            "import-wav" => crate::practice_wav::import(&self.directory, shared, ctx)?,
            "inspect-wav" => crate::practice_wav::inspect(&self.directory, shared, ctx)?,
            "play-wav" | "pause-wav" => {
                let playing = command.action == "play-wav";
                crate::practice_wav::set_playing(shared, ctx, playing)?;
                return Ok(wav_playing_observation(shared, playing));
            },
            "play-practice" => {
                native_backend(shared)?;
                crate::practice_clip::play(shared, ctx)?;
            },
            "quit" => self.after_publish = Some(AfterPublish::Quit),
            "release" if !self.attached => self.after_publish = Some(AfterPublish::Release),
            "release" => {
                return Err("Return transport to Woodshed before releasing the bridge".into());
            },
            action => return Err(format!("Unknown transport action: {action}")),
        }
        Ok(None)
    }

    fn publish(
        &self,
        backend: Option<&CpalBackend>,
        output: woodshed_audio::OutputSnapshot,
    ) -> io::Result<()> {
        let wav = backend
            .and_then(CpalBackend::listening_wav_observation)
            .map(|wav| {
                json!({
                    "stream_epoch": wav.stream_epoch,
                    "device_frames": wav.device_frames,
                    "region_frames": wav.region_frames,
                    "region_cursor": wav.region_cursor,
                    "completed_wraps": wav.completed_wraps,
                    "playing": wav.playing,
                    "sample_rate": wav.sample_rate,
                })
            });
        let status = json!({
            "pid": std::process::id(),
            "stream_id": backend.and_then(CpalBackend::output_observation_id),
            "streams": backend.map_or(0, CpalBackend::output_stream_count),
            "device_frames": output.device_frames,
            "sample_rate": backend.and_then(CpalBackend::output_sample_rate),
            "device_errors": backend.and_then(CpalBackend::output_device_error_count),
            "bar": output.cursor.bar_idx,
            "bar_count": backend.map_or(0, CpalBackend::backing_bar_count),
            "sample_in_bar": output.cursor.sample_in_bar,
            "playing": output.playing,
            "transport_revision": output.transport_revision,
            "audio_error": backend.and_then(|backend| backend.error()),
            "music_practice": {
                "program_status": format!("{:?}", output.program.status),
                "program_revision": output.program.revision,
                "bar": output.program.card_index,
                "event": output.program.event_index,
                "beat_ticks": output.program.beat_ticks,
                "bpm": f64::from_bits(output.program.bpm_bits),
                "run_epoch": output.program.epoch.0,
            },
            "wav_loop": wav,
            "view": if self.attached { "pane" } else { "window" },
            "gain": (f32::from_bits(output.output_gain_bits) * 100.0).round() as u8,
            "quiet": output.output_gain_bits == QUIET_GAIN.to_bits(),
            "last_command": self.last_command,
            "command_error": self.command_error,
        });
        let mut temporary = tempfile::NamedTempFile::new_in(&self.directory)?;
        serde_json::to_writer(&mut temporary, &status)?;
        temporary.flush()?;
        temporary.persist(self.directory.join("status.json"))?;
        Ok(())
    }
}

fn wav_playing_observation(shared: &Shared, playing: bool) -> Option<Observation> {
    (playing || shared.wav_practice.is_loaded()).then_some(Observation::WavPlaying(playing))
}

fn read_command(path: &Path) -> io::Result<Option<Command>> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let command = serde_json::from_slice(&bytes);
    std::fs::remove_file(path)?;
    Ok(Some(command?))
}

fn native_backend(shared: &mut Shared) -> Result<&mut CpalBackend, String> {
    shared
        .backend
        .as_mut()
        .filter(|backend| backend.output_stream_count() == 1)
        .ok_or_else(|| "Transport requires a native audio output stream".into())
}

fn set_playing(shared: &mut Shared, ctx: &mut Ctx<'_>, playing: bool) -> Result<(), String> {
    ctx.runner.update(|ui| {
        // An empty isolated practice starts with two real Backing bars.
        if playing && ui.song.is_empty() {
            let first = SongBar::default();
            let mut second = SongBar {
                root_pc: 7,
                ..first.clone()
            };
            second.revoice();
            ui.song = SongDoc {
                name: "Transport Loop".into(),
                bars: vec![first, second],
                click: false,
                ..SongDoc::default()
            };
        }
        ui.song_playing = playing;
    });
    crate::sync::after_dispatch(shared, ctx);
    if native_backend(shared)?.song_playing() == playing {
        Ok(())
    } else {
        Err("The native backing rejected the play state".into())
    }
}
