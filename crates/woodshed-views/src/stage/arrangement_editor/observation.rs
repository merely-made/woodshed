//! Exact observations for typed authoring. No editor, persistence or revision owner.
use super::{Draft, SavedProgressionArrangement, Set, Span, SurfaceTheme, WorkingSetId};
use std::collections::{BTreeMap, BTreeSet};
use woodshedding::rehearsal::CardId;

/// Text stays exact, including invalid/incomplete input. Caret and focus are not
/// authored state and must not dirty a draft or cancel an otherwise valid drag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct DraftSnapshot {
    context: SavedProgressionArrangement,
    fields: BTreeMap<String, String>,
    cards: Vec<CardId>,
    placed: BTreeSet<CardId>,
    rests: Vec<String>,
}
impl DraftSnapshot {
    pub(super) fn capture(draft: &Draft) -> Self {
        Self {
            context: draft.context.clone(),
            fields: draft
                .fields
                .iter()
                .map(|(k, v)| (k.clone(), v.text().into()))
                .collect(),
            cards: draft.cards.clone(),
            placed: draft.placed.clone(),
            rests: draft.rests.clone(),
        }
    }
    pub(super) fn matches(&self, draft: &Draft) -> bool {
        self.context == draft.context
            && self.cards == draft.cards
            && self.placed == draft.placed
            && self.rests == draft.rests
            && self.fields.len() == draft.fields.len()
            && self
                .fields
                .iter()
                .zip(&draft.fields)
                .all(|((key, text), (other, input))| key == other && text == input.text())
    }
}

/// One observation is used by both preview reuse and gesture validation. The
/// entire Set remains covered, including Card order/content outside alignment.
/// Retain exact encoded bytes for the Set, preserving the prior source guard
/// even for signed zero and map ordering. This is value equality, not a
/// monotonic edit revision or durable identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ArrangementObservation {
    pub(super) owner: WorkingSetId,
    source: Vec<u8>,
    draft: Option<DraftSnapshot>,
}
impl ArrangementObservation {
    pub(super) fn capture(owner: WorkingSetId, set: &Set, draft: Option<&Draft>) -> Self {
        Self {
            owner,
            source: serde_json::to_vec(set).expect("working Set serializes"),
            draft: draft.map(DraftSnapshot::capture),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct PreviewSnapshot {
    observation: ArrangementObservation,
    theme: [[u32; 4]; 7],
    gesture: Option<(String, Span)>,
}
impl PreviewSnapshot {
    pub(super) fn capture(
        observation: ArrangementObservation,
        theme: &SurfaceTheme,
        gesture: Option<(String, Span)>,
    ) -> Self {
        Self {
            observation,
            theme: [
                theme.ground,
                theme.text,
                theme.guide,
                theme.category,
                theme.root,
                theme.fifth,
                theme.warning,
            ]
            .map(|color| color.map(f32::to_bits)),
            gesture,
        }
    }
}
