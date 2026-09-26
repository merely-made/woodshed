#![forbid(unsafe_code)]
//! Transcript subset of WebVTT CRD 2026-05-20. Preserves cue payload/settings;
//! no CSS, regions, HTML track DOM, or video subtitle layout implementation.

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Cue {
    pub identifier: Option<String>,
    pub start_ms: u64,
    pub end_ms: u64,
    pub settings: String,
    pub payload: String,
    pub text: String,
}

impl Cue {
    pub fn active_at(&self, position_ms: u64) -> bool {
        self.start_ms <= position_ms && position_ms < self.end_ms
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Transcript {
    pub cues: Vec<Cue>,
    pub skipped_blocks: usize,
}

/// Parse finite WebVTT input. Callers own input-size limits and fetching.
/// Malformed cue blocks are skipped; invalid file signatures fail.
pub fn parse(source: &str) -> Result<Transcript, &'static str> {
    let normalized = source
        .trim_start_matches('\u{feff}')
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\0', "\u{fffd}");
    let mut lines = normalized.lines().peekable();
    let header = lines.next().ok_or("Missing WEBVTT signature")?;
    if !(header == "WEBVTT" || header.starts_with("WEBVTT ") || header.starts_with("WEBVTT\t"))
        || header.contains("-->")
    {
        return Err("Invalid WEBVTT signature");
    }
    // Header metadata terminates at the first empty line.
    while lines.peek().is_some_and(|line| !line.is_empty()) {
        lines.next();
    }
    let mut transcript = Transcript::default();
    while lines.peek().is_some() {
        while lines.peek().is_some_and(|line| line.is_empty()) {
            lines.next();
        }
        let mut block = Vec::new();
        while lines.peek().is_some_and(|line| !line.is_empty()) {
            block.push(lines.next().unwrap());
        }
        if block.is_empty() {
            continue;
        }
        if block[0] == "NOTE" || block[0].starts_with("NOTE ") || block[0].starts_with("NOTE\t") {
            continue;
        }
        let timing = usize::from(!block[0].contains("-->"));
        let cue = (|| {
            let timing_line = *block.get(timing)?;
            let (start, rest) = timing_line.split_once("-->")?;
            let mut right = rest.split_whitespace();
            let start_ms = timestamp(start.trim())?;
            let end_ms = timestamp(right.next()?)?;
            if end_ms <= start_ms {
                return None;
            }
            let payload = block[timing + 1..].join("\n");
            Some(Cue {
                identifier: (timing == 1).then(|| block[0].to_owned()),
                start_ms,
                end_ms,
                settings: right.collect::<Vec<_>>().join(" "),
                text: plain_text(&payload),
                payload,
            })
        })();
        match cue {
            Some(cue) => transcript.cues.push(cue),
            None => transcript.skipped_blocks += 1,
        }
    }
    Ok(transcript)
}

fn timestamp(value: &str) -> Option<u64> {
    let (clock, fraction) = value.split_once('.')?;
    if fraction.len() != 3 || !fraction.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let parts: Vec<_> = clock.split(':').collect();
    if !(2..=3).contains(&parts.len())
        || parts.iter().any(|p| !p.bytes().all(|b| b.is_ascii_digit()))
    {
        return None;
    }
    let (hours, minutes, seconds) = if parts.len() == 3 {
        if parts[0].len() < 2 || parts[1].len() != 2 || parts[2].len() != 2 {
            return None;
        }
        (parts[0].parse::<u64>().ok()?, parts[1], parts[2])
    } else {
        if parts[0].len() != 2 || parts[1].len() != 2 {
            return None;
        }
        (0, parts[0], parts[1])
    };
    let minutes = minutes.parse::<u64>().ok()?;
    let seconds = seconds.parse::<u64>().ok()?;
    if minutes > 59 || seconds > 59 {
        return None;
    }
    hours
        .checked_mul(3600)?
        .checked_add(minutes * 60 + seconds)?
        .checked_mul(1000)?
        .checked_add(fraction.parse().ok()?)
}

// Transcript text projection only. Payload remains available for a future cue
// tree implementation. Never insert source markup into a host document.
fn plain_text(payload: &str) -> String {
    let mut output = String::new();
    let mut tag = false;
    for ch in payload.chars() {
        match ch {
            '<' => tag = true,
            '>' if tag => tag = false,
            _ if !tag => output.push(ch),
            _ => {},
        }
    }
    // Decode amp last so an escaped entity is not decoded twice.
    output
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", "\u{a0}")
        .replace("&lrm;", "\u{200e}")
        .replace("&rlm;", "\u{200f}")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bom_crlf_identifiers_overlaps_and_text_projection() {
        let parsed = parse("\u{feff}WEBVTT transcript\r\n\r\nfirst\r\n00:01.000 --> 00:03.000 align:start\r\n<v Bird><b>A</b> &amp; B\r\nsecond line\r\n\r\n00:02.000 --> 00:04.000\r\nOverlap").unwrap();
        assert_eq!(parsed.cues.len(), 2);
        assert_eq!(parsed.cues[0].text, "A & B\nsecond line");
        assert_eq!(parsed.cues[0].identifier.as_deref(), Some("first"));
        assert_eq!(parsed.cues[0].settings, "align:start");
        assert_eq!(
            parsed.cues.iter().filter(|cue| cue.active_at(2500)).count(),
            2
        );
        assert!(!parsed.cues[0].active_at(3000));
        assert!(!parsed.cues[0].active_at(999));
    }
    #[test]
    fn rejects_signature_and_recovers_after_bad_blocks() {
        assert!(parse("WEBVTTbad\n").is_err());
        let parsed = parse("WEBVTT\n\nNOTE a comment\nignored\n\n00:61.000 --> 00:62.000\nbad\n\n00:01.000 --> 00:01.000\nempty\n\n01:02:03.004 --> 01:02:04.000\ngood").unwrap();
        assert_eq!(parsed.skipped_blocks, 2);
        assert_eq!(parsed.cues[0].start_ms, 3_723_004);
        assert_eq!(timestamp("99999999999999999999:00:00.000"), None);
        assert_eq!(timestamp("0:01.000"), None);
    }
}
