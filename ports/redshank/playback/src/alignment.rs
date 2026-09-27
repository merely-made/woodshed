//! Product adapter around the pure DSP matcher. Only private, digest-checked
//! local snapshots are searched; this module performs no network requests.
use std::path::Path;

use audio_primitives::alignment::{AlignmentConfig, AlignmentSearcher, Fingerprint};
use redshank_model::{
    AudioFingerprint, DerivedPosition, ListenerSettings, RepresentationIdentity,
    RepresentationReceipt, TimedTarget,
};

use crate::backend::{Decoded, decode_packet, open_local};

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

pub(super) fn stored(value: Fingerprint) -> AudioFingerprint {
    AudioFingerprint {
        version: value.version,
        frame_ms: value.frame_ms,
        anchor_offset_ms: value.anchor_offset_ms,
        frames: value.frames,
    }
}

fn dsp(value: &AudioFingerprint) -> Fingerprint {
    Fingerprint {
        version: value.version,
        frame_ms: value.frame_ms,
        anchor_offset_ms: value.anchor_offset_ms,
        frames: value.frames.clone(),
    }
}

/// The caller supplies a published downloaded receipt. A changed cache object
/// fails before search. A result describes an estimated point, never a span.
pub fn realign_local(
    path: &Path,
    expected: &RepresentationReceipt,
    target: &TimedTarget,
    settings: &ListenerSettings,
) -> Result<DerivedPosition, String> {
    settings
        .validate_alignment_settings()
        .map_err(|e| format!("Invalid alignment settings: {e:?}"))?;
    let reference = dsp(target
        .fingerprint
        .as_ref()
        .ok_or("This note has no audio fingerprint")?);
    reference.validate().map_err(|e| e.to_string())?;
    let (mut decoder, actual) = open_local(path).map_err(|e| e.to_string())?;
    if expected.complete_digest.is_none()
        || actual.compare(expected) != RepresentationIdentity::Same
    {
        return Err("Downloaded copy changed before alignment; download it again".into());
    }
    let budget = settings.alignment_max_search_ms.clamp(60_000, 86_400_000);
    let config = AlignmentConfig {
        min_score: f32::from(settings.alignment_min_confidence_per_mille) / 1000.0,
        ..Default::default()
    };
    let mut search = None;
    let mut frames = 0_u64;
    loop {
        match decode_packet(&mut decoder)? {
            Decoded::Frames(samples) => {
                let channels = decoder.channels.ok_or("Missing audio channel layout")? as usize;
                frames += (samples.len() / channels) as u64;
                if frames * 1000 / u64::from(decoder.rate) > budget {
                    return Err("Recording exceeds the configured alignment search limit".into());
                }
                if search.is_none() {
                    search = Some(
                        AlignmentSearcher::new(&reference, decoder.rate, channels, config)
                            .map_err(|e| e.to_string())?,
                    );
                }
                search
                    .as_mut()
                    .expect("search initialized")
                    .push(&samples)
                    .map_err(|e| e.to_string())?;
            },
            Decoded::Skipped => return Err("Audio gaps prevent a reliable alignment search".into()),
            Decoded::EndOfFile => break,
        }
    }
    let found = search
        .ok_or("Downloaded copy contains no decoded audio")?
        .finish()
        .map_err(|e| e.to_string())?;
    Ok(DerivedPosition {
        original_target: target.clone(),
        destination: expected.clone(),
        offset_ms: found.position_ms,
        algorithm_version: u32::from(reference.version),
        confidence_per_mille: (found.confidence * 1000.0).floor() as u16,
        runner_up_per_mille: (found.runner_up_score * 1000.0).ceil() as u16,
        reference_duration_ms: u64::from(reference.covered_ms()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use audio_primitives::alignment::{FingerprintBuilder, FingerprintConfig};
    use redshank_model::{AnnotationId, ItemId, LibraryItem, MediaSource, RedshankModel};

    // Independently synthesized changing tones, matching the DSP fixture's
    // mathematical construction. This is not a real-speech accuracy receipt.
    fn track(seconds: usize, seed: u32) -> Vec<f32> {
        (0..seconds * 8_000)
            .map(|sample| {
                let step = sample / 1_600;
                let hash = (step as u32).wrapping_add(seed).wrapping_mul(747_796_405);
                let band = (hash ^ (hash >> 16)) % 16;
                let frequency = 110.0 * (3_500.0_f32 / 110.0).powf(band as f32 / 15.0);
                (std::f32::consts::TAU * frequency * sample as f32 / 8_000.0).sin() * 0.4
            })
            .collect()
    }

    fn write_wav(path: &Path, pcm: &[f32]) -> RepresentationReceipt {
        let mut writer = hound::WavWriter::create(
            path,
            hound::WavSpec {
                channels: 1,
                sample_rate: 8_000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for sample in pcm {
            writer
                .write_sample((sample * f32::from(i16::MAX)) as i16)
                .unwrap();
        }
        writer.finalize().unwrap();
        // Hash fixture bytes independently of the production snapshot adapter.
        let bytes = std::fs::read(path).unwrap();
        RepresentationReceipt {
            requested_url: Some("https://example.invalid/episode.wav".into()),
            byte_length: Some(bytes.len() as u64),
            complete_digest: Some(format!("blake3:{}", blake3::hash(&bytes).to_hex())),
            ..Default::default()
        }
    }

    fn target(pcm: &[f32], representation: RepresentationReceipt) -> TimedTarget {
        let mut builder = FingerprintBuilder::new(
            8_000,
            1,
            FingerprintConfig {
                window_ms: 5_000,
                ..Default::default()
            },
        )
        .unwrap();
        builder.push(pcm).unwrap();
        TimedTarget {
            item_id: ItemId("alignment-episode".into()),
            offset_ms: 16_000,
            end_offset_ms: None,
            pressed_offset_ms: Some(17_000),
            representation,
            fingerprint: Some(stored(builder.fingerprint().unwrap())),
        }
    }

    #[test]
    fn downloaded_wav_alignment_stores_a_second_position_without_changing_original() {
        let directory = tempfile::tempdir().unwrap();
        let original_path = directory.path().join("original.wav");
        let current_path = directory.path().join("downloaded.wav");
        let original = track(16, 21);
        let original_receipt = write_wav(&original_path, &original);
        let target = target(&original, original_receipt);
        let frozen = target.clone();
        let mut current = track(7, 405);
        current.extend_from_slice(&original);
        let destination = write_wav(&current_path, &current);

        let derived = realign_local(
            &current_path,
            &destination,
            &target,
            &ListenerSettings::default(),
        )
        .unwrap();
        assert_eq!(derived.offset_ms, 23_000);
        assert_eq!(derived.destination, destination);
        assert_eq!(derived.original_target, frozen);
        assert_eq!(derived.algorithm_version, 1);
        assert!(derived.confidence_per_mille >= 940);
        assert_eq!(target, frozen);

        let mut model = RedshankModel::default();
        model
            .add_item(LibraryItem::DirectAudio {
                id: target.item_id.clone(),
                title: "Alignment fixture".into(),
                source: MediaSource::Cached {
                    path: current_path.display().to_string(),
                    origin_url: "https://example.invalid/episode.wav".into(),
                    representation: Box::new(destination),
                },
            })
            .unwrap();
        let id = AnnotationId("original-note".into());
        model
            .add_text_annotation(id.clone(), target, "Keep this thought".into(), 0)
            .unwrap();
        model.store_derived_position(&id, derived.clone()).unwrap();
        assert_eq!(model.annotations[&id].target, frozen);
        assert_eq!(model.derived_positions[&id], derived);
    }

    #[test]
    fn changed_download_and_incomplete_search_refuse() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("downloaded.wav");
        let original = track(16, 21);
        let expected = write_wav(&path, &original);
        let target = target(&original, expected.clone());
        // Same length and still decodable, but no longer the published bytes.
        let changed = write_wav(&path, &track(16, 987));
        assert_eq!(expected.byte_length, changed.byte_length);
        assert_ne!(expected.complete_digest, changed.complete_digest);
        let error =
            realign_local(&path, &expected, &target, &ListenerSettings::default()).unwrap_err();
        assert!(error.contains("changed before alignment"), "{error}");

        // An early valid match cannot be accepted without scanning the rest:
        // another equally plausible passage might exist beyond the budget.
        let mut too_long = original;
        too_long.resize(61 * 8_000, 0.0);
        let expected = write_wav(&path, &too_long);
        let settings = ListenerSettings {
            alignment_max_search_ms: 60_000,
            ..Default::default()
        };
        let error = realign_local(&path, &expected, &target, &settings).unwrap_err();
        assert!(
            error.contains("configured alignment search limit"),
            "{error}"
        );
    }

    #[test]
    fn capture_context_applies_reaction_offset_and_excludes_future_audio() {
        let pcm = track(16, 21);
        let mut builder = FingerprintBuilder::new(8_000, 1, FingerprintConfig::default()).unwrap();
        builder.push(&pcm).unwrap();
        let context = FingerprintContext {
            start_ms: 30_000,
            fingerprint: stored(builder.fingerprint().unwrap()),
        };
        // Press at 43.037s with a 1s reaction offset; decoded context extends
        // through 46s and must not contribute any frames after the anchor.
        let captured = context.at(43_037 - 1_000, 5_000).unwrap();
        let expected = builder.fingerprint_at(12_037, 5_000).unwrap();
        assert_eq!(captured, stored(expected));
        assert_eq!(captured.anchor_offset_ms, 5_037);
        assert_eq!(captured.frames.len(), 50);
        assert!(context.at(29_000, 5_000).is_none());
        assert!(context.at(32_999, 5_000).is_none());
        assert!(context.at(47_000, 5_000).is_none());
        assert!(context.at(42_000, 0).is_none());
    }
}
