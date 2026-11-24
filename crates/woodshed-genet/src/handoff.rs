//! The native owner of a Woodshed app-data directory handles local activation
//! and Turnstone's practice commands on its existing window and audio lane.

use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc::{self, Receiver, SyncSender},
};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use woodshedding::rehearsal::WavPhraseSource;

use crate::{Shared, audio::CpalBackend, sync::Ctx};

const ENDPOINT: &str = "turnstone-handoff.json";
const MAX_FRAME: u64 = 16 * 1024;
const COMMAND_TIMEOUT: Duration = Duration::from_secs(12);
const ACTIVATION_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_PEERS: usize = 8;

#[derive(Serialize, Deserialize)]
struct Address {
    version: u8,
    pid: u32,
    port: u16,
    token: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum Command {
    ImportWav {
        source: WavPhraseSource,
        wav_path: PathBuf,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        preview_path: Option<PathBuf>,
    },
    ImportPagePhrase {
        source: WavPhraseSource,
        phrase_path: PathBuf,
    },
    Status,
    Attach,
    Detach,
    Play,
    Pause,
    Gain {
        value: u8,
    },
    Quiet,
    Release,
    Activate,
    OpenWav {
        path: PathBuf,
    },
    OpenListening {
        path: PathBuf,
    },
}

impl Command {
    fn is_import(&self) -> bool {
        matches!(self, Self::ImportWav { .. } | Self::ImportPagePhrase { .. })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    token: String,
    command: Command,
}

#[derive(Serialize, Deserialize)]
struct Response {
    token: String,
    ok: bool,
    error: Option<String>,
    status: Option<Value>,
}

pub(crate) struct Owner {
    _lock: File,
    endpoint: PathBuf,
}

pub(crate) fn acquire(root: &Path) -> io::Result<Option<Owner>> {
    std::fs::create_dir_all(root)?;
    // This file remains in place. The OS releases the exclusive lock when
    // the writer dies, so newcomers never guess ownership from a PID or age.
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join("instance.lock"))?;
    match file.try_lock() {
        Ok(()) => {
            let endpoint = root.join(ENDPOINT);
            match std::fs::remove_file(&endpoint) {
                Ok(()) => {},
                Err(error) if error.kind() == io::ErrorKind::NotFound => {},
                Err(error) => return Err(error),
            }
            Ok(Some(Owner {
                _lock: file,
                endpoint,
            }))
        },
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(error)) => Err(error),
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.endpoint);
    }
}

pub(crate) fn claim_or_forward(root: &Path, command: Command) -> Result<Option<Owner>, String> {
    let deadline = Instant::now() + ACTIVATION_TIMEOUT;
    loop {
        if let Some(owner) = acquire(root).map_err(|error| error.to_string())? {
            return Ok(Some(owner));
        }
        match forward_once(root, &command) {
            Ok(response) if response.ok => return Ok(None),
            Ok(response) => {
                return Err(response
                    .error
                    .unwrap_or_else(|| "Activation failed.".into()));
            },
            Err(error) if Instant::now() >= deadline => {
                return Err(format!("Could not contact the running Woodshed: {error}"));
            },
            Err(_) => std::thread::sleep(Duration::from_millis(80)),
        }
    }
}

fn forward_once(root: &Path, command: &Command) -> io::Result<Response> {
    let bytes = std::fs::read(root.join(ENDPOINT))?;
    let address: Address = serde_json::from_slice(&bytes)?;
    if address.version != 1 {
        return Err(io::Error::other(
            "The running Woodshed uses an incompatible handoff version.",
        ));
    }
    let mut stream = TcpStream::connect_timeout(
        &(Ipv4Addr::LOCALHOST, address.port).into(),
        Duration::from_millis(300),
    )?;
    stream.set_nodelay(true)?;
    stream.set_read_timeout(Some(COMMAND_TIMEOUT))?;
    stream.set_write_timeout(Some(COMMAND_TIMEOUT))?;
    write_frame(
        &mut stream,
        &json!({"token": address.token, "command": command}),
    )?;
    let response: Response = read_frame(&mut BufReader::new(stream))?
        .ok_or_else(|| io::Error::other("Woodshed closed without answering."))?;
    if response.token != address.token {
        return Err(io::Error::other("Woodshed handoff identity changed."));
    }
    Ok(response)
}

fn read_frame<T: serde::de::DeserializeOwned>(reader: &mut impl BufRead) -> io::Result<Option<T>> {
    let mut bytes = Vec::new();
    Read::take(reader, MAX_FRAME + 1).read_until(b'\n', &mut bytes)?;
    if bytes.is_empty() {
        return Ok(None);
    }
    if bytes.len() as u64 > MAX_FRAME || bytes.last() != Some(&b'\n') {
        return Err(io::Error::other("Invalid Woodshed handoff frame."));
    }
    serde_json::from_slice(&bytes).map(Some).map_err(Into::into)
}

fn write_frame(stream: &mut TcpStream, frame: &impl Serialize) -> io::Result<()> {
    let mut bytes = serde_json::to_vec(frame)?;
    bytes.push(b'\n');
    if bytes.len() as u64 > MAX_FRAME {
        return Err(io::Error::other(
            "Woodshed handoff frame exceeds its size limit.",
        ));
    }
    stream.write_all(&bytes)
}

struct Packet {
    peer: usize,
    command: Command,
    alive: Arc<AtomicBool>,
    response: SyncSender<Result<Value, String>>,
}

#[derive(Default)]
pub(crate) struct Service {
    receiver: Option<Receiver<Packet>>,
    lease: Option<Lease>,
    pending_start: Option<(PathBuf, Arc<dyn Fn() + Send + Sync>)>,
}

struct Lease {
    peer: usize,
    alive: Arc<AtomicBool>,
    session: Arc<()>,
    owner: woodshed_core::working_sets::WorkingSetId,
    attached: bool,
}

impl Lease {
    fn permits(&self, peer: usize, command: &Command) -> bool {
        self.peer == peer || (!self.attached && command.is_import())
    }

    fn new(peer: usize, alive: Arc<AtomicBool>, ctx: &Ctx<'_>, attached: bool) -> Self {
        let ui = ctx.runner.state();
        Self {
            peer,
            alive,
            session: ui.audio_session.clone(),
            owner: ui.working_sets.active_id,
            attached,
        }
    }

    fn current(&self, ctx: &Ctx<'_>) -> bool {
        let ui = ctx.runner.state();
        self.alive.load(Ordering::Acquire)
            && Arc::ptr_eq(&self.session, &ui.audio_session)
            && self.owner == ui.working_sets.active_id
            && ui.set.wav_phrase.is_some()
    }
}

impl Service {
    pub(crate) fn prepare(&mut self, owner: &Owner, wake: Arc<dyn Fn() + Send + Sync>) {
        self.pending_start = Some((owner.endpoint.clone(), wake));
    }

    pub(crate) fn publish_ready(&mut self) -> Result<(), String> {
        let Some((endpoint, wake)) = self.pending_start.take() else {
            return Ok(());
        };
        self.start(&endpoint, wake)
    }

    fn start(&mut self, endpoint: &Path, wake: Arc<dyn Fn() + Send + Sync>) -> Result<(), String> {
        let (sender, receiver) = mpsc::sync_channel(MAX_PEERS);
        let listener =
            TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).map_err(|error| error.to_string())?;
        let mut secret = [0; 32];
        getrandom::fill(&mut secret).map_err(|error| error.to_string())?;
        let address = Address {
            version: 1,
            pid: std::process::id(),
            port: listener
                .local_addr()
                .map_err(|error| error.to_string())?
                .port(),
            token: blake3::Hash::from_bytes(secret).to_hex().to_string(),
        };
        let mut file = tempfile::NamedTempFile::new_in(endpoint.parent().unwrap())
            .map_err(|error| error.to_string())?;
        serde_json::to_writer(&mut file, &address).map_err(|error| error.to_string())?;
        file.flush().map_err(|error| error.to_string())?;
        file.persist(endpoint).map_err(|error| error.to_string())?;
        self.receiver = Some(receiver);
        std::thread::Builder::new()
            .name("woodshed-handoff".into())
            .spawn(move || {
                let peers = Arc::new(AtomicUsize::new(0));
                let mut next_peer = 0;
                for stream in listener.incoming() {
                    let Ok(stream) = stream else { break };
                    if peers.load(Ordering::Relaxed) >= MAX_PEERS {
                        continue;
                    }
                    next_peer += 1;
                    let peer = next_peer;
                    let peers = peers.clone();
                    let sender = sender.clone();
                    let wake = wake.clone();
                    let token = address.token.clone();
                    peers.fetch_add(1, Ordering::Relaxed);
                    std::thread::spawn(move || {
                        let _ = run_peer(stream, peer, &token, &sender, wake.as_ref());
                        peers.fetch_sub(1, Ordering::Relaxed);
                    });
                }
            })
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub(crate) fn poll(&mut self, shared: &mut Shared, ctx: &mut Ctx<'_>) {
        if self.lease.as_ref().is_some_and(|lease| !lease.current(ctx)) {
            self.restore(ctx);
        }
        for _ in 0..MAX_PEERS {
            let Some(packet) = self
                .receiver
                .as_ref()
                .and_then(|receiver| receiver.try_recv().ok())
            else {
                break;
            };
            if packet.alive.load(Ordering::Acquire) {
                let result = self.apply(packet.peer, packet.alive, packet.command, shared, ctx);
                let _ = packet.response.send(result);
            }
        }
    }

    fn restore(&mut self, ctx: &mut Ctx<'_>) {
        self.lease = None;
        ctx.window_commands.show();
    }

    fn apply(
        &mut self,
        peer: usize,
        alive: Arc<AtomicBool>,
        command: Command,
        shared: &mut Shared,
        ctx: &mut Ctx<'_>,
    ) -> Result<Value, String> {
        match command {
            Command::Status => {},
            Command::Activate => self.restore(ctx),
            Command::Release | Command::Detach => {
                if self.lease.as_ref().is_some_and(|lease| lease.peer != peer) {
                    return Err("Another Turnstone window owns the Woodshed pane.".into());
                }
                if matches!(command, Command::Release) {
                    self.restore(ctx);
                } else {
                    if let Some(lease) = self.lease.as_mut() {
                        lease.attached = false;
                    }
                    ctx.window_commands.show();
                }
            },
            command => self.practice(peer, alive, command, shared, ctx)?,
        }
        Ok(status(
            shared,
            ctx,
            self.lease.as_ref().is_some_and(|lease| lease.attached),
        ))
    }

    fn practice(
        &mut self,
        peer: usize,
        alive: Arc<AtomicBool>,
        command: Command,
        shared: &mut Shared,
        ctx: &mut Ctx<'_>,
    ) -> Result<(), String> {
        if self
            .lease
            .as_ref()
            .is_some_and(|lease| !lease.permits(peer, &command))
        {
            return Err("Another Turnstone window owns the Woodshed pane.".into());
        }
        crate::practice_clip::require_sealed_store(shared)?;
        if !shared
            .store_binding
            .as_ref()
            .is_some_and(crate::storage::StoreBinding::is_unlocked)
        {
            return Err("Unlock your Woodshed persona before sending a recording.".into());
        }
        if ctx.runner.state().pending_persona_switch.is_some()
            || ctx.runner.state().persona.is_some()
            || ctx.runner.state().practice_close_pending
        {
            return Err("Finish opening your Woodshed practice before sending a recording.".into());
        }
        let importing = command.is_import();
        if importing
            && !shared
                .backend
                .as_ref()
                .is_some_and(|backend| backend.output_stream_count() == 1)
        {
            return Err("Woodshed has no native audio output stream.".into());
        }
        match command {
            Command::ImportWav {
                source,
                wav_path,
                preview_path,
            } => {
                crate::practice_wav::import_region(
                    crate::practice_wav::Region::Recording {
                        path: &wav_path,
                        preview: preview_path.as_deref(),
                    },
                    source,
                    shared,
                    ctx,
                )?;
            },
            Command::ImportPagePhrase {
                source,
                phrase_path,
            } => {
                crate::practice_wav::import_region(
                    crate::practice_wav::Region::PagePhrase(&phrase_path),
                    source,
                    shared,
                    ctx,
                )?;
            },
            Command::Attach => {
                if ctx.runner.state().set.wav_phrase.is_none() {
                    return Err("Open a practice recording before attaching its controls.".into());
                }
                ctx.window_commands.minimize();
                self.lease = Some(Lease::new(peer, alive.clone(), ctx, true));
            },
            Command::Play | Command::Pause => {
                if self.lease.is_none() {
                    return Err("Reconnect to the active Woodshed recording.".into());
                }
                crate::practice_wav::set_playing(shared, ctx, matches!(command, Command::Play))?;
            },
            Command::Quiet | Command::Gain { .. } => {
                if self.lease.is_none() {
                    return Err("Reconnect to the active Woodshed recording.".into());
                }
                let backend = shared
                    .backend
                    .as_mut()
                    .ok_or("Native audio is unavailable.")?;
                let gain = match command {
                    Command::Gain { value } if value <= 100 => f32::from(value) / 100.0,
                    Command::Gain { .. } => {
                        return Err("Woodshed volume must be between 0 and 100.".into());
                    },
                    _ if backend
                        .output_snapshot()
                        .is_some_and(|out| out.output_gain_bits == 0.15_f32.to_bits()) =>
                    {
                        1.0
                    },
                    _ => 0.15,
                };
                backend.set_output_gain(gain)?;
                backend.service();
            },
            Command::OpenWav { path } => {
                crate::practice_wav::open(&path, shared, ctx)?;
                self.restore(ctx);
            },
            Command::OpenListening { path } => {
                crate::practice_clip::open_listening(&path, shared, ctx)?;
                self.restore(ctx);
            },
            _ => unreachable!(),
        }
        if importing {
            crate::practice_wav::set_playing(shared, ctx, true)?;
            let attached = self.lease.as_ref().is_some_and(|lease| lease.attached);
            self.lease = Some(Lease::new(peer, alive, ctx, attached));
        }
        Ok(())
    }
}

fn run_peer(
    stream: TcpStream,
    peer: usize,
    token: &str,
    sender: &SyncSender<Packet>,
    wake: &(dyn Fn() + Send + Sync),
) -> io::Result<()> {
    let alive = Arc::new(AtomicBool::new(true));
    let outcome = serve(stream, peer, token, sender, wake, &alive);
    alive.store(false, Ordering::Release);
    wake();
    outcome
}

fn serve(
    stream: TcpStream,
    peer: usize,
    token: &str,
    sender: &SyncSender<Packet>,
    wake: &(dyn Fn() + Send + Sync),
    alive: &Arc<AtomicBool>,
) -> io::Result<()> {
    stream.set_nodelay(true)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(3)))?;
    // The read deadline belongs to this exact socket handle. Winsock can
    // retain the original receive timeout on a duplicated handle.
    let mut reader = BufReader::new(stream);
    let mut authenticated = false;
    while let Some(Request {
        token: proof,
        command,
    }) = read_frame(&mut reader)?
    {
        if proof != token {
            return Err(io::Error::other("Invalid Woodshed handoff token."));
        }
        if !authenticated {
            reader.get_ref().set_read_timeout(None)?;
            authenticated = true;
        }
        let (response, receive) = mpsc::sync_channel(1);
        sender
            .try_send(Packet {
                peer,
                command,
                alive: alive.clone(),
                response,
            })
            .map_err(|error| io::Error::other(error.to_string()))?;
        wake();
        let result = receive
            .recv_timeout(COMMAND_TIMEOUT)
            .map_err(|error| io::Error::other(error.to_string()))?;
        let response = match result {
            Ok(status) => Response {
                token: token.into(),
                ok: true,
                error: None,
                status: Some(status),
            },
            Err(error) => Response {
                token: token.into(),
                ok: false,
                error: Some(error),
                status: None,
            },
        };
        write_frame(reader.get_mut(), &response)?;
    }
    Ok(())
}

pub(super) fn local_file_path(path: &Path) -> bool {
    #[cfg(windows)]
    {
        use std::path::{Component, Prefix};
        if !matches!(path.components().next(),
            Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)))
        {
            return false;
        }
    }
    path.is_absolute() && std::fs::metadata(path).is_ok_and(|metadata| metadata.is_file())
}

fn status(shared: &Shared, ctx: &Ctx<'_>, attached: bool) -> Value {
    let backend = shared.backend.as_ref();
    let output = backend.and_then(CpalBackend::output_snapshot);
    let gain = output.map_or(1.0, |sample| f32::from_bits(sample.output_gain_bits));
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
    json!({
        "pid": std::process::id(),
        "stream_id": backend.and_then(CpalBackend::output_observation_id),
        "streams": backend.map_or(0, CpalBackend::output_stream_count),
        "view": if attached { "pane" } else { "window" },
        "wav_selected": ctx.runner.state().set.wav_phrase.is_some(),
        "wav_loop": wav,
        "gain": (gain * 100.0).round() as u8,
        "quiet": gain.to_bits() == 0.15_f32.to_bits(),
    })
}

#[cfg(test)]
mod tests;
