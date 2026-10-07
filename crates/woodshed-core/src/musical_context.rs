//! Explicitly kept musical context. Keeping material associates it with a Set
//! owner, without adding a Card or claiming harmonic derivation.
use crate::{harmony::KeyedCatalogRef, working_sets::WorkingSetId};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeSet;

pub const MAX_CONTEXT_ITEMS: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ContextItemId(pub u64);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MusicalContextItem {
    pub id: ContextItemId,
    pub owner: WorkingSetId,
    pub subject: KeyedCatalogRef,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct MusicalContext {
    entries: Vec<MusicalContextItem>,
    next_id: u64,
}

impl<'de> Deserialize<'de> for MusicalContext {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Default, Deserialize)]
        #[serde(default)]
        struct Wire {
            entries: Vec<MusicalContextItem>,
            next_id: u64,
        }
        let wire = Wire::deserialize(deserializer)?;
        Ok(Self {
            entries: wire.entries,
            next_id: wire.next_id,
        }
        .normalized())
    }
}

impl MusicalContextItem {
    pub fn available(&self) -> bool {
        is_supported_subject(&self.subject)
    }
}

pub fn is_supported_subject(subject: &KeyedCatalogRef) -> bool {
    (subject.formula_id.starts_with("chord:") || subject.formula_id.starts_with("scale:"))
        && subject.to_material().is_some()
}

impl MusicalContext {
    /// Normalize imported structure while retaining stale catalog references.
    /// First occurrences win; malformed duplicate/zero IDs and over-capacity
    /// entries cannot create ambiguous targets or an unbounded scene.
    pub fn normalized(&self) -> Self {
        let mut identities = BTreeSet::new();
        let mut subjects = BTreeSet::new();
        let entries: Vec<_> = self
            .entries
            .iter()
            .filter(|item| {
                item.id.0 != 0
                    && identities.insert(item.id)
                    && subjects.insert((item.owner, item.subject.clone()))
            })
            .take(MAX_CONTEXT_ITEMS)
            .cloned()
            .collect();
        let next_id = self
            .next_id
            .max(self.entries.iter().map(|item| item.id.0).max().unwrap_or(0));
        Self { entries, next_id }
    }

    pub fn items(&self) -> &[MusicalContextItem] {
        &self.entries
    }

    /// Refuse ambiguous imported occurrence identities rather than acting on
    /// whichever entry happens to be first.
    pub fn get(&self, id: ContextItemId) -> Option<&MusicalContextItem> {
        let mut matches = self.entries.iter().filter(|item| item.id == id);
        let item = matches.next()?;
        matches.next().is_none().then_some(item)
    }

    pub fn keep(
        &mut self,
        owner: WorkingSetId,
        subject: KeyedCatalogRef,
    ) -> Result<ContextItemId, String> {
        if !is_supported_subject(&subject) {
            return Err("Only an available catalog chord or scale can be kept nearby.".into());
        }
        if let Some(item) = self
            .entries
            .iter()
            .find(|item| item.owner == owner && item.subject == subject)
        {
            return self
                .get(item.id)
                .map(|item| item.id)
                .ok_or_else(|| "The retained context identity is ambiguous.".into());
        }
        if self.entries.len() >= MAX_CONTEXT_ITEMS {
            return Err(
                "Nearby material is full (12 items). Remove an item before keeping another.".into(),
            );
        }
        let next_id = self
            .next_id
            .max(self.entries.iter().map(|item| item.id.0).max().unwrap_or(0))
            .checked_add(1)
            .ok_or("Nearby material identity exhausted.")?;
        self.next_id = next_id;
        let id = ContextItemId(next_id);
        self.entries.push(MusicalContextItem { id, owner, subject });
        Ok(id)
    }

    pub fn remove(&mut self, id: ContextItemId) -> bool {
        let before = self.entries.len();
        self.entries.retain(|item| item.id != id);
        before != self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use woodshedding::pitch::PitchClass;
    fn chord(root: u8) -> KeyedCatalogRef {
        KeyedCatalogRef {
            formula_id: "chord:Major".into(),
            root: PitchClass::new(root),
        }
    }
    #[test]
    fn imported_structure_is_bounded_unique_and_keeps_allocator_history() {
        let mut entries = Vec::new();
        for index in 1..=20 {
            entries.push(serde_json::json!({"id":index,"owner":index,"subject":{"formula_id":"chord:Deleted", "root":0}}));
        }
        entries.insert(0, entries[0].clone());
        entries.insert(
            0,
            serde_json::json!({"id":0,"owner":1,"subject":{"formula_id":"chord:Major", "root":0}}),
        );
        let mut reopened: MusicalContext =
            serde_json::from_value(serde_json::json!({"entries":entries})).unwrap();
        assert_eq!(reopened.items().len(), MAX_CONTEXT_ITEMS);
        assert!(reopened.items().iter().all(|entry| !entry.available()));
        let ids: BTreeSet<_> = reopened.items().iter().map(|entry| entry.id).collect();
        assert_eq!(ids.len(), MAX_CONTEXT_ITEMS);
        reopened.remove(ContextItemId(1));
        assert_eq!(
            reopened.keep(WorkingSetId(1), chord(0)).unwrap(),
            ContextItemId(21)
        );
    }

    #[test]
    fn identity_deduplication_owner_and_capacity() {
        let mut context = MusicalContext::default();
        let first = context.keep(WorkingSetId(1), chord(0)).unwrap();
        assert_eq!(context.keep(WorkingSetId(1), chord(0)).unwrap(), first);
        let other_owner = context.keep(WorkingSetId(2), chord(0)).unwrap();
        assert_ne!(first, other_owner);
        for root in 1..=10 {
            context.keep(WorkingSetId(1), chord(root)).unwrap();
        }
        assert!(context.keep(WorkingSetId(1), chord(11)).is_err());
        assert_eq!(context.keep(WorkingSetId(1), chord(0)).unwrap(), first);
        assert!(context.remove(first));
        let later = context.keep(WorkingSetId(1), chord(11)).unwrap();
        assert!(later.0 > other_owner.0);
        assert_eq!(context.items().len(), MAX_CONTEXT_ITEMS);
    }
    #[test]
    fn plain_valid_material_only_and_stale_payload_survives_roundtrip() {
        let mut context = MusicalContext::default();
        let mut subject = chord(0);
        subject.formula_id = "arpeggio:Major".into();
        assert!(context.keep(WorkingSetId(1), subject).is_err());
        context.keep(WorkingSetId(1), chord(0)).unwrap();
        let mut value = serde_json::to_value(&context).unwrap();
        value["entries"][0]["subject"]["formula_id"] = "chord:Deleted formula".into();
        let reopened: MusicalContext = serde_json::from_value(value.clone()).unwrap();
        assert!(reopened.items()[0].subject.to_material().is_none());
        assert_eq!(serde_json::to_value(reopened).unwrap(), value);
        assert!(
            serde_json::from_str::<MusicalContext>("{}")
                .unwrap()
                .items()
                .is_empty()
        );
    }
    #[test]
    fn imported_high_identity_is_not_reused() {
        let context = MusicalContext {
            entries: vec![MusicalContextItem {
                id: ContextItemId(90),
                owner: WorkingSetId(1),
                subject: chord(0),
            }],
            next_id: 0,
        };
        let mut reopened: MusicalContext =
            serde_json::from_str(&serde_json::to_string(&context).unwrap()).unwrap();
        assert_eq!(
            reopened.keep(WorkingSetId(1), chord(1)).unwrap(),
            ContextItemId(91)
        );
        let mut ambiguous = reopened.clone();
        ambiguous.entries.push(ambiguous.entries[0].clone());
        assert!(ambiguous.get(ContextItemId(90)).is_none());
    }
}
