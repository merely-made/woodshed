use std::{
    cell::RefCell,
    collections::VecDeque,
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{Context, Result, anyhow};
use audio_primitives::Stretcher;
use fetch::Fetch;
use firewheel::{
    node::NodeID,
    nodes::stream::{
        StreamStats,
        writer::{PushStatus, StreamWriterState},
    },
};
use redshank_model::RepresentationReceipt;
use servo_media_player::controller::PlaybackRateRange;
use symphonia::core::{
    audio::SampleBuffer,
    codecs::DecoderOptions,
    errors::Error as SymphoniaError,
    formats::{FormatOptions, SeekMode, SeekTo},
    io::{MediaSource, MediaSourceStream},
    meta::MetadataOptions,
    probe::Hint,
    units::Time,
};

use crate::{
    http_range::{DEFAULT_CACHE_BYTES, HttpRangeSource},
    output::{AudioRuntime, BUFFER_LOW_WATER_SECONDS, Output},
};

/// Slowest and fastest rates the dock may ask for. Outside this the WSOLA
/// window stops holding a musical period and the retiming stops being honest.
pub(super) const MIN_RATE_PERCENT: u16 = 50;
pub(super) const MAX_RATE_PERCENT: u16 = 200;
pub(super) const UNITY_RATE_PERCENT: u16 = 100;
/// Packets decoded per pump while the output queue is below its low water
/// mark. Enough source for the fastest rate even on a slow decode.
const DECODE_PACKETS_PER_PUMP: usize = 4;

/// Rates the loaded source can be played at, which is now every rate the
/// stretcher accepts rather than none.
pub(super) fn rate_capability() -> Option<PlaybackRateRange> {
    Some(PlaybackRateRange {
        minimum: f64::from(MIN_RATE_PERCENT) / 100.0,
        maximum: f64::from(MAX_RATE_PERCENT) / 100.0,
    })
}

/// Stretch ratio (output duration over input duration) for a rate percent.
pub(super) fn ratio_for(percent: u16) -> f32 {
    100.0 / f32::from(percent.clamp(MIN_RATE_PERCENT, MAX_RATE_PERCENT))
}

/// The retiming stage between the decoder and the sink: source frames in,
/// output frames out, plus enough of the two clocks to map a presented output
/// frame back to the source frame it came from.
pub(super) struct RateStage {
    stretcher: Stretcher,
    /// (output frames produced, source frames emitted) pairs. One mark per
    /// push, so a rate change mid-queue still maps exactly.
    marks: VecDeque<(u64, u64)>,
}

impl RateStage {
    pub(super) fn new(sample_rate: u32, channels: usize, percent: u16) -> Self {
        let mut stretcher = Stretcher::new(sample_rate, channels);
        stretcher.set_ratio(ratio_for(percent));
        Self {
            stretcher,
            marks: VecDeque::from([(0, 0)]),
        }
    }

    pub(super) fn set_percent(&mut self, percent: u16) {
        self.stretcher.set_ratio(ratio_for(percent));
    }

    pub(super) fn push(&mut self, source: &[f32], output: &mut Vec<f32>) {
        self.stretcher.process(source, output);
        self.mark();
    }

    pub(super) fn flush(&mut self, output: &mut Vec<f32>) {
        self.stretcher.flush(output);
        self.mark();
    }

    /// Drop the stream but keep the rate: a seek re-anchors both clocks at 0.
    pub(super) fn restart(&mut self) {
        self.stretcher.reset();
        self.marks.clear();
        self.marks.push_back((0, 0));
    }

    fn mark(&mut self) {
        let mark = (
            self.stretcher.output_frames_produced(),
            self.stretcher.source_frames_emitted(),
        );
        if self.marks.back() != Some(&mark) {
            self.marks.push_back(mark);
        }
    }

    /// The source frame presented once `output_frames` have been played.
    pub(super) fn source_frames_at(&self, output_frames: u64) -> u64 {
        let mut behind = *self.marks.front().unwrap_or(&(0, 0));
        if output_frames <= behind.0 {
            return behind.1;
        }
        for &(produced, source) in self.marks.iter().skip(1) {
            if produced > output_frames {
                let span = produced - behind.0;
                let advance =
                    (output_frames - behind.0) * source.saturating_sub(behind.1) / span.max(1);
                return behind.1 + advance;
            }
            behind = (produced, source);
        }
        behind.1
    }

    /// Forget marks the presentation point has already passed.
    pub(super) fn prune(&mut self, output_frames: u64) {
        while self.marks.len() > 1 && self.marks[1].0 <= output_frames {
            self.marks.pop_front();
        }
    }
}

pub(super) struct Sink {
    pub(super) state: Arc<Mutex<StreamWriterState>>,
    pub(super) node: NodeID,
    pub(super) rate: u32,
    pub(super) channels: usize,
    pub(super) generation: Arc<()>,
}

pub(super) fn consume_frames(pending: &mut Vec<f32>, accepted_frames: usize, channels: usize) {
    pending.drain(..accepted_frames.saturating_mul(channels).min(pending.len()));
}

/// End of stream: the decoder is finished, the stretcher has been flushed,
/// nothing is waiting for the sink, and the output queue has emptied.
pub(super) fn drained(flushed: bool, held: usize, writer_drained: bool) -> bool {
    flushed && held == 0 && writer_drained
}

impl Sink {
    pub(super) fn push(&mut self, samples: &[f32]) -> Result<usize> {
        let mut writer = self
            .state
            .lock()
            .map_err(|_| anyhow!("audio writer lock poisoned"))?;
        if !writer.is_active() {
            return Err(anyhow!("audio writer stream was stopped"));
        }
        let frames = writer.available_frames().min(samples.len() / self.channels);
        if frames == 0 {
            return Ok(0);
        }
        let status = writer.push_interleaved(&samples[..frames * self.channels]);
        let accepted = match status {
            PushStatus::Ok | PushStatus::UnderflowCorrected { .. } => frames,
            PushStatus::OutputNotReady => 0,
            PushStatus::OverflowOccurred { num_frames_pushed } => num_frames_pushed,
        };
        Ok(accepted)
    }

    pub(super) fn stats(&self) -> Result<StreamStats> {
        let writer = self
            .state
            .lock()
            .map_err(|_| anyhow!("audio writer lock poisoned"))?;
        if !writer.is_active() {
            return Err(anyhow!("audio writer stream was stopped"));
        }
        writer
            .stats()
            .ok_or_else(|| anyhow!("audio writer stream is unavailable"))
    }

    pub(super) fn finish(&self) -> Result<bool> {
        Ok(self
            .state
            .lock()
            .map_err(|_| anyhow!("audio writer lock poisoned"))?
            .finish_stream())
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

    pub(super) fn stop(&self) -> Result<()> {
        self.state
            .lock()
            .map_err(|_| anyhow!("audio writer lock poisoned"))?
            .stop_stream();
        Ok(())
    }
}

pub(super) struct Decoder {
    pub(crate) format: Box<dyn symphonia::core::formats::FormatReader>,
    pub(crate) decoder: Box<dyn symphonia::core::codecs::Decoder>,
    pub(crate) track_id: u32,
    sample_buffer: Option<SampleBuffer<f32>>,
    pub(super) rate: u32,
    pub(super) channels: Option<u32>,
    time_base: symphonia::core::units::TimeBase,
    duration_ms: Option<u64>,
    /// Accurate demux seeks land at a preceding packet. Decode that preroll,
    /// but admit only source frames at or after this track timestamp.
    discard_before_ts: Option<u64>,
}

pub(super) fn open_memory(bytes: Vec<u8>) -> Result<(Decoder, RepresentationReceipt)> {
    if bytes.is_empty() || bytes.len() > crate::MAX_HOST_BLOB_BYTES {
        return Err(anyhow!("host audio exceeds its byte budget"));
    }
    let receipt = RepresentationReceipt {
        byte_length: Some(bytes.len() as u64),
        complete_digest: Some(format!("blake3:{}", blake3::hash(&bytes).to_hex())),
        ..RepresentationReceipt::default()
    };
    let decoder = open_decoder(Box::new(std::io::Cursor::new(bytes)), "host-blob.wav")?;
    Ok((decoder, receipt))
}

pub(super) fn open_local(path: &Path) -> Result<(Decoder, RepresentationReceipt)> {
    let (file, receipt) = snapshot_local(path)?;
    let decoder = open_decoder(Box::new(file), path.to_str().unwrap_or_default())?;
    Ok((decoder, receipt))
}

/// Hash exactly the bytes copied to a private, automatically deleted file.
/// The decoder owns this snapshot, so later writes to the original (including
/// in-place writes through an already open handle) cannot change playback.
/// Memory is bounded by the copy buffer; disk use lasts only for this load.
pub(super) fn snapshot_local(path: &Path) -> Result<(File, RepresentationReceipt)> {
    let mut file =
        File::open(path).with_context(|| format!("could not open {}", path.display()))?;
    let mut snapshot = tempfile::tempfile().context("could not create private audio snapshot")?;
    let mut length = 0_u64;
    let mut digest = blake3::Hasher::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .context("could not read local audio snapshot")?;
        if count == 0 {
            break;
        }
        snapshot
            .write_all(&buffer[..count])
            .context("could not write audio snapshot")?;
        digest.update(&buffer[..count]);
        length += count as u64;
    }
    snapshot
        .seek(SeekFrom::Start(0))
        .context("could not rewind audio snapshot")?;
    Ok((
        snapshot,
        RepresentationReceipt {
            byte_length: Some(length),
            complete_digest: Some(format!("blake3:{}", digest.finalize().to_hex())),
            ..RepresentationReceipt::default()
        },
    ))
}

pub(super) fn open_http(
    fetch: Arc<dyn Fetch>,
    url: &str,
) -> Result<(Decoder, RepresentationReceipt)> {
    let (source, receipt) = HttpRangeSource::open(fetch, url, DEFAULT_CACHE_BYTES)?;
    let decoder = open_decoder(Box::new(source), url)?;
    Ok((decoder, receipt))
}

fn open_decoder(source: Box<dyn MediaSource>, hint_source: &str) -> Result<Decoder> {
    let mut hint = Hint::new();
    let path = hint_source.split(['?', '#']).next().unwrap_or(hint_source);
    if let Some(extension) = Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
    {
        hint.with_extension(extension);
    }
    let stream = MediaSourceStream::new(source, Default::default());
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
    let channels = track
        .codec_params
        .channels
        .map(|layout| layout.count() as u32);
    Ok(Decoder {
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
        discard_before_ts: None,
    })
}

pub(super) struct Backend {
    audio: Rc<RefCell<Output>>,
    /// The host's fetch handle: every remote read goes through its policy.
    fetch: Arc<dyn Fetch>,
    blobs: Option<crate::HostBlobReader>,
    decoder: Option<Decoder>,
    sink: Option<Sink>,
    /// Decoded source frames waiting to be retimed.
    pending: Vec<f32>,
    /// Retimed output frames waiting for the sink.
    stretched: Vec<f32>,
    /// Pitch-preserving retiming between the decoder and the sink.
    stage: Option<RateStage>,
    eof: bool,
    /// Whether the stretcher's held frames were flushed after end of file.
    eof_flushed: bool,
    source: Option<String>,
    representation: Option<RepresentationReceipt>,
    base_ms: u64,
    requested_playing: bool,
    source_positioned: bool,
    decoded_first_frame: bool,
    /// Playback rate in percent, clamped; survives loads like the volume does.
    rate_percent: u16,
    /// How much of the source can be seeked without waiting on the network.
    buffered_percent: u8,
    fingerprint: Option<audio_primitives::alignment::FingerprintBuilder>,
    fingerprint_failed: bool,
    #[cfg(test)]
    test_source: bool,
    #[cfg(test)]
    test_ready_sent: bool,
}

impl Backend {
    pub(super) fn new(fetch: Arc<dyn Fetch>) -> Self {
        Self::with_audio(Rc::new(RefCell::new(Output::default())), fetch)
    }

    pub(super) fn with_audio(audio: Rc<RefCell<Output>>, fetch: Arc<dyn Fetch>) -> Self {
        Self {
            audio,
            fetch,
            blobs: None,
            decoder: None,
            sink: None,
            pending: Vec::new(),
            stretched: Vec::new(),
            stage: None,
            eof: false,
            eof_flushed: false,
            source: None,
            representation: None,
            base_ms: 0,
            requested_playing: false,
            source_positioned: false,
            decoded_first_frame: false,
            rate_percent: UNITY_RATE_PERCENT,
            buffered_percent: 0,
            fingerprint: None,
            fingerprint_failed: false,
            #[cfg(test)]
            test_source: false,
            #[cfg(test)]
            test_ready_sent: false,
        }
    }

    pub(super) fn with_blobs(mut self, blobs: Option<crate::HostBlobReader>) -> Self {
        self.blobs = blobs;
        self
    }

    pub(super) fn blob_reader(&self) -> Option<crate::HostBlobReader> {
        self.blobs.clone()
    }

    pub(super) fn fetch_handle(&self) -> Arc<dyn Fetch> {
        self.fetch.clone()
    }

    pub(super) fn audio_handle(&self) -> Rc<RefCell<Output>> {
        Rc::clone(&self.audio)
    }

    fn retire_sink(&mut self) -> Result<(), String> {
        let Some(sink) = self.sink.take() else {
            return Ok(());
        };
        if let Some(audio) = self.audio.borrow_mut().runtime.as_mut() {
            audio.remove_sink(sink).map_err(|error| error.to_string())
        } else {
            sink.stop().map_err(|error| error.to_string())
        }
    }

    pub(super) fn load(
        &mut self,
        source: &servo_media_player::controller::MediaSource,
    ) -> Result<servo_media_player::controller::MediaInfo, String> {
        #[cfg(test)]
        if let servo_media_player::controller::MediaSource::Local { path } = source
            && path.starts_with("redshank-test://")
        {
            self.decoder = None;
            self.representation = Some(RepresentationReceipt::default());
            self.source = Some(path.clone());
            self.discard_retiming();
            self.eof = false;
            self.base_ms = 0;
            self.requested_playing = false;
            self.source_positioned = true;
            self.decoded_first_frame = false;
            self.buffered_percent = 100;
            self.test_source = true;
            self.test_ready_sent = false;
            return Ok(servo_media_player::controller::MediaInfo {
                duration: Some(Duration::from_secs(1)),
                capabilities: servo_media_player::controller::PlaybackCapabilities {
                    seekable: true,
                    playback_rates: rate_capability(),
                },
            });
        }
        self.retire_sink()?;
        let (decoder, representation, source_label) = match source {
            servo_media_player::controller::MediaSource::Local { path } => {
                let path = Path::new(path);
                let (decoder, representation) =
                    open_local(path).map_err(|error| error.to_string())?;
                (decoder, representation, path.display().to_string())
            },
            servo_media_player::controller::MediaSource::Http { url } => {
                let (decoder, representation) =
                    open_http(self.fetch.clone(), url).map_err(|error| error.to_string())?;
                let source_label = representation
                    .final_url
                    .clone()
                    .unwrap_or_else(|| url.clone());
                (decoder, representation, source_label)
            },
            servo_media_player::controller::MediaSource::HostBlob { id } => {
                let reader = self
                    .blobs
                    .as_ref()
                    .ok_or_else(|| format!("host blob playback needs the embedding host: {id}"))?;
                let bytes = reader(id)?;
                let (decoder, representation) = open_memory(bytes).map_err(|e| e.to_string())?;
                (decoder, representation, format!("blob:{id}"))
            },
        };
        // A local or published cache object is wholly seekable; the HTTP range
        // source keeps only a sliding window, so it can claim no buffered head.
        self.buffered_percent = match source {
            servo_media_player::controller::MediaSource::Local { .. }
            | servo_media_player::controller::MediaSource::HostBlob { .. } => 100,
            servo_media_player::controller::MediaSource::Http { .. } => 0,
        };
        let duration = decoder.duration_ms.map(Duration::from_millis);
        self.decoder = Some(decoder);
        self.representation = Some(representation);
        self.source = Some(source_label);
        self.discard_retiming();
        self.eof = false;
        self.base_ms = 0;
        self.requested_playing = false;
        self.source_positioned = true;
        self.decoded_first_frame = false;
        #[cfg(test)]
        {
            self.test_source = false;
            self.test_ready_sent = false;
        }
        Ok(servo_media_player::controller::MediaInfo {
            duration,
            capabilities: servo_media_player::controller::PlaybackCapabilities {
                seekable: true,
                playback_rates: rate_capability(),
            },
        })
    }

    pub(super) fn seek(&mut self, position: Duration) -> Result<(), String> {
        #[cfg(test)]
        if self.test_source {
            self.base_ms = position.as_millis() as u64;
            self.source_positioned = true;
            self.decoded_first_frame = false;
            self.test_ready_sent = false;
            return Ok(());
        }
        let decoder = self.decoder.as_mut().ok_or("no local decoder is loaded")?;
        // A demuxer's required_ts may floor a millisecond cue to the sample
        // just before it (e.g. 5123 ms at 44.1 kHz). Admit the first track tick
        // at or after the request, using integer arithmetic at that boundary.
        let requested_ts = u64::try_from(
            (position.as_nanos() * u128::from(decoder.time_base.denom))
                .div_ceil(u128::from(decoder.time_base.numer) * 1_000_000_000),
        )
        .map_err(|_| "seek position exceeds the track timestamp range")?;
        let seeked = decoder
            .format
            .seek(
                SeekMode::Accurate,
                SeekTo::Time {
                    time: Time::from(position.as_secs_f64()),
                    track_id: Some(decoder.track_id),
                },
            )
            .map_err(|error| format!("seek unavailable: {error}"))?;
        decoder.decoder.reset();
        let required_ts = seeked.required_ts.max(requested_ts);
        decoder.discard_before_ts = Some(required_ts);
        // Use track time, not the earlier demux packet boundary. Integer
        // conversion also avoids rounding an exact millisecond down in f64.
        self.base_ms = (u128::from(required_ts) * u128::from(decoder.time_base.numer) * 1000
            / u128::from(decoder.time_base.denom)) as u64;
        // Both clocks re-anchor at the seek point: the sink is new, so the
        // stretcher restarts rather than carrying the old stream's frames.
        self.discard_retiming();
        self.eof = false;
        self.retire_sink()?;
        self.source_positioned = true;
        self.decoded_first_frame = false;
        Ok(())
    }

    pub(super) fn set_playing(&mut self, playing: bool) -> Result<(), String> {
        self.requested_playing = playing;
        if let Some(sink) = self.sink.as_ref() {
            if playing { sink.resume() } else { sink.pause() }
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    pub(super) fn reset(&mut self, position: Duration) -> Result<(), String> {
        if !self.source_positioned {
            self.seek(position)?;
        }
        self.source_positioned = false;
        self.decoded_first_frame = false;
        Ok(())
    }

    /// Position in SOURCE time: the presented output frame mapped back through
    /// the stretcher's clocks, so 1.5x still reports the recording's own clock.
    pub(super) fn position(&self) -> Result<Duration, String> {
        let played = match (self.sink.as_ref(), self.stage.as_ref()) {
            (Some(sink), Some(stage)) => {
                let presented = sink
                    .stats()
                    .map_err(|error| error.to_string())?
                    .presented_source_frames;
                stage.source_frames_at(presented) * 1000 / u64::from(sink.rate)
            },
            _ => 0,
        };
        Ok(Duration::from_millis(played.saturating_add(self.base_ms)))
    }

    pub(super) fn facts(&self) -> (Option<RepresentationReceipt>, Option<String>, Option<u64>) {
        (
            self.representation.clone(),
            self.source.clone(),
            self.decoder
                .as_ref()
                .and_then(|decoder| decoder.duration_ms),
        )
    }

    pub(super) fn buffered_percent(&self) -> u8 {
        self.buffered_percent
    }

    /// Record the requested output volume and apply it if the output exists.
    pub(super) fn set_volume(&mut self, percent: u8) {
        self.audio.borrow_mut().set_volume(percent);
    }

    /// Duck to a percent of the volume, or restore it with `None`.
    pub(super) fn set_duck(&mut self, percent: Option<u8>) {
        self.audio.borrow_mut().set_duck(percent);
    }

    /// The gain in effect: the volume, scaled by the duck while one is held.
    #[cfg(test)]
    pub(super) fn effective_volume(&self) -> u8 {
        self.audio.borrow().effective_volume()
    }

    #[cfg(test)]
    pub(super) fn rate_percent(&self) -> u16 {
        self.rate_percent
    }

    pub(super) fn output_levels(&self) -> (u8, Option<u8>) {
        let output = self.audio.borrow();
        (output.volume_percent(), output.duck_percent())
    }

    /// Apply a playback rate live and report the rate actually in effect.
    pub(super) fn set_rate(&mut self, percent: u16) -> u16 {
        let clamped = percent.clamp(MIN_RATE_PERCENT, MAX_RATE_PERCENT);
        self.rate_percent = clamped;
        if let Some(stage) = self.stage.as_mut() {
            stage.set_percent(clamped);
        }
        clamped
    }

    /// Drop every retimed frame and restart the stage's clocks at zero.
    fn discard_retiming(&mut self) {
        self.fingerprint = None;
        self.fingerprint_failed = false;
        self.pending.clear();
        self.stretched.clear();
        self.eof_flushed = false;
        if let Some(stage) = self.stage.as_mut() {
            stage.restart();
        }
    }

    pub(super) fn admit_cached_representation(
        &mut self,
        expected: &RepresentationReceipt,
    ) -> Result<(), String> {
        let actual = self
            .representation
            .as_ref()
            .ok_or("cached audio produced no representation receipt")?;
        if actual.complete_digest.is_none()
            || actual.complete_digest != expected.complete_digest
            || actual.byte_length != expected.byte_length
        {
            return Err(format!(
                "cached audio failed integrity validation: expected {:?} at {:?} bytes, found {:?} at {:?} bytes",
                expected.complete_digest,
                expected.byte_length,
                actual.complete_digest,
                actual.byte_length
            ));
        }
        self.representation = Some(expected.clone());
        Ok(())
    }

    pub(super) fn clear(&mut self) -> Result<(), String> {
        self.requested_playing = false;
        self.eof = false;
        self.discard_retiming();
        self.stage = None;
        self.decoder = None;
        self.source = None;
        self.representation = None;
        self.base_ms = 0;
        self.source_positioned = false;
        self.decoded_first_frame = false;
        self.buffered_percent = 0;
        #[cfg(test)]
        {
            self.test_source = false;
            self.test_ready_sent = false;
        }
        self.retire_sink()
    }

    fn queued_seconds(&self) -> Result<f64, String> {
        self.sink
            .as_ref()
            .map(|sink| {
                sink.state
                    .lock()
                    .map_err(|_| anyhow!("audio writer lock poisoned"))?
                    .occupied_seconds()
                    .ok_or_else(|| anyhow!("audio writer did not report queue occupancy"))
            })
            .transpose()
            .map_err(|error| error.to_string())
            .map(|queued| queued.unwrap_or(0.0))
    }

    pub(super) fn pump(&mut self) -> Result<PumpState, String> {
        #[cfg(test)]
        if self.test_source {
            if !self.test_ready_sent {
                self.test_ready_sent = true;
                return Ok(PumpState::Ready);
            }
            return Ok(if self.requested_playing {
                PumpState::EndOfStream
            } else {
                PumpState::Idle
            });
        }
        if self.sink.as_ref().is_some_and(|sink| {
            self.audio
                .borrow()
                .runtime
                .as_ref()
                .is_none_or(|audio| !audio.owns_sink(sink))
        }) {
            return Err("shared audio output was retired; reload this stream to restart".into());
        }
        let audio_tick = self
            .audio
            .borrow_mut()
            .runtime
            .as_mut()
            .map(AudioRuntime::tick);
        if let Some(Err(error)) = audio_tick {
            // Retire the failed backend before reporting Unavailable. A later
            // load must open a fresh CpalStream, not reuse a stopped device.
            self.audio.borrow_mut().runtime.take();
            return Err(error.to_string());
        }
        if self.decoder.is_none() {
            return Ok(PumpState::Idle);
        }
        if !self.decoded_first_frame {
            self.decode_next_packet()?;
            if !self.decoded_first_frame {
                return Ok(if self.eof {
                    PumpState::EndOfStream
                } else {
                    PumpState::Idle
                });
            }
        }
        let (rate, channels) = self
            .decoder
            .as_ref()
            .and_then(|decoder| decoder.channels.map(|channels| (decoder.rate, channels)))
            .ok_or("first decoded frame did not report a channel layout")?;
        if self.sink.is_none() {
            self.sink = Some(
                self.audio
                    .borrow_mut()
                    .sink(rate, channels)
                    .map_err(|error| error.to_string())?,
            );
            // The stage output clock and the new sink accepted-frame clock
            // start together, which is what makes the position mapping exact.
            self.stage = Some(RateStage::new(rate, channels as usize, self.rate_percent));
            return Ok(PumpState::Ready);
        }
        if !self.requested_playing {
            return Ok(PumpState::Idle);
        }
        {
            let stage = self
                .stage
                .as_mut()
                .ok_or("rate stage was not initialized")?;
            if !self.pending.is_empty() {
                stage.push(&self.pending, &mut self.stretched);
                self.pending.clear();
            }
            // The stretcher holds up to a window of source frames, so end of
            // file is not end of audio until it has been flushed.
            if self.eof && !self.eof_flushed {
                stage.flush(&mut self.stretched);
                self.eof_flushed = true;
            }
        }
        if !self.stretched.is_empty() {
            let (accepted, channels) = {
                let sink = self.sink.as_mut().ok_or("audio sink was not initialized")?;
                (
                    sink.push(&self.stretched)
                        .map_err(|error| error.to_string())?,
                    sink.channels,
                )
            };
            consume_frames(&mut self.stretched, accepted, channels);
        }
        let queued = self.queued_seconds()?;
        if self.stretched.is_empty() && !self.eof && queued < BUFFER_LOW_WATER_SECONDS {
            // A rate above 1.0 needs more source per second of output than one
            // packet a pump supplies, and an empty output queue is a dropout.
            for _ in 0..DECODE_PACKETS_PER_PUMP {
                if self.eof {
                    break;
                }
                self.decode_next_packet()?;
            }
        }
        if let (Some(sink), Some(stage)) = (self.sink.as_ref(), self.stage.as_mut()) {
            stage.prune(
                sink.stats()
                    .map_err(|error| error.to_string())?
                    .presented_source_frames,
            );
        }
        // Decoder EOF is not graph EOF: seal and publish the final partial
        // resampler packet before accepting the bridge's exact drain report.
        let writer_drained = if self.eof_flushed && self.stretched.is_empty() {
            let sink = self.sink.as_ref().ok_or("audio sink was not initialized")?;
            sink.finish().map_err(|error| error.to_string())?
                && sink.stats().map_err(|error| error.to_string())?.drained
        } else {
            false
        };
        Ok(
            if drained(self.eof_flushed, self.stretched.len(), writer_drained) {
                PumpState::EndOfStream
            } else {
                PumpState::Idle
            },
        )
    }

    fn decode_next_packet(&mut self) -> Result<(), String> {
        let decoder = self.decoder.as_mut().ok_or("no local decoder is loaded")?;
        match decode_packet(decoder)? {
            Decoded::Frames(samples) => {
                if !self.fingerprint_failed {
                    if self.fingerprint.is_none() {
                        self.fingerprint = audio_primitives::alignment::FingerprintBuilder::new(
                            decoder.rate,
                            decoder.channels.unwrap_or(1) as usize,
                            audio_primitives::alignment::FingerprintConfig {
                                window_ms: 60_000,
                                history_ms: 65_000,
                                ..Default::default()
                            },
                        )
                        .ok();
                    }
                    if self
                        .fingerprint
                        .as_mut()
                        .is_none_or(|builder| builder.push(&samples).is_err())
                    {
                        self.fingerprint_failed = true;
                        self.fingerprint = None;
                    }
                }
                self.decoded_first_frame = true;
                self.pending.extend_from_slice(&samples);
            },
            // Skipped audio cannot support an uninterrupted anchor interval.
            Decoded::Skipped => {
                self.fingerprint = None;
                self.fingerprint_failed = true;
            },
            Decoded::EndOfFile => self.eof = true,
        }
        Ok(())
    }

    pub(super) fn fingerprint_context(&self) -> Option<Arc<crate::FingerprintContext>> {
        let relative = (self.position().ok()?.as_millis() as u64).checked_sub(self.base_ms)?;
        let fingerprint = self
            .fingerprint
            .as_ref()?
            .fingerprint_at(relative, 60_000)
            .ok()?;
        let start_ms =
            self.base_ms + relative.checked_sub(u64::from(fingerprint.anchor_offset_ms))?;
        Some(Arc::new(crate::FingerprintContext {
            start_ms,
            fingerprint: crate::alignment::stored(fingerprint),
        }))
    }
}

/// What one packet yielded. An undecodable packet is skipped, not fatal.
pub(super) enum Decoded {
    Frames(Vec<f32>),
    Skipped,
    EndOfFile,
}

/// Decode one packet into interleaved source frames.
pub(super) fn decode_packet(decoder: &mut Decoder) -> Result<Decoded, String> {
    loop {
        match decoder.format.next_packet() {
            Ok(packet) if packet.track_id() == decoder.track_id => {
                match decoder.decoder.decode(&packet) {
                    Ok(decoded) => {
                        let spec = *decoded.spec();
                        if spec.rate != decoder.rate {
                            return Err(format!(
                                "decoded sample rate changed from {} to {}",
                                decoder.rate, spec.rate
                            ));
                        }
                        let channels = spec.channels.count() as u32;
                        if let Some(expected) = decoder.channels
                            && expected != channels
                        {
                            return Err(format!(
                                "decoded channel count changed from {expected} to {channels}"
                            ));
                        }
                        decoder.channels = Some(channels);
                        let buffer = decoder.sample_buffer.get_or_insert_with(|| {
                            SampleBuffer::new(decoded.capacity() as u64, spec)
                        });
                        buffer.copy_interleaved_ref(decoded);
                        let samples = buffer.samples();
                        let frames = samples.len() / channels as usize;
                        let discard = decoder.discard_before_ts.map_or(0, |required| {
                            // Packet timestamps use the track time base, which need
                            // not be 1/sample_rate. Round up so no earlier frame is
                            // emitted, and trim whole interleaved channel frames.
                            (u128::from(required.saturating_sub(packet.ts()))
                                * u128::from(decoder.time_base.numer)
                                * u128::from(spec.rate))
                            .div_ceil(u128::from(decoder.time_base.denom))
                            .min(frames as u128) as usize
                        });
                        if discard == frames && decoder.discard_before_ts.is_some() {
                            // Intentional seek preroll is not a decode gap. Keep
                            // decoding without disabling the new fingerprint run.
                            continue;
                        }
                        decoder.discard_before_ts = None;
                        return Ok(Decoded::Frames(
                            samples[discard * channels as usize..].to_vec(),
                        ));
                    },
                    Err(SymphoniaError::DecodeError(_)) => return Ok(Decoded::Skipped),
                    Err(error) => return Err(format!("decode failed: {error}")),
                }
            },
            Ok(_) => return Ok(Decoded::Skipped),
            Err(SymphoniaError::IoError(error))
                if error.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                return Ok(Decoded::EndOfFile);
            },
            Err(error) => return Err(format!("read failed: {error}")),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PumpState {
    Idle,
    Ready,
    EndOfStream,
}

#[cfg(test)]
mod seek_tests {
    use super::*;

    fn pcm_sample(frame: u32, channel: u16) -> i16 {
        (frame % 8192) as i16 + channel as i16 * 9000 - 16384
    }

    #[test]
    fn empty_admitted_wav_reports_eof_without_a_sink() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("empty.wav");
        hound::WavWriter::create(
            &path,
            hound::WavSpec {
                channels: 1,
                sample_rate: 48_000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap()
        .finalize()
        .unwrap();
        let mut backend = Backend::new(crate::own_fetch());
        backend
            .load(&servo_media_player::controller::MediaSource::Local {
                path: path.display().to_string(),
            })
            .unwrap();
        assert_eq!(backend.pump().unwrap(), PumpState::EndOfStream);
        assert_eq!(backend.position().unwrap(), Duration::ZERO);
        assert!(backend.sink.is_none() && backend.audio.borrow().runtime.is_none());
    }

    #[test]
    fn preview_and_episode_share_gain_before_output_exists_and_after_each_gain_change() {
        let mut episode = Backend::new(crate::own_fetch());
        episode.set_rate(150);
        episode.set_volume(0);
        let mut preview = Backend::with_audio(episode.audio_handle(), crate::own_fetch());
        assert_eq!(episode.rate_percent(), 150);
        assert_eq!(
            preview.rate_percent(),
            UNITY_RATE_PERCENT,
            "voice preview retains its unity-rate policy"
        );
        assert_eq!(
            preview.effective_volume(),
            0,
            "preview cannot introduce default full gain"
        );
        preview.set_volume(40);
        assert_eq!(episode.effective_volume(), 40);
        episode.set_duck(Some(25));
        assert_eq!(preview.effective_volume(), 10);
        preview.set_duck(Some(50));
        assert_eq!(episode.effective_volume(), 20);
        episode.set_volume(0);
        assert_eq!(preview.effective_volume(), 0);
        preview.set_duck(None);
        assert_eq!(episode.effective_volume(), 0);
        episode.set_volume(37);
        preview.clear().unwrap();
        assert_eq!(preview.effective_volume(), 37);
        assert_eq!(episode.output_levels(), (37, None));
        assert_eq!(preview.rate_percent(), UNITY_RATE_PERCENT);
        assert!(episode.audio.borrow().runtime.is_none());
    }

    #[test]
    fn exact_duration_seek_reports_eof_before_first_frame_without_opening_output() {
        for rate in [8_000, 44_100, 48_000] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("exact-end.wav");
            let mut writer = hound::WavWriter::create(
                &path,
                hound::WavSpec {
                    channels: 1,
                    sample_rate: rate,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                },
            )
            .unwrap();
            for frame in 0..rate {
                writer.write_sample(pcm_sample(frame, 0)).unwrap();
            }
            writer.finalize().unwrap();
            let mut backend = Backend::new(crate::own_fetch());
            backend
                .load(&servo_media_player::controller::MediaSource::Local {
                    path: path.display().to_string(),
                })
                .unwrap();
            let end = Duration::from_secs(1);
            backend.seek(end).unwrap();
            backend.reset(end).unwrap();
            assert_eq!(backend.pump().unwrap(), PumpState::EndOfStream);
            assert_eq!(backend.position().unwrap(), end);
            assert!(backend.pending.is_empty());
            assert!(backend.sink.is_none());
            assert!(backend.audio.borrow().runtime.is_none());
        }
    }

    #[test]
    fn accurate_wav_seek_discards_packet_prefix_before_pcm_and_source_clock() {
        for rate in [8_000, 44_100, 48_000] {
            for channels in [1, 2] {
                let directory = tempfile::tempdir().unwrap();
                let path = directory.path().join("packet-boundary.wav");
                let mut writer = hound::WavWriter::create(
                    &path,
                    hound::WavSpec {
                        channels,
                        sample_rate: rate,
                        bits_per_sample: 16,
                        sample_format: hound::SampleFormat::Int,
                    },
                )
                .unwrap();
                for frame in 0..rate * 6 {
                    for channel in 0..channels {
                        writer.write_sample(pcm_sample(frame, channel)).unwrap();
                    }
                }
                writer.finalize().unwrap();
                let mut backend = Backend::new(crate::own_fetch());
                backend
                    .load(&servo_media_player::controller::MediaSource::Local {
                        path: path.display().to_string(),
                    })
                    .unwrap();
                // First seek is the load/resume path; later seeks exercise a
                // paused cue click and reset the previous decode/fingerprint run.
                // Fractional-second cues must not retain the sample preceding
                // the cue or report the preceding millisecond at 44.1 kHz.
                for milliseconds in [5_000, 1_123, 5_123, 1_020, 5_000] {
                    let position = Duration::from_millis(milliseconds);
                    backend.seek(position).unwrap();
                    backend.reset(position).unwrap();
                    backend.set_playing(false).unwrap();
                    assert_eq!(backend.position().unwrap(), position);
                    assert!(backend.pending.is_empty());
                    assert!(backend.fingerprint.is_none());
                    backend.decode_next_packet().unwrap();
                    assert!(!backend.pending.is_empty());
                    assert!(backend.fingerprint.is_some());
                    assert!(!backend.fingerprint_failed);
                    assert_eq!(backend.position().unwrap(), position);
                    let mut samples = backend.pending.clone();
                    loop {
                        match decode_packet(backend.decoder.as_mut().unwrap()).unwrap() {
                            Decoded::Frames(packet) => samples.extend(packet),
                            Decoded::EndOfFile => break,
                            Decoded::Skipped => panic!("valid WAV packet was skipped"),
                        }
                    }
                    let first_frame = (milliseconds * u64::from(rate)).div_ceil(1000) as u32;
                    assert_eq!(
                        samples.len(),
                        ((6 * rate - first_frame) * u32::from(channels)) as usize,
                        "seek must retain exactly the requested suffix at {rate} Hz"
                    );
                    for (index, sample) in samples.iter().enumerate() {
                        let frame = first_frame + (index / channels as usize) as u32;
                        let channel = (index % channels as usize) as u16;
                        assert_eq!(
                            *sample,
                            f32::from(pcm_sample(frame, channel)) / 32768.0,
                            "wrong PCM after seek: {rate} Hz, {channels} channels, frame {frame}"
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod runtime_generation_tests {
    use super::*;
    use firewheel::{
        FirewheelContext,
        nodes::stream::{media_channel_config, writer::StreamWriterNode},
    };
    use std::num::NonZeroU32;

    #[test]
    fn preview_with_retired_shared_runtime_fails_and_clears_its_sink() {
        // Actual bridge state can retain frozen counters after the device's
        // context disappears. A separate preview backend must reject that
        // generation before assuming Some(sink) still means live output.
        let mut context = FirewheelContext::new(Default::default());
        let node = context.add_node(StreamWriterNode, None).unwrap();
        let mut state = context
            .node_state::<StreamWriterState>(node)
            .unwrap()
            .clone();
        let _event = state
            .start_stream(
                NonZeroU32::new(48000).unwrap(),
                NonZeroU32::new(48000).unwrap(),
                media_channel_config(1.0),
            )
            .unwrap();
        assert!(state.is_active());
        let mut preview =
            Backend::with_audio(Rc::new(RefCell::new(Output::default())), crate::own_fetch());
        preview.sink = Some(Sink {
            state: Arc::new(Mutex::new(state.clone())),
            node,
            rate: 48000,
            channels: 2,
            generation: Arc::new(()),
        });
        preview.requested_playing = true;
        assert!(
            preview
                .pump()
                .unwrap_err()
                .contains("shared audio output was retired")
        );
        preview.clear().unwrap();
        assert!(preview.sink.is_none());
        assert!(!state.is_active());
    }
}

#[cfg(test)]
mod retained_blob_tests {
    use super::*;
    #[test]
    fn host_blob_decodes_without_a_filesystem_source_and_has_exact_evidence() {
        let mut wav = Vec::new();
        let samples = [100i16; 64];
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36u32 + samples.len() as u32 * 2).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&48000u32.to_le_bytes());
        wav.extend_from_slice(&96000u32.to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(samples.len() as u32 * 2).to_le_bytes());
        for sample in samples {
            wav.extend_from_slice(&sample.to_le_bytes());
        }
        let (mut decoder, receipt) = open_memory(wav.to_vec()).unwrap();
        assert_eq!(
            receipt.complete_digest,
            Some(format!("blake3:{}", blake3::hash(&wav).to_hex()))
        );
        let mut frames = 0;
        loop {
            match decode_packet(&mut decoder).unwrap() {
                Decoded::Frames(samples) => frames += samples.len(),
                Decoded::Skipped => continue,
                Decoded::EndOfFile => break,
            }
        }
        assert!(frames > 0);
        let supplied = wav.clone();
        let reader: crate::HostBlobReader = Arc::new(move |id| {
            if id == "admitted" {
                Ok(supplied.clone())
            } else {
                Err("body not admitted".into())
            }
        });
        let mut admitted = Backend::new(crate::own_fetch()).with_blobs(Some(reader));
        admitted
            .load(&servo_media_player::controller::MediaSource::HostBlob {
                id: "admitted".into(),
            })
            .unwrap();
        assert_eq!(admitted.representation.as_ref().unwrap(), &receipt);
        assert!(
            admitted
                .load(&servo_media_player::controller::MediaSource::HostBlob { id: "other".into() })
                .is_err()
        );
        assert!(open_memory(vec![]).is_err());
        assert!(open_memory(b"invalid audio".to_vec()).is_err());
        assert!(open_memory(vec![0; crate::MAX_HOST_BLOB_BYTES + 1]).is_err());
        let mut backend = Backend::new(crate::own_fetch());
        assert!(
            backend
                .load(&servo_media_player::controller::MediaSource::HostBlob {
                    id: "absent".into()
                })
                .is_err()
        );
    }
}
