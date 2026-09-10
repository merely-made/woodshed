use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{Context, Result, anyhow};
use firewheel::{
    node::NodeID,
    nodes::stream::writer::{PushStatus, StreamWriterState},
};
use redshank_model::RepresentationReceipt;
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
    output::AudioRuntime,
};

pub(super) struct Sink {
    pub(super) state: Arc<Mutex<StreamWriterState>>,
    pub(super) node: NodeID,
    pub(super) accepted_frames: u64,
    pub(super) rate: u32,
    pub(super) channels: usize,
}

pub(super) fn presented_ms(accepted_frames: u64, rate: u32, queued_seconds: f64) -> u64 {
    ((((accepted_frames as f64 / rate as f64) - queued_seconds).max(0.0)) * 1000.0) as u64
}

pub(super) fn consume_frames(pending: &mut Vec<f32>, accepted_frames: usize, channels: usize) {
    pending.drain(..accepted_frames.saturating_mul(channels).min(pending.len()));
}

pub(super) fn drained(eof: bool, pending: &[f32], queued_seconds: f64) -> bool {
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
            .ok_or_else(|| anyhow!("audio writer did not report queue occupancy"))?;
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

#[derive(Default)]
pub(super) struct Backend {
    audio: Option<AudioRuntime>,
    decoder: Option<Decoder>,
    sink: Option<Sink>,
    pending: Vec<f32>,
    eof: bool,
    source: Option<String>,
    representation: Option<RepresentationReceipt>,
    base_ms: u64,
    requested_playing: bool,
    source_positioned: bool,
    decoded_first_frame: bool,
    #[cfg(test)]
    test_source: bool,
    #[cfg(test)]
    test_ready_sent: bool,
}

impl Backend {
    fn retire_sink(&mut self) -> Result<(), String> {
        let Some(sink) = self.sink.take() else {
            return Ok(());
        };
        if let Some(audio) = self.audio.as_mut() {
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
            self.pending.clear();
            self.eof = false;
            self.base_ms = 0;
            self.requested_playing = false;
            self.source_positioned = true;
            self.decoded_first_frame = false;
            self.test_source = true;
            self.test_ready_sent = false;
            return Ok(servo_media_player::controller::MediaInfo {
                duration: Some(Duration::from_secs(1)),
                capabilities: servo_media_player::controller::PlaybackCapabilities {
                    seekable: true,
                    playback_rates: None,
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
        let duration = decoder.duration_ms.map(Duration::from_millis);
        self.decoder = Some(decoder);
        self.representation = Some(representation);
        self.source = Some(source_label);
        self.pending.clear();
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
                playback_rates: None,
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
        self.pending.clear();
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

    pub(super) fn position(&self) -> Result<Duration, String> {
        let played = self
            .sink
            .as_ref()
            .map(|sink| sink.presented_ms())
            .transpose()
            .map_err(|error| error.to_string())?
            .unwrap_or(0)
            .saturating_add(self.base_ms);
        Ok(Duration::from_millis(played))
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
        self.pending.clear();
        self.decoder = None;
        self.source = None;
        self.representation = None;
        self.base_ms = 0;
        self.source_positioned = false;
        self.decoded_first_frame = false;
        #[cfg(test)]
        {
            self.test_source = false;
            self.test_ready_sent = false;
        }
        let retired = self.retire_sink();
        self.audio = None;
        retired
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
        if let Some(audio) = self.audio.as_mut() {
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
            if self.audio.is_none() {
                self.audio = Some(AudioRuntime::new().map_err(|error| error.to_string())?);
            }
            self.sink = Some(
                self.audio
                    .as_mut()
                    .expect("audio runtime was initialized")
                    .sink(rate, channels)
                    .map_err(|error| error.to_string())?,
            );
            return Ok(PumpState::Ready);
        }
        if !self.requested_playing {
            return Ok(PumpState::Idle);
        }
        if !self.pending.is_empty() {
            let (accepted, channels) = {
                let sink = self.sink.as_mut().ok_or("audio sink was not initialized")?;
                (
                    sink.push(&self.pending)
                        .map_err(|error| error.to_string())?,
                    sink.channels,
                )
            };
            consume_frames(&mut self.pending, accepted, channels);
        }
        if self.pending.is_empty() && !self.eof && self.queued_seconds()? < 0.15 {
            self.decode_next_packet()?;
        }
        Ok(
            if drained(self.eof, &self.pending, self.queued_seconds()?) {
                PumpState::EndOfStream
            } else {
                PumpState::Idle
            },
        )
    }

    fn decode_next_packet(&mut self) -> Result<(), String> {
        let decoded = {
            let decoder = self.decoder.as_mut().ok_or("no local decoder is loaded")?;
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
                            let buffer = decoder.sample_buffer.get_or_insert_with(|| {
                                SampleBuffer::new(decoded.capacity() as u64, spec)
                            });
                            buffer.copy_interleaved_ref(decoded);
                            Some(buffer.samples().to_vec())
                        },
                        Err(SymphoniaError::DecodeError(_)) => None,
                        Err(error) => return Err(format!("decode failed: {error}")),
                    }
                },
                Ok(_) => None,
                Err(SymphoniaError::IoError(error))
                    if error.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    self.eof = true;
                    None
                },
                Err(error) => return Err(format!("read failed: {error}")),
            }
        };
        if let Some(samples) = decoded {
            self.decoded_first_frame = true;
            self.pending.extend_from_slice(&samples);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PumpState {
    Idle,
    Ready,
    EndOfStream,
}
