//! Authored musical alignment, carried with its owning Set. No host geometry
//! or playback clock belongs in these integer musical coordinates.
use super::CardId;
use crate::progression::ChordRole;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Span {
    pub start: u32,
    pub length: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Placement {
    pub lane: u16,
    pub span: Span,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardAlignment {
    pub card: CardId,
    pub placement: Option<Placement>,
    pub role: Option<usize>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArrangementRest {
    pub key: String,
    pub placement: Placement,
    pub role: Option<usize>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgressionArrangement {
    pub name: String,
    pub extent: u32,
    pub role_spans: Vec<Span>,
    pub cards: Vec<CardAlignment>,
    pub rests: Vec<ArrangementRest>,
}
/// Capture the musical template and inherited neck setup, so navigation or
/// catalog reordering cannot reinterpret the attached alignment or voicings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedProgressionArrangement {
    pub version: u16,
    pub progression: String,
    pub roles: Vec<ChordRole>,
    pub root_pc: u8,
    pub instrument: String,
    pub tuning: String,
    pub open_midi: Vec<i32>,
    pub fret_start: u8,
    pub fret_end: u8,
    pub alignment: ProgressionArrangement,
}
