//! Explicit retained snapshots of this session's working Set.
//!
//! These are artifacts, not separate sessions or live aliases. Opening a copy
//! mints new working occurrences while the retained instructions stay unchanged.

use serde::{Deserialize, Serialize};
use woodshedding::rehearsal::Set;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SavedSetId(pub u64);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedSet {
    pub id: SavedSetId,
    pub name: String,
    pub set: Set,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_working_set_id: Option<crate::working_sets::WorkingSetId>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RetainedSets {
    pub entries: Vec<SavedSet>,
    next_id: u64,
}

impl RetainedSets {
    /// Save an independent instruction snapshot under a fresh artifact identity.
    pub fn save_snapshot(&mut self, set: &Set, name: impl Into<String>) -> SavedSetId {
        self.next_id = self.next_id.max(
            self.entries
                .iter()
                .map(|entry| entry.id.0)
                .max()
                .unwrap_or(0),
        );
        self.next_id = self
            .next_id
            .checked_add(1)
            .expect("saved Set identity exhausted");
        let id = SavedSetId(self.next_id);
        let name = name.into();
        self.entries.push(SavedSet {
            id,
            name: if name.trim().is_empty() {
                format!("Saved Set {}", id.0)
            } else {
                name
            },
            set: set.clone(),
            source_working_set_id: None,
        });
        id
    }

    pub fn save_snapshot_from(
        &mut self,
        set: &Set,
        name: impl Into<String>,
        owner: crate::working_sets::WorkingSetId,
    ) -> SavedSetId {
        let id = self.save_snapshot(set, name);
        self.entries
            .last_mut()
            .expect("new snapshot")
            .source_working_set_id = Some(owner);
        id
    }

    pub fn get(&self, id: SavedSetId) -> Option<&SavedSet> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    /// Replace the working contents with fresh occurrences of this snapshot.
    /// Unknown identity leaves the working Set untouched. Transport and views
    /// belong to the caller; this operation cannot start a rehearsal.
    pub fn restore_into(&self, id: SavedSetId, working: &mut Set) -> bool {
        let Some(saved) = self.get(id) else {
            return false;
        };
        working.ensure_card_ids();
        working.cards.clear();
        for card in &saved.set.cards {
            working.push(card.clone());
        }
        working.cursor = saved.set.cursor.min(working.cards.len().saturating_sub(1));
        working.loop_mode = saved.set.loop_mode;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StageState;

    #[test]
    fn snapshots_are_independent_and_open_mints_fresh_working_occurrences() {
        let mut working = Set::default();
        working.push(StageState::new().card_from_lens().unwrap());
        let original_id = working.cards[0].id;
        let original_label = working.cards[0].label.clone();
        let mut saved = RetainedSets::default();
        let first = saved.save_snapshot(&working, "First");
        let second = saved.save_snapshot(&working, "Second");
        assert_ne!(first, second);
        working.cards[0].label = "Edited working instruction".into();
        assert_eq!(saved.get(first).unwrap().set.cards[0].label, original_label);
        assert!(saved.restore_into(first, &mut working));
        assert_ne!(working.cards[0].id, original_id);
        let opened_id = working.cards[0].id;
        assert!(saved.restore_into(first, &mut working));
        assert_ne!(working.cards[0].id, opened_id);
        assert_eq!(saved.get(first).unwrap().set.cards[0].id, original_id);
        let before = serde_json::to_value(&working).unwrap();
        assert!(!saved.restore_into(SavedSetId(999), &mut working));
        assert_eq!(serde_json::to_value(&working).unwrap(), before);
    }

    #[test]
    fn restore_preserves_order_setup_timing_cursor_and_loop_without_reusing_ids() {
        use woodshedding::rehearsal::{CardId, LoopMode};
        let mut working = Set::default();
        let mut first = StageState::new().card_from_lens().unwrap();
        first.label = "First instruction".into();
        first.setting.capo = Some(2);
        first.setting.tuning = Some("Standard (high-G)".into());
        first.timing.bpm = Some(83.0);
        working.push(first);
        let mut second = StageState::new().card_from_lens().unwrap();
        second.label = "Second instruction".into();
        working.push(second);
        working.cursor = 1;
        working.loop_mode = LoopMode::All;
        let mut retained = RetainedSets::default();
        let id = retained.save_snapshot(&working, "Ordered study");
        let original = retained.get(id).unwrap().set.clone();
        working.cards.clear();
        assert!(retained.restore_into(id, &mut working));
        assert_eq!(working.cursor, 1);
        assert_eq!(working.loop_mode, original.loop_mode);
        for (opened, saved) in working.cards.iter().zip(&original.cards) {
            assert!(opened.id.0 > original.cards.last().unwrap().id.0);
            let mut opened = opened.clone();
            let mut saved = saved.clone();
            opened.id = CardId::UNASSIGNED;
            saved.id = CardId::UNASSIGNED;
            assert_eq!(
                serde_json::to_value(opened).unwrap(),
                serde_json::to_value(saved).unwrap()
            );
        }
    }

    #[test]
    fn old_empty_library_and_reopened_allocator_are_safe() {
        let empty: RetainedSets = serde_json::from_str("{}").unwrap();
        assert!(empty.entries.is_empty());
        let mut saved = RetainedSets::default();
        let first = saved.save_snapshot(&Set::default(), "");
        let mut reopened: RetainedSets =
            serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap();
        let second = reopened.save_snapshot(&Set::default(), "");
        assert!(second.0 > first.0);
        assert_eq!(reopened.get(second).unwrap().name, "Saved Set 2");
    }
}
