// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0
//! A progression's relative roles and a supplied song arrangement are separate
//! readings. Ordinal alignment is explicit here; it is not harmonic analysis.
use crate::{StageState, format_role, song::SongDoc};
use sceno::SourceRef;
use scenomise::{
    backdrops::{BACKDROP_SCENE_VERSION, SceneFacts, SceneInput},
    template::{
        Placement, SequenceTemplate, Span, TemplateGroup, TemplateOccurrence, TemplateRelation,
        TemplateSlot, TemplatedSequence,
    },
};

pub fn progression_scene(
    stage: &StageState,
    song: &SongDoc,
    reading_id: &str,
) -> Result<SceneInput, String> {
    let progression = stage
        .progression_idx
        .and_then(|i| stage.progressions().get(i))
        .ok_or("Select a progression before exporting its template")?;
    if song.bars.len() != progression.roles.len() || song.bars.is_empty() {
        return Err("Ordinal arrangement needs one song block for each progression role".into());
    }
    let mut extent = 0u32;
    let mut slots = Vec::new();
    let mut occurrences = Vec::new();
    let mut bars = Vec::new();
    for (i, (bar, role)) in song.bars.iter().zip(progression.roles).enumerate() {
        if bar.beats == 0
            || bar.length == 0
            || !bar.bpm.is_finite()
            || bar.bpm <= 0.0
            || bar.root_pc >= 12
        {
            return Err("Song block needs valid beats, length, tempo and pitch class".into());
        }
        let span = Span {
            start: extent,
            length: u32::from(bar.beats) * u32::from(bar.length) * 4,
        };
        extent = span.end().ok_or("Song arrangement exceeds tick range")?;
        let slot = format!("role-{i}");
        let numeral = format_role(role);
        slots.push(TemplateSlot {
            key: slot.clone(),
            lane: 0,
            span,
            label: numeral.clone(),
            expectation: format!(
                "{} in {} major; supplied ordinal role {}",
                numeral,
                stage.root_name(),
                i + 1
            ),
        });
        occurrences.push(TemplateOccurrence {
            key:format!("bar-{i}"),
            source:SourceRef {adapter:"woodshed.chord".into(),id:if bar.chord_label.is_empty(){"rest".into()}else{format!("{}:{}",bar.root_pc,bar.formula_name)}},
            label:if bar.chord_label.is_empty(){"Rest".into()}else{bar.chord_label.clone()},
            detail:format!("{}#bar={} · {} · {} beats × {} bars · {} BPM · ordinal alignment supplied by Woodshed",reading_id,i,bar.label,bar.beats,bar.length,bar.bpm),
            placement:Some(Placement {lane:0,span}),slot:Some(slot),
        });
        bars.push(TemplateGroup {
            key: format!("block-{i}"),
            lane: 0,
            span,
            parent: Some("phrase".into()),
            label: format!("{} × {}/4", bar.length, bar.beats),
        });
    }
    let mut groups = vec![TemplateGroup {
        key: "phrase".into(),
        lane: 0,
        span: Span {
            start: 0,
            length: extent,
        },
        parent: None,
        label: progression.name.into(),
    }];
    groups.extend(bars);
    let relations = (1..occurrences.len())
        .map(|i| TemplateRelation {
            from: format!("bar-{}", i - 1),
            to: format!("bar-{i}"),
            kind: "woodshed.sequence".into(),
            label: "Next chord block".into(),
            why: format!(
                "Adjacent authored blocks in {}; no harmonic function or tension inferred",
                song.name
            ),
        })
        .collect();
    let facts = SceneFacts::TemplatedSequence(TemplatedSequence {
        template: SequenceTemplate {
            lanes: vec![format!(
                "{} · expected roles below / arranged chords above",
                song.name
            )],
            extent,
            ticks_per_unit: 4,
            unit_label: "quarter-note beat (4 ticks); no playback clock".into(),
            groups,
        },
        slots,
        occurrences,
        relations,
    });
    let revision = blake3::hash(
        &serde_json::to_vec(&(&facts, stage.root_idx, progression.name))
            .map_err(|e| e.to_string())?,
    )
    .to_hex()
    .to_string();
    let input = SceneInput {
        version: BACKDROP_SCENE_VERSION,
        source: SourceRef {
            adapter: "woodshed.progression-arrangement".into(),
            id: reading_id.into(),
        },
        revision,
        title: format!(
            "{} in {} · role template and song arrangement",
            progression.name,
            stage.root_name()
        ),
        facts,
    };
    input.compile().map_err(|e| e.to_string())?;
    Ok(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::song::song_from_progression;
    fn stage() -> StageState {
        let mut stage = StageState::default();
        let i = stage
            .progressions()
            .iter()
            .position(|p| p.name == "ii-V-I (Jazz)")
            .unwrap();
        stage.select_progression(i);
        stage
    }
    #[test]
    fn role_template_transposes_but_keeps_relative_identity() {
        let mut stage = stage();
        let a = progression_scene(
            &stage,
            &song_from_progression(&stage, 100.0).unwrap(),
            "song",
        )
        .unwrap();
        stage.root_idx = 7;
        let b = progression_scene(
            &stage,
            &song_from_progression(&stage, 100.0).unwrap(),
            "song",
        )
        .unwrap();
        let (SceneFacts::TemplatedSequence(a), SceneFacts::TemplatedSequence(b)) =
            (a.facts, b.facts)
        else {
            unreachable!()
        };
        assert_eq!(
            a.slots.iter().map(|s| &s.label).collect::<Vec<_>>(),
            b.slots.iter().map(|s| &s.label).collect::<Vec<_>>()
        );
        assert_ne!(a.occurrences[0].source, b.occurrences[0].source);
    }
    #[test]
    fn longer_bars_and_rests_keep_spans_without_source_edits_or_judgment() {
        let stage = stage();
        let mut song = song_from_progression(&stage, 100.0).unwrap();
        song.bars[2].length = 2;
        song.bars[1].toggle_silent();
        let before = song.clone();
        let input = progression_scene(&stage, &song, "song").unwrap();
        assert_eq!(song, before);
        let SceneFacts::TemplatedSequence(seq) = &input.facts else {
            unreachable!()
        };
        assert_eq!(
            seq.occurrences[2].placement.as_ref().unwrap().span.length,
            32
        );
        assert_eq!(seq.occurrences[1].label, "Rest");
        assert!(input.compile().unwrap().diagnostics.is_empty());
        assert_eq!(
            input,
            SceneInput::parse(&serde_json::to_string(&input).unwrap()).unwrap()
        );
    }
    #[test]
    fn repeated_chord_resource_keeps_distinct_occurrences() {
        let stage = stage();
        let mut song = song_from_progression(&stage, 100.0).unwrap();
        song.bars[2] = song.bars[0].clone();
        let input = progression_scene(&stage, &song, "song").unwrap();
        let SceneFacts::TemplatedSequence(seq) = &input.facts else {
            unreachable!()
        };
        assert_eq!(seq.occurrences[0].source, seq.occurrences[2].source);
        assert_ne!(seq.occurrences[0].key, seq.occurrences[2].key);
        assert_ne!(seq.occurrences[0].placement, seq.occurrences[2].placement);
        assert!(input.compile().unwrap().diagnostics.is_empty());
    }
    #[test]
    fn missing_progression_and_unusable_arrangements_are_refused() {
        let stage = stage();
        let mut song = song_from_progression(&stage, 100.0).unwrap();
        song.bars[0].beats = 0;
        assert!(progression_scene(&stage, &song, "song").is_err());
        song.bars.clear();
        assert!(progression_scene(&stage, &song, "song").is_err());
        assert!(progression_scene(&StageState::default(), &song, "song").is_err());
    }
}
