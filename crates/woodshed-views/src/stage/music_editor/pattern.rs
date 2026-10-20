//! Explicit, temporary picking proposals; only Apply changes musical authority.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PickingPattern {
    QuarterAscending,
    QuarterDescending,
    EighthUpDown,
}
impl PickingPattern {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::QuarterAscending => "Quarter notes · low to high",
            Self::QuarterDescending => "Quarter notes · high to low",
            Self::EighthUpDown => "Eighth notes · up and down",
        }
    }
}
#[derive(Clone, Debug)]
pub(super) struct PickingPreview {
    pub(super) owner: WorkingSetId,
    pub(super) selected: (usize, usize),
    pub(super) baseline: Score,
    pub(super) source_voicing: Vec<FrettedNote>,
    pub(super) score: Score,
    pub(super) pattern: PickingPattern,
    pub(super) summary: String,
}
impl PickingPreview {
    pub(super) fn summary(&self) -> &str {
        &self.summary
    }
    fn new(
        owner: WorkingSetId,
        draft: &MusicDraft,
        pattern: PickingPattern,
    ) -> Result<Self, String> {
        draft.score.validate()?;
        let (bar, event) = draft.selected;
        let measure = draft
            .score
            .measures
            .get(bar)
            .ok_or("Select a chord bar first.")?;
        let source = measure.events.get(event).ok_or("Select a chord first.")?;
        if source.rest || source.notes.len() < 2 {
            return Err("Select a complete chord with at least two notes.".into());
        }
        if measure
            .events
            .iter()
            .any(|e| e.rest || e.notes != source.notes)
        {
            return Err("Picking replaces one chord bar. Select a bar containing only repetitions of this complete chord.".into());
        }
        let source_voicing = source.notes.clone();
        let mut ordered = source_voicing.clone();
        ordered.sort_by_key(|n| (draft.score.pitch(*n).unwrap_or(0), n.string));
        if pattern == PickingPattern::QuarterDescending {
            ordered.reverse();
        }
        let mut cycle = ordered.clone();
        if pattern == PickingPattern::EighthUpDown && ordered.len() > 2 {
            cycle.extend(ordered[1..ordered.len() - 1].iter().rev().copied());
        }
        let value = if pattern == PickingPattern::EighthUpDown {
            NoteValue::Eighth
        } else {
            NoteValue::Quarter
        };
        let units = u32::from(measure.meter.beats) * 24 / u32::from(measure.meter.beat_type);
        if units % value.units() != 0 {
            return Err("This bar does not fit complete quarter-note picks. Choose the eighth-note pattern.".into());
        }
        let events = (0..units / value.units())
            .map(|i| NoteEvent {
                notes: vec![cycle[i as usize % cycle.len()]],
                value,
                rest: false,
            })
            .collect::<Vec<_>>();
        let used = ordered
            .iter()
            .filter(|n| events.iter().any(|e| e.notes.contains(n)))
            .count();
        let picks = events
            .iter()
            .map(|e| {
                let n = e.notes[0];
                format!(
                    "{} (string {}, fret {}; {})",
                    note_name(draft.score.pitch(n).unwrap()),
                    n.string,
                    n.fret,
                    value.label()
                )
            })
            .collect::<Vec<_>>()
            .join(" → ");
        let summary = format!(
            "{} · Bar {} · {used} of {} chord tones used. Full source voicing: {}. Picks: {picks}. Written harmony stays unchanged. Apply replaces this bar; Undo restores the complete source chord.",
            pattern.label(),
            bar + 1,
            source_voicing.len(),
            source_voicing
                .iter()
                .map(|n| format!(
                    "{} [s{} f{}]",
                    note_name(draft.score.pitch(*n).unwrap()),
                    n.string,
                    n.fret
                ))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let mut score = draft.score.clone();
        score.measures[bar].events = events;
        score.validate()?;
        Ok(Self {
            owner,
            selected: draft.selected,
            baseline: draft.score.clone(),
            source_voicing,
            score,
            pattern,
            summary,
        })
    }
}
impl UiState {
    pub(super) fn preview_music_pattern(&mut self, pattern: PickingPattern) {
        let result = self
            .music_draft()
            .ok_or("Start music first.".into())
            .and_then(|d| PickingPreview::new(self.working_sets.active_id, d, pattern));
        match result {
            Ok(preview) => {
                self.music_editor.notice =
                    Some("Picking preview only. Choose Apply to change this bar.".into());
                self.music_editor.picking_preview = Some(preview);
            },
            Err(e) => {
                self.music_editor.picking_preview = None;
                self.music_editor.notice = Some(e);
            },
        }
    }
    pub fn cancel_music_pattern(&mut self) {
        self.music_editor.picking_preview = None;
        self.music_editor.notice = Some("Picking preview cancelled; music unchanged.".into());
    }
    pub fn apply_music_pattern(&mut self) {
        let Some(preview) = self.music_editor.picking_preview.take() else {
            return;
        };
        if self.working_sets.active_id != preview.owner
            || !self
                .music_draft()
                .is_some_and(|d| d.selected == preview.selected && d.score == preview.baseline)
        {
            self.music_editor.notice = Some(
                "Music or selection changed. Choose a fresh picking preview before applying."
                    .into(),
            );
            return;
        }
        self.edit_music(
            move |score, _| {
                *score = preview.score;
                Ok(())
            },
            "Picking pattern applied. Written harmony preserved; Undo restores the complete chord.",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> UiState {
        let mut ui = UiState::new();
        ui.activate_workspace_panel(WorkspacePanel::Arrangement);
        ui.music_example(false);
        ui
    }
    #[test]
    fn patterns_fill_four_four_and_six_eight_exactly() {
        for pattern in [
            PickingPattern::QuarterAscending,
            PickingPattern::QuarterDescending,
            PickingPattern::EighthUpDown,
        ] {
            for (beats, beat_type, expected) in [(4, 4, 24), (6, 8, 18)] {
                let mut ui = fixture();
                let d = ui
                    .music_editor
                    .drafts
                    .get_mut(&ui.working_sets.active_id)
                    .unwrap();
                let mut event = d.score.measures[0].events[0].clone();
                event.value = NoteValue::Quarter;
                d.score.measures[0].meter = TimeSignature { beats, beat_type };
                d.score.measures[0].events = vec![event; if beats == 6 { 3 } else { 4 }];
                ui.preview_music_pattern(pattern);
                let p = ui.music_editor.picking_preview.as_ref().unwrap();
                assert_eq!(
                    p.score.measures[0]
                        .events
                        .iter()
                        .map(|e| e.value.units())
                        .sum::<u32>(),
                    expected
                );
                assert_eq!(p.score.measures[0].harmony, p.baseline.measures[0].harmony);
            }
        }
    }
    #[test]
    fn preview_cancel_and_export_preserve_source_apply_is_one_undo() {
        let mut ui = fixture();
        ui.save_music();
        let before = ui.music_draft().unwrap().score.clone();
        let saved = ui.set.music.clone();
        ui.preview_music_pattern(PickingPattern::QuarterAscending);
        ui.export_music();
        assert_eq!(
            ui.music_editor.export_request.as_ref().unwrap().1,
            saved.as_ref().unwrap().musicxml
        );
        assert_eq!(ui.music_draft().unwrap().score, before);
        assert!(!ui.music_draft().unwrap().can_undo());
        ui.cancel_music_pattern();
        assert!(!ui.music_draft().unwrap().can_undo());
        ui.preview_music_pattern(PickingPattern::EighthUpDown);
        ui.apply_music_pattern();
        assert_ne!(ui.music_draft().unwrap().score, before);
        assert_eq!(ui.set.music, saved);
        ui.music_undo();
        assert_eq!(ui.music_draft().unwrap().score, before);
        assert!(!ui.music_draft().unwrap().can_undo());
    }
    #[test]
    fn incomplete_coverage_is_disclosed_and_source_preserved() {
        let mut ui = fixture();
        let owner = ui.working_sets.active_id;
        let d = ui.music_editor.drafts.get_mut(&owner).unwrap();
        d.score.measures[0].events[0].notes = (1..=6)
            .map(|string| FrettedNote { string, fret: 0 })
            .collect();
        let before = d.score.clone();
        ui.preview_music_pattern(PickingPattern::QuarterAscending);
        let p = ui.music_editor.picking_preview.as_ref().unwrap();
        assert_eq!(p.source_voicing.len(), 6);
        assert!(p.summary().contains("4 of 6 chord tones used"));
        ui.apply_music_pattern();
        assert_eq!(
            ui.music_draft().unwrap().score.measures[0].harmony,
            before.measures[0].harmony
        );
        ui.music_undo();
        assert_eq!(ui.music_draft().unwrap().score, before);
    }
    #[test]
    fn odd_eighth_span_requires_eighth_pattern_and_owner_is_frozen() {
        let mut ui = fixture();
        let owner = ui.working_sets.active_id;
        let d = ui.music_editor.drafts.get_mut(&owner).unwrap();
        let mut event = d.score.measures[0].events[0].clone();
        event.value = NoteValue::Eighth;
        d.score.measures[0].meter = TimeSignature {
            beats: 3,
            beat_type: 8,
        };
        d.score.measures[0].events = vec![event; 3];
        ui.preview_music_pattern(PickingPattern::QuarterAscending);
        assert!(ui.music_editor.picking_preview.is_none());
        ui.preview_music_pattern(PickingPattern::EighthUpDown);
        assert_eq!(
            ui.music_editor
                .picking_preview
                .as_ref()
                .unwrap()
                .score
                .measures[0]
                .events
                .len(),
            3
        );
        let before = ui.music_draft().unwrap().score.clone();
        ui.create_working_set();
        ui.apply_music_pattern();
        assert!(
            ui.music_editor.picking_preview.is_none(),
            "changing owner discards a pending proposal"
        );
        assert!(
            ui.music_draft().is_none(),
            "the new Set has no authored music"
        );
        assert_eq!(ui.music_editor.draft(owner).unwrap().score, before);
    }
    #[test]
    fn mixed_bar_wrong_span_and_stale_targets_refused() {
        let mut ui = fixture();
        ui.preview_music_pattern(PickingPattern::QuarterAscending);
        ui.select_music(1, 0);
        let before = ui.music_draft().unwrap().score.clone();
        ui.apply_music_pattern();
        assert_eq!(ui.music_draft().unwrap().score, before);
        ui.preview_music_pattern(PickingPattern::QuarterAscending);
        ui.music_tempo(1);
        let before = ui.music_draft().unwrap().score.clone();
        ui.apply_music_pattern();
        assert_eq!(ui.music_draft().unwrap().score, before);
        let d = ui
            .music_editor
            .drafts
            .get_mut(&ui.working_sets.active_id)
            .unwrap();
        d.score = Score::rhythm_example();
        d.selected = (0, 0);
        ui.preview_music_pattern(PickingPattern::QuarterAscending);
        assert!(ui.music_editor.picking_preview.is_none());
    }
}
