// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0

use super::*;

#[test]
fn a_contract_only_build_refuses_playback_without_starting_a_worker() {
    assert!(!NATIVE_AUDIO_AVAILABLE);
    let runtime = PlaybackRuntime::start();
    assert!(matches!(
        runtime.snapshot().state,
        PlaybackState::Unavailable(_)
    ));
    assert!(runtime.command(PlaybackCommand::Play).is_err());
}
