//! Editable fretted score profile. MusicXML remains the saved document authority.
use serde::{Deserialize, Serialize};
use std::fmt::Write;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MusicDocument {
    pub musicxml: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Score {
    pub title: String,
    pub instrument: String,
    pub tuning: Vec<u8>,
    pub tempo: u16,
    pub measures: Vec<Measure>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Measure {
    pub meter: TimeSignature,
    pub harmony: Option<ChordSymbol>,
    pub events: Vec<NoteEvent>,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimeSignature {
    pub beats: u8,
    pub beat_type: u8,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NoteEvent {
    pub notes: Vec<FrettedNote>,
    pub value: NoteValue,
    pub rest: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrettedNote {
    pub string: u8,
    pub fret: u8,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum NoteValue {
    Whole,
    Half,
    Quarter,
    Eighth,
    TripletEighth,
}
impl NoteValue {
    pub fn units(self) -> u32 {
        match self {
            Self::Whole => 24,
            Self::Half => 12,
            Self::Quarter => 6,
            Self::Eighth => 3,
            Self::TripletEighth => 2,
        }
    }
    pub fn quarter_beats(self) -> f64 {
        self.units() as f64 / 6.0
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Whole => "Whole",
            Self::Half => "Half",
            Self::Quarter => "Quarter",
            Self::Eighth => "Eighth",
            Self::TripletEighth => "Triplet eighth",
        }
    }
    fn xml_type(self) -> &'static str {
        match self {
            Self::Whole => "whole",
            Self::Half => "half",
            Self::Quarter => "quarter",
            _ => "eighth",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChordSymbol {
    pub root: u8,
    pub kind: ChordKind,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ChordKind {
    Major,
    Diminished,
    Augmented,
    Sus2,
    Sus4,
    Minor7,
    Dominant7,
    Major7,
    Minor,
}
impl ChordKind {
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Minor7 => "m7",
            Self::Dominant7 => "7",
            Self::Major7 => "maj7",
            Self::Minor => "m",
            Self::Major => "",
            Self::Diminished => "dim",
            Self::Augmented => "+",
            Self::Sus2 => "sus2",
            Self::Sus4 => "sus4",
        }
    }
    fn xml(self) -> &'static str {
        match self {
            Self::Minor7 => "minor-seventh",
            Self::Dominant7 => "dominant",
            Self::Major7 => "major-seventh",
            Self::Minor => "minor",
            Self::Major => "major",
            Self::Diminished => "diminished",
            Self::Augmented => "augmented",
            Self::Sus2 => "suspended-second",
            Self::Sus4 => "suspended-fourth",
        }
    }
    fn intervals(self) -> &'static [u8] {
        match self {
            Self::Minor7 => &[0, 3, 7, 10],
            Self::Dominant7 => &[0, 4, 7, 10],
            Self::Major7 => &[0, 4, 7, 11],
            Self::Minor => &[0, 3, 7],
            Self::Major => &[0, 4, 7],
            Self::Diminished => &[0, 3, 6],
            Self::Augmented => &[0, 4, 8],
            Self::Sus2 => &[0, 2, 7],
            Self::Sus4 => &[0, 5, 7],
        }
    }
}
impl ChordSymbol {
    pub fn label(self) -> String {
        format!(
            "{}{}",
            [
                "C", "C♯", "D", "E♭", "E", "F", "F♯", "G", "A♭", "A", "B♭", "B"
            ][usize::from(self.root % 12)],
            self.kind.symbol()
        )
    }
}
impl Score {
    pub fn pitch(&self, n: FrettedNote) -> Result<u8, String> {
        let base = *self
            .tuning
            .get(usize::from(n.string).wrapping_sub(1))
            .ok_or("Invalid string")?;
        base.checked_add(n.fret)
            .filter(|p| *p <= 127)
            .ok_or("Pitch exceeds MIDI range".into())
    }
    pub fn validate(&self) -> Result<(), String> {
        for text in [&self.title, &self.instrument] {
            if text.len() > 4096
                || text.chars().any(|c| {
                    !(c == '\t'
                        || c == '\n'
                        || c == '\r'
                        || ('\u{20}'..='\u{d7ff}').contains(&c)
                        || ('\u{e000}'..='\u{fffd}').contains(&c)
                        || c >= '\u{10000}')
                })
            {
                return Err("Title and instrument require bounded XML text".into());
            }
        }
        if self.tuning.is_empty() || self.tuning.len() > 16 || self.tuning.iter().any(|p| *p > 127)
        {
            return Err("Tuning requires 1–16 MIDI pitches".into());
        }
        if self.tempo == 0
            || self.tempo > 400
            || self.measures.is_empty()
            || self.measures.len() > 1024
        {
            return Err("Invalid tempo or measure count".into());
        }
        for m in &self.measures {
            if m.meter.beats == 0
                || m.meter.beats > 32
                || ![1, 2, 4, 8].contains(&m.meter.beat_type)
            {
                return Err("Unsupported meter".into());
            }
            if m.harmony.is_some_and(|h| h.root > 11) {
                return Err("Invalid harmony root".into());
            }
            let mut triplets = 0;
            for e in &m.events {
                if e.value == NoteValue::TripletEighth {
                    triplets += 1;
                } else {
                    if triplets % 3 != 0 {
                        return Err(
                            "Triplet eighths require complete adjacent groups of three".into()
                        );
                    }
                    triplets = 0;
                }
            }
            if triplets % 3 != 0 {
                return Err("Triplet eighths require complete adjacent groups of three".into());
            }
            let mut total = 0u32;
            for e in &m.events {
                if e.rest != e.notes.is_empty() {
                    return Err("Rest must have no notes; pitched event requires notes".into());
                }
                let mut strings = std::collections::BTreeSet::new();
                for n in &e.notes {
                    self.pitch(*n)?;
                    if n.fret > 36 || !strings.insert(n.string) {
                        return Err("Unsupported fret or repeated string".into());
                    }
                }
                total = total
                    .checked_add(e.value.units())
                    .ok_or("Duration overflow")?;
            }
            if total != u32::from(m.meter.beats) * 24 / u32::from(m.meter.beat_type) {
                return Err("Events must fill each measure exactly".into());
            }
        }
        Ok(())
    }
    fn fits(&self, m: usize, h: ChordSymbol) -> bool {
        if h.root > 11 {
            return false;
        }
        self.measures.get(m).is_some_and(|m| {
            m.events.iter().flat_map(|e| &e.notes).all(|n| {
                self.pitch(*n)
                    .is_ok_and(|p| h.kind.intervals().contains(&((p % 12 + 12 - h.root) % 12)))
            })
        })
    }
    pub fn chord_mismatch(&self, m: usize) -> bool {
        self.measures
            .get(m)
            .and_then(|m| m.harmony)
            .is_some_and(|h| !self.fits(m, h))
    }
    pub fn chord_suggestions(&self, m: usize) -> Vec<ChordSymbol> {
        if !self
            .measures
            .get(m)
            .is_some_and(|m| m.events.iter().any(|e| !e.notes.is_empty()))
        {
            return vec![];
        }
        (0..12)
            .flat_map(|root| {
                [
                    ChordKind::Minor7,
                    ChordKind::Dominant7,
                    ChordKind::Major7,
                    ChordKind::Minor,
                    ChordKind::Major,
                    ChordKind::Diminished,
                    ChordKind::Augmented,
                    ChordKind::Sus2,
                    ChordKind::Sus4,
                ]
                .map(|kind| ChordSymbol { root, kind })
            })
            .filter(|h| self.fits(m, *h))
            .collect()
    }
    pub fn progression_example() -> Self {
        let mut s = Self {
            title: "ii–V–I study".into(),
            instrument: "Guitar".into(),
            tuning: vec![64, 59, 55, 50, 45, 40],
            tempo: 96,
            measures: vec![],
        };
        for (root, kind, frets) in [
            (2, ChordKind::Minor7, [1, 1, 2, 0]),
            (7, ChordKind::Dominant7, [1, 0, 0, 0]),
            (0, ChordKind::Major7, [0, 0, 0, 2]),
            (0, ChordKind::Major7, [0, 0, 0, 2]),
        ] {
            s.measures.push(Measure {
                meter: TimeSignature {
                    beats: 4,
                    beat_type: 4,
                },
                harmony: Some(ChordSymbol { root, kind }),
                events: vec![NoteEvent {
                    notes: frets
                        .into_iter()
                        .enumerate()
                        .map(|(i, fret)| FrettedNote {
                            string: i as u8 + 1,
                            fret,
                        })
                        .collect(),
                    value: NoteValue::Whole,
                    rest: false,
                }],
            })
        }
        s
    }
    pub fn rhythm_example() -> Self {
        let mut s = Self::progression_example();
        s.title = "Rhythm study".into();
        let event = |string, fret, value| NoteEvent {
            notes: vec![FrettedNote { string, fret }],
            value,
            rest: false,
        };
        s.measures = vec![
            Measure {
                meter: TimeSignature {
                    beats: 4,
                    beat_type: 4,
                },
                harmony: None,
                events: vec![
                    event(2, 1, NoteValue::Quarter),
                    event(2, 3, NoteValue::Eighth),
                    event(1, 0, NoteValue::Eighth),
                    event(1, 1, NoteValue::Quarter),
                    NoteEvent {
                        notes: vec![],
                        value: NoteValue::Quarter,
                        rest: true,
                    },
                ],
            },
            Measure {
                meter: TimeSignature {
                    beats: 4,
                    beat_type: 4,
                },
                harmony: None,
                events: vec![
                    event(1, 3, NoteValue::TripletEighth),
                    event(1, 5, NoteValue::TripletEighth),
                    event(1, 7, NoteValue::TripletEighth),
                    event(1, 8, NoteValue::Quarter),
                    event(1, 3, NoteValue::Quarter),
                    event(1, 0, NoteValue::Quarter),
                ],
            },
        ];
        s
    }
}
fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
fn spelling(p: u8) -> (&'static str, u8) {
    [
        ("C", 0),
        ("C", 1),
        ("D", 0),
        ("E", 1),
        ("E", 0),
        ("F", 0),
        ("F", 1),
        ("G", 0),
        ("A", 1),
        ("A", 0),
        ("B", 1),
        ("B", 0),
    ][usize::from(p % 12)]
}
fn pitch_xml(p: u8) -> String {
    let (step, alter) = spelling(p);
    let alter = if alter == 0 {
        0
    } else if [3, 8, 10].contains(&(p % 12)) {
        -1
    } else {
        1
    };
    // MusicXML octaves span 0..=9. Extreme MIDI pitches retain their exact
    // sound through an enharmonic alteration at that notation boundary.
    let octave = i16::from(p) / 12 - 1;
    let bounded_octave = octave.clamp(0, 9);
    let alter = alter + (octave - bounded_octave) * 12;
    format!("<step>{step}</step><alter>{alter}</alter><octave>{bounded_octave}</octave>")
}
#[cfg(test)]
mod tests;

/// Explicit authored-notation conversion; never called by a read-only view.
pub mod draft;

mod musicxml;
