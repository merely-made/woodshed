use std::{
    cell::RefCell,
    collections::VecDeque,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{Context, Result, anyhow};
use audio_primitives::Stretcher;
use firewheel::{
    node::NodeID,
    nodes::stream::writer::{PushStatus, StreamWriterState},
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
    output::{AudioRuntime, BUFFER_LOW_WATER_SECONDS},
};

/// Slowest and fastest rates the dock may ask for. Outside this the WSOLA
/// window stops holding a musical period and the retiming stops being honest.
pub(super) const MIN_RATE_PERCENT: u16 = 50;
pub(super) const MAX_RATE_PERCENT: u16 = 200;
pub(super) const UNITY_RATE_PERCENT: u16 = 100;
/// Packets decoded per pump while the output queue is below its low water
/// mark. Enough source for the fastest rate even on a slow decode.
const DECODE_PACKETS_PER_PUMP: usize = 4;
/// Queue occupancy below which the output counts as emptied.
const DRAINED_SECONDS: f64 = 0.005;

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
    pub(super) accepted_frames: u64,
    pub(super) rate: u32,
    pub(super) channels: usize,
}

/// Output frames the listener has actually heard: everything the sink took,
/// less everything still sitting in its queue.
pub(super) fn presented_frames(accepted_frames: u64, rate: u32, queued_seconds: f64) -> u64 {
    let queued = (queued_seconds.max(0.0) * f64::from(rate)) as u64;
    accepted_frames.saturating_sub(queued)
}

pub(super) fn consume_frames(pending: &mut Vec<f32>, accepted_frames: usize, channels: usize) {
    pending.drain(..accepted_frames.saturating_mul(channels).min(pending.len()));
}

/// End of stream: the decoder is finished, the stretcher has been flushed,
/// nothing is waiting for the sink, and the output queue has emptied.
pub(super) fn drained(flushed: bool, held: usize, queued_seconds: f64) -> bool {
    flushed && held == 0 && queued_seconds <= DRAINED_SECONDS
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
    rate: u32,
    channels: Option<u32>,
    time_base: symphonia::core::units::TimeBase,
    duration_ms: Option<u64>,
}

pub(super) fn open_local(path: &Path) -> Result<(Decoder, RepresentationReceipt)> {
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
    let decoder = open_decoder(Box::new(file), path.to_str().unwrap_or_default())?;
    Ok((
        decoder,
        RepresentationReceipt {
            byte_length: length,
            complete_digest: Some(format!("blake3:{}", digest.finalize().to_hex())),
            ..RepresentationReceipt::default()
        },
    ))
}

pub(super) fn open_http(url: &str) -> Result<(Decoder, RepresentationReceipt)> {
    let (source, receipt) = HttpRangeSource::open(url, DEFAULT_CACHE_BYTES)?;
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
    })
}

pub(super) struct Backend {
    audio: Rc<RefCell<Option<AudioRuntime>>>,
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
    /// Requested output volume in percent, applied as soon as a runtime exists.
    volume_percent: u8,
    /// Playback rate in percent, clamped; survives loads like the volume does.
    rate_percent: u16,
    /// How much of the source can be seeked without waiting on the network.
    buffered_percent: u8,
    #[cfg(test)]
    test_source: bool,
    #[cfg(test)]
    test_ready_sent: bool,
}

impl Default for Backend {
    fn default() -> Self {
        Self::with_audio(Rc::new(RefCell::new(None)))
    }
}

impl Backend {
    pub(super) fn with_audio(audio: Rc<RefCell<Option<AudioRuntime>>>) -> Self {
        Self {
            audio,
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
            volume_percent: 100,
            rate_percent: UNITY_RATE_PERCENT,
            buffered_percent: 0,
            #[cfg(test)]
            test_source: false,
            #[cfg(test)]
            test_ready_sent: false,
        }
    }

    pub(super) fn audio_handle(&self) -> Rc<RefCell<Option<AudioRuntime>>> {
        Rc::clone(&self.audio)
    }

    fn retire_sink(&mut self) -> Result<(), String> {
        let Some(sink) = self.sink.take() else {
            return Ok(());
        };
        if let Some(audio) = self.audio.borrow_mut().as_mut() {
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
                    open_http(url).map_err(|error| error.to_string())?;
                let source_label = representation
                    .final_url
                    .clone()
                    .unwrap_or_else(|| url.clone());
                (decoder, representation, source_label)
            },
            servo_media_player::controller::MediaSource::HostBlob { id } => {
                return Err(format!("host blob playback needs the embedding host: {id}"));
            },
        };
        // A local or published cache object is wholly seekable; the HTTP range
        // source keeps only a sliding window, so it can claim no buffered head.
        self.buffered_percent = match source {
            servo_media_player::controller::MediaSource::Local { .. } => 100,
            servo_media_player::controller::MediaSource::Http { .. }
            | servo_media_player::controller::MediaSource::HostBlob { .. } => 0,
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
        let actual = decoder.time_base.calc_time(seeked.actual_ts);
        self.base_ms = actual.seconds * 1000 + (actual.frac * 1000.0) as u64;
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
                let presented =
                    presented_frames(sink.accepted_frames, sink.rate, self.queued_seconds()?);
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
        self.volume_percent = percent;
        if let Some(audio) = self.audio.borrow_mut().as_mut() {
            audio.set_volume(percent);
        }
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
        if actual.complete_digest != expected.complete_digest
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
        if let Some(audio) = self.audio.borrow_mut().as_mut() {
            audio.tick().map_err(|error| error.to_string())?;
        }
        if self.decoder.is_none() {
            return Ok(PumpState::Idle);
        }
        if !self.decoded_first_frame {
            self.decode_next_packet()?;
            if !self.decoded_first_frame {
                return Ok(PumpState::Idle);
            }
        }
        let (rate, channels) = self
            .decoder
            .as_ref()
            .and_then(|decoder| decoder.channels.map(|channels| (decoder.rate, channels)))
            .ok_or("first decoded frame did not report a channel layout")?;
        if self.sink.is_none() {
            if self.audio.borrow().is_none() {
                let mut runtime = AudioRuntime::new().map_err(|error| error.to_string())?;
                runtime.set_volume(self.volume_percent);
                *self.audio.borrow_mut() = Some(runtime);
            }
            self.sink = Some(
                self.audio
                    .borrow_mut()
                    .as_mut()
                    .expect("audio runtime was initialized")
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
            stage.prune(presented_frames(sink.accepted_frames, sink.rate, queued));
        }
        Ok(if drained(self.eof_flushed, self.stretched.len(), queued) {
            PumpState::EndOfStream
        } else {
            PumpState::Idle
        })
    }

    fn decode_next_packet(&mut self) -> Result<(), String> {
        let decoder = self.decoder.as_mut().ok_or("no local decoder is loaded")?;
        match decode_packet(decoder)? {
            Decoded::Frames(samples) => {
                self.decoded_first_frame = true;
                self.pending.extend_from_slice(&samples);
            },
            Decoded::Skipped => {},
            Decoded::EndOfFile => self.eof = true,
        }
        Ok(())
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
    match decoder.format.next_packet() {
        Ok(packet) if packet.track_id() == decoder.track_id => {
            match decoder.decoder.decode(&packet) {
                Ok(decoded) => {
                    let spec = *decoded.spec();
                    let channels = spec.channels.count() as u32;
                    if let Some(expected) = decoder.channels
                        && expected != channels
                    {
                        return Err(format!(
                            "decoded channel count changed from {expected} to {channels}"
                        ));
                    }
                    decoder.channels = Some(channels);
                    let buffer = decoder
                        .sample_buffer
                        .get_or_insert_with(|| SampleBuffer::new(decoded.capacity() as u64, spec));
                    buffer.copy_interleaved_ref(decoded);
                    Ok(Decoded::Frames(buffer.samples().to_vec()))
                },
                Err(SymphoniaError::DecodeError(_)) => Ok(Decoded::Skipped),
                Err(error) => Err(format!("decode failed: {error}")),
            }
        },
        Ok(_) => Ok(Decoded::Skipped),
        Err(SymphoniaError::IoError(error))
            if error.kind() == std::io::ErrorKind::UnexpectedEof =>
        {
            Ok(Decoded::EndOfFile)
        },
        Err(error) => Err(format!("read failed: {error}")),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PumpState {
    Idle,
    Ready,
    EndOfStream,
}
