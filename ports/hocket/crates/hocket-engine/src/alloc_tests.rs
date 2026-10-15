// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0

//! Allocation guard for the per-tick capture feed.
//!
//! Counterpart of `woodshed-audio`'s `output_runtime/tests.rs` guard, scoped to
//! what Hocket actually runs per control tick. Hocket's capture feed is *not*
//! on the audio thread: `Engine::tick` runs on the host's ~60 fps tick, so the
//! guard covers [`Engine::drain_and_advance_capture`] and
//! [`Engine::advance_pending_layers`], not `tick` as a whole (`FirewheelContext::
//! update` and stream polling allocate by design and are out of scope).
//!
//! What this pins:
//! - steady `Waiting` ticks (drained and wait-only), steady `Recording` ticks
//!   and the `Recording`→`Complete` transition allocate nothing;
//! - the one allocation on the capture path is the phrase buffer, taken when
//!   `Waiting` becomes `Recording` (`Capture::new` reserves the whole phrase);
//! - with no queued layers `advance_pending_layers` allocates nothing. With a
//!   queued layer it currently rebuilds two `Vec`s per tick (known, unguarded).
//!
//! The allocator counts per thread and only inside [`measure`], so the rest of
//! the test binary is unaffected.

use super::*;
use crate::offline_tests::Offline;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static TRACK: Cell<bool> = const { Cell::new(false) };
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}

struct CountingAllocator;

fn count() {
    // `try_with`: the allocator also runs during thread teardown.
    let _ = TRACK.try_with(|t| {
        if t.get() {
            let _ = ALLOCS.try_with(|n| n.set(n.get() + 1));
        }
    });
}

// SAFETY: every method forwards unchanged to `System`; counting touches only
// `Cell`s in thread-locals and never allocates.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count();
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count();
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        count();
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// Allocations made by `f` on this thread.
fn measure(f: impl FnOnce()) -> usize {
    ALLOCS.with(|n| n.set(0));
    TRACK.with(|t| t.set(true));
    f();
    TRACK.with(|t| t.set(false));
    ALLOCS.with(Cell::get)
}

/// One control tick's capture work on already-processed input: the feed and
/// the layer advance, without `FirewheelContext::update`.
fn feed(engine: &mut Engine) -> usize {
    measure(|| {
        engine.step_tempo_resync();
        engine.drain_and_advance_capture();
        engine.advance_pending_layers().unwrap();
    })
}

#[test]
fn capture_feed_allocates_only_the_phrase_buffer_at_start() {
    let mut offline = Offline::new(48_000);
    offline.fast_bar();
    // Warm the engine's own scratch to its working size before measuring; the
    // production engine reserves 8192 frames at construction.
    assert!(offline.engine.input_scratch.capacity() >= 1000);
    offline.engine.arm_bar_aligned_capture(1, 1).unwrap();

    let mut waiting_ticks = 0;
    let mut start_allocs = None;
    let mut steady_recording_ticks = 0;
    for _ in 0..120 {
        // Two thirds of the ticks have input; the rest model an empty reader
        // so the wait-only path is exercised too.
        let batch = vec![0.25; 1000];
        offline.process(&batch);
        if offline.frames.is_multiple_of(3) {
            offline.engine.reader_state.discard_frames();
        }
        let before = offline.engine.pending_capture_progress();
        let allocs = feed(&mut offline.engine);
        offline.engine.cx.update().unwrap();
        match (before, offline.engine.pending_capture_progress()) {
            (CapturePhase::Waiting { .. }, CapturePhase::Waiting { .. }) => {
                assert_eq!(allocs, 0, "steady Waiting tick allocated");
                waiting_ticks += 1;
            },
            (
                CapturePhase::Waiting { .. },
                CapturePhase::Recording { .. } | CapturePhase::Complete,
            ) => {
                start_allocs = Some(allocs);
            },
            (
                CapturePhase::Recording { .. },
                CapturePhase::Recording { .. } | CapturePhase::Complete,
            ) => {
                assert_eq!(allocs, 0, "steady Recording tick allocated");
                steady_recording_ticks += 1;
            },
            _ => {},
        }
        if offline.engine.pending_capture_progress() == CapturePhase::Complete {
            break;
        }
    }
    assert!(waiting_ticks >= 8, "waited {waiting_ticks} ticks");
    assert!(
        steady_recording_ticks >= 2,
        "recorded {steady_recording_ticks} ticks"
    );
    assert_eq!(
        start_allocs,
        Some(1),
        "Waiting -> Recording allocates exactly the phrase buffer"
    );
    assert_eq!(
        offline.engine.take_bar_aligned_capture().unwrap().len(),
        4800
    );
}

#[test]
fn free_capture_feed_allocation_is_only_vec_growth() {
    // Free capture appends to an unreserved `Vec`: allocation happens only when
    // it grows. Pin that it is amortized (logarithmic), not per tick.
    let mut offline = Offline::new(48_000);
    offline.engine.start_free_capture().unwrap();
    let mut total = 0;
    for _ in 0..64 {
        offline.process(&[0.1; 256]);
        total += feed(&mut offline.engine);
    }
    assert!(
        total <= 16,
        "free capture allocated {total} times in 64 ticks"
    );
}
