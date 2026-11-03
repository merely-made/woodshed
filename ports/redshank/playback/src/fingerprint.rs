// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0

use redshank_model::AudioFingerprint;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FingerprintContext {
    pub start_ms: u64,
    pub fingerprint: AudioFingerprint,
}

impl FingerprintContext {
    /// Freeze only context preceding the presented anchor. Decode lookahead
    /// and reaction offsets cannot silently move the fingerprint's endpoint.
    pub fn at(&self, anchor_ms: u64, window_ms: u64) -> Option<AudioFingerprint> {
        if !(5_000..=60_000).contains(&window_ms) {
            return None;
        }
        let relative = anchor_ms.checked_sub(self.start_ms)?;
        let frame_ms = u64::from(self.fingerprint.frame_ms);
        if frame_ms != 100 || relative > u64::from(self.fingerprint.anchor_offset_ms) {
            return None;
        }
        let end = usize::try_from(relative / frame_ms).ok()?;
        let start = end.saturating_sub((window_ms / frame_ms) as usize);
        if end > self.fingerprint.frames.len() || end - start < 30 {
            return None;
        }
        Some(AudioFingerprint {
            version: self.fingerprint.version,
            frame_ms: self.fingerprint.frame_ms,
            anchor_offset_ms: u32::try_from(relative - start as u64 * frame_ms).ok()?,
            frames: self.fingerprint.frames[start..end].to_vec(),
        })
    }
}
