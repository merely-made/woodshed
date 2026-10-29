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
use fetch::Fetch;
use redshank_model::{MediaSource, RepresentationReceipt};

#[cfg(feature = "native")]
mod alignment;
#[cfg(feature = "native")]
mod backend;
mod fingerprint;
#[cfg(feature = "native")]
pub use alignment::realign_local;
pub use fingerprint::FingerprintContext;
#[cfg(not(feature = "native"))]
pub fn realign_local(
    _: &std::path::Path,
    _: &redshank_model::RepresentationReceipt,
    _: &redshank_model::TimedTarget,
    _: &redshank_model::ListenerSettings,
) -> std::result::Result<redshank_model::DerivedPosition, String> {
    Err("Native audio is not included in this build".into())
}
#[cfg(feature = "native")]
mod controller_adapter;
#[cfg(feature = "native")]
mod http_range;
#[cfg(feature = "native")]
mod output;
#[cfg(feature = "native")]
mod worker;

#[cfg(feature = "native")]
pub(crate) use backend::Backend;
#[cfg(all(test, feature = "native"))]
use backend::{Decoded, RateStage, consume_frames, decode_packet, drained, open_http, open_local};

/// Supplied, already-admitted bytes. The player never resolves a host blob as a path or URL.
pub type HostBlobReader = Arc<dyn Fn(&str) -> std::result::Result<Vec<u8>, String> + Send + Sync>;
pub const MAX_HOST_BLOB_BYTES: usize = 16 * 1024 * 1024;
/// Whether this build installs the decoder and native output implementation.
pub const NATIVE_AUDIO_AVAILABLE: bool = cfg!(feature = "native");

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
    /// The last successful note seek, identified by the host's request ID.
    pub completed_note_seek: Option<u64>,
    pub representation: Option<RepresentationReceipt>,
    /// Bounded, original-speed context ending at the presented source clock.
    pub fingerprint_context: Option<Arc<FingerprintContext>>,
    pub state: PlaybackState,
    pub position_ms: u64,
    pub duration_ms: Option<u64>,
    pub source: Option<String>,
    /// The controller-admitted rate for the main source. Between sources this
    /// retains the last admitted rate; unloaded preferences wait for Load.
    pub rate_percent: u16,
    /// The listener's volume. While ducked, the gain in effect is this
    /// scaled by `duck_percent`; the listener's number does not move.
    pub volume_percent: u8,
    /// The duck in effect for a capture, as a percent of the volume.
    pub duck_percent: Option<u8>,
    /// Percent of the source seekable without waiting on the network.
    pub buffered_percent: u8,
    /// A short secondary source using the same host-owned output authority.
    pub preview: Option<PreviewSnapshot>,
}

impl Default for PlaybackSnapshot {
    fn default() -> Self {
        Self {
            load_token: None,
            completed_note_seek: None,
            representation: None,
            fingerprint_context: None,
            state: PlaybackState::Empty,
            position_ms: 0,
            duration_ms: None,
            source: None,
            rate_percent: 100,
            volume_percent: 100,
            duck_percent: None,
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
    /// Seek a note only if its source is still loaded; acknowledge this request
    /// ID once the resulting position has been published.
    SeekNote {
        token: u64,
        request_id: u64,
        position_ms: u64,
    },
    /// Requested playback rate in percent, retimed without moving pitch.
    /// Clamped to 50-200; the snapshot reports the rate actually in effect.
    SetRate(u16),
    /// Output volume in percent, applied as a Firewheel gain.
    SetVolume(u8),
    /// Scale the volume to this percent of itself for the length of a
    /// capture. The same gain, lowered; the listener's volume is untouched.
    Duck(u8),
    /// Restore the volume after a capture.
    Unduck,
    StartPreview {
        id: String,
        source: MediaSource,
    },
    StopPreview,
    Shutdown,
    #[cfg(test)]
    CrashWorker,
    /// Inspect actual backend/controller controls without requiring a device.
    #[cfg(test)]
    InspectControls(mpsc::Sender<(u16, Option<u16>, u8)>),
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

/// A fetch handle of the runtime's own, over in-memory stores: what a host
/// without a session of its own gets. A host with one passes its own handle to
/// [`PlaybackRuntime::start_with`] so playback shares its cookies, cache and wire.
pub fn own_fetch() -> Arc<dyn Fetch> {
    Arc::new(fetch::NetFetch::new(&fetch::Stores::in_memory()).expect("build the fetch runtime"))
}

impl PlaybackRuntime {
    pub fn start() -> Self {
        Self::start_with(own_fetch())
    }

    /// Start with the host's fetch handle.
    pub fn start_with(fetch: Arc<dyn Fetch>) -> Self {
        Self::start_with_blobs(fetch, None)
    }

    /// Start with a bounded host-blob source, for retained private audio.
    pub fn start_with_blobs(fetch: Arc<dyn Fetch>, blobs: Option<HostBlobReader>) -> Self {
        let (sender, receiver) = mpsc::channel();
        let snapshot = Arc::new(Mutex::new(SnapshotCell {
            value: PlaybackSnapshot::default(),
            wake: None,
            last_wake: Instant::now(),
        }));
        #[cfg(feature = "native")]
        let worker_snapshot = Arc::clone(&snapshot);
        #[cfg(not(feature = "native"))]
        {
            drop((receiver, fetch, blobs));
            snapshot.lock().unwrap().value.state =
                PlaybackState::Unavailable("Native audio is not included in this build".into());
        }
        #[cfg(feature = "native")]
        thread::Builder::new()
            .name("redshank-playback".into())
            .spawn(move || worker::run(receiver, worker_snapshot, fetch, blobs))
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

#[cfg(all(test, feature = "native"))]
mod tests;

#[cfg(all(test, not(feature = "native")))]
mod contract_tests;
