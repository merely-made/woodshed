// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Inverse decorrelation: metadata parsing and the mono/stereo passes.
//!
//! Portions derived from WavPack (dbry/WavPack, `decorr_utils.c`, `unpack.c`,
//! and the weight macros in `wavpack_local.h`), Copyright (c) David Bryant /
//! Conifer Software, BSD-3-Clause; see ATTRIBUTION.md.
//!
//! Terms, weights, and sample history arrive in metadata sub-blocks stored in
//! the *opposite* order from the pass array (last pass first), and every
//! arithmetic step below must match the reference bit-for-bit: the weight
//! application rounding, the sign-trick weight updates, and the +/-1024 clip
//! used by the cross-channel terms.

#[cfg(feature = "decode")]
use crate::entropy::wp_exp2s;
use crate::error::Error;

pub const MAX_TERM: usize = 8;
const MAX_NTERMS: usize = 16;

#[derive(Clone, Debug, Default)]
pub struct DecorrPass {
    pub term: i32,
    pub delta: i32,
    pub weight_a: i32,
    pub weight_b: i32,
    pub samples_a: [i32; MAX_TERM],
    pub samples_b: [i32; MAX_TERM],
}

/// Parse `ID_DECORR_TERMS`. `passes[0]` receives the *last* stored byte.
#[cfg(feature = "decode")]
pub fn read_decorr_terms(data: &[u8], mono: bool) -> Result<Vec<DecorrPass>, Error> {
    if data.len() > MAX_NTERMS {
        return Err(Error::BadSubBlock { id: 0x02 });
    }
    let mut passes = vec![DecorrPass::default(); data.len()];
    for (i, &byte) in data.iter().enumerate() {
        let pass = &mut passes[data.len() - 1 - i];
        pass.term = i32::from(byte & 0x1f) - 5;
        pass.delta = i32::from((byte >> 5) & 0x7);
        let term = pass.term;
        let valid = term != 0
            && term >= -3
            && !(term > MAX_TERM as i32 && term < 17)
            && term <= 18
            && !(mono && term < 0);
        if !valid {
            return Err(Error::BadSubBlock { id: 0x02 });
        }
    }
    Ok(passes)
}

/// `restore_weight` from `entropy_utils.c`.
#[cfg(feature = "decode")]
fn restore_weight(weight: i8) -> i32 {
    let mut result = i32::from(weight) * 8;
    if result > 0 {
        result += (result + 64) >> 7;
    }
    result
}

/// Parse `ID_DECORR_WEIGHTS`: stored from the last pass backward.
#[cfg(feature = "decode")]
pub fn read_decorr_weights(
    data: &[u8],
    passes: &mut [DecorrPass],
    mono: bool,
) -> Result<(), Error> {
    let termcnt = if mono { data.len() } else { data.len() / 2 };
    if termcnt > passes.len() {
        return Err(Error::BadSubBlock { id: 0x03 });
    }
    let mut bytes = data.iter();
    for pass in passes.iter_mut().rev().take(termcnt) {
        pass.weight_a = restore_weight(*bytes.next().expect("length checked") as i8);
        if !mono {
            pass.weight_b = restore_weight(*bytes.next().expect("length checked") as i8);
        }
    }
    Ok(())
}

/// Parse `ID_DECORR_SAMPLES`: history from the last pass backward, layout
/// depending on each pass's term.
#[cfg(feature = "decode")]
pub fn read_decorr_samples(
    data: &[u8],
    passes: &mut [DecorrPass],
    mono: bool,
) -> Result<(), Error> {
    fn take<'a>(data: &'a [u8], pos: &mut usize, n: usize) -> Result<&'a [u8], Error> {
        if *pos + n > data.len() {
            return Err(Error::BadSubBlock { id: 0x04 });
        }
        let s = &data[*pos..*pos + n];
        *pos += n;
        Ok(s)
    }
    fn exp(b: &[u8], o: usize) -> i32 {
        wp_exp2s(i32::from(i16::from_le_bytes([b[o], b[o + 1]])))
    }

    let mut pos = 0usize;
    for pass in passes.iter_mut().rev() {
        if pos >= data.len() {
            break;
        }
        if pass.term > MAX_TERM as i32 {
            let b = take(data, &mut pos, if mono { 4 } else { 8 })?;
            pass.samples_a[0] = exp(b, 0);
            pass.samples_a[1] = exp(b, 2);
            if !mono {
                pass.samples_b[0] = exp(b, 4);
                pass.samples_b[1] = exp(b, 6);
            }
        } else if pass.term < 0 {
            let b = take(data, &mut pos, 4)?;
            pass.samples_a[0] = exp(b, 0);
            pass.samples_b[0] = exp(b, 2);
        } else {
            for m in 0..pass.term as usize {
                let b = take(data, &mut pos, if mono { 2 } else { 4 })?;
                pass.samples_a[m] = exp(b, 0);
                if !mono {
                    pass.samples_b[m] = exp(b, 2);
                }
            }
        }
    }
    if pos != data.len() {
        return Err(Error::BadSubBlock { id: 0x04 });
    }
    Ok(())
}

/// `apply_weight`: the reference's dual path, chosen per sample magnitude.
#[inline]
fn apply_weight(weight: i32, sample: i32) -> i32 {
    if sample != i32::from(sample as i16) {
        // apply_weight_f: split multiply to survive 32-bit overflow.
        ((((sample & 0xffff).wrapping_mul(weight)) >> 9)
            .wrapping_add(((sample & !0xffff) >> 9).wrapping_mul(weight))
            .wrapping_add(1))
            >> 1
    } else {
        // apply_weight_i
        (weight.wrapping_mul(sample).wrapping_add(512)) >> 10
    }
}

/// `update_weight`: nudge by delta toward agreement of signs.
#[inline]
fn update_weight(weight: &mut i32, delta: i32, source: i32, result: i32) {
    if source != 0 && result != 0 {
        let s = (source ^ result) >> 31;
        *weight = (delta ^ s).wrapping_add(weight.wrapping_sub(s));
    }
}

/// `update_weight_clip`: as above but clipped to +/-1024 (cross-channel terms).
#[inline]
#[cfg(feature = "decode")]
fn update_weight_clip(weight: &mut i32, delta: i32, source: i32, result: i32) {
    if source != 0 && result != 0 {
        let s = (source ^ result) >> 31;
        let mut w = (*weight ^ s).wrapping_add(delta - s);
        if w > 1024 {
            w = 1024;
        }
        *weight = (w ^ s) - s;
    }
}

/// One forward mono pass (`decorr_mono_buffer` for a single term), the exact
/// inverse of [`decorr_mono_pass`]. Positive terms 1..8 and 17/18. Sample
/// history is left un-normalized, which is fine for single-block encoding.
#[cfg(feature = "encode")]
pub fn forward_decorr_mono_pass(dpp: &mut DecorrPass, buffer: &mut [i32]) {
    let delta = dpp.delta;
    let mut weight = dpp.weight_a;
    let mut m = 0usize;
    let mut k = (dpp.term as usize) & (MAX_TERM - 1);

    for s in buffer.iter_mut() {
        let orig = *s;
        let sam = if dpp.term > MAX_TERM as i32 {
            let sam = if dpp.term & 1 != 0 {
                2i32.wrapping_mul(dpp.samples_a[0])
                    .wrapping_sub(dpp.samples_a[1])
            } else {
                (3i32
                    .wrapping_mul(dpp.samples_a[0])
                    .wrapping_sub(dpp.samples_a[1]))
                    >> 1
            };
            dpp.samples_a[1] = dpp.samples_a[0];
            dpp.samples_a[0] = orig;
            sam
        } else {
            let sam = dpp.samples_a[m];
            dpp.samples_a[k] = orig;
            sam
        };
        let code = orig.wrapping_sub(apply_weight(weight, sam));
        update_weight(&mut weight, delta, sam, code);
        *s = code;
        m = (m + 1) & (MAX_TERM - 1);
        k = (k + 1) & (MAX_TERM - 1);
    }
    dpp.weight_a = weight;
}

/// One forward stereo pass, the exact inverse of [`decorr_stereo_pass`], for
/// positive terms 1..8 and 17/18 (no cross-channel terms at this milestone).
#[cfg(feature = "encode")]
pub fn forward_decorr_stereo_pass(dpp: &mut DecorrPass, buffer: &mut [i32]) {
    let delta = dpp.delta;
    match dpp.term {
        17 | 18 => {
            for f in buffer.chunks_exact_mut(2) {
                for (samples, weight, val) in [
                    (&mut dpp.samples_a, &mut dpp.weight_a, 0usize),
                    (&mut dpp.samples_b, &mut dpp.weight_b, 1usize),
                ] {
                    let sam = if dpp.term == 17 {
                        2i32.wrapping_mul(samples[0]).wrapping_sub(samples[1])
                    } else {
                        samples[0].wrapping_add(samples[0].wrapping_sub(samples[1]) >> 1)
                    };
                    samples[1] = samples[0];
                    samples[0] = f[val];
                    let tmp = f[val].wrapping_sub(apply_weight(*weight, sam));
                    f[val] = tmp;
                    update_weight(weight, delta, sam, tmp);
                }
            }
        }
        term if term > 0 => {
            let mut m = 0usize;
            let mut k = (term as usize) & (MAX_TERM - 1);
            for f in buffer.chunks_exact_mut(2) {
                let sam = dpp.samples_a[m];
                dpp.samples_a[k] = f[0];
                let tmp = f[0].wrapping_sub(apply_weight(dpp.weight_a, sam));
                f[0] = tmp;
                update_weight(&mut dpp.weight_a, delta, sam, tmp);

                let sam = dpp.samples_b[m];
                dpp.samples_b[k] = f[1];
                let tmp = f[1].wrapping_sub(apply_weight(dpp.weight_b, sam));
                f[1] = tmp;
                update_weight(&mut dpp.weight_b, delta, sam, tmp);

                m = (m + 1) & (MAX_TERM - 1);
                k = (k + 1) & (MAX_TERM - 1);
            }
        }
        _ => unreachable!("encode uses only positive terms"),
    }
}

/// One inverse pass over a mono buffer (`decorr_mono_pass`).
#[cfg(feature = "decode")]
pub fn decorr_mono_pass(dpp: &mut DecorrPass, buffer: &mut [i32]) {
    let delta = dpp.delta;
    let mut weight = dpp.weight_a;

    match dpp.term {
        17 => {
            for s in buffer.iter_mut() {
                let sam = 2i32
                    .wrapping_mul(dpp.samples_a[0])
                    .wrapping_sub(dpp.samples_a[1]);
                dpp.samples_a[1] = dpp.samples_a[0];
                let out = apply_weight(weight, sam).wrapping_add(*s);
                update_weight(&mut weight, delta, sam, *s);
                dpp.samples_a[0] = out;
                *s = out;
            }
        }
        18 => {
            for s in buffer.iter_mut() {
                let sam = (3i32
                    .wrapping_mul(dpp.samples_a[0])
                    .wrapping_sub(dpp.samples_a[1]))
                    >> 1;
                dpp.samples_a[1] = dpp.samples_a[0];
                let out = apply_weight(weight, sam).wrapping_add(*s);
                update_weight(&mut weight, delta, sam, *s);
                dpp.samples_a[0] = out;
                *s = out;
            }
        }
        _ => {
            let mut m = 0usize;
            let mut k = (dpp.term as usize) & (MAX_TERM - 1);
            for s in buffer.iter_mut() {
                let sam = dpp.samples_a[m];
                let out = apply_weight(weight, sam).wrapping_add(*s);
                update_weight(&mut weight, delta, sam, *s);
                dpp.samples_a[k] = out;
                *s = out;
                m = (m + 1) & (MAX_TERM - 1);
                k = (k + 1) & (MAX_TERM - 1);
            }
            if m != 0 {
                let temp = dpp.samples_a;
                for (k, slot) in dpp.samples_a.iter_mut().enumerate() {
                    *slot = temp[(m + k) & (MAX_TERM - 1)];
                }
            }
        }
    }
    dpp.weight_a = weight;
}

/// One inverse pass over an interleaved stereo buffer (`decorr_stereo_pass`).
#[cfg(feature = "decode")]
pub fn decorr_stereo_pass(dpp: &mut DecorrPass, buffer: &mut [i32]) {
    let delta = dpp.delta;

    match dpp.term {
        17 => {
            for f in buffer.chunks_exact_mut(2) {
                let sam = 2i32
                    .wrapping_mul(dpp.samples_a[0])
                    .wrapping_sub(dpp.samples_a[1]);
                dpp.samples_a[1] = dpp.samples_a[0];
                let tmp = f[0];
                let out = apply_weight(dpp.weight_a, sam).wrapping_add(tmp);
                dpp.samples_a[0] = out;
                f[0] = out;
                update_weight(&mut dpp.weight_a, delta, sam, tmp);

                let sam = 2i32
                    .wrapping_mul(dpp.samples_b[0])
                    .wrapping_sub(dpp.samples_b[1]);
                dpp.samples_b[1] = dpp.samples_b[0];
                let tmp = f[1];
                let out = apply_weight(dpp.weight_b, sam).wrapping_add(tmp);
                dpp.samples_b[0] = out;
                f[1] = out;
                update_weight(&mut dpp.weight_b, delta, sam, tmp);
            }
        }
        18 => {
            for f in buffer.chunks_exact_mut(2) {
                let sam = dpp.samples_a[0]
                    .wrapping_add(dpp.samples_a[0].wrapping_sub(dpp.samples_a[1]) >> 1);
                dpp.samples_a[1] = dpp.samples_a[0];
                let tmp = f[0];
                let out = apply_weight(dpp.weight_a, sam).wrapping_add(tmp);
                dpp.samples_a[0] = out;
                f[0] = out;
                update_weight(&mut dpp.weight_a, delta, sam, tmp);

                let sam = dpp.samples_b[0]
                    .wrapping_add(dpp.samples_b[0].wrapping_sub(dpp.samples_b[1]) >> 1);
                dpp.samples_b[1] = dpp.samples_b[0];
                let tmp = f[1];
                let out = apply_weight(dpp.weight_b, sam).wrapping_add(tmp);
                dpp.samples_b[0] = out;
                f[1] = out;
                update_weight(&mut dpp.weight_b, delta, sam, tmp);
            }
        }
        term if term > 0 => {
            let mut m = 0usize;
            let mut k = (term as usize) & (MAX_TERM - 1);
            for f in buffer.chunks_exact_mut(2) {
                let sam = dpp.samples_a[m];
                let out = apply_weight(dpp.weight_a, sam).wrapping_add(f[0]);
                update_weight(&mut dpp.weight_a, delta, sam, f[0]);
                dpp.samples_a[k] = out;
                f[0] = out;

                let sam = dpp.samples_b[m];
                let out = apply_weight(dpp.weight_b, sam).wrapping_add(f[1]);
                update_weight(&mut dpp.weight_b, delta, sam, f[1]);
                dpp.samples_b[k] = out;
                f[1] = out;

                m = (m + 1) & (MAX_TERM - 1);
                k = (k + 1) & (MAX_TERM - 1);
            }
        }
        -1 => {
            for f in buffer.chunks_exact_mut(2) {
                let sam = f[0].wrapping_add(apply_weight(dpp.weight_a, dpp.samples_a[0]));
                update_weight_clip(&mut dpp.weight_a, delta, dpp.samples_a[0], f[0]);
                f[0] = sam;
                let out = f[1].wrapping_add(apply_weight(dpp.weight_b, sam));
                update_weight_clip(&mut dpp.weight_b, delta, sam, f[1]);
                dpp.samples_a[0] = out;
                f[1] = out;
            }
        }
        -2 => {
            for f in buffer.chunks_exact_mut(2) {
                let sam = f[1].wrapping_add(apply_weight(dpp.weight_b, dpp.samples_b[0]));
                update_weight_clip(&mut dpp.weight_b, delta, dpp.samples_b[0], f[1]);
                f[1] = sam;
                let out = f[0].wrapping_add(apply_weight(dpp.weight_a, sam));
                update_weight_clip(&mut dpp.weight_a, delta, sam, f[0]);
                dpp.samples_b[0] = out;
                f[0] = out;
            }
        }
        -3 => {
            for f in buffer.chunks_exact_mut(2) {
                let sam_a = f[0].wrapping_add(apply_weight(dpp.weight_a, dpp.samples_a[0]));
                update_weight_clip(&mut dpp.weight_a, delta, dpp.samples_a[0], f[0]);
                let sam_b = f[1].wrapping_add(apply_weight(dpp.weight_b, dpp.samples_b[0]));
                update_weight_clip(&mut dpp.weight_b, delta, dpp.samples_b[0], f[1]);
                f[0] = sam_a;
                dpp.samples_b[0] = sam_a;
                f[1] = sam_b;
                dpp.samples_a[0] = sam_b;
            }
        }
        _ => unreachable!("terms validated at parse"),
    }
}
