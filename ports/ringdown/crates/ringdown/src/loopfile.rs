//! Loop recordings: the WAV header the instrument writes, and the tempo in it.
//!
//! A loop is an ordinary RIFF/WAVE file with one vendor extension: a `JUNK`
//! chunk labelled `"HyVibe loop file"` carrying six 32-bit little-endian
//! values. `JUNK` is the RIFF chunk type readers are required to skip, so the
//! audio plays anywhere while the metadata is invisible to everything that does
//! not look for it.
//!
//! Recovering it makes a loop importable *in time*: a recording that arrives
//! knowing its own tempo can be dropped onto a grid instead of stretched by ear.
//!
//! # The six values
//!
//! ```text
//! version, tempo_bpm, beats_per_bar, beat_unit, bars, partial
//! ```
//!
//! Fields three to five are a time signature and a bar count: the instrument's
//! owner confirms `loop0031.wav` is **200 BPM, 7/8, 4 bars**, which is
//! `beats_per_bar = 7`, `beat_unit = 8`, `bars = 4` exactly.
//!
//! `tempo_bpm` counts [`beat_unit`](LoopMeta::beat_unit) notes, not quarters:
//! 4 bars of 7/8 is 28 eighth-notes, and at 200 of them per minute that is the
//! 8.4 s the file holds.
//!
//! Established against **all 31 loops on the reference instrument** — five
//! tempos, four bar counts, two meters, and both states of the last field. The
//! relation that holds across every one of them:
//!
//! ```text
//! beats    = beats_per_bar × bars
//! nominal  = beats × 60 / tempo_bpm            (seconds)
//! recorded = ceil(nominal_samples / 256) × 256 (when partial == 0)
//! ```
//!
//! Every one of the 31 files is a whole number of **256-sample blocks**, and
//! for all 24 with `partial == 0` the block-rounded nominal length reproduces
//! the recorded length exactly, with no exceptions.
//!
//! Why the whole corpus and not one file: **the block is 256, and a single
//! file cannot show that.** `loop0031.wav` alone is also divisible by 2048,
//! which is wrong for every other loop. `probe --index` reads all 31 headers in
//! one round trip each, so the check costs about a minute of instrument time
//! and is worth running against any instrument this code meets.
//!
//! # `beats_per_bar` and `bars` are separable, and the corpus shows which
//!
//! [`beats_per_bar`](LoopMeta::beats_per_bar) takes two values across the
//! corpus, `4` and `7`; [`bars`](LoopMeta::bars) takes four, `4`, `8`, `12` and
//! `14`. Read the other way round, a player would be recording in 12- and
//! 14-beat bars and changing meter between consecutive takes while holding the
//! bar count at 7. Read this way they set 7/8 for three takes and recorded 14,
//! 12 and 4 bars of it. The owner confirms the latter for `loop0031`, and the
//! field that moves often is the bar count.
//!
//! `beat_unit` is `8` on 28 loops and `4` on three, and reads as a denominator
//! throughout — most of these loops are in 4/8, three in 4/4, three in 7/8. It
//! does not enter the duration arithmetic; it only says what note
//! [`tempo_bpm`](LoopMeta::tempo_bpm) counts.
//!
//! `ReadMetronome` reports the same field, and reports it correctly. What
//! `UpdateMetronome` will not do is *write* it — see [`crate::rpc::params`].

use core::fmt;

/// Bytes to read from the start of a loop to cover everything here.
///
/// Every loop seen lays its header out identically — `RIFF`/`WAVE`, the 40-byte
/// `JUNK`, a 16-byte `fmt `, then `data` — which puts the first audio sample at
/// exactly this offset. [`parse`] walks the chunks rather than assuming that,
/// so a file that differs is reported as [`LoopError::Truncated`] instead of
/// being misread.
pub const HEADER_PREFIX: usize = 92;

/// The label the vendor writes at the start of its `JUNK` chunk.
pub const JUNK_LABEL: &[u8] = b"HyVibe loop file";

/// Count of 32-bit values following [`JUNK_LABEL`].
pub const META_VALUES: usize = 6;

/// The block the recorder writes in, in samples.
///
/// Every one of the 31 loops on the reference instrument is a whole number of
/// these, and 512 divides only some of them, so 256 is the actual granularity
/// rather than the largest that happened to fit one file.
pub const BLOCK_SAMPLES: u32 = 256;

/// The vendor's metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoopMeta {
    /// Format version. `1` on every loop seen.
    pub version: u32,
    /// Tempo in beats per minute, counting [`beat_unit`](Self::beat_unit)
    /// notes — eighths when `beat_unit` is 8, not quarters.
    pub tempo_bpm: u32,
    /// Beats in one bar: the time signature's numerator. `4` on 28 loops,
    /// `7` on three.
    pub beats_per_bar: u32,
    /// The note a beat is: the time signature's denominator. `8` on 28 loops,
    /// `4` on three.
    ///
    /// Does not enter the duration arithmetic — it only says what unit
    /// [`tempo_bpm`](Self::tempo_bpm) counts. Spelled `den` on the wire.
    pub beat_unit: u32,
    /// Bars the loop was set to run. `4`, `8`, `12` or `14` across the corpus.
    pub bars: u32,
    /// Non-zero when the take did not run its full bar count.
    ///
    /// Every loop with this set is *shorter* than its grid length — between 29%
    /// and 96% of it — and every loop without it lands on the grid exactly. Use
    /// [`is_partial`](Self::is_partial) rather than comparing to 1.
    ///
    /// Very likely the `free` flag `StartRecording` takes, which selects
    /// free-running over bar-locked recording. That is an inference from the
    /// two matching descriptions, not something observed, so the field is named
    /// for what it demonstrably indicates.
    pub partial: u32,
}

impl LoopMeta {
    /// The loop's length in beats.
    pub fn beats(&self) -> u32 {
        self.beats_per_bar.saturating_mul(self.bars)
    }

    /// Whether the take stopped short of its bar count.
    pub fn is_partial(&self) -> bool {
        self.partial != 0
    }

    /// How long [`beats`](Self::beats) lasts at this tempo, in samples, before
    /// block rounding.
    ///
    /// This is the *grid* length. A partial take is shorter than it; a complete
    /// one is [`expected_samples`](Self::expected_samples).
    ///
    /// `None` when the tempo is zero, which no real file should carry but a
    /// corrupt one might.
    pub fn nominal_samples(&self, sample_rate: u32) -> Option<u64> {
        if self.tempo_bpm == 0 {
            return None;
        }
        Some(u64::from(self.beats()) * u64::from(sample_rate) * 60 / u64::from(self.tempo_bpm))
    }

    /// What a complete take of this loop actually occupies: the grid length
    /// rounded up to a whole [`BLOCK_SAMPLES`] block.
    ///
    /// Meaningless for a partial take, which stops wherever it was stopped.
    pub fn expected_samples(&self, sample_rate: u32) -> Option<u64> {
        let nominal = self.nominal_samples(sample_rate)?;
        let block = u64::from(BLOCK_SAMPLES);
        Some(nominal.div_ceil(block) * block)
    }
}

/// The `fmt ` chunk's description of the audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Format {
    /// WAVE format tag; `1` is uncompressed PCM.
    pub format_tag: u16,
    /// Channel count.
    pub channels: u16,
    /// Samples per second.
    pub sample_rate: u32,
    /// Bits in one sample of one channel.
    pub bits_per_sample: u16,
}

impl Format {
    /// Bytes occupied by one sample across all channels.
    pub fn frame_bytes(&self) -> u32 {
        u32::from(self.channels) * u32::from(self.bits_per_sample).div_ceil(8)
    }
}

/// A parsed loop header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoopHeader {
    /// What the `fmt ` chunk declares.
    pub format: Format,
    /// Length of the `data` chunk in bytes.
    pub audio_bytes: u32,
    /// Offset of the first audio byte within the file.
    pub audio_offset: usize,
    /// Total file length implied by the `RIFF` header.
    pub file_bytes: u64,
    /// The vendor's metadata, when the `JUNK` chunk carries its label.
    ///
    /// `None` for a WAV that is not one of these — a file copied in over USB,
    /// say — which is worth surfacing rather than defaulting away.
    pub meta: Option<LoopMeta>,
}

/// How a loop's audio compares to the length its header implies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LengthCheck {
    /// A complete take landing exactly on its block-rounded grid length.
    OnGrid,
    /// A take marked partial, and shorter than the grid as one should be.
    PartialAsMarked {
        /// Samples short of the grid.
        short_by: u64,
    },
    /// The audio does not match what the header implies.
    ///
    /// Worth more attention than any agreement: the reading in this module was
    /// built from a corpus of one instrument, and a file that disagrees is how
    /// it gets corrected again.
    Disagrees {
        /// What a complete take would occupy.
        expected: u64,
        /// What this file actually holds.
        actual: u64,
    },
}

impl LoopHeader {
    /// Audio length in whole samples per channel.
    pub fn samples(&self) -> u32 {
        let frame = self.format.frame_bytes();
        if frame == 0 {
            return 0;
        }
        self.audio_bytes / frame
    }

    /// Audio duration in seconds.
    pub fn seconds(&self) -> f64 {
        if self.format.sample_rate == 0 {
            return 0.0;
        }
        f64::from(self.samples()) / f64::from(self.format.sample_rate)
    }

    /// Whether the audio is the length the metadata implies.
    ///
    /// The check that established what these fields mean, kept executable and
    /// applied to the two cases separately: a complete take must land on its
    /// block-rounded grid length exactly, and a partial one must be shorter
    /// than the grid.
    ///
    /// `None` when there is no metadata to check against.
    pub fn check_length(&self) -> Option<LengthCheck> {
        let meta = self.meta?;
        let rate = self.format.sample_rate;
        let actual = u64::from(self.samples());
        let nominal = meta.nominal_samples(rate)?;
        let expected = meta.expected_samples(rate)?;

        if meta.is_partial() {
            return Some(if actual < nominal {
                LengthCheck::PartialAsMarked {
                    short_by: nominal - actual,
                }
            } else {
                LengthCheck::Disagrees { expected, actual }
            });
        }
        Some(if actual == expected {
            LengthCheck::OnGrid
        } else {
            LengthCheck::Disagrees { expected, actual }
        })
    }

    /// Whether the audio agrees with the header, either way.
    pub fn length_agrees(&self) -> Option<bool> {
        Some(!matches!(
            self.check_length()?,
            LengthCheck::Disagrees { .. }
        ))
    }
}

/// Why a loop header could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopError {
    /// The bytes do not begin `RIFF....WAVE`.
    NotRiffWave,
    /// The header ran out before a required chunk was found.
    ///
    /// Carries how many bytes were supplied, since the usual cause is asking
    /// the instrument for too small a prefix rather than a damaged file.
    Truncated {
        /// Bytes that were available.
        got: usize,
    },
    /// No `fmt ` chunk appeared before the end of the supplied bytes.
    NoFormat,
    /// No `data` chunk appeared before the end of the supplied bytes.
    NoData,
}

impl fmt::Display for LoopError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoopError::NotRiffWave => write!(f, "not a RIFF/WAVE file"),
            LoopError::Truncated { got } => write!(
                f,
                "header incomplete in {got} bytes; read at least {HEADER_PREFIX}"
            ),
            LoopError::NoFormat => write!(f, "no fmt chunk"),
            LoopError::NoData => write!(f, "no data chunk"),
        }
    }
}

impl core::error::Error for LoopError {}

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*data.get(at)?, *data.get(at + 1)?]))
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes([
        *data.get(at)?,
        *data.get(at + 1)?,
        *data.get(at + 2)?,
        *data.get(at + 3)?,
    ]))
}

/// Read a loop's header from the first bytes of the file.
///
/// Needs only a prefix — [`HEADER_PREFIX`] covers every loop seen — which is
/// what makes indexing a library over Bluetooth affordable.
pub fn parse(data: &[u8]) -> Result<LoopHeader, LoopError> {
    if data.len() < 12 || &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        if data.len() < 12 {
            return Err(LoopError::Truncated { got: data.len() });
        }
        return Err(LoopError::NotRiffWave);
    }
    let file_bytes =
        u64::from(u32_at(data, 4).ok_or(LoopError::Truncated { got: data.len() })?) + 8;

    let mut format = None;
    let mut audio = None;
    let mut meta = None;

    // Walk the chunk list rather than assuming the vendor's exact layout. A
    // chunk's payload is padded to an even length, which is easy to forget and
    // shifts every later chunk by one when it happens.
    let mut at = 12usize;
    while at + 8 <= data.len() {
        let id = &data[at..at + 4];
        let size = u32_at(data, at + 4).ok_or(LoopError::Truncated { got: data.len() })? as usize;
        let body = at + 8;

        match id {
            b"fmt " => {
                if body + 16 > data.len() {
                    return Err(LoopError::Truncated { got: data.len() });
                }
                format = Some(Format {
                    format_tag: u16_at(data, body).unwrap(),
                    channels: u16_at(data, body + 2).unwrap(),
                    sample_rate: u32_at(data, body + 4).unwrap(),
                    bits_per_sample: u16_at(data, body + 14).unwrap(),
                });
            }
            b"data" => {
                // The payload is the audio, and is not expected to be present.
                audio = Some((size as u32, body));
                break;
            }
            b"JUNK" => {
                meta = parse_meta(data.get(body..body + size).unwrap_or(&[]));
            }
            _ => {}
        }

        at = body + size + (size & 1);
    }

    let format = format.ok_or(if data.len() < HEADER_PREFIX {
        LoopError::Truncated { got: data.len() }
    } else {
        LoopError::NoFormat
    })?;
    let (audio_bytes, audio_offset) = audio.ok_or(if data.len() < HEADER_PREFIX {
        LoopError::Truncated { got: data.len() }
    } else {
        LoopError::NoData
    })?;

    Ok(LoopHeader {
        format,
        audio_bytes,
        audio_offset,
        file_bytes,
        meta,
    })
}

/// Read the vendor's six values out of a `JUNK` payload, if it is theirs.
fn parse_meta(body: &[u8]) -> Option<LoopMeta> {
    if body.len() < JUNK_LABEL.len() + META_VALUES * 4 {
        return None;
    }
    if &body[..JUNK_LABEL.len()] != JUNK_LABEL {
        return None;
    }
    let v = |i: usize| u32_at(body, JUNK_LABEL.len() + i * 4);
    Some(LoopMeta {
        version: v(0)?,
        tempo_bpm: v(1)?,
        beats_per_bar: v(2)?,
        beat_unit: v(3)?,
        bars: v(4)?,
        partial: v(5)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The first 92 bytes of `/Loops/loop0031.wav`, read off the reference
    /// instrument on 2026-08-27 and checked byte-identical against the copy on
    /// disk rather than transcribed by eye.
    const LOOP0031: [u8; 92] = [
        0x52, 0x49, 0x46, 0x46, 0x54, 0x50, 0x0b, 0x00, // RIFF, size
        0x57, 0x41, 0x56, 0x45, // WAVE
        0x4a, 0x55, 0x4e, 0x4b, 0x28, 0x00, 0x00, 0x00, // JUNK, 40
        0x48, 0x79, 0x56, 0x69, 0x62, 0x65, 0x20, 0x6c, // "HyVibe l"
        0x6f, 0x6f, 0x70, 0x20, 0x66, 0x69, 0x6c, 0x65, // "oop file"
        0x01, 0x00, 0x00, 0x00, // version 1
        0xc8, 0x00, 0x00, 0x00, // 200 bpm
        0x07, 0x00, 0x00, 0x00, // 7 beats per bar
        0x08, 0x00, 0x00, 0x00, // beat_unit 8 (eighth notes)
        0x04, 0x00, 0x00, 0x00, // 4 bars
        0x00, 0x00, 0x00, 0x00, // not partial
        0x66, 0x6d, 0x74, 0x20, 0x10, 0x00, 0x00, 0x00, // fmt , 16
        0x01, 0x00, // PCM
        0x01, 0x00, // mono
        0x44, 0xac, 0x00, 0x00, // 44100
        0x88, 0x58, 0x01, 0x00, // byte rate
        0x02, 0x00, // block align
        0x10, 0x00, // 16 bits
        0x64, 0x61, 0x74, 0x61, 0x00, 0x50, 0x0b, 0x00, // data, 741376
    ];

    /// Every loop on the reference instrument, read by `probe --index` on
    /// 2026-08-28: `(name, samples, bpm, beats_per_bar, beat_unit, bars, partial)`.
    ///
    /// The whole corpus rather than a sample of it. Five tempos, four bar
    /// counts, two meters and both states of `partial` between them constrain
    /// the reading in ways no single file can: one loop is consistent with
    /// several block sizes and with either assignment of the two length
    /// fields, and thirty-one are not.
    const CORPUS: [(&str, u32, u32, u32, u32, u32, u32); 31] = [
        ("loop0001", 1_620_736, 160, 7, 8, 14, 0),
        ("loop0002", 1_389_312, 160, 7, 8, 12, 0),
        ("loop0003", 529_408, 160, 4, 8, 8, 0),
        ("loop0004", 509_952, 160, 4, 8, 8, 1),
        ("loop0005", 529_408, 160, 4, 8, 8, 0),
        ("loop0006", 155_648, 160, 4, 8, 8, 1),
        ("loop0007", 398_848, 160, 4, 8, 8, 1),
        ("loop0008", 370_432, 160, 4, 8, 8, 1),
        ("loop0009", 529_408, 160, 4, 8, 8, 0),
        ("loop0010", 529_408, 160, 4, 8, 8, 0),
        ("loop0011", 434_688, 160, 4, 8, 8, 1),
        ("loop0012", 264_192, 160, 4, 8, 8, 1),
        ("loop0013", 529_408, 160, 4, 8, 8, 0),
        ("loop0014", 529_408, 160, 4, 8, 8, 0),
        ("loop0015", 529_408, 160, 4, 8, 8, 0),
        ("loop0016", 264_704, 160, 4, 8, 4, 0),
        ("loop0017", 264_704, 160, 4, 8, 4, 0),
        ("loop0018", 264_704, 160, 4, 8, 4, 0),
        ("loop0019", 264_704, 160, 4, 8, 4, 0),
        ("loop0020", 264_704, 160, 4, 8, 4, 0),
        ("loop0021", 353_024, 120, 4, 8, 4, 0),
        ("loop0022", 705_792, 120, 4, 8, 8, 0),
        ("loop0023", 502_784, 120, 4, 8, 8, 1),
        ("loop0024", 705_792, 120, 4, 8, 8, 0),
        ("loop0025", 1_411_328, 60, 4, 4, 8, 0),
        ("loop0026", 1_411_328, 60, 4, 4, 8, 0),
        ("loop0027", 1_411_328, 60, 4, 4, 8, 0),
        ("loop0028", 470_528, 90, 4, 8, 4, 0),
        ("loop0029", 470_528, 90, 4, 8, 4, 0),
        ("loop0030", 470_528, 90, 4, 8, 4, 0),
        ("loop0031", 370_688, 200, 7, 8, 4, 0),
    ];

    const RATE: u32 = 44_100;

    fn meta_of(row: (&str, u32, u32, u32, u32, u32, u32)) -> LoopMeta {
        LoopMeta {
            version: 1,
            tempo_bpm: row.2,
            beats_per_bar: row.3,
            beat_unit: row.4,
            bars: row.5,
            partial: row.6,
        }
    }

    fn reference() -> LoopHeader {
        parse(&LOOP0031).expect("the reference header must parse")
    }

    #[test]
    fn the_reference_header_reads_as_the_file_it_came_from() {
        let h = reference();
        assert_eq!(h.format.format_tag, 1);
        assert_eq!(h.format.channels, 1);
        assert_eq!(h.format.sample_rate, RATE);
        assert_eq!(h.format.bits_per_sample, 16);
        assert_eq!(h.audio_bytes, 741_376);
        assert_eq!(h.audio_offset, HEADER_PREFIX);
        assert_eq!(h.samples(), 370_688);
    }

    /// `GetFileInfo` reported 741,468 bytes for this path. The RIFF header says
    /// the same thing by a completely different route, which is what made the
    /// file transfer trustworthy in the first place.
    #[test]
    fn the_riff_size_agrees_with_what_the_instrument_reported() {
        assert_eq!(reference().file_bytes, 741_468);
    }

    #[test]
    fn the_reference_metadata_reads_as_four_bars_of_seven() {
        let m = reference().meta.expect("the JUNK chunk is labelled");
        assert_eq!(m.version, 1);
        assert_eq!(m.tempo_bpm, 200);
        assert_eq!(m.beats_per_bar, 7);
        assert_eq!(m.beat_unit, 8);
        assert_eq!(m.bars, 4);
        assert!(!m.is_partial());
        assert_eq!(m.beats(), 28);
    }

    /// **The corpus test.** Every complete take lands exactly on its
    /// block-rounded grid length, and every partial one falls short of the
    /// grid. One counterexample refutes the whole reading, which is the point.
    #[test]
    fn the_model_predicts_all_thirty_one_loops() {
        let mut complete = 0;
        let mut partial = 0;
        for row in CORPUS {
            let (name, samples, ..) = row;
            let m = meta_of(row);
            let nominal = m.nominal_samples(RATE).unwrap();
            if m.is_partial() {
                partial += 1;
                assert!(
                    u64::from(samples) < nominal,
                    "{name} is marked partial but is not short of its grid"
                );
            } else {
                complete += 1;
                assert_eq!(
                    u64::from(samples),
                    m.expected_samples(RATE).unwrap(),
                    "{name} does not land on its grid"
                );
            }
        }
        assert_eq!(complete, 24);
        assert_eq!(partial, 7);
    }

    /// 256 is the block because it divides every loop and 512 does not. The
    /// earlier reading said 2048, which divides exactly one of them — pinning
    /// both halves keeps that from being "tidied" back to a rounder number.
    #[test]
    fn two_hundred_and_fifty_six_is_the_block_and_512_is_not() {
        assert_eq!(BLOCK_SAMPLES, 256);
        for (name, samples, ..) in CORPUS {
            assert_eq!(samples % BLOCK_SAMPLES, 0, "{name} is not whole blocks");
        }
        assert!(
            CORPUS.iter().any(|(_, s, ..)| s % (BLOCK_SAMPLES * 2) != 0),
            "512 must fail on something, or 256 is not the real granularity"
        );
    }

    /// Block rounding has to be doing real work. If every grid length were
    /// already aligned, the corpus test would pass whether or not this module
    /// understood why.
    #[test]
    fn block_rounding_is_load_bearing() {
        let rounded = CORPUS
            .iter()
            .filter(|row| row.6 == 0)
            .filter(|row| {
                let m = meta_of(**row);
                !m.nominal_samples(RATE)
                    .unwrap()
                    .is_multiple_of(u64::from(BLOCK_SAMPLES))
            })
            .count();
        assert_eq!(rounded, 24, "every complete take needed rounding up");
    }

    /// The separation of the two length fields, as the corpus shows it: the
    /// meter takes few values and the bar count many. Read the other way the
    /// player would be recording in 12- and 14-beat bars.
    #[test]
    fn the_bar_count_varies_and_the_meter_does_not() {
        let mut meters: alloc::vec::Vec<u32> = CORPUS.iter().map(|r| r.3).collect();
        meters.sort_unstable();
        meters.dedup();
        let mut bars: alloc::vec::Vec<u32> = CORPUS.iter().map(|r| r.5).collect();
        bars.sort_unstable();
        bars.dedup();

        assert_eq!(meters, [4, 7]);
        assert_eq!(bars, [4, 8, 12, 14]);
        assert!(
            bars.len() > meters.len(),
            "the bar count must be the field that moves"
        );
    }

    /// The partial flag is only ever 0 or 1, and it is not constant — a flag
    /// that never varied would explain nothing.
    #[test]
    fn the_partial_flag_is_a_flag_and_it_varies() {
        assert!(CORPUS.iter().all(|r| r.6 <= 1));
        assert!(CORPUS.iter().any(|r| r.6 == 0));
        assert!(CORPUS.iter().any(|r| r.6 == 1));
    }

    #[test]
    fn the_length_check_reports_which_case_it_found() {
        assert_eq!(reference().check_length(), Some(LengthCheck::OnGrid));
        assert_eq!(reference().length_agrees(), Some(true));

        // A partial take that is short of its grid, as one should be.
        let partial = LoopMeta {
            partial: 1,
            ..reference().meta.unwrap()
        };
        let mut header = reference();
        header.meta = Some(partial);
        header.audio_bytes = 200_000;
        assert!(matches!(
            header.check_length(),
            Some(LengthCheck::PartialAsMarked { .. })
        ));

        // A complete take that misses its grid is a disagreement, and must be
        // reported rather than smoothed over.
        let mut wrong = reference();
        wrong.audio_bytes = 12_345;
        assert!(matches!(
            wrong.check_length(),
            Some(LengthCheck::Disagrees { .. })
        ));
        assert_eq!(wrong.length_agrees(), Some(false));
    }

    /// A plain WAV written by anything else has no vendor metadata, and saying
    /// so is more useful than inventing a tempo for it.
    #[test]
    fn a_wav_without_the_label_parses_but_carries_no_metadata() {
        let mut bytes = LOOP0031;
        // Break the label, leaving a well-formed JUNK chunk of the right size.
        bytes[20] = b'X';
        let h = parse(&bytes).unwrap();
        assert!(h.meta.is_none());
        assert_eq!(h.check_length(), None);
        // The audio is still fully described, which is the point of the chunk
        // being JUNK.
        assert_eq!(h.samples(), 370_688);
    }

    #[test]
    fn a_short_read_is_reported_as_truncation_not_as_a_bad_file() {
        for len in [0usize, 4, 11, 12, 40, 60, 80, 91] {
            let err = parse(&LOOP0031[..len]).unwrap_err();
            assert!(
                matches!(err, LoopError::Truncated { .. }),
                "{len} bytes gave {err:?}"
            );
        }
        assert!(parse(&LOOP0031).is_ok(), "the full prefix must suffice");
    }

    #[test]
    fn something_that_is_not_a_wav_is_refused() {
        let mut bytes = LOOP0031;
        bytes[0] = b'X';
        assert_eq!(parse(&bytes), Err(LoopError::NotRiffWave));
    }

    /// Odd-sized chunks are padded to an even boundary. No vendor file has one,
    /// but a reader that ignores the rule silently misreads every chunk after
    /// the first odd one, which surfaces as "some other person's loops don't
    /// work".
    #[test]
    fn an_odd_length_chunk_is_padded_before_the_next_one() {
        let mut bytes = alloc::vec::Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&40u32.to_le_bytes());
        bytes.extend_from_slice(b"WAVE");
        // A three-byte chunk, padded to four.
        bytes.extend_from_slice(b"odd ");
        bytes.extend_from_slice(&3u32.to_le_bytes());
        bytes.extend_from_slice(&[1, 2, 3, 0]);
        bytes.extend_from_slice(b"fmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&48_000u32.to_le_bytes());
        bytes.extend_from_slice(&192_000u32.to_le_bytes());
        bytes.extend_from_slice(&4u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&8u32.to_le_bytes());

        let h = parse(&bytes).unwrap();
        assert_eq!(h.format.sample_rate, 48_000);
        assert_eq!(h.format.channels, 2);
        assert_eq!(h.audio_bytes, 8);
        // Two channels of 16 bits is four bytes per frame.
        assert_eq!(h.samples(), 2);
    }

    #[test]
    fn a_zero_tempo_does_not_divide_by_zero() {
        let m = LoopMeta {
            version: 1,
            tempo_bpm: 0,
            beats_per_bar: 4,
            beat_unit: 8,
            bars: 4,
            partial: 0,
        };
        assert_eq!(m.nominal_samples(RATE), None);
        assert_eq!(m.expected_samples(RATE), None);
    }
}
