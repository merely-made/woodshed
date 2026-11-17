// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! The median-adaptive entropy decoder (WavPack's Rice variation).
//!
//! Portions derived from WavPack (dbry/WavPack, `read_words.c`,
//! `entropy_utils.c`, and the median macros in `wavpack_local.h`), Copyright
//! (c) David Bryant / Conifer Software, BSD-3-Clause; see ATTRIBUTION.md.
//!
//! Residuals are coded against three adaptive per-channel medians that split
//! the distribution at 5/7, 10/49, and 20/343. A unary `ones_count` selects
//! the zone, `read_code` picks the position inside it, and the medians nudge
//! after every sample (INC/DEC with divisors 128/64/32). Runs of zeros get
//! their own escape when both channels' first medians collapse. This module
//! implements only the lossless path (`error_limit == 0`).

use crate::bitstream::BitReader;
use crate::error::Error;

const LIMIT_ONES: u32 = 16;
const DIV0: u32 = 128;
const DIV1: u32 = 64;
const DIV2: u32 = 32;

/// One channel's three medians, stored *16 like the reference.
#[derive(Clone, Copy, Debug, Default)]
pub struct Medians {
    pub median: [u32; 3],
}

impl Medians {
    /// `GET_MED`: the current breakpoint, minimum 1.
    fn get(&self, i: usize) -> u32 {
        (self.median[i] >> 4) + 1
    }

    // The reference medians are uint32_t; these updates wrap on overflow the
    // same way. Overflow does not arise for real (decorrelated) residuals, but
    // matching C's wrapping keeps encoder and decoder state bit-identical in
    // every case rather than panicking in a debug build.
    fn inc0(&mut self) {
        let d = (self.median[0].wrapping_add(DIV0) / DIV0).wrapping_mul(5);
        self.median[0] = self.median[0].wrapping_add(d);
    }
    fn dec0(&mut self) {
        let d = (self.median[0].wrapping_add(DIV0 - 2) / DIV0).wrapping_mul(2);
        self.median[0] = self.median[0].wrapping_sub(d);
    }
    fn inc1(&mut self) {
        let d = (self.median[1].wrapping_add(DIV1) / DIV1).wrapping_mul(5);
        self.median[1] = self.median[1].wrapping_add(d);
    }
    fn dec1(&mut self) {
        let d = (self.median[1].wrapping_add(DIV1 - 2) / DIV1).wrapping_mul(2);
        self.median[1] = self.median[1].wrapping_sub(d);
    }
    fn inc2(&mut self) {
        let d = (self.median[2].wrapping_add(DIV2) / DIV2).wrapping_mul(5);
        self.median[2] = self.median[2].wrapping_add(d);
    }
    fn dec2(&mut self) {
        let d = (self.median[2].wrapping_add(DIV2 - 2) / DIV2).wrapping_mul(2);
        self.median[2] = self.median[2].wrapping_sub(d);
    }
}

/// Decoder state shared across one block's samples (both channels).
#[cfg(feature = "decode")]
pub struct WordsDecoder {
    pub c: [Medians; 2],
    holding_one: u32,
    holding_zero: bool,
    zeros_acc: u32,
}

#[cfg(feature = "decode")]
impl WordsDecoder {
    /// Initialize from an `ID_ENTROPY_VARS` payload: three little-endian u16
    /// log2 medians per channel (6 bytes mono, 12 stereo), expanded via
    /// `wp_exp2s`.
    pub fn from_entropy_vars(data: &[u8], mono: bool) -> Result<Self, Error> {
        let need = if mono { 6 } else { 12 };
        if data.len() != need {
            return Err(Error::BadSubBlock { id: 0x05 });
        }
        let mut c = [Medians::default(); 2];
        let channels = if mono { 1 } else { 2 };
        for (ch, medians) in c.iter_mut().enumerate().take(channels) {
            for i in 0..3 {
                let o = ch * 6 + i * 2;
                let log = u32::from(data[o]) | u32::from(data[o + 1]) << 8;
                medians.median[i] = wp_exp2s(log as i32) as u32;
            }
        }
        Ok(Self {
            c,
            holding_one: 0,
            holding_zero: false,
            zeros_acc: 0,
        })
    }

    /// The reference `get_words_lossless`: decode `nframes` frames (each one
    /// sample mono, two interleaved stereo) into `buffer`. Returns the number
    /// of frames actually produced (short only on a truncated stream).
    pub fn get_words_lossless(
        &mut self,
        bs: &mut BitReader<'_>,
        buffer: &mut [i32],
        nframes: u32,
        mono: bool,
    ) -> u32 {
        let nsamples = if mono { nframes } else { nframes * 2 } as usize;
        let mut csamples = 0usize;

        while csamples < nsamples {
            let mut ch = if mono { 0 } else { csamples & 1 };

            if self.holding_zero {
                self.holding_zero = false;
                let max = self.c[ch].get(0) - 1;
                let low = bs.read_code(max);
                self.c[ch].dec0();
                buffer[csamples] = if bs.getbit() != 0 {
                    !(low as i32)
                } else {
                    low as i32
                };
                csamples += 1;
                if csamples == nsamples {
                    break;
                }
                if !mono {
                    ch = csamples & 1;
                }
            }

            if self.c[0].median[0] < 2 && self.holding_one == 0 && self.c[1].median[0] < 2 {
                if self.zeros_acc > 0 {
                    self.zeros_acc -= 1;
                    if self.zeros_acc > 0 {
                        buffer[csamples] = 0;
                        csamples += 1;
                        continue;
                    }
                } else {
                    let Some(count) = bs.read_egc_count() else {
                        break;
                    };
                    self.zeros_acc = count;
                    if self.zeros_acc > 0 {
                        self.c[0] = Medians::default();
                        self.c[1] = Medians::default();
                        buffer[csamples] = 0;
                        csamples += 1;
                        continue;
                    }
                }
            }

            let Some(raw_ones) = bs.read_ones_count(LIMIT_ONES) else {
                break;
            };

            let low_carry = self.holding_one;
            self.holding_one = raw_ones & 1;
            self.holding_zero = self.holding_one == 0;
            let ones_count = (raw_ones >> 1).wrapping_add(low_carry);

            let c = &mut self.c[ch];
            let (mut low, high);
            if ones_count == 0 {
                low = 0;
                high = c.get(0) - 1;
                c.dec0();
            } else {
                low = c.get(0);
                c.inc0();
                if ones_count == 1 {
                    high = low.wrapping_add(c.get(1)).wrapping_sub(1);
                    c.dec1();
                } else {
                    low = low.wrapping_add(c.get(1));
                    c.inc1();
                    if ones_count == 2 {
                        high = low.wrapping_add(c.get(2)).wrapping_sub(1);
                        c.dec2();
                    } else {
                        low = low.wrapping_add((ones_count - 2).wrapping_mul(c.get(2)));
                        high = low.wrapping_add(c.get(2)).wrapping_sub(1);
                        c.inc2();
                    }
                }
            }

            low = low.wrapping_add(bs.read_code(high.wrapping_sub(low) & 0x7fffffff));
            buffer[csamples] = if bs.getbit() != 0 {
                !(low as i32)
            } else {
                low as i32
            };
            csamples += 1;
        }

        (if mono { csamples } else { csamples / 2 }) as u32
    }
}

#[cfg(feature = "encode")]
#[inline]
fn count_bits(v: u32) -> u32 {
    if v == 0 { 0 } else { 32 - v.leading_zeros() }
}

/// The median-adaptive entropy *encoder*, the exact mirror of [`WordsDecoder`]
/// (`send_words_lossless` + `flush_word`). Starts from all-zero state, which is
/// what a fresh single block's `ID_ENTROPY_VARS` records.
#[cfg(feature = "encode")]
pub struct WordsEncoder {
    c: [Medians; 2],
    holding_one: u32,
    holding_zero: bool,
    zeros_acc: u32,
    pend_data: u32,
    pend_count: u32,
}

#[cfg(feature = "encode")]
impl WordsEncoder {
    pub fn new() -> Self {
        Self {
            c: [Medians::default(); 2],
            holding_one: 0,
            holding_zero: false,
            zeros_acc: 0,
            pend_data: 0,
            pend_count: 0,
        }
    }

    /// The starting medians are all zero, so `ID_ENTROPY_VARS` for a fresh
    /// single block is six (mono) or twelve (stereo) zero bytes.
    pub fn entropy_vars(mono: bool) -> Vec<u8> {
        vec![0u8; if mono { 6 } else { 12 }]
    }

    fn flush_word(&mut self, bw: &mut crate::bitstream::BitWriter) {
        if self.zeros_acc != 0 {
            let cbits = count_bits(self.zeros_acc);
            for _ in 0..cbits {
                bw.putbit(1);
            }
            bw.putbit(0);
            while self.zeros_acc > 1 {
                bw.putbit(self.zeros_acc & 1);
                self.zeros_acc >>= 1;
            }
            self.zeros_acc = 0;
        }

        if self.holding_one != 0 {
            if self.holding_one >= LIMIT_ONES {
                bw.putbits((1 << LIMIT_ONES) - 1, LIMIT_ONES + 1);
                self.holding_one -= LIMIT_ONES;
                let cbits = count_bits(self.holding_one);
                for _ in 0..cbits {
                    bw.putbit(1);
                }
                bw.putbit(0);
                while self.holding_one > 1 {
                    bw.putbit(self.holding_one & 1);
                    self.holding_one >>= 1;
                }
                self.holding_zero = false;
            } else {
                bw.putbits((1u32 << self.holding_one) - 1, self.holding_one);
            }
            self.holding_one = 0;
        }

        if self.holding_zero {
            bw.putbit(0);
            self.holding_zero = false;
        }

        if self.pend_count != 0 {
            bw.putbits(self.pend_data, self.pend_count);
            self.pend_data = 0;
            self.pend_count = 0;
        }
    }

    /// Encode `nframes` frames (mono: one sample each; stereo: two interleaved)
    /// into `bw`. The residuals in `buffer` are consumed as-is.
    pub fn send_words_lossless(
        &mut self,
        bw: &mut crate::bitstream::BitWriter,
        buffer: &[i32],
        nframes: u32,
        mono: bool,
    ) {
        let nsamples = if mono { nframes } else { nframes * 2 } as usize;

        for (idx, &value) in buffer.iter().take(nsamples).enumerate() {
            let chan = if mono { 0 } else { idx & 1 };
            let sign = value < 0;

            if self.c[0].median[0] < 2 && !self.holding_zero && self.c[1].median[0] < 2 {
                if self.zeros_acc != 0 {
                    if value != 0 {
                        self.flush_word(bw);
                    } else {
                        self.zeros_acc += 1;
                        continue;
                    }
                } else if value != 0 {
                    bw.putbit(0);
                } else {
                    self.c[0] = Medians::default();
                    self.c[1] = Medians::default();
                    self.zeros_acc = 1;
                    continue;
                }
            }

            let v: u32 = if sign { !value as u32 } else { value as u32 };

            let cm = &mut self.c[chan];
            let (mut ones_count, mut low, high);
            let m0 = cm.get(0);
            if v < m0 {
                ones_count = 0;
                low = 0;
                high = m0 - 1;
                cm.dec0();
            } else {
                low = m0;
                cm.inc0();
                let m1 = cm.get(1);
                if v - low < m1 {
                    ones_count = 1;
                    high = low + m1 - 1;
                    cm.dec1();
                } else {
                    low += m1;
                    cm.inc1();
                    let m2 = cm.get(2);
                    if v - low < m2 {
                        ones_count = 2;
                        high = low + m2 - 1;
                        cm.dec2();
                    } else {
                        ones_count = 2 + (v - low) / m2;
                        low += (ones_count - 2) * m2;
                        high = low + m2 - 1;
                        cm.inc2();
                    }
                }
            }

            if self.holding_zero {
                if ones_count != 0 {
                    self.holding_one += 1;
                }
                self.flush_word(bw);
                if ones_count != 0 {
                    self.holding_zero = true;
                    ones_count -= 1;
                } else {
                    self.holding_zero = false;
                }
            } else {
                self.holding_zero = true;
            }

            self.holding_one = ones_count * 2;

            if high != low {
                let maxcode = high - low;
                let code = v - low;
                let bitcount = count_bits(maxcode);
                let extras = (1u32 << bitcount) - maxcode - 1;
                if code < extras {
                    self.pend_data |= code << self.pend_count;
                    self.pend_count += bitcount - 1;
                } else {
                    self.pend_data |= ((code + extras) >> 1) << self.pend_count;
                    self.pend_count += bitcount - 1;
                    self.pend_data |= ((code + extras) & 1) << self.pend_count;
                    self.pend_count += 1;
                }
            }

            self.pend_data |= (sign as u32) << self.pend_count;
            self.pend_count += 1;

            if !self.holding_zero {
                self.flush_word(bw);
            }
        }
    }

    /// Final flush after the last sample, matching the reference `pack_samples`
    /// which calls `flush_word` once more before closing the bitstream.
    pub fn finish(&mut self, bw: &mut crate::bitstream::BitWriter) {
        self.flush_word(bw);
    }
}

#[cfg(feature = "encode")]
impl Default for WordsEncoder {
    fn default() -> Self {
        Self::new()
    }
}

/// `wp_exp2s` from `entropy_utils.c`: expand a signed 16.8-style log2 value
/// back to a 32-bit integer, table-driven.
#[cfg(feature = "decode")]
pub fn wp_exp2s(log: i32) -> i32 {
    let magnitude = log.unsigned_abs();
    let value = u32::from(EXP2_TABLE[(magnitude & 0xff) as usize]) | 0x100;
    let exponent = magnitude >> 8;
    let expanded = if exponent <= 9 {
        (value >> (9 - exponent)) as i32
    } else {
        (value << ((exponent - 9) & 0x1f)) as i32
    };
    if log < 0 {
        expanded.wrapping_neg()
    } else {
        expanded
    }
}

#[cfg(feature = "decode")]
const EXP2_TABLE: [u8; 256] = [
    0x00, 0x01, 0x01, 0x02, 0x03, 0x03, 0x04, 0x05, 0x06, 0x06, 0x07, 0x08, 0x08, 0x09, 0x0a, 0x0b,
    0x0b, 0x0c, 0x0d, 0x0e, 0x0e, 0x0f, 0x10, 0x10, 0x11, 0x12, 0x13, 0x13, 0x14, 0x15, 0x16, 0x16,
    0x17, 0x18, 0x19, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1d, 0x1e, 0x1f, 0x20, 0x20, 0x21, 0x22, 0x23,
    0x24, 0x24, 0x25, 0x26, 0x27, 0x28, 0x28, 0x29, 0x2a, 0x2b, 0x2c, 0x2c, 0x2d, 0x2e, 0x2f, 0x30,
    0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x3a, 0x3b, 0x3c, 0x3d,
    0x3e, 0x3f, 0x40, 0x41, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x48, 0x49, 0x4a, 0x4b,
    0x4c, 0x4d, 0x4e, 0x4f, 0x50, 0x51, 0x51, 0x52, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a,
    0x5b, 0x5c, 0x5d, 0x5e, 0x5e, 0x5f, 0x60, 0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69,
    0x6a, 0x6b, 0x6c, 0x6d, 0x6e, 0x6f, 0x70, 0x71, 0x72, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79,
    0x7a, 0x7b, 0x7c, 0x7d, 0x7e, 0x7f, 0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x87, 0x88, 0x89, 0x8a,
    0x8b, 0x8c, 0x8d, 0x8e, 0x8f, 0x90, 0x91, 0x92, 0x93, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0x9b,
    0x9c, 0x9d, 0x9f, 0xa0, 0xa1, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa8, 0xa9, 0xaa, 0xab, 0xac, 0xad,
    0xaf, 0xb0, 0xb1, 0xb2, 0xb3, 0xb4, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xbc, 0xbd, 0xbe, 0xbf, 0xc0,
    0xc2, 0xc3, 0xc4, 0xc5, 0xc6, 0xc8, 0xc9, 0xca, 0xcb, 0xcd, 0xce, 0xcf, 0xd0, 0xd2, 0xd3, 0xd4,
    0xd6, 0xd7, 0xd8, 0xd9, 0xdb, 0xdc, 0xdd, 0xde, 0xe0, 0xe1, 0xe2, 0xe4, 0xe5, 0xe6, 0xe8, 0xe9,
    0xea, 0xec, 0xed, 0xee, 0xf0, 0xf1, 0xf2, 0xf4, 0xf5, 0xf6, 0xf8, 0xf9, 0xfa, 0xfc, 0xfd, 0xff,
];

#[cfg(all(test, feature = "decode"))]
mod tests {
    use super::*;

    #[test]
    fn exp2s_matches_reference_anchor_points() {
        // wp_exp2s(0) = (0x00|0x100) >> 9 = 0; log 0x100 (1.0) -> 1<<... :
        // value = 0x100, log=1 -> 0x100 >> 8 = 1.
        assert_eq!(wp_exp2s(0), 0);
        assert_eq!(wp_exp2s(0x100), 1);
        // Negation mirrors through the reference's ~(exp2s(-log) - 1) = -exp2s.
        assert_eq!(wp_exp2s(-0x100), -1);
    }

    #[test]
    fn extreme_history_logs_and_median_updates_keep_reference_wrapping() {
        // This signed-16-bit history log expands to INT32_MIN. Negating it
        // must use the same modulo arithmetic as the reference integer path.
        assert_eq!(wp_exp2s(-0x2000), i32::MIN);
        assert_eq!(wp_exp2s(0x2000), i32::MIN);
        let mut medians = Medians {
            median: [u32::MAX; 3],
        };
        medians.inc0();
        medians.dec0();
        medians.inc1();
        medians.dec1();
        medians.inc2();
        medians.dec2();
        assert_eq!(medians.median, [u32::MAX; 3]);
    }

    #[test]
    fn large_escaped_residual_range_is_bounded_without_arithmetic_panics() {
        // Start with nonzero medians so no zero-run prefix is needed. The
        // escape decodes raw_ones=64; its residual interval wraps uint32_t.
        let mut bits = vec![1u8; 16];
        bits.push(0);
        bits.extend([1; 6]);
        bits.push(0);
        bits.extend([0, 0, 0, 0, 1]); // Elias count 48 + LIMIT_ONES = 64
        bits.extend([0; 64]);
        let mut bytes = vec![0u8; bits.len().div_ceil(8)];
        for (i, bit) in bits.into_iter().enumerate() {
            bytes[i / 8] |= bit << (i % 8);
        }
        let mut decoder = WordsDecoder::from_entropy_vars(&[0; 6], true).unwrap();
        decoder.c[0].median = [u32::MAX; 3];
        let mut buffer = [0i32; 1];
        assert_eq!(
            decoder.get_words_lossless(&mut BitReader::new(&bytes), &mut buffer, 1, true),
            1
        );
    }
}
