// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! 32-bit IEEE float reconstruction (decode half).
//!
//! Portions derived from WavPack (dbry/WavPack, `unpack_floats.c` and the f32
//! accessor macros in `wavpack_local.h`), Copyright (c) David Bryant / Conifer
//! Software, BSD-3-Clause; see ATTRIBUTION.md.
//!
//! No floating-point math runs here. The decorrelated integers are aligned
//! mantissas; this module rebuilds the exact 32-bit IEEE-754 pattern (sign,
//! 8-bit exponent, 23-bit mantissa) using integer ops only, restoring the
//! residual low bits from the `wvx` stream and preserving signed zero,
//! denormals, infinities, and NaN payloads bit-for-bit. The decoded buffer
//! then holds those bit patterns as `i32`, which a consumer reads with
//! `f32::from_bits`.

#[cfg(feature = "decode")]
use crate::bitstream::BitReader;
#[cfg(feature = "decode")]
use crate::error::Error;

pub const FLOAT_SHIFT_ONES: u8 = 1;
pub const FLOAT_SHIFT_SAME: u8 = 2;
pub const FLOAT_SHIFT_SENT: u8 = 4;
pub const FLOAT_ZEROS_SENT: u8 = 8;
pub const FLOAT_NEG_ZEROS: u8 = 0x10;
#[allow(dead_code)]
pub const FLOAT_EXCEPTIONS: u8 = 0x20;

/// The `ID_FLOAT_INFO` payload (4 bytes).
#[derive(Clone, Copy, Debug)]
pub struct FloatInfo {
    pub flags: u8,
    pub shift: u8,
    pub max_exp: u8,
    /// Only used by host-side normalization, which wavicle does not apply.
    pub norm_exp: u8,
}

impl FloatInfo {
    #[cfg(feature = "decode")]
    pub fn parse(data: &[u8]) -> Result<Self, Error> {
        let b: [u8; 4] = data
            .try_into()
            .map_err(|_| Error::BadSubBlock { id: 0x08 })?;
        Ok(Self {
            flags: b[0],
            shift: b[1],
            max_exp: b[2],
            norm_exp: b[3],
        })
    }
}

// The read-side f32 field accessors (also used by the encoder).
#[inline]
fn get_mantissa(f: u32) -> u32 {
    f & 0x7fffff
}
#[inline]
fn get_exponent(f: u32) -> u32 {
    (f >> 23) & 0xff
}
#[inline]
fn get_sign(f: u32) -> u32 {
    (f >> 31) & 1
}
#[cfg(feature = "encode")]
#[inline]
fn get_magnitude(f: u32) -> u32 {
    f & 0x7fffffff
}

// The f32-as-i32 field write accessors (decode reconstruction).
#[cfg(feature = "decode")]
#[inline]
fn set_mantissa(f: &mut u32, v: u32) {
    *f = (*f & !0x7fffff) | (v & 0x7fffff);
}
#[cfg(feature = "decode")]
#[inline]
fn set_exponent(f: &mut u32, v: u32) {
    *f = (*f & !0x7f80_0000) | ((v << 23) & 0x7f80_0000);
}
#[cfg(feature = "decode")]
#[inline]
fn set_sign(f: &mut u32, v: u32) {
    *f = (*f & !0x8000_0000) | ((v << 31) & 0x8000_0000);
}

/// The lossless `float_values`: reconstruct exact IEEE bits, restoring residual
/// bits from `xbits`. `min_shifted_zeros`/`max_shifted_ones` come from the wvx
/// "new"-format prefix (0 for classic wvx). Returns the updated `crc_x`.
#[cfg(feature = "decode")]
#[allow(clippy::too_many_arguments)]
pub fn float_values(
    values: &mut [i32],
    info: FloatInfo,
    min_shifted_zeros: u32,
    max_shifted_ones: u32,
    xbits: &mut BitReader<'_>,
    mut crc: u32,
) -> u32 {
    let shift = u32::from(info.shift) & 0x1f;
    let flags = info.flags;

    for value in values.iter_mut() {
        let mut exp = i32::from(info.max_exp);
        let mut outval: u32 = 0;

        if *value == 0 {
            if flags & FLOAT_ZEROS_SENT != 0 {
                if xbits.getbit() != 0 {
                    set_mantissa(&mut outval, xbits.getbits(23));
                    if exp >= 25 {
                        set_exponent(&mut outval, xbits.getbits(8));
                    }
                    set_sign(&mut outval, xbits.getbit());
                } else if flags & FLOAT_NEG_ZEROS != 0 {
                    set_sign(&mut outval, xbits.getbit());
                }
            }
        } else {
            let mut v = (*value as u32) << shift;
            if (v as i32) < 0 {
                v = (v as i32).wrapping_neg() as u32;
                set_sign(&mut outval, 1);
            }

            if v == 0x1000000 {
                if xbits.getbit() != 0 {
                    set_mantissa(&mut outval, xbits.getbits(23));
                }
                set_exponent(&mut outval, 255);
            } else {
                let mut shift_count = 0i32;
                if exp != 0 {
                    loop {
                        if v & 0x800000 != 0 {
                            break;
                        }
                        exp -= 1;
                        if exp == 0 {
                            break;
                        }
                        shift_count += 1;
                        v <<= 1;
                    }
                }
                shift_count &= 0x1f;
                if shift_count != 0 {
                    let sc = shift_count as u32;
                    if flags & FLOAT_SHIFT_ONES != 0
                        || (flags & FLOAT_SHIFT_SAME != 0 && xbits.getbit() != 0)
                    {
                        v |= (1u32 << sc) - 1;
                    } else if flags & FLOAT_SHIFT_SENT != 0 {
                        let mask = (1u32 << sc) - 1;
                        let mut num_zeros = 0u32;
                        if max_shifted_ones != 0 && sc > max_shifted_ones {
                            num_zeros = sc - max_shifted_ones;
                        }
                        if min_shifted_zeros > num_zeros {
                            num_zeros = if min_shifted_zeros > sc {
                                sc
                            } else {
                                min_shifted_zeros
                            };
                        }
                        if sc > num_zeros {
                            let n = sc - num_zeros;
                            let temp = xbits.getbits(n);
                            v |= (temp << num_zeros) & mask;
                        }
                    }
                }
                set_mantissa(&mut outval, v);
                set_exponent(&mut outval, exp as u32);
            }
        }

        crc = crc
            .wrapping_mul(27)
            .wrapping_add(get_mantissa(outval).wrapping_mul(9))
            .wrapping_add(get_exponent(outval).wrapping_mul(3))
            .wrapping_add(get_sign(outval));
        *value = outval as i32;
    }

    crc
}

/// `float_values_nowvx`: the reconstruction used when no `wvx` stream is
/// present. Lossless only when the encoder determined no residual was needed.
#[cfg(feature = "decode")]
pub fn float_values_nowvx(values: &mut [i32], info: FloatInfo) {
    let shift = u32::from(info.shift) & 0x1f;
    let flags = info.flags;

    for value in values.iter_mut() {
        let mut exp = i32::from(info.max_exp);
        let mut outval: u32 = 0;

        if *value != 0 {
            let mut v = (*value as u32) << shift;
            if (v as i32) < 0 {
                v = (v as i32).wrapping_neg() as u32;
                set_sign(&mut outval, 1);
            }

            if v >= 0x1000000 {
                while v & 0xf000000 != 0 {
                    v >>= 1;
                    exp += 1;
                }
            } else if exp != 0 {
                let mut shift_count = 0i32;
                loop {
                    if v & 0x800000 != 0 {
                        break;
                    }
                    exp -= 1;
                    if exp == 0 {
                        break;
                    }
                    shift_count += 1;
                    v <<= 1;
                }
                shift_count &= 0x1f;
                if shift_count != 0 && flags & FLOAT_SHIFT_ONES != 0 {
                    v |= (1u32 << shift_count) - 1;
                }
            }

            set_mantissa(&mut outval, v);
            set_exponent(&mut outval, exp as u32);
        }

        *value = outval as i32;
    }
}

/// Everything `scan_float_data` determines, plus the integer mantissa buffer it
/// produces (which then goes through decorrelation and entropy coding).
#[cfg(feature = "encode")]
pub struct FloatScan {
    /// Mantissa integers (sign-applied), reduced by `shift`.
    pub ints: Vec<i32>,
    pub flags: u8,
    pub shift: u8,
    pub max_exp: u8,
    /// The wvx CRC over the original float fields.
    pub crc_x: u32,
    /// Header magnitude field (bit count of the OR of the mantissa integers).
    pub magnitude: u32,
    /// Whether a `wvx` residual stream is required for losslessness.
    pub needs_wvx: bool,
}

/// The encode-side mirror of [`float_values`]: convert IEEE floats to the
/// integer mantissas WavPack compresses, and decide how the shifted-out bits
/// are carried. Ported from `pack_floats.c` `scan_float_data`. Input is the
/// raw 32-bit patterns (`f32::to_bits`).
#[cfg(feature = "encode")]
pub fn scan_float_data(values: &[u32]) -> FloatScan {
    let mut crc: u32 = 0xffffffff;
    let mut max_mag: i32 = 0;
    for &dp in values {
        crc = crc
            .wrapping_mul(27)
            .wrapping_add(get_mantissa(dp).wrapping_mul(9))
            .wrapping_add(get_exponent(dp).wrapping_mul(3))
            .wrapping_add(get_sign(dp));
        if get_exponent(dp) < 255 && get_magnitude(dp) as i32 > max_mag {
            max_mag = get_magnitude(dp) as i32;
        }
    }
    let crc_x = crc;

    let mut max_exp: i32 = 0;
    if get_exponent(max_mag as u32) != 0 {
        max_exp = get_exponent((max_mag as u32).wrapping_add(0x7F0000)) as i32;
    }

    let mut flags: u8 = 0;
    let mut shift: u8 = 0;
    let (mut shifted_ones, mut shifted_zeros, mut shifted_both) = (0i32, 0i32, 0i32);
    let (mut false_zeros, mut neg_zeros) = (0i32, 0i32);
    let mut ordata: u32 = 0;
    let mut ints = vec![0i32; values.len()];

    for (i, &dp) in values.iter().enumerate() {
        let exp = get_exponent(dp);
        let (mut value, shift_count): (i32, i32) = if exp == 255 {
            flags |= FLOAT_EXCEPTIONS;
            (0x1000000, 0)
        } else if exp != 0 {
            (0x800000 + get_mantissa(dp) as i32, max_exp - exp as i32)
        } else {
            (
                get_mantissa(dp) as i32,
                if max_exp != 0 { max_exp - 1 } else { 0 },
            )
        };

        if shift_count < 25 {
            value >>= shift_count;
        } else {
            value = 0;
        }

        if value == 0 {
            if exp != 0 || get_mantissa(dp) != 0 {
                false_zeros += 1;
            } else if get_sign(dp) != 0 {
                neg_zeros += 1;
            }
        } else if shift_count != 0 {
            let mask = (1u32 << shift_count) - 1;
            let mbits = get_mantissa(dp) & mask;
            if mbits == 0 {
                shifted_zeros += 1;
            } else if mbits == mask {
                shifted_ones += 1;
            } else {
                shifted_both += 1;
            }
        }

        ordata |= value as u32;
        ints[i] = if get_sign(dp) != 0 { -value } else { value };
    }

    if shifted_both != 0 {
        flags |= FLOAT_SHIFT_SENT;
    } else if shifted_ones != 0 && shifted_zeros == 0 {
        flags |= FLOAT_SHIFT_ONES;
    } else if shifted_ones != 0 && shifted_zeros != 0 {
        flags |= FLOAT_SHIFT_SAME;
    } else if ordata != 0 && ordata & 1 == 0 {
        while ordata & 1 == 0 {
            shift += 1;
            ordata >>= 1;
        }
        for v in ints.iter_mut() {
            *v >>= shift;
        }
    }

    let magnitude = if ordata == 0 {
        0
    } else {
        32 - ordata.leading_zeros()
    };

    if false_zeros != 0 || neg_zeros != 0 {
        flags |= FLOAT_ZEROS_SENT;
    }
    if neg_zeros != 0 {
        flags |= FLOAT_NEG_ZEROS;
    }

    let needs_wvx =
        flags & (FLOAT_EXCEPTIONS | FLOAT_ZEROS_SENT | FLOAT_SHIFT_SENT | FLOAT_SHIFT_SAME) != 0;

    FloatScan {
        ints,
        flags,
        shift,
        max_exp: max_exp as u8,
        crc_x,
        magnitude,
        needs_wvx,
    }
}

/// Write the `wvx` residual bits for the original floats, mirroring the decode
/// in [`float_values`]. Ported from `pack_floats.c` `send_float_data`. Must be
/// given the original float patterns and the flags from [`scan_float_data`].
#[cfg(feature = "encode")]
pub fn send_float_data(
    values: &[u32],
    flags: u8,
    max_exp: u8,
    bw: &mut crate::bitstream::BitWriter,
) {
    let max_exp = i32::from(max_exp);
    for &dp in values {
        let exp = get_exponent(dp);
        let (mut value, shift_count): (i32, i32) = if exp == 255 {
            if get_mantissa(dp) != 0 {
                bw.putbit(1);
                bw.putbits(get_mantissa(dp), 23);
            } else {
                bw.putbit(0);
            }
            (0x1000000, 0)
        } else if exp != 0 {
            (0x800000 + get_mantissa(dp) as i32, max_exp - exp as i32)
        } else {
            (
                get_mantissa(dp) as i32,
                if max_exp != 0 { max_exp - 1 } else { 0 },
            )
        };

        if shift_count < 25 {
            value >>= shift_count;
        } else {
            value = 0;
        }

        if value == 0 {
            if flags & FLOAT_ZEROS_SENT != 0 {
                if exp != 0 || get_mantissa(dp) != 0 {
                    bw.putbit(1);
                    bw.putbits(get_mantissa(dp), 23);
                    if max_exp >= 25 {
                        bw.putbits(exp, 8);
                    }
                    bw.putbit(get_sign(dp));
                } else {
                    bw.putbit(0);
                    if flags & FLOAT_NEG_ZEROS != 0 {
                        bw.putbit(get_sign(dp));
                    }
                }
            }
        } else if shift_count != 0 {
            if flags & FLOAT_SHIFT_SENT != 0 {
                let data = get_mantissa(dp) & ((1u32 << shift_count) - 1);
                bw.putbits(data, shift_count as u32);
            } else if flags & FLOAT_SHIFT_SAME != 0 {
                bw.putbit(get_mantissa(dp) & 1);
            }
        }
    }
}
