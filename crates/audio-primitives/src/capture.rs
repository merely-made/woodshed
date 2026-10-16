// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0

//! Allocation-free accumulation into caller-owned, initialized PCM storage.
//! The host owns capture timing, input integrity, storage and completion.

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
