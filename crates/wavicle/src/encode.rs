// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! The encode driver (M4 integer, M5 float).
//!
//! Shaped after `pack_samples` in the reference `pack.c` (dbry/WavPack,
//! BSD-3-Clause; see ATTRIBUTION.md), narrowed to the configuration this
//! project ships: a single block, one fixed decorrelation term, no joint
//! stereo. Byte-identity with the reference encoder is a non-goal (see the
//! founding plan); the gate is that the reference `wvunpack` decodes our
//! output losslessly. More/better decorrelation terms are a later compression
//! improvement that does not change the format the decoder reads.

use crate::bitstream::BitWriter;
use crate::block;
use crate::decorr::{DecorrPass, forward_decorr_mono_pass, forward_decorr_stereo_pass};
use crate::entropy::WordsEncoder;
use crate::error::{Error, Scope};
use crate::float::{scan_float_data, send_float_data};
use crate::format::{self, Flags, meta};

/// The fixed decorrelation cascade this encoder stamps, in forward order
/// `(term, delta)`. A short cascade of an extrapolating predictor (18), a
/// second-order term (2), a linear extrapolation (17), and the first two
/// differences (1, 2) captures most of the redundancy in real audio without an
/// adaptive per-file term search. A conforming decoder reads whatever config we
/// write, so any valid set is lossless; this one just makes the files smaller
/// than a single term. Weights and history start at zero (written as empty
/// metadata), and each pass adapts within the block.
const FIXED_DECORR: &[(i32, i32)] = &[(18, 2), (2, 2), (17, 2), (1, 2), (2, 2)];

/// Frames per block. Well under the 131072-frame and 1 MB block limits even for
/// worst-case incompressible stereo 32-bit data. Blocks are independent: each
/// carries its own complete starting state, so the decoder needs no continuity
/// between them. Long inputs split into several such blocks.
const BLOCK_FRAMES: usize = 32768;

/// Parameters describing the integer PCM to encode.
#[derive(Clone, Copy, Debug)]
pub struct EncodeParams {
    /// 1 or 2. Two channels are stored as independent left/right.
    pub channels: u32,
    pub sample_rate: u32,
    /// Stored bits per sample: 8, 16, 24, or 32.
    pub bits_per_sample: u32,
}

/// Encode interleaved integer `samples` to a single-block `.wv` stream.
///
/// `samples` holds sign-extended `i32` values (`channels * frames` of them),
/// each fitting in `bits_per_sample`.
pub fn encode_int(params: EncodeParams, samples: &[i32]) -> Result<Vec<u8>, Error> {
    let (mono, total_frames, srate_index) =
        prepare(params.channels, params.sample_rate, samples.len())?;
    if !matches!(params.bits_per_sample, 8 | 16 | 24 | 32) {
        return Err(Error::NotYetImplemented(
            "bit depth must be 8, 16, 24, or 32",
        ));
    }
    let bytes_per_sample = params.bits_per_sample / 8;
    let channels = params.channels as usize;

    let mut out = Vec::new();
    let mut block_index: u64 = 0;
    for chunk in samples.chunks(BLOCK_FRAMES * channels) {
        let block_frames = (chunk.len() / channels) as u32;
        let magnitude = magnitude_of(chunk)?;
        let flags = base_flags(mono, bytes_per_sample, magnitude, srate_index);
        assemble_block(
            &mut out,
            mono,
            flags,
            params.sample_rate,
            chunk,
            block_index,
            block_frames,
            total_frames,
            None,
            None,
        );
        block_index += u64::from(block_frames);
    }
    Ok(out)
}

/// Encode interleaved 32-bit float `samples` (`channels * frames`) to a `.wv`
/// stream, bit-exact and losslessly. Splits into blocks for long inputs.
pub fn encode_float(channels: u32, sample_rate: u32, samples: &[f32]) -> Result<Vec<u8>, Error> {
    let (mono, total_frames, srate_index) = prepare(channels, sample_rate, samples.len())?;
    let ch = channels as usize;

    let mut out = Vec::new();
    let mut block_index: u64 = 0;
    for chunk in samples.chunks(BLOCK_FRAMES * ch) {
        let block_frames = (chunk.len() / ch) as u32;
        let bits: Vec<u32> = chunk.iter().map(|f| f.to_bits()).collect();
        let scan = scan_float_data(&bits);
        if scan.magnitude > 31 {
            return Err(Error::OverMagnitude);
        }
        let mut flags = base_flags(mono, 4, scan.magnitude, srate_index);
        flags |= Flags::FLOAT_DATA;
        let float_info = [scan.flags, scan.shift, scan.max_exp, 127];
        let wvx = if scan.needs_wvx {
            let mut bw = BitWriter::new();
            send_float_data(&bits, scan.flags, scan.max_exp, &mut bw);
            Some((scan.crc_x, bw.close()))
        } else {
            None
        };
        assemble_block(
            &mut out,
            mono,
            flags,
            sample_rate,
            &scan.ints,
            block_index,
            block_frames,
            total_frames,
            Some(float_info),
            wvx.as_ref().map(|(c, b)| (*c, b.as_slice())),
        );
        block_index += u64::from(block_frames);
    }
    Ok(out)
}

/// Validate channel/rate/length and return `(mono, total_frames, srate_index)`.
fn prepare(channels: u32, sample_rate: u32, len: usize) -> Result<(bool, u64, u32), Error> {
    if channels != 1 && channels != 2 {
        return Err(Error::OutOfScope(Scope::MoreThanTwoChannels));
    }
    // A standard rate goes in the header index; anything else uses index 0xF
    // and an ID_SAMPLE_RATE sub-block, so any capture rate round-trips.
    let srate_index = format::SAMPLE_RATES
        .iter()
        .position(|&r| r == sample_rate)
        .map(|i| i as u32)
        .unwrap_or(0xF);
    if sample_rate == 0 || sample_rate >= (1 << 24) {
        return Err(Error::NotYetImplemented("sample rate out of range"));
    }
    if !len.is_multiple_of(channels as usize) {
        return Err(Error::Truncated {
            need: channels as usize,
            have: len,
        });
    }
    Ok((channels == 1, (len / channels as usize) as u64, srate_index))
}

/// The actual max magnitude of integer data, like the reference (the OR of
/// every sample folded through its own sign). Avoids a 1<<31 mute-limit
/// overflow at 32-bit that a nominal `bits-1` would cause.
fn magnitude_of(samples: &[i32]) -> Result<u32, Error> {
    let acc = samples
        .iter()
        .fold(0u32, |a, &s| a | if s < 0 { !s as u32 } else { s as u32 });
    let magnitude = if acc == 0 {
        0
    } else {
        32 - acc.leading_zeros()
    };
    if magnitude > 31 {
        return Err(Error::OverMagnitude);
    }
    Ok(magnitude)
}

fn base_flags(mono: bool, bytes_per_sample: u32, magnitude: u32, srate_index: u32) -> u32 {
    let mut flags = bytes_per_sample - 1;
    if mono {
        flags |= 1 << 2; // MONO_FLAG
    }
    flags |= Flags::INITIAL_BLOCK | Flags::FINAL_BLOCK;
    flags |= magnitude << 18;
    flags |= srate_index << 23;
    flags
}

/// Compute the block CRC over `ints`, forward-decorrelate a copy with the fixed
/// term, entropy-encode the residuals, and append the finished block (header +
/// metadata) to `out`. `float_info` and `wvx` are per-block.
#[allow(clippy::too_many_arguments)]
fn assemble_block(
    out: &mut Vec<u8>,
    mono: bool,
    flags: u32,
    sample_rate: u32,
    ints: &[i32],
    block_index: u64,
    block_frames: u32,
    total_frames: u64,
    float_info: Option<[u8; 4]>,
    wvx: Option<(u32, &[u8])>,
) {
    // Block CRC over the integers (== decoder's CRC over the reconstructed
    // integers before any float expansion): seed 0xffffffff.
    let mut crc: u32 = 0xffffffff;
    if mono {
        for &s in ints {
            crc = crc.wrapping_add(crc << 1).wrapping_add(s as u32);
        }
    } else {
        for f in ints.chunks_exact(2) {
            crc = crc
                .wrapping_add(crc << 3)
                .wrapping_add((f[0] as u32) << 1)
                .wrapping_add(f[0] as u32)
                .wrapping_add(f[1] as u32);
        }
    }

    // Forward-decorrelate a working copy through the fixed cascade, in order.
    // Each pass starts with zero weights and history and adapts within the
    // block. Applying passes one-at-a-time in forward order is equivalent to the
    // reference's per-sample interleaving (each pass sees the same intermediate
    // signal either way), and the decoder inverts them in reverse because the
    // terms are stored reversed.
    let mut residuals = ints.to_vec();
    for &(term, delta) in FIXED_DECORR {
        let mut pass = DecorrPass {
            term,
            delta,
            ..DecorrPass::default()
        };
        if mono {
            forward_decorr_mono_pass(&mut pass, &mut residuals);
        } else {
            forward_decorr_stereo_pass(&mut pass, &mut residuals);
        }
    }

    let mut words = WordsEncoder::new();
    let mut bw = BitWriter::new();
    words.send_words_lossless(&mut bw, &residuals, block_frames, mono);
    words.finish(&mut bw);
    let wv = bw.close();

    // Metadata in the reference's order. Terms in forward order; weights and
    // history are empty sub-blocks, which the decoder reads as all-zero starting
    // state (the same state the forward passes began from).
    let term_bytes: Vec<u8> = FIXED_DECORR
        .iter()
        .map(|&(t, d)| (((t + 5) as u8) & 0x1f) | ((d as u8) << 5))
        .collect();

    let mut m = Vec::new();
    push_sub_block(&mut m, meta::DECORR_TERMS, &term_bytes);
    push_sub_block(&mut m, meta::DECORR_WEIGHTS, &[]);
    push_sub_block(&mut m, meta::DECORR_SAMPLES, &[]);
    push_sub_block(
        &mut m,
        meta::ENTROPY_VARS,
        &WordsEncoder::entropy_vars(mono),
    );
    if let Some(info) = float_info {
        push_sub_block(&mut m, meta::FLOAT_INFO, &info);
    }
    // Non-standard rate (header index 0xF): carry the true rate here. Emitted
    // per block so each independent block is self-describing.
    if (flags >> 23) & 0xf == 0xf {
        let r = sample_rate;
        push_sub_block(
            &mut m,
            meta::SAMPLE_RATE,
            &[r as u8, (r >> 8) as u8, (r >> 16) as u8],
        );
    }
    push_wv_sub_block(&mut m, &wv);
    if let Some((crc_x, xbits)) = wvx {
        push_wvx_sub_block(&mut m, crc_x, xbits);
    }

    let ck_size = (block::HEADER_LEN - 8 + m.len()) as u32;
    out.reserve(block::HEADER_LEN + m.len());
    write_header(
        out,
        ck_size,
        block_index,
        block_frames,
        total_frames,
        flags,
        crc,
    );
    out.extend_from_slice(&m);
}

fn write_header(
    out: &mut Vec<u8>,
    ck_size: u32,
    block_index: u64,
    block_frames: u32,
    total_frames: u64,
    flags: u32,
    crc: u32,
) {
    out.extend_from_slice(&format::MAGIC);
    out.extend_from_slice(&ck_size.to_le_bytes());
    out.extend_from_slice(&format::MAX_STREAM_VERS.to_le_bytes()); // 0x410
    out.push((block_index >> 32) as u8); // block_index high byte (40-bit)
    out.push((total_frames >> 32) as u8); // total_samples high byte (40-bit)
    // total_samples is defined only on the first block; later blocks carry it
    // too and decoders ignore it there.
    out.extend_from_slice(&(total_frames as u32).to_le_bytes());
    out.extend_from_slice(&(block_index as u32).to_le_bytes());
    out.extend_from_slice(&block_frames.to_le_bytes());
    out.extend_from_slice(&flags.to_le_bytes());
    out.extend_from_slice(&crc.to_le_bytes());
}

/// Append a small metadata sub-block (up to 510 data bytes). Odd-length data
/// sets ODD_SIZE and is padded with one byte, as the decoder expects.
fn push_sub_block(out: &mut Vec<u8>, id: u8, data: &[u8]) {
    let odd = data.len() % 2 == 1;
    let words = data.len().div_ceil(2);
    debug_assert!(words <= 255);
    out.push(if odd { id | meta::ODD_SIZE } else { id });
    out.push(words as u8);
    out.extend_from_slice(data);
    if odd {
        out.push(0);
    }
}

/// The `ID_WV_BITSTREAM` sub-block, always in the large (3-byte size) form.
fn push_wv_sub_block(out: &mut Vec<u8>, wv: &[u8]) {
    debug_assert!(wv.len().is_multiple_of(2));
    let words = (wv.len() / 2) as u32;
    out.push(meta::WV_BITSTREAM | meta::LARGE);
    out.push(words as u8);
    out.push((words >> 8) as u8);
    out.push((words >> 16) as u8);
    out.extend_from_slice(wv);
}

/// The `ID_WVX_BITSTREAM` (classic form) sub-block: a 4-byte little-endian CRC
/// prefix, then the residual bitstream. This is the form the reference uses for
/// all float data.
fn push_wvx_sub_block(out: &mut Vec<u8>, crc_x: u32, xbits: &[u8]) {
    debug_assert!(xbits.len().is_multiple_of(2));
    let words = ((4 + xbits.len()) / 2) as u32;
    out.push(meta::WVX_BITSTREAM | meta::LARGE);
    out.push(words as u8);
    out.push((words >> 8) as u8);
    out.push((words >> 16) as u8);
    out.extend_from_slice(&crc_x.to_le_bytes());
    out.extend_from_slice(xbits);
}
