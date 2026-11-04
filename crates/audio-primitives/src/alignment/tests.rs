use super::*;

// A reproducible sequence of changing tone mixtures, not donor audio and
// not a substitute for spoken-word or codec acceptance fixtures.
fn track(seconds: usize, sample_rate: usize, seed: u32) -> Vec<f32> {
    (0..seconds * sample_rate)
        .map(|sample| {
            let step = sample / (sample_rate / 5);
            let hash = (step as u32).wrapping_add(seed).wrapping_mul(747_796_405);
            let band = ((hash ^ (hash >> 16)) % BANDS as u32) as usize;
            let frequency = 110.0 * (3_500.0_f32 / 110.0).powf(band as f32 / (BANDS - 1) as f32);
            (std::f32::consts::TAU * frequency * sample as f32 / sample_rate as f32).sin() * 0.4
        })
        .collect()
}

fn reference(pcm: &[f32]) -> Fingerprint {
    let mut builder = FingerprintBuilder::new(
        8_000,
        1,
        FingerprintConfig {
            window_ms: 5_000,
            ..Default::default()
        },
    )
    .unwrap();
    // Deliberately split away from feature boundaries.
    for chunk in pcm.chunks(317) {
        builder.push(chunk).unwrap();
    }
    builder.fingerprint().unwrap()
}

fn search(
    reference: &Fingerprint,
    pcm: &[f32],
    rate: u32,
) -> Result<AlignmentMatch, AlignmentError> {
    let mut searcher =
        AlignmentSearcher::new(reference, rate, 1, AlignmentConfig::default()).unwrap();
    for chunk in pcm.chunks(977) {
        searcher.push(chunk).unwrap();
    }
    searcher.finish()
}

#[test]
fn finds_thirty_second_insert_and_longer_preroll() {
    let base = track(16, 8_000, 21);
    let reference = reference(&base);
    let mut inserted = base[..8 * 8_000].to_vec();
    inserted.extend(track(30, 8_000, 999));
    inserted.extend_from_slice(&base[8 * 8_000..]);
    let found = search(&reference, &inserted, 8_000).unwrap();
    assert_eq!(found.position_ms, 46_000);
    assert!(found.confidence > 0.99);

    let mut preroll = track(7, 8_000, 405);
    preroll.extend_from_slice(&base);
    assert_eq!(
        search(&reference, &preroll, 8_000).unwrap().position_ms,
        23_000
    );
}

#[test]
fn refuses_unrelated_repeated_deleted_and_discontinuous_audio() {
    let base = track(16, 8_000, 21);
    let reference = reference(&base);
    assert!(search(&reference, &track(16, 8_000, 987), 8_000).is_err());
    let mut repeated = base.clone();
    repeated.extend_from_slice(&base);
    assert_eq!(
        search(&reference, &repeated, 8_000),
        Err(AlignmentError::Ambiguous)
    );
    assert!(search(&reference, &base[..15 * 8_000], 8_000).is_err());
    let mut interrupted = base[..15 * 8_000 + 6_400].to_vec();
    interrupted.extend(track(1, 8_000, 987));
    interrupted.extend_from_slice(&base[15 * 8_000 + 6_400..]);
    assert!(search(&reference, &interrupted, 8_000).is_err());
}

#[test]
fn tolerates_gain_noise_and_different_pcm_sample_rate() {
    let base = track(16, 8_000, 21);
    let reference = reference(&base);
    let noisy: Vec<_> = base
        .iter()
        .enumerate()
        .map(|(i, sample)| sample * 0.3 + ((i * 17 % 101) as f32 / 101.0 - 0.5) * 0.0001)
        .collect();
    assert_eq!(
        search(&reference, &noisy, 8_000).unwrap().position_ms,
        16_000
    );
    assert_eq!(
        search(&reference, &track(16, 16_000, 21), 16_000)
            .unwrap()
            .position_ms,
        16_000
    );
}

#[test]
fn strong_earlier_match_cannot_override_a_different_tail() {
    let base = track(16, 8_000, 21);
    let reference = reference(&base);
    let mut changed = base;
    changed[16 * 8_000 - 800..].fill(0.0);
    assert_eq!(
        search(&reference, &changed, 8_000),
        Err(AlignmentError::AnchorDiscontinuity)
    );
}

#[test]
fn silence_stationary_and_short_material_refuse() {
    let mut builder = FingerprintBuilder::new(8_000, 1, FingerprintConfig::default()).unwrap();
    builder.push(&[0.0; 800]).unwrap();
    assert_eq!(
        builder.fingerprint(),
        Err(AlignmentError::InsufficientMaterial)
    );
    builder.push(&vec![0.0; 40_000]).unwrap();
    assert_eq!(builder.fingerprint(), Err(AlignmentError::LowInformation));
    let constant_tone: Vec<_> = (0..40_000)
        .map(|i| (std::f32::consts::TAU * 440.0 * i as f32 / 8_000.0).sin())
        .collect();
    let mut builder = FingerprintBuilder::new(8_000, 1, FingerprintConfig::default()).unwrap();
    builder.push(&constant_tone).unwrap();
    assert_eq!(builder.fingerprint(), Err(AlignmentError::LowInformation));
}

#[test]
fn presented_anchor_excludes_lookahead_and_tracks_eviction() {
    let base = track(16, 8_000, 21);
    let mut builder = FingerprintBuilder::new(
        8_000,
        1,
        FingerprintConfig {
            window_ms: 5_000,
            history_ms: 10_000,
            ..Default::default()
        },
    )
    .unwrap();
    builder.push(&base[..10 * 8_000]).unwrap();
    let presented = builder.fingerprint_at(8_037, 5_000).unwrap();
    let expected = reference(&base[..8 * 8_000]);
    assert_eq!(presented.frames, expected.frames);
    assert_eq!(presented.anchor_offset_ms, 5_037);
    assert_eq!(
        builder.fingerprint_at(11_000, 5_000),
        Err(AlignmentError::InsufficientMaterial)
    );
    builder.push(&base[10 * 8_000..]).unwrap();
    assert_eq!(
        builder.fingerprint_at(5_000, 5_000),
        Err(AlignmentError::InsufficientMaterial)
    );
    assert_eq!(
        builder.fingerprint_at(10_000, 5_000).unwrap().covered_ms(),
        4_000
    );
    assert!(builder.frames.len() <= 100);
}

#[test]
fn validates_bounds_versions_and_pcm_without_partial_mutation() {
    assert!(
        FingerprintBuilder::new(
            8_000,
            1,
            FingerprintConfig {
                history_ms: 90_100,
                ..Default::default()
            }
        )
        .is_err()
    );
    let mut builder = FingerprintBuilder::new(8_000, 2, FingerprintConfig::default()).unwrap();
    assert_eq!(builder.push(&[0.0]), Err(AlignmentError::InvalidPcm));
    assert_eq!(
        builder.push(&[0.0, f32::NAN]),
        Err(AlignmentError::InvalidPcm)
    );
    assert_eq!(builder.frames_seen, 0);
    let mut fingerprint = reference(&track(5, 8_000, 21));
    fingerprint.version += 1;
    assert_eq!(
        fingerprint.validate(),
        Err(AlignmentError::UnsupportedVersion)
    );
    fingerprint.version = FINGERPRINT_VERSION;
    fingerprint.anchor_offset_ms += 100;
    assert_eq!(
        fingerprint.validate(),
        Err(AlignmentError::InvalidFingerprint)
    );
    assert!(
        AlignmentSearcher::new(
            &reference(&track(5, 8_000, 21)),
            8_000,
            1,
            AlignmentConfig {
                min_score: f32::NAN,
                ..Default::default()
            }
        )
        .is_err()
    );
}
