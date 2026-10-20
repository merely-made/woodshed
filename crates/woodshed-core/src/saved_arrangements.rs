//! Set-owned musical arrangements. Retention is independent of the optional
//! scene exporter and of the live Stage's navigation or graph presentation.
use crate::{StageState, storage::PersistedSession};
use std::collections::HashSet;
use woodshedding::{
    rehearsal::{
        Set,
        arrangement::{ProgressionArrangement, SavedProgressionArrangement, Span},
    },
    tuning::Tuning,
};

#[derive(Clone, Copy, Debug)]
pub enum SessionSet {
    Working(crate::working_sets::WorkingSetId),
    Retained(crate::retained_sets::SavedSetId),
}
impl SessionSet {
    pub fn get(self, session: &PersistedSession) -> Option<&Set> {
        match self {
            Self::Working(id) => session.working_sets.get(id, &session.set),
            Self::Retained(id) => session.retained_sets.get(id).map(|s| &s.set),
        }
    }
    /// Retained snapshots can be read/exported; author on a working copy.
    pub fn working_mut(self, session: &mut PersistedSession) -> Option<&mut Set> {
        match self {
            Self::Working(id) => session.working_sets.get_mut(id, &mut session.set),
            Self::Retained(_) => None,
        }
    }
}

/// Explicitly author or replace this Set's alignment. Failed validation leaves
/// the previous attachment and the Set's instructions unchanged.
pub fn save_progression_arrangement(
    stage: &StageState,
    set: &mut Set,
    alignment: ProgressionArrangement,
) -> Result<(), String> {
    let progression = stage
        .progression_idx
        .and_then(|i| stage.progressions().get(i))
        .ok_or("Select a progression before saving its arrangement")?;
    let tuning = stage.tuning();
    let saved = SavedProgressionArrangement {
        version: 1,
        progression: progression.name.into(),
        roles: progression.roles.to_vec(),
        root_pc: stage.root().pitch_class(),
        instrument: tuning.instrument.to_string(),
        tuning: tuning.name.into(),
        open_midi: tuning.strings.iter().map(|p| p.midi()).collect(),
        fret_start: stage.fret_start,
        fret_end: stage.fret_count,
        alignment,
    };
    validate_alignment(set, &saved)?;
    set.progression_arrangement = Some(saved);
    Ok(())
}

fn validate_alignment(set: &Set, saved: &SavedProgressionArrangement) -> Result<(), String> {
    let a = &saved.alignment;
    if saved.version != 1 {
        return Err("Unsupported saved arrangement version; attachment retained".into());
    }
    if a.name.trim().is_empty()
        || a.name.len() > 1024
        || a.extent == 0
        || a.extent > 1_000_000
        || a.role_spans.len() != saved.roles.len()
        || saved.roles.is_empty()
        || saved.roles.len() > 512
        || set.cards.len() > 512
        || a.cards.len() > 512
        || a.rests.len() > 512
    {
        return Err("Arrangement requires a named, bounded template with one span per role".into());
    }
    woodshedding::rehearsal::arrangement::bars::validate_groups(a)?;
    let valid_span = |s: Span| {
        s.length > 0
            && s.start
                .checked_add(s.length)
                .is_some_and(|end| end <= a.extent)
    };
    let valid_role = |role: Option<usize>| role.is_none_or(|i| i < saved.roles.len());
    let valid_placement =
        |p: &woodshedding::rehearsal::arrangement::Placement| p.lane == 0 && valid_span(p.span);
    if a.role_spans.iter().any(|s| !valid_span(*s)) {
        return Err("Expectation span lies outside the arrangement".into());
    }
    let mut ids = HashSet::new();
    if set
        .cards
        .iter()
        .any(|c| !c.id.is_assigned() || !ids.insert(c.id))
    {
        return Err("Set needs assigned, unique Card identities".into());
    }
    let mut bound = HashSet::new();
    for b in &a.cards {
        if !ids.contains(&b.card) {
            return Err(format!(
                "Unresolved arrangement: Card {} is missing; repair its alignment explicitly",
                b.card.0
            ));
        }
        if !bound.insert(b.card)
            || !valid_role(b.role)
            || b.placement.as_ref().is_some_and(|p| !valid_placement(p))
        {
            return Err("Card alignment is duplicated or outside the template".into());
        }
    }
    let mut rests = HashSet::new();
    if a.rests.iter().any(|r| {
        r.key.trim().is_empty()
            || r.key.len() > 1024
            || !rests.insert(&r.key)
            || !valid_role(r.role)
            || !valid_placement(&r.placement)
    }) {
        return Err("Rest needs a unique identity and valid placement".into());
    }
    Ok(())
}

/// Re-resolve names and compare exact roles and tuning pitches. A changed or
/// unavailable catalog leaves the authored attachment intact and refuses export.
/// Live Stage selection and application settings cannot retarget this reading.
pub fn saved_arrangement_stage(set: &Set) -> Result<StageState, String> {
    let saved = set
        .progression_arrangement
        .as_ref()
        .ok_or("This Set has no saved progression arrangement")?;
    validate_alignment(set, saved)?;
    if saved.root_pc >= 12
        || saved.fret_start > saved.fret_end
        || saved.fret_end == 0
        || saved.fret_end > 30
    {
        return Err("Saved arrangement has an invalid root or neck window".into());
    }
    let mut stage = StageState::new();
    let progression = stage
        .progressions()
        .iter()
        .position(|p| p.name == saved.progression)
        .ok_or("Saved progression is unavailable; attachment retained for repair")?;
    if stage.progressions()[progression].roles != saved.roles {
        return Err(
            "Saved progression roles have changed; explicit realignment is required".into(),
        );
    }
    let tuning_index = crate::tunings()
        .iter()
        .position(|t| t.name == saved.tuning && t.instrument.to_string() == saved.instrument)
        .ok_or("Saved instrument/tuning is unavailable; attachment retained for repair")?;
    let tuning = Tuning::from_spec(&crate::tunings()[tuning_index]);
    if tuning.strings.iter().map(|p| p.midi()).collect::<Vec<_>>() != saved.open_midi {
        return Err("Saved tuning pitches have changed; explicit setup review is required".into());
    }
    stage.set_tuning(tuning_index);
    stage.set_root(usize::from((saved.root_pc + 3) % 12));
    stage.select_progression(progression);
    stage.fret_start = saved.fret_start;
    stage.fret_count = saved.fret_end;
    Ok(stage)
}

#[cfg(test)]
mod tests {
    use super::*;
    use woodshedding::rehearsal::arrangement::{CardAlignment, Placement};
    #[test]
    fn retained_alignment_is_available_without_scene_feature() {
        let mut stage = StageState::new();
        stage.select_progression(
            stage
                .progressions()
                .iter()
                .position(|p| p.name == "ii-V-I (Jazz)")
                .unwrap(),
        );
        let mut set = Set::default();
        set.push(stage.card_from_lens().unwrap());
        let a = ProgressionArrangement {
            name: "Portable Set attachment".into(),
            extent: 24,
            role_spans: (0..3)
                .map(|i| Span {
                    start: i * 8,
                    length: 8,
                })
                .collect(),
            cards: vec![CardAlignment {
                card: set.cards[0].id,
                role: Some(0),
                placement: Some(Placement {
                    lane: 0,
                    span: Span {
                        start: 0,
                        length: 8,
                    },
                }),
            }],
            rests: vec![],
            time_signatures: vec![],
            bar_groups: vec![],
        };
        save_progression_arrangement(&stage, &mut set, a).unwrap();
        let bytes = serde_json::to_vec(&set).unwrap();
        let reopened: Set = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            set.progression_arrangement,
            reopened.progression_arrangement
        );
        assert_eq!(
            saved_arrangement_stage(&reopened).unwrap().root_name(),
            stage.root_name()
        );
    }
}
