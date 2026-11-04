use super::*;

#[test]
fn drawn_path_saves_as_a_playable_card() {
    let mut s = StageState::new();
    assert!(
        s.card_from_drawn_path().is_none(),
        "nothing drawn, nothing to save"
    );
    for &(string, fret) in &[(0u8, 5u8), (1, 7), (2, 5)] {
        s.append_to_path(string as usize, fret);
    }
    let card = s.card_from_drawn_path().expect("a drawn path makes a card");
    assert_eq!(card.material.tag(), "Path");
    // A drawn path carries its own order, so it walks — Block would sound
    // the notes on top of each other.
    assert!(
        matches!(card.touch, Touch::Walk),
        "a drawn path's touch is walk, not block"
    );

    // The saved card resolves back to exactly the drawn positions, in the
    // drawn order — the path *is* the material, not a re-derived set.
    let dots = s.dots_for_card(&card);
    assert_eq!(
        dots.iter()
            .map(|d| (d.string_index, d.fret))
            .collect::<Vec<_>>(),
        vec![(0, 5), (1, 7), (2, 5)],
        "path card resolves to the drawn positions in drawn order"
    );
    // Unlike riff steps, a drawn path's notes carry real pitch, so the
    // detail card and the run's audio have something to say.
    assert!(
        dots.iter()
            .all(|d| !d.label.is_empty() && d.frequency > 0.0),
        "every drawn note resolves to a named, sounding pitch"
    );
    // And it sounds every drawn note (a riff card voices nothing).
    assert_eq!(
        s.card_voicing(&card).unwrap().0.len(),
        3,
        "the voicing sounds every drawn note"
    );
    // A hand-drawn path is its own content, not a catalog subject.
    assert!(
        crate::history::catalog_id_for_card(&card).is_none(),
        "a drawn path has no catalog identity"
    );
}

#[test]
fn saved_path_uses_retained_setup_and_viewport_only_clips_dots() {
    let mut stage = StageState::new();
    stage.append_to_path(0, 2);
    let card = stage.card_from_drawn_path().expect("path is authored");
    let expected = stage.card_voicing(&card).unwrap().0;

    stage.set_tuning(1);
    stage.fret_start = 8;
    stage.fret_count = 12;
    assert!(
        stage.dots_for_card(&card).is_empty(),
        "viewport clips display"
    );
    assert_eq!(
        stage.card_voicing(&card).unwrap().0,
        expected,
        "a valid saved contact remains playable with its retained setup"
    );
}

#[test]
fn stale_solo_marks_do_not_fall_back_to_the_full_chord() {
    let mut stage = StageState::new();
    stage.set_lens(Lens::Chords);
    let mut card = stage.card_from_lens().unwrap();
    card.setting.mark_mode = MarkMode::Solo;
    card.setting.marked = vec![(usize::MAX, u8::MAX)];
    assert!(stage.card_sounding_pitches(&card).unwrap().0.is_empty());
}

#[test]
fn neck_window_bounds_the_board_and_the_dots() {
    let mut s = StageState::new();
    s.set_lens(Lens::Scales);

    // Full neck: the instrument's standard count, open strings included.
    s.apply_neck(0, None);
    assert_eq!(s.fret_start, 0);
    assert!(s.fret_count >= 22, "a guitar's full neck reaches fret 22+");
    assert!(
        s.dots().iter().any(|d| d.fret == 0),
        "a nut-anchored window shows open strings"
    );

    // A mid-neck window (8-16): every dot lands inside it, none below.
    s.apply_neck(8, Some(16));
    assert_eq!((s.fret_start, s.fret_count), (8, 16));
    let dots = s.dots();
    assert!(!dots.is_empty(), "the window still has notes");
    assert!(
        dots.iter().all(|d| d.fret >= 8 && d.fret <= 16),
        "no dot falls outside the neck window"
    );

    // A start past the end is clamped to the end, not left inverted.
    s.apply_neck(30, Some(12));
    assert!(s.fret_start <= s.fret_count);
}

#[test]
fn marker_a11y_label_speaks_note_role_and_place() {
    let s = StageState::new(); // A Minor Pentatonic, root A, by default.
    let dots = s.dots();
    let strings = s.string_count();

    let root = dots
        .iter()
        .find(|d| d.is_root)
        .expect("a root on the board");
    let label = marker_a11y_label(root, strings);
    assert!(label.starts_with('A'), "names the note: {label}");
    assert!(label.contains("root"), "names its role: {label}");
    assert!(label.contains("string "), "gives the string: {label}");

    // An open-string marker says "open", never "fret 0".
    if let Some(open) = dots.iter().find(|d| d.fret == 0) {
        let l = marker_a11y_label(open, strings);
        assert!(
            l.contains("open") && !l.contains("fret 0"),
            "open string: {l}"
        );
    }
    // A non-root tone names its interval, not "root".
    if let Some(other) = dots
        .iter()
        .find(|d| !d.is_root && !d.interval_name.is_empty())
    {
        let l = marker_a11y_label(other, strings);
        assert!(!l.contains("root"), "non-root omits root: {l}");
        assert!(l.contains(&other.interval_name), "names the interval: {l}");
    }
}

#[test]
fn octave_shift_moves_the_shape_and_keeps_its_notes() {
    let mut s = StageState::new();
    // An octave shift needs neck to land on: on the default 12-fret board it
    // never fits, which is why the controls are gated on can_shift_path.
    s.fret_count = 24;
    for &(string, fret) in &[(0usize, 5u8), (1, 7), (2, 5)] {
        s.append_to_path(string, fret);
    }
    assert!(
        s.can_shift_path(12),
        "a 24-fret neck has room for the octave"
    );
    assert!(!s.can_shift_path(-12), "there is no room below fret 5");
    let before = s.effective_run_path();
    s.shift_path(12);
    assert_eq!(
        s.authored_path,
        vec![(0, 17), (1, 19), (2, 17)],
        "an octave shift moves every note 12 frets, keeping the shape"
    );
    // Same notes an octave up: pitch classes hold, so the shape stays on the
    // material's tones and its degrees still read.
    let after = s.effective_run_path();
    for (b, a) in before.iter().zip(after.iter()) {
        assert_eq!(
            pc_from_hz(b.2),
            pc_from_hz(a.2),
            "an octave shift preserves each step's pitch class"
        );
        assert!(
            (a.2 / b.2 - 2.0).abs() < 0.01,
            "each step lands exactly one octave up"
        );
    }
    // Refuses rather than distorting when the shape would run off the neck.
    let at_top = s.authored_path.clone();
    assert!(!s.can_shift_path(12), "no room for a second octave");
    s.shift_path(12);
    assert_eq!(
        s.authored_path, at_top,
        "a shift that would leave the neck is a no-op, not a clamp"
    );
}

#[test]
fn related_material_selects_through_catalog_identity() {
    let mut s = StageState::new();
    s.set_lens(Lens::Chords);
    let major_7 = s.chords().iter().position(|c| c.name == "Major 7").unwrap();
    s.select_chord(major_7);
    let related = s.related_material(64);
    let major = related
        .iter()
        .find(|item| item.kind == "Scale" && item.title == "Major")
        .expect("Major 7 relates to Major");
    assert!(
        major.reason().contains("Fits Major"),
        "{:?}",
        major.reasons()
    );
    s.select_related(major.target);
    assert_eq!(s.lens, Lens::Scales);
    assert_eq!(s.scale().name, "Major");
}

#[test]
fn the_panel_shows_more_than_one_relation_family() {
    // Nine progressions all score 96 for `Major`, so the raw ranking filled
    // a six-row panel with one family and the harmonic frontier never
    // appeared. The display policy interleaves families; it drops nothing a
    // longer list would have kept.
    let mut s = StageState::new();
    s.set_lens(Lens::Chords);
    let major = s.chords().iter().position(|c| c.name == "Major").unwrap();
    s.select_chord(major);
    let history = history::PracticeHistory::default();
    let settings = storage::RelatedSettings::default();
    let shown = s.related_material_configured(&history, &settings, 6);
    assert_eq!(shown.len(), 6);
    let families: std::collections::BTreeSet<_> =
        shown.iter().map(|item| item.relations[0].kind).collect();
    assert!(
        families.len() >= 3,
        "the panel shows several relation families: {:?}",
        shown.iter().map(|i| i.reason()).collect::<Vec<_>>()
    );
    // And at least one visible pair carries several relations at once,
    // which is what the flattened boundary used to hide.
    assert!(
        shown.iter().any(|item| item.relation_count() > 1),
        "multiplicity reaches the panel: {:?}",
        shown.iter().map(|i| i.relation_count()).collect::<Vec<_>>()
    );
}

#[test]
fn related_history_promotes_a_prior_practice_path() {
    let mut s = StageState::new();
    s.set_lens(Lens::Scales);
    let dorian = s
        .scales()
        .iter()
        .position(|scale| scale.name == "Dorian")
        .unwrap();
    s.select_scale(dorian);
    let mut history = history::PracticeHistory::default();
    history.record(
        Some(1_000),
        woodshed_graph::chord_id("Minor 7"),
        history::EngagementKind::Rehearsed,
        Some(woodshed_graph::scale_id("Dorian")),
        None,
    );
    let ranked = s.related_material_with_history(&history, 5);
    assert_eq!(ranked[0].title, "Minor 7");
    assert_eq!(ranked[0].reason(), "Previously practiced from here");
    assert!(ranked[0].has_evidence());
    // The theory relation that was there first survives the promotion: the
    // old boundary replaced it, which is the erasure P4b removes.
    assert!(
        ranked[0].relation_count() > 1,
        "evidence joins the reasons rather than replacing them: {:?}",
        ranked[0].reasons()
    );
    assert!(
        ranked[0]
            .relations
            .iter()
            .any(|r| r.authority != woodshed_graph::RelationAuthority::Evidence),
        "a deterministic relation remains: {:?}",
        ranked[0].reasons()
    );
}

#[test]
fn related_settings_disable_history_and_hide_stable_identities() {
    let mut s = StageState::new();
    s.set_lens(Lens::Scales);
    let dorian = s
        .scales()
        .iter()
        .position(|scale| scale.name == "Dorian")
        .unwrap();
    s.select_scale(dorian);
    let mut history = history::PracticeHistory::default();
    history.record(
        Some(1_000),
        woodshed_graph::chord_id("Minor 7"),
        history::EngagementKind::Staged,
        Some(woodshed_graph::scale_id("Dorian")),
        None,
    );
    let settings = storage::RelatedSettings {
        use_history: false,
        show_neighborhood: true,
        dismissed_ids: vec![woodshed_graph::chord_id("Minor 7")],
        ..Default::default()
    };
    let related = s.related_material_configured(&history, &settings, 5);
    assert!(related.iter().all(|item| item.title != "Minor 7"));
    assert!(related.iter().all(|item| !item.has_evidence()));
    assert!(
        related
            .iter()
            .all(|item| !item.reasons().iter().any(|r| r.contains("Previously")))
    );
}
