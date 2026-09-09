//! The version banner read at connection time.
//!
//! Before any JSON-RPC happens, the client *reads* the response characteristic
//! once and gets back a short plain-text banner naming the firmware versions of
//! both processors. This is separate from the `GetVersion` RPC method and comes
//! earlier: it arrives during connection setup, over a GATT read rather than a
//! notification.
//!
//! Two formats exist, because the older one could only report one processor:
//!
//! ```text
//! S1.2.3_E2.7.0\n     both versions: STM (audio DSP), then ESP (connectivity)
//! @version 2.7.0\n    older firmware: ESP only; STM is implicitly 1.2.2
//! ```
//!
//! # Provenance
//!
//! Recovered by static analysis; unconfirmed against hardware until the Phase 1
//! control run. See `design_docs/2026-08-27_ringdown_founding.md`, Findings F9
//! and F10.

use core::fmt;

/// The implied STM version when a device reports only its ESP version.
///
/// Firmware old enough to use the `@version` form predates the STM being
/// reported at all, and the vendor's client assumes this value. Adopted here so
/// both banner forms yield a complete pair, but treat it as an assumption
/// inherited from the vendor rather than something the device said.
pub const IMPLIED_STM_VERSION: Version = Version {
    major: 1,
    minor: 2,
    patch: 2,
};

/// A three-part firmware version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    /// Major version.
    pub major: u16,
    /// Minor version.
    pub minor: u16,
    /// Patch version; zero when the device reported only two components.
    pub patch: u16,
}

impl Version {
    /// Parse a dotted version the way the vendor's client does.
    ///
    /// Deliberately permissive, because the reference implementation is: it
    /// splits on `.`, then **discards every non-digit character** within each
    /// component before parsing it. So `"V1.2.3"` and `"1.2.3"` are the same
    /// version, which matters — the instrument's own System Menu displays these
    /// with a leading `V`, and a stricter parser would reject a banner that the
    /// vendor's client accepts happily.
    ///
    /// Components containing no digits are dropped rather than treated as zero,
    /// and any beyond the third are ignored. Both match the reference
    /// behaviour. Being liberal here is the right trade: this parses a banner
    /// whose exact spelling is not yet confirmed against hardware, and the cost
    /// of over-accepting is far lower than the cost of rejecting a real device.
    pub fn parse(text: &str) -> Option<Version> {
        let mut parts = text.split('.').filter_map(|part| {
            let digits: heapless_digits::Digits =
                part.chars().filter(char::is_ascii_digit).collect();
            digits.parse()
        });

        let major = parts.next()?;
        let minor = parts.next().unwrap_or(0);
        let patch = parts.next().unwrap_or(0);

        Some(Version {
            major,
            minor,
            patch,
        })
    }
}

/// Digit accumulation without an allocation per component.
mod heapless_digits {
    /// A component's digits, capped at a length beyond which a version number
    /// is not a version number.
    pub struct Digits {
        buf: [u8; 8],
        len: usize,
        overflowed: bool,
    }

    impl FromIterator<char> for Digits {
        fn from_iter<T: IntoIterator<Item = char>>(iter: T) -> Self {
            let mut out = Digits {
                buf: [0; 8],
                len: 0,
                overflowed: false,
            };
            for c in iter {
                if out.len == out.buf.len() {
                    out.overflowed = true;
                    break;
                }
                out.buf[out.len] = c as u8;
                out.len += 1;
            }
            out
        }
    }

    impl Digits {
        /// The accumulated digits as a number, or `None` if there were none (or
        /// too many to be meaningful).
        pub fn parse(&self) -> Option<u16> {
            if self.len == 0 || self.overflowed {
                return None;
            }
            let mut value: u32 = 0;
            for &b in &self.buf[..self.len] {
                value = value * 10 + u32::from(b - b'0');
                if value > u16::MAX as u32 {
                    return None;
                }
            }
            Some(value as u16)
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// The firmware versions of the guitar's two processors.
///
/// The instrument runs two: one handles connectivity and speaks this protocol,
/// the other runs the audio DSP. They are flashed independently and version
/// independently, which is why both are reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Banner {
    /// The audio DSP processor's firmware version.
    pub stm: Version,
    /// The connectivity processor's firmware version.
    pub esp: Version,
    /// Whether `stm` was actually reported, or assumed because the device used
    /// the older single-version banner.
    pub stm_was_implied: bool,
}

impl Banner {
    /// Parse a banner read from the response characteristic.
    pub fn parse(text: &str) -> Option<Banner> {
        let text = text.trim_end_matches(['\n', '\r']);

        if let Some(rest) = text.strip_prefix('S') {
            let (stm, esp) = rest.split_once("_E")?;
            return Some(Banner {
                stm: Version::parse(stm)?,
                esp: Version::parse(esp)?,
                stm_was_implied: false,
            });
        }

        if let Some(rest) = text.strip_prefix("@version ") {
            return Some(Banner {
                stm: IMPLIED_STM_VERSION,
                esp: Version::parse(rest)?,
                stm_was_implied: true,
            });
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;

    #[test]
    fn parses_the_two_processor_banner() {
        let b = Banner::parse("S1.2.3_E2.7.0\n").unwrap();
        assert_eq!(
            b.stm,
            Version {
                major: 1,
                minor: 2,
                patch: 3
            }
        );
        assert_eq!(
            b.esp,
            Version {
                major: 2,
                minor: 7,
                patch: 0
            }
        );
        assert!(!b.stm_was_implied);
    }

    #[test]
    fn parses_the_legacy_single_version_banner_and_flags_the_assumption() {
        let b = Banner::parse("@version 2.7.0\n").unwrap();
        assert_eq!(
            b.esp,
            Version {
                major: 2,
                minor: 7,
                patch: 0
            }
        );
        assert_eq!(b.stm, IMPLIED_STM_VERSION);
        assert!(
            b.stm_was_implied,
            "a caller must be able to tell an assumed version from a reported one"
        );
    }

    #[test]
    fn tolerates_a_missing_newline_and_crlf() {
        assert!(Banner::parse("S1.2.3_E2.7.0").is_some());
        assert!(Banner::parse("S1.2.3_E2.7.0\r\n").is_some());
    }

    #[test]
    fn short_versions_fill_with_zero() {
        assert_eq!(
            Version::parse("2.7"),
            Some(Version {
                major: 2,
                minor: 7,
                patch: 0
            })
        );
        assert_eq!(
            Version::parse("3"),
            Some(Version {
                major: 3,
                minor: 0,
                patch: 0
            })
        );
    }

    #[test]
    fn rejects_things_that_are_not_banners() {
        // A JSON-RPC reply must not be mistaken for a banner.
        assert!(Banner::parse(r#"{"jsonrpc":"2.0","id":1}"#).is_none());
        assert!(Banner::parse("").is_none());
        assert!(
            Banner::parse("S1.2.3").is_none(),
            "missing the _E separator"
        );
        assert!(Banner::parse("Snonsense_Emore").is_none());
        assert!(Banner::parse("@version ").is_none());
    }

    /// The instrument's System Menu shows versions with a leading `V`, and the
    /// vendor's parser strips non-digits rather than rejecting them. A stricter
    /// parser would fail on a banner the vendor's client reads fine.
    #[test]
    fn a_leading_v_is_ignored_the_way_the_vendor_ignores_it() {
        let expected = Version {
            major: 1,
            minor: 2,
            patch: 3,
        };
        assert_eq!(Version::parse("V1.2.3"), Some(expected));
        assert_eq!(Version::parse("v1.2.3"), Some(expected));
        assert_eq!(Version::parse("1.2.3"), Some(expected));
        assert_eq!(Banner::parse("SV1.2.3_EV1.3.0").unwrap().stm, expected);
    }

    /// The exact readout from the System Menu of the instrument this was first
    /// built against, in both spellings the banner might use.
    #[test]
    fn the_reference_instruments_versions_parse() {
        let stm = Version {
            major: 1,
            minor: 2,
            patch: 3,
        };
        let esp = Version {
            major: 1,
            minor: 3,
            patch: 0,
        };
        for text in ["S1.2.3_E1.3.0", "SV1.2.3_EV1.3.0"] {
            let banner = Banner::parse(text).unwrap_or_else(|| panic!("failed to parse {text}"));
            assert_eq!(banner.stm, stm);
            assert_eq!(banner.esp, esp);
            assert!(!banner.stm_was_implied);
        }
    }

    /// Matching the reference implementation's quirks, not just its happy path.
    #[test]
    fn extra_components_are_ignored_and_digitless_ones_dropped() {
        assert_eq!(
            Version::parse("1.2.3.4"),
            Some(Version {
                major: 1,
                minor: 2,
                patch: 3
            }),
            "the vendor takes the first three and ignores the rest"
        );
        assert!(
            Version::parse("nonsense").is_none(),
            "a component with no digits yields nothing to parse"
        );
        assert!(Version::parse("").is_none());
        // A component too large to be one is dropped, exactly as a digitless
        // component is — so the remaining components shift left rather than the
        // whole parse failing. Asserted because it is surprising, not because it
        // is desirable: it only arises on input no real device sends.
        assert_eq!(
            Version::parse("999999.1.2"),
            Some(Version {
                major: 1,
                minor: 2,
                patch: 0
            })
        );
    }

    #[test]
    fn versions_display_and_order() {
        assert_eq!(format!("{}", Version::parse("2.7.1").unwrap()), "2.7.1");
        assert!(Version::parse("2.7.0") < Version::parse("2.7.1"));
        assert!(Version::parse("1.9.9") < Version::parse("2.0.0"));
    }
}
