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
