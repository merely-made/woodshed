//! Bounded mono input handoff to the output processor.
//!
//! The input analyzer owns the sole producer and the output processor owns
//! the sole consumer. Allocation and consumer extraction happen on control
//! threads. Queue operations never lock or allocate. A dropped sample is
//! counted explicitly: an armed/recording take must fail if that count changes.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

#[derive(Clone, Copy)]
struct Frame {
    sample: f32,
    generation: u64,
}

struct State {
    // Even generations are disabled; odd generations are enabled. Frames
    // carry this token so reset/disable cannot expose an earlier take.
    generation: AtomicU64,
    source_rate: AtomicU32,
    overflow: AtomicU64,
}

#[derive(Clone)]
pub(crate) struct CaptureFeedControl(Arc<State>);

impl CaptureFeedControl {
    pub(crate) fn set_enabled(&self, enabled: bool) {
        let _ = self
            .0
            .generation
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |generation| {
                ((generation & 1 != 0) != enabled).then(|| generation.wrapping_add(1))
            });
    }

    pub(crate) fn is_enabled(&self) -> bool {
        self.0.generation.load(Ordering::Acquire) & 1 != 0
    }

    pub(crate) fn clear(&self) {
        self.0.generation.fetch_add(2, Ordering::AcqRel);
    }

    pub(crate) fn source_rate(&self) -> u32 {
        self.0.source_rate.load(Ordering::Acquire)
    }
}

pub(crate) struct CaptureFeedProducer {
    queue: rtrb::Producer<Frame>,
    control: CaptureFeedControl,
}

impl CaptureFeedProducer {
    pub(crate) fn prepare(&mut self, source_rate: u32) {
        self.control
            .0
            .source_rate
            .store(source_rate, Ordering::Release);
    }

    pub(crate) fn push_samples(&mut self, samples: &[f32]) {
        let generation = self.control.0.generation.load(Ordering::Acquire);
        if generation & 1 == 0 {
            return;
        }
        for &sample in samples {
            if self.queue.push(Frame { sample, generation }).is_err() {
                self.control.0.overflow.fetch_add(1, Ordering::Release);
            }
        }
    }
}

/// Move-only consumer owned by the output processor after attachment.
pub struct CaptureFeedConsumer {
    queue: rtrb::Consumer<Frame>,
    control: CaptureFeedControl,
}

/// Capture currently requires matching, known input and output rates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaptureRateMismatch {
    pub source_rate: u32,
    pub output_rate: u32,
}

impl std::fmt::Display for CaptureRateMismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "capture needs equal nonzero input/output rates (input {}, output {})",
            self.source_rate, self.output_rate
        )
    }
}

impl std::error::Error for CaptureRateMismatch {}

impl CaptureFeedConsumer {
    /// Pop one current-generation sample. At most the currently queued
    /// slots are examined, even if the producer is concurrently active.
    pub fn pop_sample(&mut self) -> Option<f32> {
        for _ in 0..self.queue.slots() {
            let frame = self.queue.pop().ok()?;
            let generation = self.control.0.generation.load(Ordering::Acquire);
            if generation & 1 != 0 && frame.generation == generation {
                return Some(frame.sample);
            }
        }
        None
    }

    /// Lifetime count of rejected samples. Snapshot before arming and
    /// compare throughout the take, including before publishing its buffer.
    pub fn overflow_count(&self) -> u64 {
        self.control.0.overflow.load(Ordering::Acquire)
    }

    /// Discard a bounded snapshot of pending samples. To reset concurrently
    /// produced audio as well, call `LooperCaptureHandle::clear` first.
    pub fn discard_pending(&mut self) {
        for _ in 0..self.queue.slots() {
            let _ = self.queue.pop();
        }
    }

    /// Actual CPAL input configuration, published before the stream starts.
    /// Zero means the analyzer has not yet been prepared.
    pub fn source_rate(&self) -> u32 {
        self.control.source_rate()
    }

    pub fn validate_output_rate(&self, output_rate: u32) -> Result<(), CaptureRateMismatch> {
        let source_rate = self.source_rate();
        if source_rate != 0 && source_rate == output_rate {
            Ok(())
        } else {
            Err(CaptureRateMismatch {
                source_rate,
                output_rate,
            })
        }
    }
}

pub(crate) fn channel(
    capacity: usize,
) -> (CaptureFeedProducer, CaptureFeedConsumer, CaptureFeedControl) {
    let (producer, consumer) = rtrb::RingBuffer::new(capacity);
    let control = CaptureFeedControl(Arc::new(State {
        generation: AtomicU64::new(0),
        source_rate: AtomicU32::new(0),
        overflow: AtomicU64::new(0),
    }));
    (
        CaptureFeedProducer {
            queue: producer,
            control: control.clone(),
        },
        CaptureFeedConsumer {
            queue: consumer,
            control: control.clone(),
        },
        control,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overflow_is_reported_without_evicting_accepted_frames() {
        let (mut producer, mut consumer, control) = channel(2);
        control.set_enabled(true);
        let before = consumer.overflow_count();
        producer.push_samples(&[0.1, 0.2, 0.3, 0.4]);
        assert_eq!(consumer.overflow_count() - before, 2);
        assert_eq!(consumer.pop_sample(), Some(0.1));
        assert_eq!(consumer.pop_sample(), Some(0.2));
        assert_eq!(consumer.pop_sample(), None);
        producer.push_samples(&[0.5]);
        assert_eq!(consumer.pop_sample(), Some(0.5));
        // Draining does not erase the evidence that the take lost frames.
        assert_eq!(consumer.overflow_count() - before, 2);
    }

    #[test]
    fn disable_and_clear_exclude_previous_generations() {
        let (mut producer, mut consumer, control) = channel(8);
        producer.push_samples(&[0.1]);
        assert_eq!(consumer.pop_sample(), None);
        control.set_enabled(true);
        producer.push_samples(&[0.2]);
        control.set_enabled(false);
        producer.push_samples(&[0.3]);
        control.set_enabled(true);
        producer.push_samples(&[0.4]);
        assert_eq!(consumer.pop_sample(), Some(0.4));
        producer.push_samples(&[0.5]);
        control.clear();
        assert!(control.is_enabled());
        producer.push_samples(&[0.6]);
        assert_eq!(consumer.pop_sample(), Some(0.6));
        assert_eq!(consumer.pop_sample(), None);
    }

    #[test]
    fn rate_admission_requires_actual_matching_configuration() {
        let (mut producer, consumer, _) = channel(2);
        assert_eq!(consumer.source_rate(), 0);
        assert!(consumer.validate_output_rate(48_000).is_err());
        producer.prepare(44_100);
        assert_eq!(consumer.source_rate(), 44_100);
        assert!(consumer.validate_output_rate(44_100).is_ok());
        assert_eq!(
            consumer.validate_output_rate(48_000),
            Err(CaptureRateMismatch {
                source_rate: 44_100,
                output_rate: 48_000,
            })
        );
        assert!(consumer.validate_output_rate(0).is_err());
    }

    #[test]
    fn discard_consumes_only_pending_samples() {
        let (mut producer, mut consumer, control) = channel(4);
        control.set_enabled(true);
        producer.push_samples(&[0.1, 0.2]);
        consumer.discard_pending();
        producer.push_samples(&[0.3]);
        assert_eq!(consumer.pop_sample(), Some(0.3));
    }
}
