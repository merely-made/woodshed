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
    pending_quiet: Option<(u64, f32)>,
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
            pending_quiet: None,
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
        if self.after_publish.is_none() && self.pending_quiet.is_none() {
            match read_command(&self.directory.join("command.json")) {
                Ok(Some(command)) if self.last_command.is_none_or(|last| last < command.id) => {
                    self.command_error = self.apply(&command, shared, ctx).err();
                    if self.pending_quiet.is_none() {
                        self.last_command = Some(command.id);
                    }
                },
                Ok(_) => {},
                Err(error) => self.command_error = Some(error.to_string()),
            }
        }
        let backend = shared.backend.as_ref();
        if let Some((id, gain)) = self.pending_quiet {
            if backend.is_none_or(|backend| backend.output_stream_count() != 1) {
                self.command_error = Some("Native audio output lost during gain change".into());
            }
            let applied = backend
                .and_then(CpalBackend::output_snapshot)
                .is_some_and(|output| output.output_gain_bits == gain.to_bits());
            if applied || self.command_error.is_some() {
                self.last_command = Some(id);
                self.pending_quiet = None;
            }
        }
        if self.publish(backend).is_ok() {
            match self.after_publish.take() {
                Some(AfterPublish::Quit) => *ctx.close = true,
                Some(AfterPublish::Release) => return true,
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
    ) -> Result<(), String> {
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
                let backend = shared.backend.as_mut().ok_or("Audio output unavailable")?;
                let quiet = backend
                    .output_snapshot()
                    .is_some_and(|output| output.output_gain_bits == QUIET_GAIN.to_bits());
                let gain = if quiet { 1.0 } else { QUIET_GAIN };
                backend.set_output_gain(gain)?;
                self.pending_quiet = Some((command.id, gain));
            },
            "play" => play(shared, ctx)?,
            "quit" => self.after_publish = Some(AfterPublish::Quit),
            "release" if !self.attached => self.after_publish = Some(AfterPublish::Release),
            "release" => {
                return Err("Return transport to Woodshed before releasing the bridge".into());
            },
            action => return Err(format!("Unknown transport action: {action}")),
        }
        Ok(())
    }

    fn publish(&self, backend: Option<&CpalBackend>) -> io::Result<()> {
        let output = backend
            .and_then(CpalBackend::output_snapshot)
            .unwrap_or_default();
        let status = json!({
            "pid": std::process::id(),
            "stream_id": backend.and_then(CpalBackend::output_observation_id),
            "streams": backend.map_or(0, CpalBackend::output_stream_count),
            "device_frames": output.device_frames,
            "sample_rate": backend.and_then(CpalBackend::output_sample_rate),
            "device_errors": backend.and_then(CpalBackend::output_device_error_count),
            "bar": output.cursor.bar_idx,
            "sample_in_bar": output.cursor.sample_in_bar,
            "playing": output.playing,
            "transport_revision": output.transport_revision,
            "audio_error": backend.and_then(|backend| backend.error()),
            "view": if self.attached { "pane" } else { "window" },
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

fn play(shared: &mut Shared, ctx: &mut Ctx<'_>) -> Result<(), String> {
    if !shared
        .backend
        .as_ref()
        .is_some_and(|backend| backend.output_stream_count() == 1)
    {
        return Err("Play requires a native audio output stream".into());
    }
    ctx.runner.update(|ui| {
        // An empty isolated practice starts with two real Backing bars.
        if ui.song.is_empty() {
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
        ui.song_playing = true;
    });
    crate::sync::after_dispatch(shared, ctx);
    if shared
        .backend
        .as_ref()
        .is_some_and(CpalBackend::song_playing)
    {
        Ok(())
    } else {
        Err("The native backing did not start".into())
    }
}
