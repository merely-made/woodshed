use super::*;

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
