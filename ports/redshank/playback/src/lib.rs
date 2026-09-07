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
mod output;
mod worker;

pub(crate) use backend::Backend;
#[cfg(test)]
use backend::{consume_frames, drained, open_local, presented_ms};

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
    /// Host-supplied identity. Hosts reject snapshots for a stale selection.
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
            .map(|cell| cell.value.clone())
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
    fn unsupported_load_publishes_its_token_and_controller_error() {
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
                    url: "https://example.test/episode.mp3".into(),
                },
                resume_ms: 0,
            })
            .unwrap();
        let snapshot = wait_for(&runtime, |snapshot| {
            snapshot.load_token == Some(9)
                && matches!(snapshot.state, PlaybackState::Unavailable(_))
        });
        assert!(
            matches!(snapshot.state, PlaybackState::Unavailable(ref error) if error.contains("HTTP playback"))
        );
        assert_eq!(snapshot.position_ms, 0);
        assert_eq!(snapshot.duration_ms, None);
        assert_eq!(snapshot.source, None);
        assert_eq!(snapshot.representation, None);
    }

    #[test]
    fn sink_clock_helpers_preserve_frame_boundaries() {
        let mut pcm = vec![0.0; 10];
        consume_frames(&mut pcm, 3, 2);
        assert_eq!(pcm.len(), 4);
        assert_eq!(presented_ms(48_000, 48_000, 0.08), 920);
        assert!(!drained(true, &[0.0, 0.0], 0.0));
        assert!(drained(true, &[], 0.005));
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
}
