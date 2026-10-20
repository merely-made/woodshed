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
impl MusicDocument {
    pub fn from_score(s: &Score) -> Result<Self, String> {
        s.validate()?;
        let instrument = s.instrument.to_ascii_lowercase();
        let transposing = instrument.contains("guitar") || instrument.contains("bass");
        let (clef, clef_line) = if instrument.contains("bass") || instrument.contains("cello") {
            ("F", 4)
        } else if instrument.contains("viola") {
            ("C", 3)
        } else {
            ("G", 2)
        };
        let mut x = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><score-partwise version=\"4.0\"><work><work-title>{}</work-title></work><part-list><score-part id=\"P1\"><part-name>{}</part-name></score-part></part-list><part id=\"P1\">",
            esc(&s.title),
            esc(&s.instrument)
        );
        for (i, m) in s.measures.iter().enumerate() {
            write!(x,"<measure number=\"{}\"><attributes><divisions>6</divisions><time><beats>{}</beats><beat-type>{}</beat-type></time><staves>2</staves><clef number=\"1\"><sign>{clef}</sign><line>{clef_line}</line></clef><clef number=\"2\"><sign>TAB</sign><line>5</line></clef><staff-details number=\"2\"><staff-lines>{}</staff-lines>",i+1,m.meter.beats,m.meter.beat_type,s.tuning.len()).unwrap();
            for (n, p) in s.tuning.iter().rev().enumerate() {
                write!(
                    x,
                    "<staff-tuning line=\"{}\">{}</staff-tuning>",
                    n + 1,
                    pitch_xml(*p)
                        .replace("<step>", "<tuning-step>")
                        .replace("</step>", "</tuning-step>")
                        .replace("<alter>", "<tuning-alter>")
                        .replace("</alter>", "</tuning-alter>")
                        .replace("<octave>", "<tuning-octave>")
                        .replace("</octave>", "</tuning-octave>")
                )
                .unwrap();
            }
            x.push_str("</staff-details>");
            if transposing {
                x.push_str("<transpose number=\"1\"><diatonic>0</diatonic><chromatic>0</chromatic><octave-change>-1</octave-change></transpose>");
            }
            x.push_str("</attributes>");
            if i == 0 {
                write!(x,"<direction><direction-type><metronome><beat-unit>quarter</beat-unit><per-minute>{}</per-minute></metronome></direction-type><sound tempo=\"{}\"/></direction>",s.tempo,s.tempo).unwrap();
            }
            if let Some(h) = m.harmony {
                let (step, _) = spelling(h.root);
                let a = match h.root {
                    1 | 6 => 1,
                    3 | 8 | 10 => -1,
                    _ => 0,
                };
                write!(x,"<harmony><root><root-step>{step}</root-step><root-alter>{a}</root-alter></root><kind>{}</kind></harmony>",h.kind.xml()).unwrap();
            }
            for staff in 1..=2 {
                if staff == 2 {
                    write!(
                        x,
                        "<backup><duration>{}</duration></backup>",
                        u32::from(m.meter.beats) * 24 / u32::from(m.meter.beat_type)
                    )
                    .unwrap();
                }
                let mut triplet_index = 0;
                for e in &m.events {
                    if e.value != NoteValue::TripletEighth {
                        triplet_index = 0;
                    }
                    for n in 0..e.notes.len().max(1) {
                        x.push_str("<note>");
                        if n > 0 {
                            x.push_str("<chord/>")
                        };
                        if e.rest {
                            x.push_str("<rest/>")
                        } else {
                            let p = s.pitch(e.notes[n])?;
                            write!(
                                x,
                                "<pitch>{}</pitch>",
                                pitch_xml(if staff == 1 && transposing {
                                    p.checked_add(12).ok_or("Written octave exceeds range")?
                                } else {
                                    p
                                })
                            )
                            .unwrap();
                        }
                        write!(
                            x,
                            "<duration>{}</duration><voice>{staff}</voice><type>{}</type>",
                            e.value.units(),
                            e.value.xml_type()
                        )
                        .unwrap();
                        if e.value == NoteValue::TripletEighth {
                            x.push_str("<time-modification><actual-notes>3</actual-notes><normal-notes>2</normal-notes><normal-type>eighth</normal-type></time-modification>")
                        };
                        write!(x, "<staff>{staff}</staff>").unwrap();
                        let tuplet = e.value == NoteValue::TripletEighth && n == 0;
                        if (!e.rest && staff == 2) || tuplet {
                            x.push_str("<notations>");
                            if tuplet && triplet_index % 3 == 0 {
                                x.push_str("<tuplet type=\"start\" bracket=\"yes\" number=\"1\"/>");
                            }
                            if tuplet && triplet_index % 3 == 2 {
                                x.push_str("<tuplet type=\"stop\" number=\"1\"/>");
                            }
                            if !e.rest && staff == 2 {
                                let f = e.notes[n];
                                write!(
                                    x,
                                    "<technical><string>{}</string><fret>{}</fret></technical>",
                                    f.string, f.fret
                                )
                                .unwrap();
                            }
                            x.push_str("</notations>");
                        }
                        x.push_str("</note>");
                    }
                    if e.value == NoteValue::TripletEighth {
                        triplet_index += 1;
                    }
                }
            }
            x.push_str("</measure>");
        }
        x.push_str("</part></score-partwise>");
        if x.len() > 2_000_000 {
            return Err("MusicXML exceeds editable profile size".into());
        }
        Ok(Self { musicxml: x })
    }
    /// Parse only the editable profile emitted by this module. Unknown markings,
    /// extra parts, voices, ties, lyrics and other source features are refused.
    /// The original bytes remain available regardless of projection success.
    pub fn score(&self) -> Result<Score, String> {
        if self.musicxml.len() > 2_000_000 {
            return Err("MusicXML exceeds editable profile size".into());
        }
        let doc = roxmltree::Document::parse(&self.musicxml).map_err(|e| e.to_string())?;
        let root = doc.root_element();
        fn child<'a, 'input>(
            n: roxmltree::Node<'a, 'input>,
            tag: &str,
        ) -> Result<roxmltree::Node<'a, 'input>, String> {
            n.children()
                .find(|n| n.has_tag_name(tag))
                .ok_or_else(|| format!("Missing {tag}"))
        }
        let text = |n: roxmltree::Node<'_, '_>| n.text().unwrap_or("").to_owned();
        let number = |n: roxmltree::Node<'_, '_>, tag: &str| -> Result<i16, String> {
            text(child(n, tag)?)
                .parse()
                .map_err(|_| format!("Invalid {tag}"))
        };
        let pitch = |n: roxmltree::Node<'_, '_>, prefix: &str| -> Result<u8, String> {
            let step = text(child(n, &format!("{prefix}step"))?);
            let base = match step.as_str() {
                "C" => 0,
                "D" => 2,
                "E" => 4,
                "F" => 5,
                "G" => 7,
                "A" => 9,
                "B" => 11,
                _ => return Err("Invalid pitch step".into()),
            };
            let a = number(n, &format!("{prefix}alter"))?;
            let o = number(n, &format!("{prefix}octave"))?;
            u8::try_from((o + 1) * 12 + base + a).map_err(|_| "Invalid pitch".into())
        };
        if !root.has_tag_name("score-partwise") {
            return Err("Expected MusicXML score-partwise".into());
        }
        let title = text(child(child(root, "work")?, "work-title")?);
        let instrument = text(child(
            child(child(root, "part-list")?, "score-part")?,
            "part-name",
        )?);
        let part = child(root, "part")?;
        let first = child(part, "measure")?;
        let attrs = child(first, "attributes")?;
        let details = child(attrs, "staff-details")?;
        let mut tuning = vec![];
        for t in details
            .children()
            .filter(|n| n.has_tag_name("staff-tuning"))
        {
            tuning.push(pitch(t, "tuning-")?)
        }
        tuning.reverse();
        let sound = first
            .descendants()
            .find(|n| n.has_tag_name("sound"))
            .ok_or("Missing tempo")?;
        let tempo = sound
            .attribute("tempo")
            .ok_or("Missing tempo")?
            .parse()
            .map_err(|_| "Invalid tempo")?;
        let mut score = Score {
            title,
            instrument,
            tuning,
            tempo,
            measures: vec![],
        };
        for m in part.children().filter(|n| n.has_tag_name("measure")) {
            let time = child(child(m, "attributes")?, "time")?;
            let meter = TimeSignature {
                beats: u8::try_from(number(time, "beats")?).map_err(|_| "Invalid beats")?,
                beat_type: u8::try_from(number(time, "beat-type")?)
                    .map_err(|_| "Invalid beat type")?,
            };
            let harmony = if let Some(h) = m.children().find(|n| n.has_tag_name("harmony")) {
                let r = child(h, "root")?;
                let step = text(child(r, "root-step")?);
                let base = match step.as_str() {
                    "C" => 0,
                    "D" => 2,
                    "E" => 4,
                    "F" => 5,
                    "G" => 7,
                    "A" => 9,
                    "B" => 11,
                    _ => return Err("Invalid root".into()),
                };
                let root = (base + number(r, "root-alter")?).rem_euclid(12) as u8;
                let kind = match text(child(h, "kind")?).as_str() {
                    "minor-seventh" => ChordKind::Minor7,
                    "dominant" => ChordKind::Dominant7,
                    "major-seventh" => ChordKind::Major7,
                    "minor" => ChordKind::Minor,
                    "major" => ChordKind::Major,
                    "diminished" => ChordKind::Diminished,
                    "augmented" => ChordKind::Augmented,
                    "suspended-second" => ChordKind::Sus2,
                    "suspended-fourth" => ChordKind::Sus4,
                    _ => return Err("Unsupported harmony".into()),
                };
                Some(ChordSymbol { root, kind })
            } else {
                None
            };
            let mut events: Vec<NoteEvent> = vec![];
            for n in m.children().filter(|n| n.has_tag_name("note")) {
                if number(n, "staff")? != 2 {
                    continue;
                }
                let duration = number(n, "duration")?;
                let value = match duration {
                    24 => NoteValue::Whole,
                    12 => NoteValue::Half,
                    6 => NoteValue::Quarter,
                    3 => NoteValue::Eighth,
                    2 => NoteValue::TripletEighth,
                    _ => return Err("Unsupported duration".into()),
                };
                let rest = n.children().any(|n| n.has_tag_name("rest"));
                let notes = if rest {
                    vec![]
                } else {
                    let t = child(child(n, "notations")?, "technical")?;
                    vec![FrettedNote {
                        string: u8::try_from(number(t, "string")?).map_err(|_| "Invalid string")?,
                        fret: u8::try_from(number(t, "fret")?).map_err(|_| "Invalid fret")?,
                    }]
                };
                if n.children().any(|n| n.has_tag_name("chord")) {
                    let e = events.last_mut().ok_or("Chord without event")?;
                    if e.value != value || e.rest || rest {
                        return Err("Invalid chord duration".into());
                    }
                    e.notes.extend(notes)
                } else {
                    events.push(NoteEvent { notes, value, rest })
                }
            }
            score.measures.push(Measure {
                meter,
                harmony,
                events,
            });
        }
        score.validate()?;
        // Comparing complete XML trees makes rejection exhaustive, including
        // attributes and the duplicate TAB staff, without dropping source data.
        let canonical = Self::from_score(&score)?;
        let expected =
            roxmltree::Document::parse(&canonical.musicxml).map_err(|e| e.to_string())?;
        fn same(a: roxmltree::Node<'_, '_>, b: roxmltree::Node<'_, '_>) -> bool {
            if a.tag_name() != b.tag_name() {
                return false;
            }
            let attrs = |n: roxmltree::Node<'_, '_>| {
                let mut a = n
                    .attributes()
                    .map(|a| {
                        (
                            a.namespace().unwrap_or("").to_owned(),
                            a.name().to_owned(),
                            a.value().to_owned(),
                        )
                    })
                    .collect::<Vec<_>>();
                a.sort();
                a
            };
            if attrs(a) != attrs(b) {
                return false;
            }
            let ac = a
                .children()
                .filter(|n| {
                    n.is_element() || n.is_text() && !n.text().unwrap_or("").trim().is_empty()
                })
                .collect::<Vec<_>>();
            let bc = b
                .children()
                .filter(|n| {
                    n.is_element() || n.is_text() && !n.text().unwrap_or("").trim().is_empty()
                })
                .collect::<Vec<_>>();
            ac.len() == bc.len()
                && ac.into_iter().zip(bc).all(|(a, b)| {
                    if a.is_element() && b.is_element() {
                        same(a, b)
                    } else {
                        a.is_text() && b.is_text() && a.text() == b.text()
                    }
                })
        }
        if !same(root, expected.root_element()) {
            return Err("MusicXML contains notation outside the editable Woodshed profile; original XML retained".into());
        }
        Ok(score)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn examples_round_trip() {
        for s in [Score::progression_example(), Score::rhythm_example()] {
            assert_eq!(MusicDocument::from_score(&s).unwrap().score().unwrap(), s)
        }
    }
    #[test]
    fn alternate_tuning_and_rests() {
        let mut s = Score::rhythm_example();
        s.tuning = vec![64, 60, 55, 48];
        assert_eq!(MusicDocument::from_score(&s).unwrap().score().unwrap(), s);
        assert_eq!(
            s.measures[1]
                .events
                .iter()
                .map(|e| e.value.units())
                .sum::<u32>(),
            24
        )
    }
    #[test]
    fn extreme_midi_tunings_round_trip() {
        let mut s = Score::progression_example();
        s.tuning = vec![127, 0];
        for m in &mut s.measures {
            m.events[0].notes = vec![
                FrettedNote { string: 1, fret: 0 },
                FrettedNote { string: 2, fret: 0 },
            ];
        }
        assert_eq!(MusicDocument::from_score(&s).unwrap().score().unwrap(), s);
        s.measures[0].events[0].notes[0].fret = 1;
        assert!(s.validate().is_err());
    }
    #[test]
    fn instrument_clefs_and_written_pitch() {
        for (name, tuning, sign, line, written) in [
            ("Guitar", vec![64, 59, 55, 50, 45, 40], "G", "2", "5"),
            ("Bass", vec![43, 38, 33, 28], "F", "4", "3"),
            ("Ukulele", vec![69, 64, 60, 67], "G", "2", "4"),
            ("Viola", vec![69, 62, 55, 48], "C", "3", "4"),
            ("Cello", vec![57, 50, 43, 36], "F", "4", "3"),
            ("DoubleBass", vec![43, 38, 33, 28], "F", "4", "3"),
        ] {
            let mut s = Score::progression_example();
            s.instrument = name.into();
            s.tuning = tuning;
            for m in &mut s.measures {
                m.events[0].notes = vec![FrettedNote { string: 1, fret: 0 }];
            }
            let d = MusicDocument::from_score(&s).unwrap();
            assert_eq!(d.score().unwrap(), s);
            let xml = roxmltree::Document::parse(&d.musicxml).unwrap();
            let clef = xml.descendants().find(|n| n.has_tag_name("clef")).unwrap();
            assert_eq!(
                clef.children()
                    .find(|n| n.has_tag_name("sign"))
                    .unwrap()
                    .text(),
                Some(sign)
            );
            assert_eq!(
                clef.children()
                    .find(|n| n.has_tag_name("line"))
                    .unwrap()
                    .text(),
                Some(line)
            );
            assert!(!d.musicxml.contains("clef-octave-change"));
            assert_eq!(
                d.musicxml.contains("<transpose"),
                ["Guitar", "Bass", "DoubleBass"].contains(&name)
            );
            assert_eq!(
                xml.descendants()
                    .find(|n| n.has_tag_name("octave"))
                    .unwrap()
                    .text(),
                Some(written)
            );
        }
    }
    #[test]
    fn triplet_brackets_and_incomplete_groups() {
        let mut s = Score::rhythm_example();
        let d = MusicDocument::from_score(&s).unwrap();
        assert_eq!(d.musicxml.matches("<tuplet type=\"start\"").count(), 2);
        assert_eq!(d.musicxml.matches("<tuplet type=\"stop\"").count(), 2);
        let events = &mut s.measures[1].events;
        events.swap(1, 3); // still exactly four beats; isolated triplet runs unsupported.
        assert!(s.validate().unwrap_err().contains("adjacent groups"));
    }
    #[test]
    fn rejects_invalid_model() {
        let mut s = Score::rhythm_example();
        s.measures[0].events[0].notes[0].string = 0;
        assert!(s.validate().is_err());
        s = Score::rhythm_example();
        s.tuning[0] = 128;
        assert!(s.validate().is_err());
        s = Score::rhythm_example();
        s.measures[0].events[0].notes[0].fret = 37;
        assert!(s.validate().is_err());
        s = Score::rhythm_example();
        s.measures[0].meter.beat_type = 3;
        assert!(s.validate().is_err());
        s = Score::rhythm_example();
        s.measures[0].events.pop();
        assert!(s.validate().is_err())
    }
    #[test]
    fn unknown_source_refused() {
        let mut d = MusicDocument::from_score(&Score::rhythm_example()).unwrap();
        d.musicxml = d.musicxml.replacen("<note>", "<note><grace/>", 1);
        assert!(d.score().is_err());
        assert!(d.musicxml.contains("<grace/>"))
    }
    #[test]
    fn changed_tab_refused() {
        let mut d = MusicDocument::from_score(&Score::rhythm_example()).unwrap();
        d.musicxml = d.musicxml.replace("<staff>2</staff>", "<staff>3</staff>");
        assert!(d.score().is_err())
    }
    #[test]
    fn harmony_stays_written() {
        let mut s = Score::progression_example();
        let h = s.measures[0].harmony;
        s.measures[0].events[0].notes = vec![FrettedNote { string: 1, fret: 0 }];
        assert!(s.chord_mismatch(0));
        assert_eq!(s.measures[0].harmony, h);
        assert!(s.chord_suggestions(0).contains(&ChordSymbol {
            root: 0,
            kind: ChordKind::Major7
        }));
        assert!(!s.chord_suggestions(0).contains(&h.unwrap()))
    }
}
