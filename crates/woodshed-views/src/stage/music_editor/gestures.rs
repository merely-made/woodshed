//! Captured fretboard gestures. Preview is presentation state; only release
//! commits one ordinary musical edit. The native host clicks on press before
//! pointer Down, so the parent routes pointer clicks here and handles keyboard
//! button activation explicitly rather than editing from both routes.
use super::{FrettedNote, Score, UiState, WorkingSetId, music_width};
use cambium::{PointerButton, PointerEvent, PointerPhase};

pub(super) struct FretGesture {
    owner: WorkingSetId,
    selected: (usize, usize),
    baseline: Score,
    pointer: i32,
    size: (f32, f32),
    board_width: f32,
    origin_px: (f32, f32),
    origin: (u8, u8),
    first: u8,
    note: Option<usize>,
    proposed: Option<(u8, u8)>,
    moved: bool,
}
impl FretGesture {
    fn cell(&self, point: (f32, f32)) -> Option<(u8, u8)> {
        if !point.0.is_finite()
            || !point.1.is_finite()
            || point.0 < 24.0
            || point.0 >= self.size.0
            || point.1 < 0.0
            || point.1 >= self.baseline.tuning.len() as f32 * 27.0
        {
            return None;
        }
        let cell = (self.size.0 - 24.0) / 9.0;
        let fret = self
            .first
            .checked_add(((point.0 - 24.0) / cell).floor() as u8)?;
        if fret > 36 {
            return None;
        }
        Some(((point.1 / 27.0).floor() as u8 + 1, fret))
    }
    fn current(&self, ui: &UiState) -> bool {
        ui.working_sets.active_id == self.owner
            && (music_width(ui).min(760.0) - self.board_width).abs() < 0.5
            && ui
                .music_draft()
                .is_some_and(|d| d.selected == self.selected && d.score == self.baseline)
    }
}
impl UiState {
    pub fn cancel_music_drag(&mut self) {
        self.music_editor.fret_gesture = None;
    }
    /// Proposed drag dot; the authored event and its undo history stay intact.
    pub fn music_drag_position(&self) -> Option<(u8, u8)> {
        let g = self.music_editor.fret_gesture.as_ref()?;
        if g.note.is_some() && g.moved && g.current(self) {
            g.proposed
        } else {
            None
        }
    }
    pub fn music_board_pointer(&mut self, event: PointerEvent) {
        if event.button != PointerButton::Primary {
            return;
        }
        if event.phase == PointerPhase::Down {
            if self.music_editor.fret_gesture.is_some() {
                return;
            }
            let Some(d) = self.music_draft() else {
                return;
            };
            let (b, n) = d.selected;
            let e = &d.score.measures[b].events[n];
            let max = e.notes.iter().map(|p| p.fret).max().unwrap_or(0);
            let first = if max > 8 { max.saturating_sub(7) } else { 0 };
            let width = music_width(self).min(760.0);
            if !event.size.0.is_finite()
                || !event.size.1.is_finite()
                || event.size.0 <= 24.0
                || event.size.1 <= 0.0
                || (event.size.0 - width).abs() > 0.5
            {
                return;
            }
            let mut g = FretGesture {
                owner: self.working_sets.active_id,
                selected: d.selected,
                baseline: d.score.clone(),
                pointer: event.pointer_id,
                size: event.size,
                board_width: width,
                origin_px: event.local,
                origin: (1, 0),
                first,
                note: None,
                proposed: None,
                moved: false,
            };
            let Some(origin) = g.cell(event.local) else {
                return;
            };
            g.origin = origin;
            g.proposed = Some(origin);
            g.note = e
                .notes
                .iter()
                .position(|p| p.string == origin.0 && p.fret == origin.1);
            event.prop.prevent_default();
            self.music_editor.active_tone = Some(FrettedNote {
                string: origin.0,
                fret: origin.1,
            });
            self.music_editor.fret_gesture = Some(g);
            return;
        }
        let Some(g) = self.music_editor.fret_gesture.as_ref() else {
            return;
        };
        if g.pointer != event.pointer_id {
            return;
        }
        event.prop.prevent_default();
        if event.phase == PointerPhase::Cancel {
            self.cancel_music_drag();
            return;
        }
        if !g.current(self)
            || !event.size.0.is_finite()
            || !event.size.1.is_finite()
            || (g.size.0 - event.size.0).abs() > 0.5
            || (g.size.1 - event.size.1).abs() > 0.5
        {
            self.cancel_music_drag();
            return;
        }
        let g = self.music_editor.fret_gesture.as_mut().unwrap();
        let dx = event.local.0 - g.origin_px.0;
        let dy = event.local.1 - g.origin_px.1;
        g.moved |= dx * dx + dy * dy >= 16.0;
        g.proposed = g.cell(event.local);
        if event.phase != PointerPhase::Up {
            return;
        }
        let g = self.music_editor.fret_gesture.take().unwrap();
        let Some(target) = g.proposed else {
            return;
        };
        if let Some(note) = g.note {
            if !g.moved || target == g.origin {
                return;
            }
            self.music_editor.active_tone = Some(FrettedNote {
                string: target.0,
                fret: target.1,
            });
            self.edit_music(move |score,(b,n)| {
                let e=&mut score.measures[b].events[n];
                if e.notes.iter().enumerate().any(|(i,p)|i!=note && p.string==target.0) {
                    return Err("That string already has a note. Move to an unused string, or remove its note first.".into());
                }
                e.notes[note]=FrettedNote{string:target.0,fret:target.1};
                Ok(())
            },"Note moved; rhythm and written chord preserved.");
        } else if !g.moved && target == g.origin {
            self.music_fret(target.0, target.1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ui() -> UiState {
        let mut ui = UiState::new();
        ui.viewport_width = 800.0;
        ui.music_example(true);
        ui
    }
    fn pointer(ui: &UiState, phase: PointerPhase, string: u8, fret: u8) -> PointerEvent {
        let d = ui.music_draft().unwrap();
        let e = &d.score.measures[d.selected.0].events[d.selected.1];
        let max = e.notes.iter().map(|n| n.fret).max().unwrap_or(0);
        let first = if max > 8 { max - 7 } else { 0 };
        let width = music_width(ui).min(760.0);
        let cell = (width - 24.0) / 9.0;
        PointerEvent::new(
            phase,
            (
                24.0 + (f32::from(fret - first) + 0.5) * cell,
                (f32::from(string) - 0.5) * 27.0,
            ),
            (width, d.score.tuning.len() as f32 * 27.0 + 26.0),
        )
    }
    #[test]
    fn preview_cancel_and_one_release_undo() {
        let mut ui = ui();
        let before = ui.music_draft().unwrap().score.clone();
        ui.music_board_pointer(pointer(&ui, PointerPhase::Down, 2, 1));
        ui.music_board_pointer(pointer(&ui, PointerPhase::Move, 1, 4));
        assert_eq!(ui.music_drag_position(), Some((1, 4)));
        assert_eq!(ui.music_draft().unwrap().score, before);
        assert!(!ui.music_draft().unwrap().can_undo());
        ui.cancel_music_drag();
        assert_eq!(ui.music_drag_position(), None);
        assert_eq!(ui.music_draft().unwrap().score, before);
        ui.music_board_pointer(pointer(&ui, PointerPhase::Down, 2, 1));
        ui.music_board_pointer(pointer(&ui, PointerPhase::Move, 1, 4));
        let up = pointer(&ui, PointerPhase::Up, 1, 4);
        let prop = up.prop.clone();
        ui.music_board_pointer(up);
        assert!(prop.default_prevented());
        assert_eq!(
            ui.music_draft().unwrap().score.measures[0].events[0].notes,
            vec![FrettedNote { string: 1, fret: 4 }]
        );
        ui.music_undo();
        assert_eq!(ui.music_draft().unwrap().score, before);
        assert!(!ui.music_draft().unwrap().can_undo());
    }
    #[test]
    fn resized_stale_and_owner_changes_cannot_commit() {
        for guard in 0..4 {
            let mut ui = ui();
            let before = ui.music_draft().unwrap().score.clone();
            let owner = ui.working_sets.active_id;
            ui.music_board_pointer(pointer(&ui, PointerPhase::Down, 2, 1));
            let mut up = pointer(&ui, PointerPhase::Up, 1, 4);
            match guard {
                0 => up.size.0 -= 10.0,
                1 => ui.music_editor.drafts.get_mut(&owner).unwrap().score.tempo += 1,
                2 => ui.working_sets.active_id = WorkingSetId(owner.0 + 1),
                _ => ui.music_editor.drafts.get_mut(&owner).unwrap().selected = (0, 1),
            }
            ui.music_board_pointer(up);
            assert!(ui.music_editor.fret_gesture.is_none());
            let actual = &ui.music_editor.drafts[&owner].score;
            assert_eq!(actual.measures, before.measures);
            assert!(!ui.music_editor.drafts[&owner].can_undo());
        }
    }
    #[test]
    fn pointer_identity_guards_and_keyboard_click_route_remains_available() {
        let mut ui = ui();
        let before = ui.music_draft().unwrap().score.clone();
        ui.music_board_pointer(pointer(&ui, PointerPhase::Down, 2, 1));
        let wrong = pointer(&ui, PointerPhase::Up, 1, 4).with_pointer_id(19);
        ui.music_board_pointer(wrong);
        assert!(ui.music_editor.fret_gesture.is_some());
        assert_eq!(ui.music_draft().unwrap().score, before);
        ui.cancel_music_drag();
        ui.music_fret(1, 4);
        assert!(
            ui.music_draft().unwrap().score.measures[0].events[0]
                .notes
                .contains(&FrettedNote { string: 1, fret: 4 })
        );
    }
    #[test]
    fn empty_click_adds_only_on_release_and_outside_drop_never_snaps() {
        let mut ui = ui();
        let before = ui.music_draft().unwrap().score.clone();
        ui.music_board_pointer(pointer(&ui, PointerPhase::Down, 1, 4));
        assert_eq!(ui.music_draft().unwrap().score, before);
        ui.music_board_pointer(pointer(&ui, PointerPhase::Up, 1, 4));
        assert!(
            ui.music_draft().unwrap().score.measures[0].events[0]
                .notes
                .contains(&FrettedNote { string: 1, fret: 4 })
        );
        ui.music_undo();
        assert_eq!(ui.music_draft().unwrap().score, before);
        ui.music_board_pointer(pointer(&ui, PointerPhase::Down, 2, 1));
        let mut up = pointer(&ui, PointerPhase::Up, 1, 4);
        up.local.0 = up.size.0 + 1.0;
        ui.music_board_pointer(up);
        assert_eq!(ui.music_draft().unwrap().score, before);
        assert!(ui.music_drag_position().is_none());
    }
    #[test]
    fn drop_on_occupied_string_refuses_without_removing_another_note() {
        let mut ui = UiState::new();
        ui.viewport_width = 800.0;
        ui.music_example(false);
        let before = ui.music_draft().unwrap().score.clone();
        let original = before.measures[0].events[0]
            .notes
            .iter()
            .find(|n| n.string == 2)
            .unwrap();
        ui.music_board_pointer(pointer(&ui, PointerPhase::Down, 2, original.fret));
        ui.music_board_pointer(pointer(&ui, PointerPhase::Up, 1, 4));
        assert_eq!(ui.music_draft().unwrap().score, before);
        assert!(!ui.music_draft().unwrap().can_undo());
        assert!(
            ui.music_editor
                .notice
                .as_deref()
                .unwrap()
                .contains("already has a note")
        );
    }
}
