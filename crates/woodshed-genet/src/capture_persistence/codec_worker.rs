//! One host-owned codec lane. No device, storage, UI or custody keys cross it.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, mpsc};

use woodshed_core::loop_capture::{EncodedCapture, encode_capture};

use super::{CaptureSnapshot, same_take};

type Encoder = Arc<dyn Fn(u32, &[f32]) -> Result<EncodedCapture, String> + Send + Sync>;

pub(super) struct PreparedTake {
    pub take: CaptureSnapshot,
    pub encoded: Arc<EncodedCapture>,
}

struct Job {
    generation: u64,
    epoch: Arc<()>,
    takes: Vec<CaptureSnapshot>,
}
struct Completion {
    generation: u64,
    epoch: Arc<()>,
    result: Result<Vec<PreparedTake>, String>,
}
struct Lane {
    sender: mpsc::SyncSender<Job>,
    receiver: mpsc::Receiver<Completion>,
}

#[derive(Default)]
pub(super) struct CodecWorker {
    lane: Option<Lane>,
    generation: Arc<AtomicU64>,
    ready: Arc<AtomicBool>,
    epoch: Option<Arc<()>>,
    busy: bool,
    prepared: Vec<PreparedTake>,
    wake: Option<Arc<dyn Fn() + Send + Sync>>,
    encoder: Option<Encoder>,
}

impl CodecWorker {
    pub fn set_wake(&mut self, wake: Arc<dyn Fn() + Send + Sync>) {
        // Boot installs this before the first capture/migration job.
        self.wake = Some(wake);
    }

    pub fn is_busy(&self) -> bool {
        self.busy
    }

    pub fn completion_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }

    pub fn cancel_epoch(&mut self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.epoch = None;
        self.prepared.clear();
        // Keep the one active lane reserved until its cancelled completion is
        // collected. Rapid resets cannot spawn more workers or queue old jobs.
    }

    pub fn prepared(&self) -> &[PreparedTake] {
        &self.prepared
    }

    pub fn clear_prepared(&mut self) {
        self.prepared.clear();
    }

    pub fn encoded(&self, take: &CaptureSnapshot) -> Option<&EncodedCapture> {
        self.prepared
            .iter()
            .find(|prepared| same_take(&prepared.take, take))
            .map(|prepared| prepared.encoded.as_ref())
    }

    pub fn prepare(
        &mut self,
        epoch: &Arc<()>,
        current: &[CaptureSnapshot],
        required: Vec<CaptureSnapshot>,
    ) -> Result<bool, String> {
        if self
            .epoch
            .as_ref()
            .is_none_or(|old| !Arc::ptr_eq(old, epoch))
        {
            self.cancel_epoch();
            self.epoch = Some(epoch.clone());
        }
        self.prepared
            .retain(|prepared| current.iter().any(|take| same_take(&prepared.take, take)));
        if self.busy {
            let completion = match self
                .lane
                .as_ref()
                .expect("active codec lane")
                .receiver
                .try_recv()
            {
                Ok(completion) => Some(completion),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.busy = false;
                    self.lane = None;
                    return Err("Captured-loop codec worker stopped unexpectedly.".into());
                },
            };
            if let Some(completion) = completion {
                self.busy = false;
                self.ready.store(false, Ordering::Release);
                if completion.generation == self.generation.load(Ordering::Acquire)
                    && Arc::ptr_eq(&completion.epoch, epoch)
                {
                    for prepared in completion.result? {
                        if current.iter().any(|take| same_take(&prepared.take, take)) {
                            self.prepared.push(prepared);
                        }
                    }
                }
            }
        }
        let missing: Vec<_> = required
            .into_iter()
            .filter(|take| self.encoded(take).is_none())
            .collect();
        if missing.is_empty() {
            return Ok(true);
        }
        if self.busy {
            return Ok(false);
        }
        self.ensure_lane()?;
        self.ready.store(false, Ordering::Release);
        self.lane
            .as_ref()
            .unwrap()
            .sender
            .try_send(Job {
                generation: self.generation.load(Ordering::Acquire),
                epoch: epoch.clone(),
                takes: missing,
            })
            .map_err(|_| "Captured-loop codec lane refused its bounded job.".to_string())?;
        self.busy = true;
        Ok(false)
    }

    fn ensure_lane(&mut self) -> Result<(), String> {
        if self.lane.is_some() {
            return Ok(());
        }
        let (sender, jobs) = mpsc::sync_channel::<Job>(1);
        let (results, receiver) = mpsc::sync_channel(1);
        let generation = self.generation.clone();
        let ready = self.ready.clone();
        let wake = self.wake.clone();
        let encoder = self
            .encoder
            .clone()
            .unwrap_or_else(|| Arc::new(encode_capture));
        std::thread::Builder::new()
            .name("woodshed-capture-codec".into())
            .spawn(move || {
                while let Ok(job) = jobs.recv() {
                    let result = (|| {
                        let mut prepared = Vec::with_capacity(job.takes.len());
                        for take in job.takes {
                            if generation.load(Ordering::Acquire) != job.generation {
                                return Ok(Vec::new());
                            }
                            let encoded = encoder(take.buffer.sample_rate_hz, &take.buffer.data)?;
                            if generation.load(Ordering::Acquire) != job.generation {
                                return Ok(Vec::new());
                            }
                            prepared.push(PreparedTake {
                                take,
                                encoded: Arc::new(encoded),
                            });
                        }
                        Ok(prepared)
                    })();
                    ready.store(true, Ordering::Release);
                    if results
                        .send(Completion {
                            generation: job.generation,
                            epoch: job.epoch,
                            result,
                        })
                        .is_err()
                    {
                        break;
                    }
                    if let Some(wake) = &wake {
                        wake();
                    }
                }
            })
            .map_err(|error| format!("Cannot start captured-loop codec worker: {error}"))?;
        self.lane = Some(Lane { sender, receiver });
        Ok(())
    }

    #[cfg(test)]
    pub fn with_encoder(encoder: Encoder) -> Self {
        let mut worker = Self::default();
        worker.encoder = Some(encoder);
        worker
    }
}

impl Drop for CodecWorker {
    fn drop(&mut self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        // Closing the only sender ends the worker after its current encode.
        // Native close waits through host wake service; no UI-thread join.
    }
}
