#![forbid(unsafe_code)]

//! Host-owned native playback for Redshank.
//!
//! The UI sends commands through [`PlaybackRuntime`]. A dedicated worker owns
//! the sole Firewheel/CPAL context and performs file I/O and Symphonia decoding.
//! Its snapshots are derived from accepted source frames less the output queue,
//! never from a UI timer.

use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    num::NonZeroU32,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow, bail};
use firewheel::{
    FirewheelConfig, FirewheelContext,
    channel_config::{ChannelCount, NonZeroChannelCount},
    cpal::CpalConfig,
    node::NodeID,
    nodes::stream::{
        ResamplingChannelConfig,
        writer::{PushStatus, StreamWriterConfig, StreamWriterNode, StreamWriterState},
    },
};
use redshank_model::{MediaSource, RepresentationReceipt};
use symphonia::core::{
    audio::SampleBuffer,
    codecs::DecoderOptions,
    errors::Error as SymphoniaError,
    formats::{FormatOptions, SeekMode, SeekTo},
    io::MediaSourceStream,
    meta::MetadataOptions,
    probe::Hint,
    units::Time,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlaybackState {
    Empty,
    Loading,
    Paused,
    Playing,
    Ended,
    Unavailable(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlaybackSnapshot {
    /// Host-supplied identity for the loaded representation. Hosts must ignore
    /// snapshots whose token is not their currently selected item.
    pub load_token: Option<u64>,
    pub representation: Option<RepresentationReceipt>,
    pub state: PlaybackState,
    pub position_ms: u64,
    pub duration_ms: Option<u64>,
    pub source: Option<String>,
}

impl Default for PlaybackSnapshot {
    fn default() -> Self {
        Self {
            load_token: None,
            representation: None,
            state: PlaybackState::Empty,
            position_ms: 0,
            duration_ms: None,
            source: None,
        }
    }
}

#[derive(Clone, Debug)]
pub enum PlaybackCommand {
    Load {
        token: u64,
        source: MediaSource,
        resume_ms: u64,
    },
    Play,
    Pause,
    Stop,
    Seek(u64),
    Shutdown,
}

type Wake = Arc<dyn Fn() + Send + Sync>;
struct SnapshotCell {
    value: PlaybackSnapshot,
    wake: Option<Wake>,
    last_wake: Instant,
}
struct RuntimeInner {
    commands: mpsc::Sender<PlaybackCommand>,
    snapshot: Arc<Mutex<SnapshotCell>>,
}
impl Drop for RuntimeInner {
    fn drop(&mut self) {
        let _ = self.commands.send(PlaybackCommand::Shutdown);
    }
}
#[derive(Clone)]
pub struct PlaybackRuntime {
    inner: Arc<RuntimeInner>,
}

impl PlaybackRuntime {
    pub fn start() -> Self {
        let (sender, receiver) = mpsc::channel();
        let snapshot = Arc::new(Mutex::new(SnapshotCell {
            value: PlaybackSnapshot::default(),
            wake: None,
            last_wake: Instant::now(),
        }));
        let worker_snapshot = Arc::clone(&snapshot);
        thread::Builder::new()
            .name("redshank-playback".into())
            .spawn(move || worker(receiver, worker_snapshot))
            .expect("spawn Redshank playback worker");
        Self {
            inner: Arc::new(RuntimeInner {
                commands: sender,
                snapshot,
            }),
        }
    }
    pub fn command(&self, command: PlaybackCommand) -> Result<()> {
        self.inner
            .commands
            .send(command)
            .map_err(|_| anyhow!("Redshank playback runtime stopped"))
    }
    pub fn snapshot(&self) -> PlaybackSnapshot {
        self.inner
            .snapshot
            .lock()
            .map(|value| value.value.clone())
            .unwrap_or_else(|_| PlaybackSnapshot {
                state: PlaybackState::Unavailable("playback snapshot lock poisoned".into()),
                ..PlaybackSnapshot::default()
            })
    }
    pub fn set_wake(&self, wake: Wake) {
        if let Ok(mut snapshot) = self.inner.snapshot.lock() {
            snapshot.wake = Some(wake);
        }
    }
}

struct Sink {
    state: Arc<Mutex<StreamWriterState>>,
    node: NodeID,
    accepted_frames: u64,
    rate: u32,
    channels: usize,
}
fn presented_ms(accepted_frames: u64, rate: u32, queued_seconds: f64) -> u64 {
    ((((accepted_frames as f64 / rate as f64) - queued_seconds).max(0.0)) * 1000.0) as u64
}
fn consume_frames(pending: &mut Vec<f32>, accepted_frames: usize, channels: usize) {
    pending.drain(..accepted_frames.saturating_mul(channels).min(pending.len()));
}
fn drained(eof: bool, pending: &[f32], queued_seconds: f64) -> bool {
    eof && pending.is_empty() && queued_seconds <= 0.005
}
impl Sink {
    fn push(&mut self, samples: &[f32]) -> Result<usize> {
        let status = self
            .state
            .lock()
            .map_err(|_| anyhow!("audio writer lock poisoned"))?
            .push_interleaved(samples);
        let accepted = match status {
            PushStatus::Ok | PushStatus::UnderflowCorrected { .. } => samples.len() / self.channels,
            PushStatus::OutputNotReady => 0,
            PushStatus::OverflowOccurred { num_frames_pushed } => num_frames_pushed,
        };
        self.accepted_frames += accepted as u64;
        Ok(accepted)
    }
    fn presented_ms(&self) -> Result<u64> {
        let queued = self
            .state
            .lock()
            .map_err(|_| anyhow!("audio writer lock poisoned"))?
            .occupied_seconds()
            .unwrap_or(0.0);
        Ok(presented_ms(self.accepted_frames, self.rate, queued))
    }
    fn pause(&self) -> Result<()> {
        self.state
            .lock()
            .map_err(|_| anyhow!("audio writer lock poisoned"))?
            .pause_stream();
        Ok(())
    }
    fn resume(&self) -> Result<()> {
        self.state
            .lock()
            .map_err(|_| anyhow!("audio writer lock poisoned"))?
            .resume();
        Ok(())
    }
    fn stop(&self) -> Result<()> {
        self.state
            .lock()
            .map_err(|_| anyhow!("audio writer lock poisoned"))?
            .stop_stream();
        Ok(())
    }
}

struct AudioRuntime {
    context: FirewheelContext,
    output_channels: u32,
}
impl AudioRuntime {
    fn new() -> Result<Self> {
        let mut context = FirewheelContext::new(FirewheelConfig {
            num_graph_inputs: ChannelCount::ZERO,
            ..Default::default()
        });
        context
            .start_stream(CpalConfig::default())
            .map_err(|error| anyhow!("could not start the default audio output: {error:?}"))?;
        let output_channels = context
            .stream_info()
            .context("Firewheel did not report output stream information")?
            .num_stream_out_channels;
        Ok(Self {
            context,
            output_channels,
        })
    }
    fn sink(&mut self, source_rate: u32, source_channels: u32) -> Result<Sink> {
        let source_rate =
            NonZeroU32::new(source_rate).context("decoded stream has zero sample rate")?;
        let channels = NonZeroChannelCount::new(source_channels)
            .context("decoded audio has an unsupported channel count")?;
        if source_channels > self.output_channels {
            bail!(
                "decoded audio has {source_channels} channels but the output has only {}",
                self.output_channels
            );
        }
        let writer = self.context.add_node(
            StreamWriterNode,
            Some(StreamWriterConfig {
                channels,
                check_for_silence: true,
            }),
        );
        let edges: Vec<_> = if source_channels == 1 {
            (0..self.output_channels)
                .map(|channel| (0, channel))
                .collect()
        } else {
            (0..source_channels)
                .map(|channel| (channel, channel))
                .collect()
        };
        self.context
            .connect(writer, self.context.graph_out_node_id(), &edges, false)
            .map_err(|error| anyhow!("could not connect audio graph: {error:?}"))?;
        let mut state = self
            .context
            .node_state::<StreamWriterState>(writer)
            .context("Firewheel stream writer missing")?
            .clone();
        let event = state
            .start_stream(
                source_rate,
                self.context
                    .stream_info()
                    .context("Firewheel output stream unavailable")?
                    .sample_rate,
                ResamplingChannelConfig {
                    latency_seconds: 0.08,
                    capacity_seconds: 0.20,
                    ..Default::default()
                },
            )
            .map_err(|_| anyhow!("could not start decoded audio stream"))?;
        self.context.queue_event_for(writer, event.into());
        Ok(Sink {
            state: Arc::new(Mutex::new(state)),
            node: writer,
            accepted_frames: 0,
            rate: source_rate.get(),
            channels: source_channels as usize,
        })
    }
    fn remove_sink(&mut self, sink: Sink) -> Result<()> {
        let stop = sink.stop();
        let remove = self
            .context
            .remove_node(sink.node)
            .map(|_| ())
            .map_err(|error| anyhow!("could not remove retired audio node: {error:?}"));

        match (stop, remove) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(stop), Ok(())) => Err(stop),
            (Ok(()), Err(remove)) => Err(remove),
            (Err(stop), Err(remove)) => Err(anyhow!(
                "could not stop retired audio: {stop}; additionally {remove}"
            )),
        }
    }
    fn tick(&mut self) -> Result<()> {
        self.context
            .update()
            .map_err(|error| anyhow!("audio runtime update failed: {error:?}"))
    }
}

struct Decoder {
    format: Box<dyn symphonia::core::formats::FormatReader>,
    decoder: Box<dyn symphonia::core::codecs::Decoder>,
    track_id: u32,
    sample_buffer: Option<SampleBuffer<f32>>,
    rate: u32,
    channels: Option<u32>,
    time_base: symphonia::core::units::TimeBase,
    duration_ms: Option<u64>,
}
fn open_local(path: &Path) -> Result<(Decoder, RepresentationReceipt)> {
    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|x| x.to_str()) {
        hint.with_extension(extension);
    }
    let mut file =
        File::open(path).with_context(|| format!("could not open {}", path.display()))?;
    let length = file.metadata().ok().map(|metadata| metadata.len());
    let mut digest = blake3::Hasher::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .context("could not hash local audio")?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    file.seek(SeekFrom::Start(0))
        .context("could not rewind local audio")?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            stream,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .context("unsupported or invalid audio")?;
    let track = probed
        .format
        .default_track()
        .context("audio file has no default audio track")?;
    let rate = track
        .codec_params
        .sample_rate
        .context("audio track has no sample rate")?;
    let time_base = track
        .codec_params
        .time_base
        .context("audio track has no time base")?;
    let duration_ms = track.codec_params.n_frames.map(|frames| {
        let time = time_base.calc_time(frames);
        time.seconds * 1000 + (time.frac * 1000.0) as u64
    });
    // M4A can omit a track-level channel layout even though the decoded frames
    // report one. Delay writer creation until that first frame arrives.
    let channels = track
        .codec_params
        .channels
        .map(|layout| layout.count() as u32);
    Ok((
        Decoder {
            decoder: symphonia::default::get_codecs()
                .make(&track.codec_params, &DecoderOptions::default())
                .context("could not create audio decoder")?,
            track_id: track.id,
            format: probed.format,
            sample_buffer: None,
            rate,
            channels,
            time_base,
            duration_ms,
        },
        RepresentationReceipt {
            byte_length: length,
            complete_digest: Some(format!("blake3:{}", digest.finalize().to_hex())),
            ..RepresentationReceipt::default()
        },
    ))
}
fn source_path(source: MediaSource) -> Result<PathBuf> {
    match source {
        MediaSource::Local { path } => Ok(path.into()),
        MediaSource::Enclosure { url } => {
            bail!("HTTP playback is not enabled in this bounded desktop runtime: {url}")
        },
        MediaSource::HostBlob { id } => bail!("host blob playback needs the embedding host: {id}"),
    }
}
fn publish(
    snapshot: &Arc<Mutex<SnapshotCell>>,
    token: Option<u64>,
    state: PlaybackState,
    position_ms: u64,
    duration_ms: Option<u64>,
    source: Option<String>,
) {
    let wake = snapshot.lock().ok().and_then(|mut cell| {
        let representation = if cell.value.load_token == token {
            cell.value.representation.clone()
        } else {
            None
        };
        let next = PlaybackSnapshot {
            load_token: token,
            representation,
            state,
            position_ms,
            duration_ms,
            source,
        };
        if cell.value == next {
            return None;
        }
        let position_only = cell.value.load_token == next.load_token
            && cell.value.representation == next.representation
            && cell.value.state == next.state
            && cell.value.duration_ms == next.duration_ms
            && cell.value.source == next.source;
        cell.value = next;
        if !position_only || cell.last_wake.elapsed() >= Duration::from_millis(100) {
            cell.last_wake = Instant::now();
            cell.wake.clone()
        } else {
            None
        }
    });
    if let Some(wake) = wake {
        wake();
    }
}

fn retire_sink(audio: &mut Option<AudioRuntime>, sink: &mut Option<Sink>) -> Result<()> {
    let Some(sink) = sink.take() else {
        return Ok(());
    };
    if let Some(audio) = audio.as_mut() {
        audio.remove_sink(sink)
    } else {
        sink.stop()
    }
}

#[allow(clippy::too_many_arguments)]
fn fail(
    audio: &mut Option<AudioRuntime>,
    decoder: &mut Option<Decoder>,
    sink: &mut Option<Sink>,
    pending: &mut Vec<f32>,
    eof: &mut bool,
    playing: &mut bool,
    source: &mut Option<String>,
    snapshot: &Arc<Mutex<SnapshotCell>>,
    token: Option<u64>,
    position_ms: u64,
    duration_ms: Option<u64>,
    error: impl std::fmt::Display,
) {
    *playing = false;
    *eof = false;
    pending.clear();
    *decoder = None;
    let source = source.take();
    let retired = retire_sink(audio, sink);
    // Fatal errors also discard partially constructed writers which never
    // reached `sink`. Dropping this worker's sole runtime guarantees silence
    // and releases those nodes before a later explicit load/play retries.
    *audio = None;
    let message = match retired {
        Ok(()) => error.to_string(),
        Err(retire) => format!("{error}; additionally failed to retire audio output: {retire}"),
    };
    publish(
        snapshot,
        token,
        PlaybackState::Unavailable(message),
        position_ms,
        duration_ms,
        source,
    );
}

fn worker(receiver: mpsc::Receiver<PlaybackCommand>, snapshot: Arc<Mutex<SnapshotCell>>) {
    let mut audio = None;
    let mut decoder = None;
    let mut sink = None;
    let mut pending = Vec::new();
    let mut eof = false;
    let mut source = None;
    let mut token = None;
    let mut playing = false;
    let mut base_ms = 0_u64;

    loop {
        while let Ok(command) = receiver.try_recv() {
            match command {
                PlaybackCommand::Shutdown => return,
                PlaybackCommand::Load {
                    token: next_token,
                    source: media,
                    resume_ms,
                } => {
                    token = Some(next_token);
                    if let Err(error) = retire_sink(&mut audio, &mut sink) {
                        fail(
                            &mut audio,
                            &mut decoder,
                            &mut sink,
                            &mut pending,
                            &mut eof,
                            &mut playing,
                            &mut source,
                            &snapshot,
                            token,
                            0,
                            None,
                            error,
                        );
                        continue;
                    }
                    pending.clear();
                    eof = false;
                    playing = false;
                    decoder = None;
                    source = None;
                    base_ms = 0;
                    publish(&snapshot, token, PlaybackState::Loading, 0, None, None);

                    match source_path(media).and_then(|path| {
                        open_local(&path).map(|(opened, receipt)| (path, opened, receipt))
                    }) {
                        Ok((path, mut opened, representation)) => {
                            let duration = opened.duration_ms;
                            let resumed = if resume_ms == 0 {
                                Ok(0)
                            } else {
                                opened
                                    .format
                                    .seek(
                                        SeekMode::Accurate,
                                        SeekTo::Time {
                                            time: Time::from(resume_ms as f64 / 1000.0),
                                            track_id: Some(opened.track_id),
                                        },
                                    )
                                    .map(|seeked| {
                                        opened.decoder.reset();
                                        let actual = opened.time_base.calc_time(seeked.actual_ts);
                                        actual.seconds * 1000 + (actual.frac * 1000.0) as u64
                                    })
                            };
                            match resumed {
                                Ok(resumed) => {
                                    source = Some(path.display().to_string());
                                    base_ms = resumed;
                                    decoder = Some(opened);
                                    if let Ok(mut value) = snapshot.lock() {
                                        value.value.representation = Some(representation);
                                    }
                                    publish(
                                        &snapshot,
                                        token,
                                        PlaybackState::Paused,
                                        resumed,
                                        duration,
                                        source.clone(),
                                    );
                                },
                                Err(error) => fail(
                                    &mut audio,
                                    &mut decoder,
                                    &mut sink,
                                    &mut pending,
                                    &mut eof,
                                    &mut playing,
                                    &mut source,
                                    &snapshot,
                                    token,
                                    0,
                                    duration,
                                    format!("resume seek unavailable: {error}"),
                                ),
                            }
                        },
                        Err(error) => fail(
                            &mut audio,
                            &mut decoder,
                            &mut sink,
                            &mut pending,
                            &mut eof,
                            &mut playing,
                            &mut source,
                            &snapshot,
                            token,
                            0,
                            None,
                            error,
                        ),
                    }
                },
                PlaybackCommand::Play => {
                    if decoder.is_some() && !eof {
                        let duration = decoder.as_ref().and_then(|active| active.duration_ms);
                        if let Some(active) = sink.as_ref()
                            && let Err(error) = active.resume()
                        {
                            fail(
                                &mut audio,
                                &mut decoder,
                                &mut sink,
                                &mut pending,
                                &mut eof,
                                &mut playing,
                                &mut source,
                                &snapshot,
                                token,
                                base_ms,
                                duration,
                                error,
                            );
                            continue;
                        }
                        playing = true;
                        let position = sink
                            .as_ref()
                            .and_then(|active| active.presented_ms().ok())
                            .unwrap_or(0)
                            .saturating_add(base_ms);
                        publish(
                            &snapshot,
                            token,
                            PlaybackState::Playing,
                            position,
                            duration,
                            source.clone(),
                        );
                    }
                },
                PlaybackCommand::Pause => {
                    let duration = decoder.as_ref().and_then(|active| active.duration_ms);
                    let position = sink
                        .as_ref()
                        .and_then(|active| active.presented_ms().ok())
                        .unwrap_or(0)
                        .saturating_add(base_ms);
                    if let Some(active) = sink.as_ref()
                        && let Err(error) = active.pause()
                    {
                        fail(
                            &mut audio,
                            &mut decoder,
                            &mut sink,
                            &mut pending,
                            &mut eof,
                            &mut playing,
                            &mut source,
                            &snapshot,
                            token,
                            position,
                            duration,
                            error,
                        );
                        continue;
                    }
                    playing = false;
                    publish(
                        &snapshot,
                        token,
                        PlaybackState::Paused,
                        position,
                        duration,
                        source.clone(),
                    );
                },
                PlaybackCommand::Stop => {
                    let duration = decoder.as_ref().and_then(|active| active.duration_ms);
                    if let Err(error) = retire_sink(&mut audio, &mut sink) {
                        fail(
                            &mut audio,
                            &mut decoder,
                            &mut sink,
                            &mut pending,
                            &mut eof,
                            &mut playing,
                            &mut source,
                            &snapshot,
                            token,
                            0,
                            duration,
                            error,
                        );
                        continue;
                    }
                    playing = false;
                    pending.clear();
                    eof = false;
                    base_ms = 0;
                    if let Some(active) = decoder.as_mut() {
                        if let Err(error) = active.format.seek(
                            SeekMode::Accurate,
                            SeekTo::Time {
                                time: Time::from(0.0),
                                track_id: Some(active.track_id),
                            },
                        ) {
                            fail(
                                &mut audio,
                                &mut decoder,
                                &mut sink,
                                &mut pending,
                                &mut eof,
                                &mut playing,
                                &mut source,
                                &snapshot,
                                token,
                                0,
                                duration,
                                format!("stop seek unavailable: {error}"),
                            );
                            continue;
                        }
                        active.decoder.reset();
                    }
                    publish(
                        &snapshot,
                        token,
                        PlaybackState::Paused,
                        0,
                        duration,
                        source.clone(),
                    );
                },
                PlaybackCommand::Seek(position) => {
                    let seek = decoder.as_mut().map(|active| {
                        let duration = active.duration_ms;
                        active
                            .format
                            .seek(
                                SeekMode::Accurate,
                                SeekTo::Time {
                                    time: Time::from(position as f64 / 1000.0),
                                    track_id: Some(active.track_id),
                                },
                            )
                            .map(|seeked| {
                                active.decoder.reset();
                                let actual = active.time_base.calc_time(seeked.actual_ts);
                                (
                                    actual.seconds * 1000 + (actual.frac * 1000.0) as u64,
                                    duration,
                                )
                            })
                    });
                    if let Some(seek) = seek {
                        match seek {
                            Ok((actual, duration)) => {
                                base_ms = actual;
                                pending.clear();
                                eof = false;
                                if let Err(error) = retire_sink(&mut audio, &mut sink) {
                                    fail(
                                        &mut audio,
                                        &mut decoder,
                                        &mut sink,
                                        &mut pending,
                                        &mut eof,
                                        &mut playing,
                                        &mut source,
                                        &snapshot,
                                        token,
                                        base_ms,
                                        duration,
                                        error,
                                    );
                                    continue;
                                }
                                publish(
                                    &snapshot,
                                    token,
                                    if playing {
                                        PlaybackState::Playing
                                    } else {
                                        PlaybackState::Paused
                                    },
                                    base_ms,
                                    duration,
                                    source.clone(),
                                );
                            },
                            Err(error) => {
                                let duration =
                                    decoder.as_ref().and_then(|active| active.duration_ms);
                                fail(
                                    &mut audio,
                                    &mut decoder,
                                    &mut sink,
                                    &mut pending,
                                    &mut eof,
                                    &mut playing,
                                    &mut source,
                                    &snapshot,
                                    token,
                                    base_ms,
                                    duration,
                                    format!("seek unavailable: {error}"),
                                );
                            },
                        }
                    }
                },
            }
        }

        if let Some(runtime) = audio.as_mut()
            && let Err(error) = runtime.tick()
        {
            let duration = decoder.as_ref().and_then(|active| active.duration_ms);
            fail(
                &mut audio,
                &mut decoder,
                &mut sink,
                &mut pending,
                &mut eof,
                &mut playing,
                &mut source,
                &snapshot,
                token,
                base_ms,
                duration,
                error,
            );
        }

        if playing {
            let Some(active) = decoder.as_mut() else {
                playing = false;
                continue;
            };
            let duration = active.duration_ms;
            if sink.is_none()
                && let Some(channels) = active.channels
            {
                if audio.is_none() {
                    match AudioRuntime::new() {
                        Ok(runtime) => audio = Some(runtime),
                        Err(error) => {
                            fail(
                                &mut audio,
                                &mut decoder,
                                &mut sink,
                                &mut pending,
                                &mut eof,
                                &mut playing,
                                &mut source,
                                &snapshot,
                                token,
                                base_ms,
                                duration,
                                error,
                            );
                            continue;
                        },
                    }
                }
                match audio
                    .as_mut()
                    .expect("audio runtime checked")
                    .sink(active.rate, channels)
                {
                    Ok(next_sink) => sink = Some(next_sink),
                    Err(error) => {
                        fail(
                            &mut audio,
                            &mut decoder,
                            &mut sink,
                            &mut pending,
                            &mut eof,
                            &mut playing,
                            &mut source,
                            &snapshot,
                            token,
                            base_ms,
                            duration,
                            error,
                        );
                        continue;
                    },
                }
            }
            if let Some(active_sink) = sink.as_mut()
                && !pending.is_empty()
                && let Err(error) = active_sink
                    .push(&pending)
                    .map(|accepted| consume_frames(&mut pending, accepted, active_sink.channels))
            {
                fail(
                    &mut audio,
                    &mut decoder,
                    &mut sink,
                    &mut pending,
                    &mut eof,
                    &mut playing,
                    &mut source,
                    &snapshot,
                    token,
                    base_ms,
                    duration,
                    error,
                );
                continue;
            }
            let output_has_space = sink.as_ref().is_none_or(|active_sink| {
                active_sink
                    .state
                    .lock()
                    .ok()
                    .and_then(|state| state.occupied_seconds())
                    .unwrap_or(1.0)
                    < 0.15
            });
            if pending.is_empty() && !eof && output_has_space {
                match active.format.next_packet() {
                    Ok(packet) if packet.track_id() == active.track_id => {
                        match active.decoder.decode(&packet) {
                            Ok(decoded) => {
                                let spec = *decoded.spec();
                                let channels = spec.channels.count() as u32;
                                if let Some(expected) = active.channels {
                                    if expected != channels {
                                        fail(
                                            &mut audio,
                                            &mut decoder,
                                            &mut sink,
                                            &mut pending,
                                            &mut eof,
                                            &mut playing,
                                            &mut source,
                                            &snapshot,
                                            token,
                                            base_ms,
                                            duration,
                                            format!(
                                                "decoded channel count changed from {expected} to {channels}"
                                            ),
                                        );
                                        continue;
                                    }
                                } else {
                                    active.channels = Some(channels);
                                }
                                let buffer = active.sample_buffer.get_or_insert_with(|| {
                                    SampleBuffer::new(decoded.capacity() as u64, spec)
                                });
                                buffer.copy_interleaved_ref(decoded);
                                pending.extend_from_slice(buffer.samples());
                            },
                            Err(SymphoniaError::DecodeError(_)) => {},
                            Err(error) => {
                                fail(
                                    &mut audio,
                                    &mut decoder,
                                    &mut sink,
                                    &mut pending,
                                    &mut eof,
                                    &mut playing,
                                    &mut source,
                                    &snapshot,
                                    token,
                                    base_ms,
                                    duration,
                                    format!("decode failed: {error}"),
                                );
                                continue;
                            },
                        }
                    },
                    Ok(_) => {},
                    Err(SymphoniaError::IoError(error))
                        if error.kind() == std::io::ErrorKind::UnexpectedEof =>
                    {
                        eof = true
                    },
                    Err(error) => {
                        fail(
                            &mut audio,
                            &mut decoder,
                            &mut sink,
                            &mut pending,
                            &mut eof,
                            &mut playing,
                            &mut source,
                            &snapshot,
                            token,
                            base_ms,
                            duration,
                            format!("read failed: {error}"),
                        );
                        continue;
                    },
                }
            }
            if playing && let Some(active_sink) = sink.as_mut() {
                let position = active_sink
                    .presented_ms()
                    .unwrap_or(0)
                    .saturating_add(base_ms);
                if drained(
                    eof,
                    &pending,
                    active_sink
                        .state
                        .lock()
                        .ok()
                        .and_then(|state| state.occupied_seconds())
                        .unwrap_or(1.0),
                ) {
                    publish(
                        &snapshot,
                        token,
                        PlaybackState::Ended,
                        position,
                        duration,
                        source.clone(),
                    );
                    playing = false;
                } else {
                    publish(
                        &snapshot,
                        token,
                        PlaybackState::Playing,
                        position,
                        duration,
                        source.clone(),
                    );
                }
            }
        }
        thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(
            std::env::var_os("REDSHANK_PHASE4_FIXTURES")
                .expect("set REDSHANK_PHASE4_FIXTURES to the generated fixture directory"),
        )
        .join(name)
    }

    fn wait_for(
        runtime: &PlaybackRuntime,
        predicate: impl Fn(&PlaybackSnapshot) -> bool,
    ) -> PlaybackSnapshot {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let snapshot = runtime.snapshot();
            if predicate(&snapshot) || Instant::now() >= deadline {
                return snapshot;
            }
            thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn local_source_keeps_its_path() {
        assert_eq!(
            source_path(MediaSource::Local {
                path: "C:/audio.mp3".into()
            })
            .unwrap(),
            PathBuf::from("C:/audio.mp3")
        );
    }
    #[test]
    fn non_local_sources_report_a_capability_error() {
        assert!(
            source_path(MediaSource::Enclosure {
                url: "https://example.test/a.mp3".into()
            })
            .unwrap_err()
            .to_string()
            .contains("HTTP playback")
        );
    }
    #[test]
    fn new_runtime_has_an_honest_empty_snapshot() {
        let runtime = PlaybackRuntime::start();
        assert_eq!(runtime.snapshot().state, PlaybackState::Empty);
    }
    #[test]
    fn stereo_partial_write_consumes_whole_frames_only() {
        let mut pcm = vec![0.0; 10];
        consume_frames(&mut pcm, 3, 2);
        assert_eq!(pcm.len(), 4);
        assert_eq!(presented_ms(48_000, 48_000, 0.08), 920);
    }
    #[test]
    fn eof_waits_for_pending_pcm_and_output_drain() {
        assert!(!drained(true, &[0.0, 0.0], 0.0));
        assert!(!drained(true, &[], 0.01));
        assert!(drained(true, &[], 0.005));
    }
    #[test]
    fn queued_audio_never_makes_presented_time_negative() {
        assert_eq!(presented_ms(240, 48_000, 0.050), 0);
    }

    #[test]
    #[ignore = "requires REDSHANK_PHASE4_FIXTURES with the generated stereo fixtures"]
    fn supplied_stereo_fixtures_preserve_two_distinct_channels() {
        for name in ["stereo.mp3", "stereo.m4a"] {
            let path = fixture(name);
            let (mut decoder, _) = open_local(&path).unwrap_or_else(|error| {
                panic!(
                    "could not open supplied {name} fixture at {}: {error:#}",
                    path.display()
                )
            });
            assert!(
                matches!(decoder.channels, None | Some(2)),
                "{name} must not advertise a non-stereo layout"
            );
            assert!(
                decoder
                    .duration_ms
                    .is_some_and(|duration| (1_800..=2_200).contains(&duration)),
                "{name} duration should be about two seconds"
            );

            let mut distinct_channels = false;
            while let Ok(packet) = decoder.format.next_packet() {
                if packet.track_id() != decoder.track_id {
                    continue;
                }
                let decoded = decoder.decoder.decode(&packet).unwrap();
                let spec = *decoded.spec();
                assert_eq!(spec.channels.count(), 2, "{name} must decode as stereo");
                let mut samples = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
                samples.copy_interleaved_ref(decoded);
                distinct_channels |= samples
                    .samples()
                    .chunks_exact(2)
                    .any(|frame| (frame[0] - frame[1]).abs() > 0.01);
                if distinct_channels {
                    break;
                }
            }
            assert!(
                distinct_channels,
                "{name} must retain distinct left and right signals"
            );
        }
    }

    #[test]
    #[ignore = "requires REDSHANK_PHASE4_FIXTURES with the generated stereo fixtures"]
    fn supplied_local_fixture_loads_at_its_resume_position_without_an_output_device() {
        let path = fixture("stereo.mp3");
        let runtime = PlaybackRuntime::start();
        runtime
            .command(PlaybackCommand::Load {
                token: 42,
                source: MediaSource::Local {
                    path: path.display().to_string(),
                },
                resume_ms: 1_000,
            })
            .unwrap();

        let snapshot = wait_for(&runtime, |snapshot| {
            snapshot.load_token == Some(42) && !matches!(snapshot.state, PlaybackState::Loading)
        });
        assert_eq!(snapshot.state, PlaybackState::Paused);
        assert!(
            (800..=1_200).contains(&snapshot.position_ms),
            "resume should publish the decoder's actual seek position, got {} ms",
            snapshot.position_ms
        );
        assert!(
            snapshot
                .representation
                .as_ref()
                .and_then(|receipt| receipt.complete_digest.as_ref())
                .is_some()
        );
    }

    #[test]
    fn unsupported_load_publishes_the_new_token_without_starting_audio() {
        let runtime = PlaybackRuntime::start();
        runtime
            .command(PlaybackCommand::Load {
                token: 9,
                source: MediaSource::Enclosure {
                    url: "https://example.test/episode.mp3".into(),
                },
                resume_ms: 0,
            })
            .unwrap();

        let snapshot = wait_for(&runtime, |snapshot| {
            snapshot.load_token == Some(9)
                && matches!(snapshot.state, PlaybackState::Unavailable(_))
        });
        assert_eq!(snapshot.load_token, Some(9));
        assert!(
            matches!(snapshot.state, PlaybackState::Unavailable(ref error) if error.contains("HTTP playback"))
        );
    }
}
