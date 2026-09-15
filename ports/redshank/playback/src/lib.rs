#![forbid(unsafe_code)]

//! Host-owned native playback for Redshank. The product facade owns load
//! tokens and representation receipts; the private controller adapters own all
//! decoder and output side effects through one backend.

use std::{
    sync::{Arc, Mutex, mpsc},
    thread,
    time::Instant,
};

use anyhow::{Result, anyhow};
use redshank_model::{MediaSource, RepresentationReceipt};

mod backend;
mod controller_adapter;
mod http_range;
mod output;
mod worker;

pub(crate) use backend::Backend;
#[cfg(test)]
use backend::{
    Decoded, RateStage, consume_frames, decode_packet, drained, open_http, open_local,
    presented_frames,
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
pub enum PreviewState {
    Loading,
    Playing,
    Ended,
    Unavailable(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreviewSnapshot {
    /// Host-supplied transient identity, such as an annotation ID.
    pub id: String,
    pub state: PreviewState,
    pub position_ms: u64,
    pub duration_ms: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlaybackSnapshot {
    /// Host-supplied identity. Hosts reject snapshots for a stale selection.
    pub load_token: Option<u64>,
    pub representation: Option<RepresentationReceipt>,
    pub state: PlaybackState,
    pub position_ms: u64,
    pub duration_ms: Option<u64>,
    pub source: Option<String>,
    /// The rate the retiming stage is actually running at. It equals the
    /// requested rate once clamped to what the stretcher supports.
    pub rate_percent: u16,
    pub volume_percent: u8,
    /// Percent of the source seekable without waiting on the network.
    pub buffered_percent: u8,
    /// A short secondary source using the same host-owned output authority.
    pub preview: Option<PreviewSnapshot>,
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
            rate_percent: 100,
            volume_percent: 100,
            buffered_percent: 0,
            preview: None,
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
    /// Requested playback rate in percent, retimed without moving pitch.
    /// Clamped to 50-200; the snapshot reports the rate actually in effect.
    SetRate(u16),
    /// Output volume in percent, applied as a Firewheel gain.
    SetVolume(u8),
    StartPreview {
        id: String,
        source: MediaSource,
    },
    StopPreview,
    Shutdown,
    #[cfg(test)]
    CrashWorker,
}

type Wake = Arc<dyn Fn() + Send + Sync>;
pub(crate) struct SnapshotCell {
    pub(crate) value: PlaybackSnapshot,
    pub(crate) wake: Option<Wake>,
    pub(crate) last_wake: Instant,
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
            .spawn(move || worker::run(receiver, worker_snapshot))
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
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .value
            .clone()
    }

    pub fn set_wake(&self, wake: Wake) {
        self.inner
            .snapshot
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .wake = Some(wake);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        path::PathBuf,
        thread,
        time::{Duration, Instant},
    };

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
    fn new_runtime_has_an_honest_empty_snapshot() {
        assert_eq!(
            PlaybackRuntime::start().snapshot().state,
            PlaybackState::Empty
        );
    }

    #[test]
    fn failed_remote_load_publishes_its_token_and_controller_error() {
        let runtime = PlaybackRuntime::start();
        runtime
            .command(PlaybackCommand::Load {
                token: 8,
                source: MediaSource::Local {
                    path: "redshank-test://previous-item".into(),
                },
                resume_ms: 0,
            })
            .unwrap();
        wait_for(&runtime, |snapshot| {
            snapshot.load_token == Some(8) && snapshot.state == PlaybackState::Paused
        });
        runtime
            .command(PlaybackCommand::Load {
                token: 9,
                source: MediaSource::Enclosure {
                    url: "redshank-test://missing/episode.mp3".into(),
                },
                resume_ms: 0,
            })
            .unwrap();
        let snapshot = wait_for(&runtime, |snapshot| {
            snapshot.load_token == Some(9)
                && matches!(snapshot.state, PlaybackState::Unavailable(_))
        });
        assert!(
            matches!(snapshot.state, PlaybackState::Unavailable(ref error) if error.contains("could not reach HTTP audio source")),
            "unexpected remote failure: {:?}",
            snapshot.state
        );
        assert_eq!(snapshot.position_ms, 0);
        assert_eq!(snapshot.duration_ms, None);
        assert_eq!(snapshot.source, None);
        assert_eq!(snapshot.representation, None);
    }

    #[test]
    fn cached_source_receipt_requires_the_published_bytes() {
        let mut backend = Backend::default();
        backend
            .load(&servo_media_player::controller::MediaSource::Local {
                path: "redshank-test://cached-integrity".into(),
            })
            .unwrap();
        backend
            .admit_cached_representation(&RepresentationReceipt::default())
            .unwrap();
        let mismatch = backend
            .admit_cached_representation(&RepresentationReceipt {
                byte_length: Some(1),
                complete_digest: Some("blake3:different".into()),
                ..Default::default()
            })
            .unwrap_err();
        assert!(mismatch.contains("failed integrity validation"));
    }

    #[test]
    fn captured_pcm_wav_is_decodable() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("voice-note.wav");
        let mut writer = hound::WavWriter::create(
            &path,
            hound::WavSpec {
                channels: 1,
                sample_rate: 48_000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for sample in 0..4_800 {
            writer
                .write_sample(if sample % 2 == 0 { i16::MAX } else { i16::MIN })
                .unwrap();
        }
        writer.finalize().unwrap();

        let (mut decoder, receipt) = open_local(&path).unwrap();
        let packet = decoder.format.next_packet().unwrap();
        let decoded = decoder.decoder.decode(&packet).unwrap();
        assert!(decoded.frames() > 0);
        assert_eq!(decoded.spec().channels.count(), 1);
        assert_eq!(decoded.spec().rate, 48_000);
        assert_eq!(receipt.byte_length, Some(9_644));
    }

    #[test]
    fn sink_clock_helpers_preserve_frame_boundaries() {
        let mut pcm = vec![0.0; 10];
        consume_frames(&mut pcm, 3, 2);
        assert_eq!(pcm.len(), 4);
        assert_eq!(presented_frames(48_000, 48_000, 0.08), 44_160);
        assert_eq!(presented_frames(100, 48_000, 1.0), 0);
        assert!(!drained(true, 4, 0.0));
        assert!(!drained(false, 0, 0.0));
        assert!(drained(true, 0, 0.005));
    }

    /// A pure tone pair written as PCM, decodable by the same Symphonia path
    /// a real enclosure takes. Returns the source frame count.
    fn synthetic_source(path: &PathBuf, seconds: u32) -> u64 {
        let rate = 48_000_u32;
        let mut writer = hound::WavWriter::create(
            path,
            hound::WavSpec {
                channels: 2,
                sample_rate: rate,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        let frames = rate * seconds;
        for frame in 0..frames {
            let phase = 2.0 * std::f32::consts::PI * frame as f32 / rate as f32;
            let scale = f32::from(i16::MAX) * 0.5;
            writer
                .write_sample(((phase * 440.0).sin() * scale) as i16)
                .unwrap();
            writer
                .write_sample(((phase * 660.0).sin() * scale) as i16)
                .unwrap();
        }
        writer.finalize().unwrap();
        u64::from(frames)
    }

    #[test]
    fn retiming_delivers_the_inverse_of_the_rate_in_source_time() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("rate.wav");
        let source_frames = synthetic_source(&path, 2);
        for percent in [80_u16, 120] {
            let (mut decoder, _) = open_local(&path).unwrap();
            let mut stage = RateStage::new(48_000, 2, percent);
            let mut output = Vec::new();
            loop {
                match decode_packet(&mut decoder).unwrap() {
                    Decoded::Frames(samples) => stage.push(&samples, &mut output),
                    Decoded::Skipped => {},
                    Decoded::EndOfFile => break,
                }
            }
            stage.flush(&mut output);

            // The sink is handed 100/percent of the source frames.
            let output_frames = (output.len() / 2) as u64;
            let expected = source_frames as f64 * 100.0 / f64::from(percent);
            assert!(
                (output_frames as f64 - expected).abs() / expected < 0.02,
                "{percent}%: {output_frames} output frames against {expected}"
            );

            // Reported position stays in source time end to end.
            assert_eq!(stage.source_frames_at(0), 0);
            assert_eq!(stage.source_frames_at(output_frames), source_frames);
            let middle = stage.source_frames_at(output_frames / 2) as f64;
            assert!(
                (middle - source_frames as f64 / 2.0).abs() / (source_frames as f64) < 0.02,
                "{percent}%: midpoint mapped to source frame {middle}"
            );

            // Output still queued is source time not yet heard, at the rate.
            let queued = 0.5_f64;
            let presented = presented_frames(output_frames, 48_000, queued);
            let behind = (source_frames - stage.source_frames_at(presented)) as f64;
            let expected_behind = queued * 48_000.0 * f64::from(percent) / 100.0;
            assert!(
                (behind - expected_behind).abs() / expected_behind < 0.05,
                "{percent}%: {behind} source frames behind, expected {expected_behind}"
            );
        }
    }

    #[test]
    fn unity_rate_leaves_the_decoded_bytes_alone() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("unity.wav");
        let source_frames = synthetic_source(&path, 1);
        let (mut decoder, _) = open_local(&path).unwrap();
        let mut stage = RateStage::new(48_000, 2, 100);
        let mut decoded = Vec::new();
        let mut output = Vec::new();
        loop {
            match decode_packet(&mut decoder).unwrap() {
                Decoded::Frames(samples) => {
                    decoded.extend_from_slice(&samples);
                    stage.push(&samples, &mut output);
                },
                Decoded::Skipped => {},
                Decoded::EndOfFile => break,
            }
        }
        stage.flush(&mut output);
        assert_eq!(output, decoded);
        assert_eq!(
            stage.source_frames_at((output.len() / 2) as u64),
            source_frames
        );
    }

    #[test]
    fn worker_controller_sequence_needs_no_output_device() {
        let runtime = PlaybackRuntime::start();
        runtime
            .command(PlaybackCommand::Load {
                token: 7,
                source: MediaSource::Local {
                    path: "redshank-test://controller-sequence".into(),
                },
                resume_ms: 0,
            })
            .unwrap();
        assert_eq!(
            wait_for(&runtime, |snapshot| {
                snapshot.load_token == Some(7) && snapshot.state == PlaybackState::Paused
            })
            .state,
            PlaybackState::Paused
        );
        runtime.command(PlaybackCommand::Pause).unwrap();
        runtime.command(PlaybackCommand::Seek(250)).unwrap();
        let seeked = wait_for(&runtime, |snapshot| {
            snapshot.load_token == Some(7) && snapshot.position_ms == 250
        });
        assert!(matches!(
            seeked.state,
            PlaybackState::Loading | PlaybackState::Paused
        ));
        runtime.command(PlaybackCommand::Play).unwrap();
        assert_eq!(
            wait_for(&runtime, |snapshot| snapshot.state == PlaybackState::Ended).state,
            PlaybackState::Ended
        );
    }

    #[test]
    fn levels_report_the_effective_rate_and_the_honest_buffered_extent() {
        let runtime = PlaybackRuntime::start();
        runtime
            .command(PlaybackCommand::Load {
                token: 21,
                source: MediaSource::Local {
                    path: "redshank-test://levels".into(),
                },
                resume_ms: 0,
            })
            .unwrap();
        let loaded = wait_for(&runtime, |snapshot| {
            snapshot.load_token == Some(21) && snapshot.state == PlaybackState::Paused
        });
        assert_eq!(loaded.buffered_percent, 100);
        runtime.command(PlaybackCommand::SetVolume(40)).unwrap();
        assert_eq!(
            wait_for(&runtime, |snapshot| snapshot.volume_percent == 40).volume_percent,
            40
        );
        // The retiming stage applies the rate, so the snapshot reports the
        // chosen rate rather than a silent 100.
        runtime.command(PlaybackCommand::SetRate(150)).unwrap();
        assert_eq!(
            wait_for(&runtime, |snapshot| snapshot.rate_percent == 150).rate_percent,
            150
        );
        // Outside the range the stage can hold honestly, the request clamps.
        runtime.command(PlaybackCommand::SetRate(400)).unwrap();
        assert_eq!(
            wait_for(&runtime, |snapshot| snapshot.rate_percent != 150).rate_percent,
            200
        );
    }

    #[test]
    fn voice_preview_preserves_the_loaded_episode_snapshot() {
        let runtime = PlaybackRuntime::start();
        runtime
            .command(PlaybackCommand::Load {
                token: 17,
                source: MediaSource::Local {
                    path: "redshank-test://episode".into(),
                },
                resume_ms: 250,
            })
            .unwrap();
        let episode = wait_for(&runtime, |snapshot| {
            snapshot.load_token == Some(17) && snapshot.state == PlaybackState::Paused
        });
        runtime
            .command(PlaybackCommand::StartPreview {
                id: "voice-note".into(),
                source: MediaSource::Local {
                    path: "redshank-test://voice-note".into(),
                },
            })
            .unwrap();
        let previewed = wait_for(&runtime, |snapshot| {
            snapshot
                .preview
                .as_ref()
                .is_some_and(|preview| preview.state == PreviewState::Ended)
        });
        assert_eq!(previewed.load_token, episode.load_token);
        assert_eq!(previewed.representation, episode.representation);
        assert_eq!(previewed.source, episode.source);
        assert_eq!(previewed.position_ms, episode.position_ms);
        assert_eq!(previewed.state, PlaybackState::Paused);
        assert_eq!(previewed.preview.unwrap().id, "voice-note");
    }

    #[test]
    fn worker_recovers_after_an_internal_failure() {
        let runtime = PlaybackRuntime::start();
        runtime.command(PlaybackCommand::CrashWorker).unwrap();
        let recovered = wait_for(
            &runtime,
            |snapshot| matches!(snapshot.state, PlaybackState::Unavailable(ref message) if message.contains("recovered")),
        );
        assert!(matches!(recovered.state, PlaybackState::Unavailable(_)));

        runtime
            .command(PlaybackCommand::Load {
                token: 12,
                source: MediaSource::Local {
                    path: "redshank-test://post-recovery".into(),
                },
                resume_ms: 0,
            })
            .unwrap();
        assert_eq!(
            wait_for(&runtime, |snapshot| {
                snapshot.load_token == Some(12) && snapshot.state == PlaybackState::Paused
            })
            .state,
            PlaybackState::Paused
        );
    }

    #[test]
    #[ignore = "requires REDSHANK_LOCAL_SEQUENCE or REDSHANK_PHASE4_FIXTURES and a default output device"]
    fn supplied_local_sources_can_be_replaced() {
        let runtime = PlaybackRuntime::start();
        let paths = std::env::var_os("REDSHANK_LOCAL_SEQUENCE")
            .map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
            .unwrap_or_else(|| vec![fixture("stereo.mp3"), fixture("stereo.m4a")]);
        assert_eq!(paths.len(), 2, "supply exactly two local audio paths");
        for (token, path) in [(60, &paths[0]), (61, &paths[1]), (62, &paths[0])] {
            runtime
                .command(PlaybackCommand::Load {
                    token,
                    source: MediaSource::Local {
                        path: path.display().to_string(),
                    },
                    resume_ms: 0,
                })
                .unwrap();
            let snapshot = wait_for(&runtime, |snapshot| {
                snapshot.load_token == Some(token) && snapshot.state != PlaybackState::Loading
            });
            assert_eq!(
                snapshot.state,
                PlaybackState::Paused,
                "{}: {snapshot:?}",
                path.display()
            );
            runtime.command(PlaybackCommand::Play).unwrap();
            let started = Instant::now();
            let playing = wait_for(&runtime, |snapshot| {
                snapshot.load_token == Some(token) && snapshot.position_ms >= 300
            });
            assert_eq!(
                playing.state,
                PlaybackState::Playing,
                "{}: {playing:?}",
                path.display()
            );
            assert!(
                started.elapsed() >= Duration::from_millis(200),
                "{} advanced 300 ms in {:?}",
                path.display(),
                started.elapsed()
            );
            runtime.command(PlaybackCommand::Pause).unwrap();
            let paused = wait_for(&runtime, |snapshot| {
                snapshot.load_token == Some(token) && snapshot.state == PlaybackState::Paused
            });
            assert_eq!(paused.state, PlaybackState::Paused);
        }
    }

    #[test]
    #[ignore = "requires REDSHANK_PHASE4_FIXTURES and a default output device"]
    fn supplied_fixture_plays_each_dock_rate_at_its_wall_clock() {
        // The timed span of source, measured between two reported source
        // positions so the output queue's one-off start-up latency drops out.
        const FROM_MS: u64 = 200;
        const TO_MS: u64 = 1_800;
        for (token, percent) in [(70, 80_u16), (71, 100), (72, 120), (73, 150)] {
            let runtime = PlaybackRuntime::start();
            runtime.command(PlaybackCommand::SetRate(percent)).unwrap();
            runtime
                .command(PlaybackCommand::Load {
                    token,
                    source: MediaSource::Local {
                        path: fixture("stereo.mp3").display().to_string(),
                    },
                    resume_ms: 0,
                })
                .unwrap();
            let loaded = wait_for(&runtime, |snapshot| {
                snapshot.load_token == Some(token) && snapshot.state != PlaybackState::Loading
            });
            assert_eq!(loaded.state, PlaybackState::Paused, "{loaded:?}");
            assert_eq!(loaded.rate_percent, percent);
            assert_eq!(loaded.duration_ms.map(|ms| ms / 100), Some(20));

            runtime.command(PlaybackCommand::Play).unwrap();
            let deadline = Instant::now() + Duration::from_secs(8);
            let until = |target: u64| loop {
                let snapshot = runtime.snapshot();
                if snapshot.position_ms >= target || Instant::now() >= deadline {
                    return (snapshot, Instant::now());
                }
                thread::sleep(Duration::from_millis(1));
            };
            let (opened, from) = until(FROM_MS);
            let (reached, to) = until(TO_MS);
            runtime.command(PlaybackCommand::Pause).unwrap();
            assert_eq!(reached.state, PlaybackState::Playing, "{reached:?}");
            assert!(reached.position_ms >= TO_MS, "{reached:?}");
            assert_eq!(reached.rate_percent, percent);
            // Source span divided by the rate is the wall clock it takes.
            let span = (reached.position_ms - opened.position_ms) as f64;
            let expected = span * 100.0 / f64::from(percent);
            let measured = (to - from).as_secs_f64() * 1000.0;
            println!(
                "{percent}%: {measured:.0} ms of wall clock for {span:.0} ms of source (expected {expected:.0} ms)"
            );
            // The floor here is the snapshot publication period, not the
            // retiming: supplied_fixtures_keep_their_pitch_at_every_dock_rate
            // checks the same ratio frame-exactly without a device.
            assert!(
                (measured - expected).abs() < expected * 0.03,
                "{percent}%: {measured:.0} ms against {expected:.0} ms"
            );
        }
    }

    /// Fundamental of one channel from hysteresis zero crossings, which a
    /// lossy decoder's noise floor cannot fake.
    fn fundamental(samples: &[f32], channels: usize, channel: usize) -> f32 {
        let peak = samples
            .iter()
            .skip(channel)
            .step_by(channels)
            .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
        let threshold = peak * 0.25;
        let mut armed = false;
        let mut first = None;
        let mut last = 0;
        let mut cycles = 0_u32;
        for frame in 0..samples.len() / channels {
            let sample = samples[frame * channels + channel];
            if sample < -threshold {
                armed = true;
            } else if armed && sample > threshold {
                armed = false;
                if first.is_none() {
                    first = Some(frame);
                } else {
                    cycles += 1;
                }
                last = frame;
            }
        }
        let first = first.expect("no cycles in the measured channel");
        f32::from(cycles as u16) * 48_000.0 / (last - first) as f32
    }

    #[test]
    #[ignore = "requires REDSHANK_PHASE4_FIXTURES with generated stereo fixtures"]
    fn supplied_fixtures_keep_their_pitch_at_every_dock_rate() {
        for name in ["stereo.mp3", "stereo.m4a"] {
            for percent in [80_u16, 100, 120, 150] {
                let (mut decoder, _) = open_local(&fixture(name)).unwrap();
                let mut stage = RateStage::new(48_000, 2, percent);
                let mut output = Vec::new();
                let mut source_frames = 0_u64;
                loop {
                    match decode_packet(&mut decoder).unwrap() {
                        Decoded::Frames(samples) => {
                            source_frames += (samples.len() / 2) as u64;
                            stage.push(&samples, &mut output);
                        },
                        Decoded::Skipped => {},
                        Decoded::EndOfFile => break,
                    }
                }
                stage.flush(&mut output);
                let frames = (output.len() / 2) as f64;
                let expected = source_frames as f64 * 100.0 / f64::from(percent);
                assert!(
                    (frames - expected).abs() / expected < 0.02,
                    "{name} at {percent}%: {frames} frames against {expected}"
                );
                // Skip the codec's leading silence and the flush tail.
                let measured = &output[4_800 * 2..output.len() - 4_800 * 2];
                let left = fundamental(measured, 2, 0);
                let right = fundamental(measured, 2, 1);
                println!("{name} at {percent}%: {frames} frames, {left:.1} Hz / {right:.1} Hz");
                assert!(
                    (left - 440.0).abs() / 440.0 < 0.01,
                    "{name} {percent}%: {left} Hz"
                );
                assert!(
                    (right - 660.0).abs() / 660.0 < 0.01,
                    "{name} {percent}%: {right} Hz"
                );
            }
        }
    }

    #[test]
    #[ignore = "requires REDSHANK_PHASE4_FIXTURES with generated stereo fixtures"]
    fn supplied_stereo_fixtures_preserve_two_distinct_channels() {
        for name in ["stereo.mp3", "stereo.m4a"] {
            let (mut decoder, _) = open_local(&fixture(name)).unwrap();
            let mut distinct = false;
            while let Ok(packet) = decoder.format.next_packet() {
                if packet.track_id() != decoder.track_id {
                    continue;
                }
                let decoded = decoder.decoder.decode(&packet).unwrap();
                let spec = *decoded.spec();
                assert_eq!(spec.channels.count(), 2);
                let mut samples = symphonia::core::audio::SampleBuffer::<f32>::new(
                    decoded.capacity() as u64,
                    spec,
                );
                samples.copy_interleaved_ref(decoded);
                distinct |= samples
                    .samples()
                    .chunks_exact(2)
                    .any(|frame| (frame[0] - frame[1]).abs() > 0.01);
                if distinct {
                    break;
                }
            }
            assert!(distinct, "{name} lost stereo separation");
        }
    }

    #[test]
    #[ignore = "requires REDSHANK_PHASE4_FIXTURES with generated stereo fixtures"]
    fn supplied_fixture_resumes_through_the_controller() {
        let runtime = PlaybackRuntime::start();
        runtime
            .command(PlaybackCommand::Load {
                token: 42,
                source: MediaSource::Local {
                    path: fixture("stereo.mp3").display().to_string(),
                },
                resume_ms: 1_000,
            })
            .unwrap();
        let snapshot = wait_for(&runtime, |snapshot| {
            snapshot.load_token == Some(42) && matches!(snapshot.state, PlaybackState::Paused)
        });
        assert!((800..=1_200).contains(&snapshot.position_ms));
        assert!(
            snapshot
                .representation
                .and_then(|receipt| receipt.complete_digest)
                .is_some()
        );
    }

    #[test]
    #[ignore = "requires REDSHANK_HTTP_FIXTURE_URL served with byte ranges"]
    fn supplied_http_fixture_decodes_progressively() {
        let url = std::env::var("REDSHANK_HTTP_FIXTURE_URL")
            .expect("set REDSHANK_HTTP_FIXTURE_URL to a range-served MP3 fixture");
        let (mut decoder, receipt) = open_http(&url).unwrap();
        assert_eq!(receipt.requested_url.as_deref(), Some(url.as_str()));
        assert!(receipt.byte_length.is_some());
        loop {
            let packet = decoder.format.next_packet().unwrap();
            if packet.track_id() != decoder.track_id {
                continue;
            }
            let decoded = decoder.decoder.decode(&packet).unwrap();
            assert!(decoded.frames() > 0);
            break;
        }
    }

    #[test]
    #[ignore = "requires REDSHANK_HTTP_FIXTURE_URL and a default output device"]
    fn supplied_http_fixture_reaches_controller_ready() {
        let url = std::env::var("REDSHANK_HTTP_FIXTURE_URL")
            .expect("set REDSHANK_HTTP_FIXTURE_URL to a range-served MP3 fixture");
        let runtime = PlaybackRuntime::start();
        runtime
            .command(PlaybackCommand::Load {
                token: 51,
                source: MediaSource::Enclosure { url },
                resume_ms: 400,
            })
            .unwrap();
        let snapshot = wait_for(&runtime, |snapshot| {
            snapshot.load_token == Some(51) && snapshot.state != PlaybackState::Loading
        });
        assert_eq!(snapshot.state, PlaybackState::Paused, "{snapshot:?}");
        assert!((300..=500).contains(&snapshot.position_ms));
        assert!(snapshot.representation.is_some());
    }
}
