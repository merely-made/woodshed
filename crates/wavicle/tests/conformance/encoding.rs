// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use super::*;

/// M4 gate. For each generated integer buffer: (1) our encode then our decode
/// reproduces the input (self round-trip, runs everywhere), and (2) our encode
/// then the reference `wvunpack -r` reproduces the input, proving a real
/// decoder accepts our stream. The reference leg is skipped when the pinned
/// CLI is not present (the same Layer-0/Layer-1 split as decode).
#[cfg(feature = "encode")]
#[test]
fn encode_round_trips_losslessly_through_ourselves_and_the_reference() {
    use wavicle::{EncodeParams, encode_int};

    fn tone(n: usize, rate: u32, freq: f64, amp: f64, scale: i32) -> Vec<i32> {
        (0..n)
            .map(|i| {
                let s = amp * (2.0 * std::f64::consts::PI * freq * i as f64 / rate as f64).sin();
                (s * scale as f64).round() as i32
            })
            .collect()
    }

    struct Case {
        name: &'static str,
        params: EncodeParams,
        samples: Vec<i32>,
    }
    let p = |channels, sample_rate, bits_per_sample| EncodeParams {
        channels,
        sample_rate,
        bits_per_sample,
    };
    let max16 = (1 << 15) - 1;
    let max24 = (1 << 23) - 1;
    let cases = vec![
        Case {
            name: "i16 mono tone",
            params: p(1, 48000, 16),
            samples: tone(4000, 48000, 220.0, 0.6, max16),
        },
        Case {
            name: "i16 stereo",
            params: p(2, 44100, 16),
            samples: {
                let l = tone(3000, 44100, 220.0, 0.6, max16);
                let r = tone(3000, 44100, 330.0, 0.5, max16);
                l.iter().zip(&r).flat_map(|(a, b)| [*a, *b]).collect()
            },
        },
        Case {
            name: "i16 silence",
            params: p(2, 48000, 16),
            samples: vec![0; 4000],
        },
        Case {
            name: "i16 full scale",
            params: p(1, 48000, 16),
            samples: (0..2000)
                .map(|i| if i % 2 == 0 { max16 } else { -max16 - 1 })
                .collect(),
        },
        Case {
            name: "i24 stereo",
            params: p(2, 48000, 24),
            samples: {
                let l = tone(2000, 48000, 261.0, 0.7, max24);
                let r = tone(2000, 48000, 392.0, 0.4, max24);
                l.iter().zip(&r).flat_map(|(a, b)| [*a, *b]).collect()
            },
        },
        Case {
            name: "i32 mono",
            params: p(1, 48000, 32),
            samples: tone(2000, 48000, 174.0, 0.5, 1 << 23),
        },
        Case {
            name: "i16 ramp mono",
            params: p(1, 48000, 16),
            samples: (0..2000).map(|i| ((i * 17) % 65535) - 32768).collect(),
        },
    ];

    // Locate the pinned reference wvunpack, if present.
    let tool = reference_tool("wvunpack");
    let have_ref = tool.exists();
    let mut ref_checked = 0;

    for case in &cases {
        let wv = encode_int(case.params, &case.samples)
            .unwrap_or_else(|e| panic!("{}: encode failed: {e}", case.name));

        // (1) self round-trip
        let decoded = wavicle::decode_stream(&wv)
            .unwrap_or_else(|e| panic!("{}: self-decode failed: {e}", case.name));
        assert_eq!(
            decoded.samples, case.samples,
            "{}: self round-trip mismatch",
            case.name
        );

        // (2) reference round-trip
        if have_ref {
            let dir = std::env::temp_dir();
            let wv_path = dir.join(format!("wavicle_m4_{}.wv", case.name.replace(' ', "_")));
            let raw_path = dir.join(format!("wavicle_m4_{}.raw", case.name.replace(' ', "_")));
            std::fs::write(&wv_path, &wv).unwrap();
            let status = std::process::Command::new(&tool)
                .args(["-r", "-y", "-q"])
                .arg(&wv_path)
                .arg("-o")
                .arg(&raw_path)
                .status()
                .expect("run wvunpack");
            assert!(
                status.success(),
                "{}: wvunpack rejected our stream",
                case.name
            );
            let raw = std::fs::read(&raw_path).unwrap();
            let bytes = (case.params.bits_per_sample / 8) as usize;
            let ref_samples: Vec<i32> = raw
                .chunks_exact(bytes)
                .map(|b| match bytes {
                    2 => i32::from(i16::from_le_bytes([b[0], b[1]])),
                    3 => i32::from_le_bytes([b[0], b[1], b[2], 0]) << 8 >> 8,
                    4 => i32::from_le_bytes([b[0], b[1], b[2], b[3]]),
                    _ => unreachable!(),
                })
                .collect();
            assert_eq!(
                ref_samples, case.samples,
                "{}: reference wvunpack round-trip mismatch",
                case.name
            );
            let _ = std::fs::remove_file(&wv_path);
            let _ = std::fs::remove_file(&raw_path);
            ref_checked += 1;
        }
    }

    if have_ref {
        assert_eq!(
            ref_checked,
            cases.len(),
            "all cases should hit the reference"
        );
    } else {
        eprintln!("note: reference wvunpack absent; ran self round-trip only");
    }
}

/// M5 gate. Float encode must round-trip bit-exactly: our encode then our
/// decode reproduces the exact f32 patterns, our encode then the reference
/// `wvunpack -r` reproduces them, and the BLAKE3 of the decoded f32 bytes
/// matches the input's (the media-identity invariant Hocket keys on). Includes
/// the adversarial specials (signed zero, denormals, NaN payloads, infinities).
#[cfg(feature = "encode")]
#[test]
fn float_encode_round_trips_bit_exact_with_blake3_identity() {
    use wavicle::encode_float;

    fn tone(n: usize, rate: u32, freq: f64, amp: f64) -> Vec<f32> {
        (0..n)
            .map(|i| {
                (amp * (2.0 * std::f64::consts::PI * freq * i as f64 / rate as f64).sin()) as f32
            })
            .collect()
    }

    let specials_bits: [u32; 16] = [
        0x00000000, 0x80000000, 0x3F800000, 0xBF800000, 0x00000001, 0x80000001, 0x007FFFFF,
        0x7F800000, 0xFF800000, 0x7FC00000, 0x7F800001, 0x7FFFFFFF, 0xFFC00000, 0x7F7FFFFF,
        0x00800000, 0x3F000000,
    ];
    let mut specials = Vec::new();
    for _ in 0..16 {
        specials.extend(specials_bits.iter().map(|&b| f32::from_bits(b)));
    }
    specials.extend(tone(240, 48000, 55.0, 0.02));

    struct Case {
        name: &'static str,
        channels: u32,
        rate: u32,
        samples: Vec<f32>,
    }
    let cases = vec![
        Case {
            name: "f32 mono",
            channels: 1,
            rate: 48000,
            samples: tone(4000, 48000, 220.0, 0.6),
        },
        Case {
            name: "f32 stereo",
            channels: 2,
            rate: 48000,
            samples: {
                let l = tone(3000, 48000, 220.0, 0.6);
                let r = tone(3000, 48000, 330.0, 0.5);
                l.iter().zip(&r).flat_map(|(a, b)| [*a, *b]).collect()
            },
        },
        Case {
            name: "f32 specials",
            channels: 1,
            rate: 48000,
            samples: specials,
        },
        Case {
            name: "f32 silence",
            channels: 2,
            rate: 44100,
            samples: vec![0.0; 4000],
        },
    ];

    let tool = reference_tool("wvunpack");
    let have_ref = tool.exists();

    for case in &cases {
        let want_bits: Vec<u32> = case.samples.iter().map(|f| f.to_bits()).collect();
        let wv = encode_float(case.channels, case.rate, &case.samples)
            .unwrap_or_else(|e| panic!("{}: encode failed: {e}", case.name));

        // Self round-trip: decode gives back the exact bit patterns.
        let decoded = wavicle::decode_stream(&wv)
            .unwrap_or_else(|e| panic!("{}: self-decode failed: {e}", case.name));
        assert!(decoded.is_float, "{}: float flag", case.name);
        let got_bits: Vec<u32> = decoded.samples.iter().map(|&s| s as u32).collect();
        for (i, (a, b)) in got_bits.iter().zip(&want_bits).enumerate() {
            assert_eq!(
                a, b,
                "{}: self round-trip sample {i}: {a:#010x} vs {b:#010x}",
                case.name
            );
        }

        // BLAKE3 identity over the decoded f32 LE bytes.
        let want_bytes: Vec<u8> = want_bits.iter().flat_map(|b| b.to_le_bytes()).collect();
        let got_bytes: Vec<u8> = got_bits.iter().flat_map(|b| b.to_le_bytes()).collect();
        assert_eq!(
            blake3::hash(&got_bytes),
            blake3::hash(&want_bytes),
            "{}: BLAKE3",
            case.name
        );

        // Reference round-trip.
        if have_ref {
            let dir = std::env::temp_dir();
            let wv_path = dir.join(format!("wavicle_m5_{}.wv", case.name.replace(' ', "_")));
            let raw_path = dir.join(format!("wavicle_m5_{}.raw", case.name.replace(' ', "_")));
            std::fs::write(&wv_path, &wv).unwrap();
            let status = std::process::Command::new(&tool)
                .args(["-r", "-y", "-q"])
                .arg(&wv_path)
                .arg("-o")
                .arg(&raw_path)
                .status()
                .expect("run wvunpack");
            assert!(
                status.success(),
                "{}: wvunpack rejected our float stream",
                case.name
            );
            let raw = std::fs::read(&raw_path).unwrap();
            let ref_bits: Vec<u32> = raw
                .chunks_exact(4)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .collect();
            for (i, (a, b)) in ref_bits.iter().zip(&want_bits).enumerate() {
                assert_eq!(
                    a, b,
                    "{}: reference sample {i}: {a:#010x} vs {b:#010x}",
                    case.name
                );
            }
            let _ = std::fs::remove_file(&wv_path);
            let _ = std::fs::remove_file(&raw_path);
        }
    }
}

/// Multi-block gate. Inputs longer than one block must split into several
/// contiguous blocks that round-trip losslessly through our own decoder and
/// the reference `wvunpack`, for both integer and float.
#[cfg(feature = "encode")]
#[test]
fn long_inputs_split_into_blocks_and_round_trip() {
    use wavicle::{EncodeParams, encode_float, encode_int};

    // ~2.5 blocks at 32768 frames/block.
    let n = 80_000usize;
    let int_samples: Vec<i32> = (0..n)
        .map(|i| (4000.0 * (i as f64 * 0.02).sin()) as i32)
        .collect();
    let float_samples: Vec<f32> = (0..n)
        .map(|i| (0.6 * (i as f64 * 0.02).sin()) as f32)
        .collect();

    let tool = reference_tool("wvunpack");
    let have_ref = tool.exists();

    let reference_raw = |wv: &[u8], name: &str, bytes: usize| -> Vec<u8> {
        let dir = std::env::temp_dir();
        let wv_path = dir.join(format!("wavicle_mb_{name}.wv"));
        let raw_path = dir.join(format!("wavicle_mb_{name}.raw"));
        std::fs::write(&wv_path, wv).unwrap();
        let ok = std::process::Command::new(&tool)
            .args(["-r", "-y", "-q"])
            .arg(&wv_path)
            .arg("-o")
            .arg(&raw_path)
            .status()
            .expect("run wvunpack")
            .success();
        assert!(ok, "{name}: wvunpack rejected multi-block stream");
        let raw = std::fs::read(&raw_path).unwrap();
        let _ = std::fs::remove_file(&wv_path);
        let _ = std::fs::remove_file(&raw_path);
        let _ = bytes;
        raw
    };

    // Integer, mono, 16-bit.
    let params = EncodeParams {
        channels: 1,
        sample_rate: 48000,
        bits_per_sample: 16,
    };
    let wv = encode_int(params, &int_samples).unwrap();
    let info = wavicle::StreamInfo::scan(&wv).unwrap();
    assert!(
        info.block_count >= 3,
        "expected multiple blocks, got {}",
        info.block_count
    );
    assert_eq!(info.total_samples, Some(n as u64));
    assert_eq!(wavicle::decode_stream(&wv).unwrap().samples, int_samples);
    if have_ref {
        let raw = reference_raw(&wv, "int16", 2);
        let ref_s: Vec<i32> = raw
            .chunks_exact(2)
            .map(|b| i32::from(i16::from_le_bytes([b[0], b[1]])))
            .collect();
        assert_eq!(ref_s, int_samples, "int16 multi-block reference mismatch");
    }

    // Float, mono.
    let wv = encode_float(1, 48000, &float_samples).unwrap();
    let info = wavicle::StreamInfo::scan(&wv).unwrap();
    assert!(
        info.block_count >= 3,
        "float: expected multiple blocks, got {}",
        info.block_count
    );
    let want_bits: Vec<u32> = float_samples.iter().map(|f| f.to_bits()).collect();
    let got_bits: Vec<u32> = wavicle::decode_stream(&wv)
        .unwrap()
        .samples
        .iter()
        .map(|&s| s as u32)
        .collect();
    assert_eq!(got_bits, want_bits, "float multi-block self round-trip");
    if have_ref {
        let raw = reference_raw(&wv, "f32", 4);
        let ref_bits: Vec<u32> = raw
            .chunks_exact(4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect();
        assert_eq!(ref_bits, want_bits, "float multi-block reference mismatch");
    }
}

/// A non-standard sample rate must round-trip via the ID_SAMPLE_RATE sub-block
/// (header index 0xF), so wavicle is a drop-in for any capture rate.
#[cfg(feature = "encode")]
#[test]
fn non_standard_sample_rate_round_trips() {
    use wavicle::{EncodeParams, encode_float, encode_int};

    let rate = 37_800; // a real but non-table rate (old CD-ROM-XA)
    let ints: Vec<i32> = (0..2000)
        .map(|i| (3000.0 * (i as f64 * 0.03).sin()) as i32)
        .collect();
    let wv = encode_int(
        EncodeParams {
            channels: 1,
            sample_rate: rate,
            bits_per_sample: 16,
        },
        &ints,
    )
    .unwrap();
    let d = wavicle::decode_stream(&wv).unwrap();
    assert_eq!(d.sample_rate, rate, "int: custom rate must survive");
    assert_eq!(d.samples, ints);

    let floats: Vec<f32> = (0..2000)
        .map(|i| (0.5 * (i as f64 * 0.03).sin()) as f32)
        .collect();
    let wv = encode_float(1, rate, &floats).unwrap();
    let d = wavicle::decode_stream(&wv).unwrap();
    assert_eq!(d.sample_rate, rate, "float: custom rate must survive");
    let got: Vec<u32> = d.samples.iter().map(|&s| s as u32).collect();
    let want: Vec<u32> = floats.iter().map(|f| f.to_bits()).collect();
    assert_eq!(got, want);

    // And the reference agrees on the rate.
    let tool = reference_tool("wvunpack");
    if tool.exists() {
        let dir = std::env::temp_dir();
        let p = dir.join("wavicle_rate.wv");
        std::fs::write(&p, &wv).unwrap();
        let out = std::process::Command::new(&tool)
            .args(["-ss"])
            .arg(&p)
            .output()
            .unwrap();
        let ss = String::from_utf8_lossy(&out.stdout);
        assert!(
            ss.contains("37800"),
            "wvunpack -ss should report 37800 Hz:\n{ss}"
        );
        let _ = std::fs::remove_file(&p);
    }
}

/// The fixed cascade should compress real-ish audio meaningfully below raw and
/// within a reasonable factor of the reference's tuned encoder. This guards
/// against a decorrelation regression that would silently balloon file sizes.
#[cfg(feature = "encode")]
#[test]
fn encoded_size_is_reasonable_versus_raw_and_reference() {
    use wavicle::encode_float;

    let r = 48000u32;
    let n = 24000usize;
    // Deterministic representative audio: harmonics + light LCG noise + decay.
    let mut lcg: u32 = 7;
    let samples: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f64 / r as f64;
            let mut s = 0.5 * (2.0 * std::f64::consts::PI * 220.0 * t).sin()
                + 0.25 * (2.0 * std::f64::consts::PI * 440.0 * t).sin()
                + 0.12 * (2.0 * std::f64::consts::PI * 660.0 * t).sin();
            lcg = lcg.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            s += 0.02 * ((lcg >> 8) as f64 / (1u32 << 23) as f64 - 1.0);
            let env = 0.3 + 0.7 * (1.0 - t / (n as f64 / r as f64)).max(0.0);
            (s * env) as f32
        })
        .collect();

    let raw_len = samples.len() * 4;
    let wv = encode_float(1, r, &samples).unwrap();
    // Must still be lossless.
    let got: Vec<u32> = wavicle::decode_stream(&wv)
        .unwrap()
        .samples
        .iter()
        .map(|&s| s as u32)
        .collect();
    let want: Vec<u32> = samples.iter().map(|f| f.to_bits()).collect();
    assert_eq!(got, want, "size test must stay lossless");

    // Beats raw comfortably.
    assert!(
        wv.len() < raw_len * 9 / 10,
        "ours {} vs raw {}",
        wv.len(),
        raw_len
    );

    let tool = reference_tool("wavpack");
    if tool.exists() {
        let dir = std::env::temp_dir();
        let raw_path = dir.join("wavicle_size.raw");
        let ref_wv = dir.join("wavicle_size_ref.wv");
        let raw: Vec<u8> = samples.iter().flat_map(|f| f.to_le_bytes()).collect();
        std::fs::write(&raw_path, &raw).unwrap();
        std::process::Command::new(&tool)
            .args([&format!("--raw-pcm={r},32f,1,le"), "-m", "-y", "-q"])
            .arg(&raw_path)
            .arg("-o")
            .arg(&ref_wv)
            .status()
            .unwrap();
        let ref_len = std::fs::metadata(&ref_wv).unwrap().len() as usize;
        eprintln!(
            "SIZE: raw {raw_len}  wavicle {}  reference {ref_len}  (ours/ref = {:.2}x, ours/raw = {:.0}%)",
            wv.len(),
            wv.len() as f64 / ref_len as f64,
            wv.len() as f64 / raw_len as f64 * 100.0
        );
        assert!(
            wv.len() < ref_len * 5 / 2,
            "ours {} should be within 2.5x reference {}",
            wv.len(),
            ref_len
        );
        let _ = std::fs::remove_file(&raw_path);
        let _ = std::fs::remove_file(&ref_wv);
    }
}
