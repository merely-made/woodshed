//! Multiple editable Sets. The active contents stay in the existing working
//! Set slot; this bank stores only inactive contents and active identity.
use serde::{Deserialize, Serialize};
use woodshedding::rehearsal::Set;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct WorkingSetId(pub u64);
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkingSet {
    pub id: WorkingSetId,
    pub name: String,
    pub set: Set,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkingSets {
    pub active_id: WorkingSetId,
    pub active_name: String,
    pub inactive: Vec<WorkingSet>,
    next_id: u64,
}
impl Default for WorkingSets {
    fn default() -> Self {
        Self {
            active_id: WorkingSetId(1),
            active_name: "Working Set 1".into(),
            inactive: Vec::new(),
            next_id: 1,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkingSetSummary {
    pub id: WorkingSetId,
    pub name: String,
    pub card_count: usize,
    pub active: bool,
}
impl WorkingSets {
    fn mint_id(&mut self) -> WorkingSetId {
        self.next_id = self.next_id.max(self.active_id.0).max(
            self.inactive
                .iter()
                .map(|entry| entry.id.0)
                .max()
                .unwrap_or(0),
        );
        self.next_id = self
            .next_id
            .checked_add(1)
            .expect("working Set identity exhausted");
        WorkingSetId(self.next_id)
    }
    pub fn create(&mut self, current: &mut Set, name: impl Into<String>, set: Set) -> WorkingSetId {
        let id = self.mint_id();
        let name = name.into();
        self.inactive.push(WorkingSet {
            id,
            name: if name.trim().is_empty() {
                format!("Working Set {}", id.0)
            } else {
                name
            },
            set,
        });
        assert!(self.activate(id, current));
        id
    }
    /// Navigation swaps owners, preserving every occurrence and transport cursor.
    /// An unknown identity leaves both owners untouched.
    pub fn activate(&mut self, id: WorkingSetId, current: &mut Set) -> bool {
        if id == self.active_id {
            return true;
        }
        let Some(index) = self.inactive.iter().position(|entry| entry.id == id) else {
            return false;
        };
        let target = self.inactive.remove(index);
        let previous = std::mem::replace(current, target.set);
        self.inactive.push(WorkingSet {
            id: self.active_id,
            name: std::mem::replace(&mut self.active_name, target.name),
            set: previous,
        });
        self.active_id = id;
        true
    }
    pub fn get<'a>(&'a self, id: WorkingSetId, current: &'a Set) -> Option<&'a Set> {
        if id == self.active_id {
            Some(current)
        } else {
            self.inactive
                .iter()
                .find(|entry| entry.id == id)
                .map(|entry| &entry.set)
        }
    }
    pub fn get_mut<'a>(
        &'a mut self,
        id: WorkingSetId,
        current: &'a mut Set,
    ) -> Option<&'a mut Set> {
        if id == self.active_id {
            Some(current)
        } else {
            self.inactive
                .iter_mut()
                .find(|entry| entry.id == id)
                .map(|entry| &mut entry.set)
        }
    }
    pub fn summaries(&self, current: &Set) -> Vec<WorkingSetSummary> {
        let mut summaries = vec![WorkingSetSummary {
            id: self.active_id,
            name: self.active_name.clone(),
            card_count: current.cards.len(),
            active: true,
        }];
        summaries.extend(self.inactive.iter().map(|entry| WorkingSetSummary {
            id: entry.id,
            name: entry.name.clone(),
            card_count: entry.set.cards.len(),
            active: false,
        }));
        summaries.sort_by_key(|entry| entry.id);
        summaries
    }
    /// Duplication is an explicit copy; its Card occurrences receive fresh IDs.
    pub fn duplicate(
        &mut self,
        id: WorkingSetId,
        current: &mut Set,
        name: impl Into<String>,
    ) -> Option<WorkingSetId> {
        let source = self.get(id, current)?.clone();
        let mut copy = source.clone();
        copy.ensure_card_ids();
        copy.cards.clear();
        for card in source.cards {
            copy.push(card);
        }
        Some(self.create(current, name, copy))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::StageState;
    #[test]
    fn switching_preserves_independent_edits_and_background_owner() {
        let mut active = Set::default();
        active.push(StageState::new().card_from_lens().unwrap());
        let first_id = active.cards[0].id;
        let mut bank = WorkingSets::default();
        let first = bank.active_id;
        let second = bank.create(&mut active, "Second", Set::default());
        assert!(active.cards.is_empty());
        bank.get_mut(first, &mut active).unwrap().cards[0].label = "Background edit".into();
        assert!(bank.activate(first, &mut active));
        assert_eq!(active.cards[0].id, first_id);
        assert_eq!(active.cards[0].label, "Background edit");
        assert!(bank.get(second, &active).unwrap().cards.is_empty());
        let before = (
            serde_json::to_value(&bank).unwrap(),
            serde_json::to_value(&active).unwrap(),
        );
        assert!(!bank.activate(WorkingSetId(999), &mut active));
        assert_eq!(
            before,
            (
                serde_json::to_value(&bank).unwrap(),
                serde_json::to_value(&active).unwrap()
            )
        );
    }
    #[test]
    fn duplicate_is_fresh_and_old_files_have_one_existing_owner() {
        let mut active = Set::default();
        active.push(StageState::new().card_from_lens().unwrap());
        let old = active.cards[0].id;
        let mut bank: WorkingSets = serde_json::from_str("{}").unwrap();
        assert_eq!(bank.active_id, WorkingSetId(1));
        assert!(bank.inactive.is_empty());
        let original = bank.active_id;
        let duplicate = bank.duplicate(original, &mut active, "Copy").unwrap();
        assert_ne!(active.cards[0].id, old);
        assert_eq!(bank.get(original, &active).unwrap().cards[0].id, old);
        let mut bank: WorkingSets =
            serde_json::from_str(&serde_json::to_string(&bank).unwrap()).unwrap();
        let third = bank.create(&mut active, "", Set::default());
        assert!(third.0 > duplicate.0);
    }
}
