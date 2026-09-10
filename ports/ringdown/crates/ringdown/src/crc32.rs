//! The checksum the instrument uses for file transfers.
//!
//! Not the CRC-32 most libraries mean. `GetFileInfo` reports a checksum
//! computed MSB-first over the unreflected polynomial `0x04C11DB7`, starting
//! from all-ones and applying no final inversion — the variant catalogued as
//! **CRC-32/MPEG-2**. Feeding a file to a stock `crc32` crate produces a
//! different number and a false mismatch, which is a confusing way to fail a
//! download that actually arrived intact.
//!
//! # Provenance
//!
//! Read from the vendor's `CRC32` class: a 256-entry table beginning
//! `0, 79764919, …` (`79764919` is `0x04C11DB7`), and an update step of
//! `crc = table[((crc >> 24) ^ byte) & 0xff] ^ (crc << 8)` over an initial
//! `0xFFFFFFFF`, returned without inversion. The table is generated here from
//! the polynomial rather than copied, so the value is derived from the
//! algorithm rather than transcribed from someone else's source.

/// The polynomial, in its unreflected (MSB-first) form.
const POLYNOMIAL: u32 = 0x04C1_1DB7;

/// The register's starting value.
const INIT: u32 = 0xFFFF_FFFF;

/// The lookup table, built from [`POLYNOMIAL`] at compile time.
const TABLE: [u32; 256] = build_table();

const fn build_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0usize;
    while i < 256 {
        let mut acc = (i as u32) << 24;
        let mut bit = 0;
        while bit < 8 {
            acc = if acc & 0x8000_0000 != 0 {
                (acc << 1) ^ POLYNOMIAL
            } else {
                acc << 1
            };
            bit += 1;
        }
        table[i] = acc;
        i += 1;
    }
    table
}

/// Checksum a complete buffer.
pub fn compute(data: &[u8]) -> u32 {
    let mut crc = Crc32::new();
    crc.update(data);
    crc.finish()
}

/// Checksum data arriving in pieces.
///
/// A file read over Bluetooth arrives a couple of hundred bytes at a time, so
/// the checksum has to accumulate rather than wait for the whole thing.
#[derive(Debug, Clone)]
pub struct Crc32 {
    state: u32,
}

impl Default for Crc32 {
    fn default() -> Self {
        Crc32::new()
    }
}

impl Crc32 {
    /// A fresh checksum.
    pub fn new() -> Crc32 {
        Crc32 { state: INIT }
    }

    /// Fold in more bytes.
    pub fn update(&mut self, data: &[u8]) {
        for byte in data {
            let index = ((self.state >> 24) ^ u32::from(*byte)) & 0xff;
            self.state = TABLE[index as usize] ^ (self.state << 8);
        }
    }

    /// The checksum so far.
    ///
    /// No final inversion, which is the detail that separates this from the
    /// commoner variants.
    pub fn finish(&self) -> u32 {
        self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The catalogue's check value for CRC-32/MPEG-2: the checksum of the
    /// nine ASCII digits. Getting this right means the polynomial, the
    /// direction, the initial value and the absence of a final xor are all
    /// correct together — which is worth far more than four separate
    /// assertions about constants.
    #[test]
    fn the_published_check_value_matches() {
        assert_eq!(compute(b"123456789"), 0x0376_E6E7);
    }

    /// The same input under the *common* CRC-32 gives 0xCBF43926. If this ever
    /// starts matching that, someone has "fixed" this into the wrong algorithm.
    #[test]
    fn this_is_not_the_common_crc32() {
        assert_ne!(compute(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn an_empty_input_is_the_initial_value() {
        assert_eq!(compute(b""), INIT);
    }

    #[test]
    fn incremental_and_whole_buffer_agree() {
        let data: alloc::vec::Vec<u8> = (0u8..=255).cycle().take(1000).collect();
        let whole = compute(&data);

        // Split at several awkward points, including ones that do not align
        // with anything, since a real transfer chunks wherever the MTU lands.
        for chunk in [1usize, 7, 200, 333, 999] {
            let mut crc = Crc32::new();
            for piece in data.chunks(chunk) {
                crc.update(piece);
            }
            assert_eq!(crc.finish(), whole, "chunked by {chunk} disagreed");
        }
    }

    #[test]
    fn the_table_is_generated_not_transcribed() {
        // The first entries the vendor's table shows, as a check that the
        // generated table is the same one.
        assert_eq!(TABLE[0], 0);
        assert_eq!(TABLE[1], 79_764_919);
        assert_eq!(TABLE[2], 159_529_838);
        assert_eq!(TABLE[3], 222_504_665);
        assert_eq!(TABLE[255], 2_985_771_188);
    }

    #[test]
    fn a_single_flipped_bit_changes_the_checksum() {
        let mut data = [0u8; 64];
        let clean = compute(&data);
        data[31] ^= 0x01;
        assert_ne!(compute(&data), clean);
    }
}
