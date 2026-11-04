use super::*;
fn leaf(width: f32, view: NotationView) -> NotationLeaf {
    NotationLeaf::new(
        Score::progression_example(),
        view,
        (0, 0),
        width,
        ColorF {
            r: 0.9,
            g: 0.9,
            b: 0.9,
            a: 1.0,
        },
        ColorF {
            r: 0.4,
            g: 0.7,
            b: 1.0,
            a: 1.0,
        },
    )
}
#[test]
fn actual_tab_digit_outlines_fit_between_adjacent_strings() {
    let face = ttf_parser::Face::parse(FONT, 0).unwrap();
    let (scale, advance) = tab_digit_metrics(&face);
    for code in 0xe080..=0xe089 {
        let glyph = face.glyph_index(char::from_u32(code).unwrap()).unwrap();
        let bounds = face.glyph_bounding_box(glyph).unwrap();
        let height = (f32::from(bounds.y_max) - f32::from(bounds.y_min)) * scale;
        assert!(
            height < 14.0,
            "fret digits must leave space between 14px string lines"
        );
        assert!((f32::from(bounds.x_max) - f32::from(bounds.x_min)) * scale < advance);
    }
}

#[test]
fn complete_measures_wrap_and_event_hitboxes_stay_inside() {
    for width in [320.0, 640.0, 1000.0] {
        for view in [NotationView::Chart, NotationView::Tab, NotationView::Score] {
            let leaf = leaf(width, view);
            for (_, _, (x, y, w, h)) in leaf.event_rects() {
                assert!(w > 0.0 && h > 0.0);
                assert!(x >= 0.0 && x + w <= leaf.size.width);
                assert!(y >= 0.0 && y + h <= leaf.size.height);
            }
            for bar in &leaf.bars {
                assert!(bar.x + bar.width <= width);
                assert!(bar.y + bar.height <= leaf.size.height);
            }
        }
    }
    assert!(
        leaf(320.0, NotationView::Score).size.height
            > leaf(1000.0, NotationView::Score).size.height
    );
}
#[test]
fn each_view_emits_native_paint_commands() {
    for view in [NotationView::Chart, NotationView::Tab, NotationView::Score] {
        for score in [Score::progression_example(), Score::rhythm_example()] {
            let mut leaf = NotationLeaf::new(
                score,
                view,
                (0, 0),
                320.0,
                ColorF {
                    r: 0.9,
                    g: 0.9,
                    b: 0.9,
                    a: 1.0,
                },
                ColorF {
                    r: 0.4,
                    g: 0.7,
                    b: 1.0,
                    a: 1.0,
                },
            );
            assert!(leaf.paint_dirty());
            let mut cmds = Vec::new();
            let size = leaf.size;
            leaf.paint(&mut PaintCx::new(&mut cmds, size));
            assert!(!leaf.paint_dirty());
            assert!(
                cmds.len() > 10,
                "{view:?} emits staves and real glyph outlines"
            );
        }
    }
}
#[test]
fn score_measures_share_staff_lines_within_each_wrapped_row() {
    let score = Score::rhythm_example();
    let wide = NotationLeaf::new(
        score.clone(),
        NotationView::Score,
        (0, 0),
        1000.0,
        leaf(320.0, NotationView::Score).ink,
        leaf(320.0, NotationView::Score).accent,
    );
    assert_eq!(wide.bars[0].y, wide.bars[1].y);
    assert_eq!(wide.bars[0].stave_top, wide.bars[1].stave_top);
    assert_eq!(wide.bars[0].height, wide.bars[1].height);
    let narrow = NotationLeaf::new(
        score,
        NotationView::Score,
        (0, 0),
        320.0,
        wide.ink,
        wide.accent,
    );
    assert!(narrow.bars[1].y >= narrow.bars[0].y + narrow.bars[0].height);
}
#[test]
fn playback_cursor_is_separate_from_selection_in_all_views_and_rests() {
    for view in [NotationView::Chart, NotationView::Tab, NotationView::Score] {
        for rest in [false, true] {
            let mut base = leaf(640.0, view);
            if rest {
                let event = &mut base.score.measures[1].events[0];
                event.notes.clear();
                event.rest = true;
            }
            let authored = base.score.clone();
            let rects = base.event_rects();
            let mut baseline = Vec::new();
            let size = base.size;
            base.paint(&mut PaintCx::new(&mut baseline, size));
            assert!(!base.paint_dirty());
            let mut playing = base.with_playhead(Some((1, 0)));
            assert!(playing.paint_dirty());
            assert_eq!(playing.selected, (0, 0));
            assert_eq!(playing.score, authored);
            assert_eq!(playing.event_rects(), rects);
            let mut commands = Vec::new();
            playing.paint(&mut PaintCx::new(&mut commands, size));
            assert_eq!(commands.len(), baseline.len() + 3);
            assert!(matches!(&commands[0], sprigging::PaintCmd::DrawRect(rect)
                if rect.color.a == playing.accent.a * 0.12));
            // Existing musical glyphs and their authoring-selection colors
            // remain identical. Playback adds only its background geometry.
            assert_eq!(
                serde_json::to_value(&commands[3..]).unwrap(),
                serde_json::to_value(&baseline).unwrap()
            );
            #[cfg(feature = "accessibility")]
            let mut node = accesskit::Node::new(accesskit::Role::GenericContainer);
            #[cfg(feature = "accessibility")]
            {
                node.set_label("Authored score label");
                playing.accessibility(&mut node);
                assert_eq!(node.label(), Some("Authored score label"));
                assert_eq!(
                    node.state_description(),
                    Some(if rest {
                        "Playback: bar 2, event 1, rest"
                    } else {
                        "Playback: bar 2, event 1, sounding notes"
                    })
                );
            }
            let mut stopped = playing.with_playhead(None);
            assert!(stopped.paint_dirty());
            #[cfg(feature = "accessibility")]
            {
                stopped.accessibility(&mut node);
                assert!(node.state_description().is_none());
            }
            let mut cleared = Vec::new();
            stopped.paint(&mut PaintCx::new(&mut cleared, size));
            assert_eq!(
                serde_json::to_value(&cleared).unwrap(),
                serde_json::to_value(&baseline).unwrap()
            );
        }
    }
}

#[test]
fn invalid_playback_positions_are_ignored_and_default_has_no_cursor() {
    let mut leaf = leaf(640.0, NotationView::Score);
    assert_eq!(leaf.playhead, None);
    let mut commands = Vec::new();
    let size = leaf.size;
    leaf.paint(&mut PaintCx::new(&mut commands, size));
    let leaf = leaf.with_playhead(Some((usize::MAX, 0)));
    assert_eq!(leaf.playhead, None);
    assert!(!leaf.paint_dirty());
    let leaf = leaf.with_playhead(Some((0, usize::MAX)));
    assert_eq!(leaf.playhead, None);
    assert_eq!(leaf.selected, (0, 0));
}

#[test]
fn chord_chart_uses_authored_meter_beat_count() {
    let mut counts = Vec::new();
    for (beats, beat_type, value) in [
        (3, 4, NoteValue::Quarter),
        (4, 4, NoteValue::Quarter),
        (6, 8, NoteValue::Eighth),
    ] {
        let mut leaf = leaf(320.0, NotationView::Chart);
        leaf.score.measures.truncate(1);
        leaf.bars.truncate(1);
        let measure = &mut leaf.score.measures[0];
        measure.meter = woodshedding::notation::TimeSignature { beats, beat_type };
        let mut event = measure.events[0].clone();
        event.value = value;
        measure.events = vec![event; usize::from(beats)];
        let mut cmds = Vec::new();
        let size = leaf.size;
        leaf.paint(&mut PaintCx::new(&mut cmds, size));
        counts.push(cmds.len());
    }
    assert_eq!(counts[1], counts[0] + 1);
    assert_eq!(counts[2], counts[0] + 3);
}

#[test]
fn bundled_font_has_real_music_outlines() {
    let face = ttf_parser::Face::parse(FONT, 0).unwrap();
    for code in [
        0xe050, 0xe062, 0xe05c, 0xe0a2, 0xe0a3, 0xe0a4, 0xe101, 0xe240, 0xe260, 0xe262, 0xe4e3,
        0xe4e4, 0xe4e5, 0xe4e6, 0xe080, 0xe089,
    ] {
        let glyph = face
            .glyph_index(char::from_u32(code).unwrap())
            .expect("SMuFL glyph");
        let mut outline = Outline {
            path: Path::new(),
            x: 0.0,
            y: 0.0,
            scale: 1.0,
        };
        assert!(face.outline_glyph(glyph, &mut outline).is_some());
    }
}
#[test]
fn instrument_clefs_and_written_octaves_match_conventional_staves() {
    for (name, profile, reference_midi, reference_line) in [
        ("Guitar", (0xe050, 30, 12), 55, 32), // G3 sounds as written G4
        ("Bass", (0xe062, 18, 12), 41, 24),   // F2 sounds as written F3
        ("Double Bass", (0xe062, 18, 12), 41, 24),
        ("Viola", (0xe05c, 24, 0), 60, 28), // C4 on alto middle line
        ("Cello", (0xe062, 18, 0), 53, 24), // F3 on bass fourth line
        ("Violin", (0xe050, 30, 0), 67, 32), // G4 on treble second line
        ("Ukulele", (0xe050, 30, 0), 67, 32),
    ] {
        assert_eq!(clef_profile(name), profile, "{name}");
        assert_eq!(
            staff_pitch(reference_midi + profile.2).0,
            reference_line,
            "{name}"
        );
    }
}

#[test]
fn arbitrary_string_count_and_extreme_ledger_space_are_measured() {
    use woodshedding::notation::{FrettedNote, Measure, NoteEvent, TimeSignature};
    let score = Score {
        title: "Range".into(),
        instrument: "Extended guitar".into(),
        tuning: vec![100, 90, 80, 70, 60, 50, 40, 30],
        tempo: 100,
        measures: vec![Measure {
            meter: TimeSignature {
                beats: 4,
                beat_type: 4,
            },
            harmony: None,
            events: vec![NoteEvent {
                notes: vec![
                    FrettedNote {
                        string: 1,
                        fret: 24,
                    },
                    FrettedNote { string: 8, fret: 0 },
                ],
                value: NoteValue::Whole,
                rest: false,
            }],
        }],
    };
    let tab = NotationLeaf::new(
        score.clone(),
        NotationView::Tab,
        (0, 0),
        320.0,
        ColorF {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        },
        ColorF {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        },
    );
    assert_eq!(tab.score.tuning.len(), 8);
    let staff = NotationLeaf::new(
        score,
        NotationView::Score,
        (0, 0),
        320.0,
        tab.ink,
        tab.accent,
    );
    assert!(staff.bars[0].stave_top > 94.0);
    assert!(staff.bars[0].height > 220.0);
    assert_eq!(staff.event_rects().len(), 1);
    assert_eq!(clef_profile("Guitar"), (0xe050, 30, 12));
    assert_eq!(clef_profile("Bass"), (0xe062, 18, 12));
    assert_eq!(clef_profile("Ukulele"), (0xe050, 30, 0));
    assert_eq!(staff_pitch(64), (30, 0));
    assert_eq!(staff_pitch(65), (31, 0));
    assert_eq!(staff_pitch(66), (31, 1));
    assert_eq!(staff_pitch(63), (30, -1)); // E-flat4
    assert_eq!(staff_pitch(68), (33, -1)); // A-flat4
    assert_eq!(staff_pitch(70), (34, -1)); // B-flat4
}
