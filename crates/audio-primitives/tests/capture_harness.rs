// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0

//! Device-free integration harness. Each product keeps its own allocator guard
//! for actual adapter execution; this binary guards the shared seams together.
use audio_primitives::capture::{
    BarCountdown, CompletedTake, InputFeed, TakeAccumulator, WriteMode,
};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static TRACK: Cell<bool> = const { Cell::new(false) };
    static EVENTS: Cell<usize> = const { Cell::new(0) };
}
struct CountingAllocator;
fn count() {
    let _ = TRACK.try_with(|track| {
        if track.get() {
            let _ = EVENTS.try_with(|events| events.set(events.get() + 1));
        }
    });
}
// SAFETY: allocation and destruction delegate unchanged to System; counting
// uses thread-local Cells without allocating, including during teardown.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count();
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count();
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count();
        unsafe { System.realloc(ptr, layout, size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        count();
        unsafe { System.dealloc(ptr, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[test]
fn borrowed_feed_countdown_and_completion_move_without_allocation_or_free() {
    let source = vec![-0.0, 2.0, -2.0, 0.25, 9.0];
    let mut pcm = vec![0.0; 4];
    let pointer = pcm.as_ptr();
    let mut feed = source.iter();
    let mut accumulator = TakeAccumulator::default();
    let mut countdown = BarCountdown::new(1, 99);
    EVENTS.with(|n| n.set(0));
    TRACK.with(|t| t.set(true));
    let ready = countdown.observe(1, 100, false);
    let written = accumulator.append_from(&mut pcm, &mut feed);
    let completed = CompletedTake {
        storage: pcm,
        stamp: (48_000u32, 7u64),
    };
    TRACK.with(|t| t.set(false));
    assert_eq!(EVENTS.with(Cell::get), 0);
    assert!(ready);
    assert_eq!(written, 4);
    assert_eq!(completed.storage.as_ptr(), pointer);
    assert_eq!(completed.stamp, (48_000, 7));
    assert_eq!(
        completed
            .storage
            .iter()
            .map(|v| v.to_bits())
            .collect::<Vec<_>>(),
        source[..4].iter().map(|v| v.to_bits()).collect::<Vec<_>>()
    );
    assert_eq!(InputFeed::pop_sample(&mut feed), Some(9.0));
}

#[test]
fn countdown_rejects_jitter_and_strict_half_bar_gap_and_refreshes_during_suppression() {
    let mut countdown = BarCountdown::new(2, 90);
    assert!(!countdown.observe(89, 100, false)); // jitter
    assert!(!countdown.observe(39, 100, false)); // exactly half a bar
    assert_eq!(countdown.remaining(), 2);
    assert!(!countdown.observe(99, 100, false));
    assert!(!countdown.observe(0, 100, true)); // restart is suppressed
    assert!(!countdown.observe(10, 100, false)); // refreshed baseline
    assert_eq!(countdown.remaining(), 2);
    assert!(!countdown.observe(99, 100, false));
    assert!(!countdown.observe(0, 100, false));
    assert_eq!(countdown.remaining(), 1);
    assert!(!countdown.observe(99, 100, false));
    assert!(countdown.observe(0, 100, false));
    assert!(countdown.observe(99, 100, false)); // saturated ready
}

#[test]
fn interrupted_feed_can_resume_without_treating_a_gap_as_end_of_stream() {
    struct Gapped {
        position: usize,
    }
    impl InputFeed for Gapped {
        fn pop_sample(&mut self) -> Option<f32> {
            let next = [Some(0.25), None, Some(-0.5), None]
                .get(self.position)
                .copied()
                .flatten();
            self.position += 1;
            next
        }
    }
    let mut feed = Gapped { position: 0 };
    let mut pcm = [0.0; 2];
    let mut accumulator = TakeAccumulator::default();
    assert_eq!(accumulator.append_from(&mut pcm, &mut feed), 1);
    assert_eq!(accumulator.append_from(&mut pcm, &mut feed), 1);
    assert_eq!(pcm, [0.25, -0.5]);
}

#[test]
fn indexed_writes_and_completion_preserve_preallocated_storage_without_allocation_or_free() {
    let mut pcm = vec![0.75, -0.75];
    let pointer = pcm.as_ptr();
    let mut accumulator = TakeAccumulator::default();
    TRACK.with(|t| t.set(true));
    EVENTS.with(|n| n.set(0));
    let first = accumulator.write(&mut pcm, WriteMode::IndexedOverdubClamped, 0, 0.5);
    let second = accumulator.write(&mut pcm, WriteMode::IndexedReplace, 1, -2.0);
    let completed = CompletedTake {
        storage: pcm,
        stamp: (11u64, true),
    };
    TRACK.with(|t| t.set(false));
    assert_eq!(EVENTS.with(Cell::get), 0);
    assert!(first && second);
    assert_eq!(completed.storage.as_ptr(), pointer);
    assert_eq!(completed.storage, [1.0, -1.0]);
    assert_eq!(completed.stamp, (11, true));
}
