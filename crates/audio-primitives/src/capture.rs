// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0

//! Allocation-free accumulation into caller-owned, initialized PCM storage.
//! The host owns capture timing, input integrity, storage and completion
//! admission. Shared state expresses only sample writes and observed bar wraps.

/// Move-only completion envelope. Storage can be a PCM buffer or a deferred-
/// destruction handle; the host stamp retains its own clock, generation and
/// admission facts. Constructing/moving this value does not allocate or copy PCM.
/// Completion alone does not imply that a host has admitted or persisted a take.
#[derive(Debug)]
pub struct CompletedTake<Storage, Stamp> {
    pub storage: Storage,
    pub stamp: Stamp,
}

/// Minimal mono input seam. Implementations retain stream and generation ownership.
/// `None` means no sample is available now, not end-of-stream. Implementations
/// used on an audio thread must neither allocate nor block.
pub trait InputFeed {
    fn pop_sample(&mut self) -> Option<f32>;
}

/// A borrowed, already-drained batch; no driver or stream ownership.
impl InputFeed for std::slice::Iter<'_, f32> {
    fn pop_sample(&mut self) -> Option<f32> {
        self.next().copied()
    }
}

/// Poll-driven countdown, preserving Hocket's half-bar jitter gate.
/// The host supplies bar length and suppresses wraps during clock restart.
/// A backwards phase gap must exceed half a bar: smaller gaps may be stale
/// cross-thread playhead observations. The host must poll often enough to see
/// each wrap; this cannot recover multiple wraps between observations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BarCountdown {
    remaining: u8,
    last_phase: usize,
}

impl BarCountdown {
    pub fn new(remaining: u8, last_phase: usize) -> Self {
        Self {
            remaining,
            last_phase,
        }
    }

    pub fn remaining(&self) -> u8 {
        self.remaining
    }

    /// Observe a phase even when suppressed, so the next tick uses a fresh
    /// baseline. Returns true once the requested number of wraps has passed.
    pub fn observe(&mut self, phase: usize, bar_frames: usize, suppress: bool) -> bool {
        if !suppress && phase < self.last_phase && self.last_phase - phase > bar_frames / 2 {
            self.remaining = self.remaining.saturating_sub(1);
        }
        self.last_phase = phase;
        self.remaining == 0
    }
}

/// How a sample changes its destination. Append preserves raw input bits;
/// indexed modes retain Woodshed's existing clipping behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteMode {
    Append,
    IndexedReplace,
    IndexedOverdubClamped,
}

/// Cursor and write count only; never owns or resizes sample storage.
#[derive(Clone, Debug, Default)]
pub struct TakeAccumulator {
    appended: usize,
}

impl TakeAccumulator {
    /// Consume only the remaining append capacity. Unwritten input stays in
    /// the feed for its caller to discard or reuse according to host policy.
    pub fn append_from(&mut self, storage: &mut [f32], input: &mut impl InputFeed) -> usize {
        let before = self.appended;
        while self.appended < storage.len() {
            let Some(sample) = input.pop_sample() else {
                break;
            };
            self.write(storage, WriteMode::Append, 0, sample);
        }
        self.appended - before
    }

    pub fn reset(&mut self) {
        self.appended = 0;
    }

    pub fn appended(&self) -> usize {
        self.appended
    }

    /// Write one sample. Append ignores `index` and stops at storage length;
    /// indexed modes use the supplied index exactly, without wrapping.
    /// Returns false for an exhausted or out-of-range destination.
    pub fn write(
        &mut self,
        storage: &mut [f32],
        mode: WriteMode,
        index: usize,
        input: f32,
    ) -> bool {
        let index = if mode == WriteMode::Append {
            self.appended
        } else {
            index
        };
        let Some(destination) = storage.get_mut(index) else {
            return false;
        };
        *destination = match mode {
            WriteMode::Append => input,
            WriteMode::IndexedReplace => input.clamp(-1.0, 1.0),
            WriteMode::IndexedOverdubClamped => (*destination + input).clamp(-1.0, 1.0),
        };
        if mode == WriteMode::Append {
            self.appended += 1;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_retains_bits_and_stops_at_capacity() {
        let input = [-0.0, 2.0, f32::from_bits(0x7fc00123)];
        let mut storage = [0.0; 3];
        let mut take = TakeAccumulator::default();
        for value in input {
            assert!(take.write(&mut storage, WriteMode::Append, usize::MAX, value));
        }
        assert!(!take.write(&mut storage, WriteMode::Append, 0, 9.0));
        assert_eq!(storage.map(f32::to_bits), input.map(f32::to_bits));
        assert_eq!(take.appended(), 3);
        take.reset();
        assert!(take.write(&mut storage, WriteMode::Append, 0, 0.5));
        assert_eq!(storage[0], 0.5);
    }

    #[test]
    fn indexed_modes_preserve_clipping_and_repeated_overdubs() {
        let mut storage = [0.75, -0.75];
        let mut take = TakeAccumulator::default();
        assert!(take.write(&mut storage, WriteMode::IndexedOverdubClamped, 0, 0.5));
        assert!(take.write(&mut storage, WriteMode::IndexedOverdubClamped, 1, -0.5));
        assert_eq!(storage, [1.0, -1.0]);
        assert!(take.write(&mut storage, WriteMode::IndexedOverdubClamped, 0, -0.25));
        assert!(take.write(&mut storage, WriteMode::IndexedReplace, 1, 2.0));
        assert_eq!(storage, [0.75, 1.0]);
        assert!(!take.write(&mut storage, WriteMode::IndexedReplace, 2, 0.0));
        assert_eq!(take.appended(), 0);
    }

    #[test]
    fn empty_storage_refuses_all_modes() {
        for mode in [
            WriteMode::Append,
            WriteMode::IndexedReplace,
            WriteMode::IndexedOverdubClamped,
        ] {
            assert!(!TakeAccumulator::default().write(&mut [], mode, 0, 0.25));
        }
    }
}
