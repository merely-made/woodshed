// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Append-only history graph with patch-based undo/redo.
//!
//! Each [`HistoryNode`] is a single edit. Edits carry enough
//! information to invert themselves (`from`/`to` field pairs, or
//! `previous` snapshots for layer operations). Undo and redo are
//! sequences of forward/inverse application; no snapshot retention
//! required.
//!
//! ## Constraints
//!
//! - **Branches are retained.** Committing from an earlier node creates a new
//!   child without deleting the existing future. Checkout moves between sibling
//!   branches through their lowest common ancestor.
//! - **No edit reconciliation yet.** A remote branch can be integrated and
//!   checked out, but Hocket does not synthesize a combined head from
//!   conflicting concurrent edits. That requires an explicit merge policy.
//! - **Phrase pool is append-only.** Inverting an `AppendLayer` does
//!   not remove the phrase from `session.phrases` — only the layer
//!   pop is undone. The pool grows monotonically.
//! - **Layers are append-only.** v0 doesn't have a `RemoveLayer`
//!   edit; "remove from playback" is `SetLayerMute(.., to: true)`.
//!   Mix-down (a future user gesture) will consolidate layers, but
//!   that operation goes via a different edit shape.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::ids::{NodeId, TrackId};
use crate::phrase::{Layer, Phrase};
use crate::session::{Session, TimeSignature};
use crate::track::{PlaybackMode, Track, TrackColor};

/// A single recorded edit. Carries its own inversion data.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Edit {
    /// Root sentinel — present once per history, at the root node.
    Genesis,

    SetBpm {
        from: f32,
        to: f32,
    },
    SetTimeSignature {
        from: TimeSignature,
        to: TimeSignature,
    },
    SetBarsPerPhrase {
        from: u8,
        to: u8,
    },
    SetMasterClock {
        from: bool,
        to: bool,
    },
    SetCountInBars {
        from: u8,
        to: u8,
    },

    /// Append a new empty track. Track creation is an edit because the track
    /// list is shared session structure, not host-local presentation state.
    AddTrack {
        track: Track,
    },

    RenameTrack {
        track_id: TrackId,
        from: String,
        to: String,
    },
    SetTrackColor {
        track_id: TrackId,
        from: TrackColor,
        to: TrackColor,
    },
    ArmTrack {
        track_id: TrackId,
        from: bool,
        to: bool,
    },
    MuteTrack {
        track_id: TrackId,
        from: bool,
        to: bool,
    },

    /// Append a new layer to a track. Apply inserts the phrase into
    /// the pool (if absent) and pushes the layer onto `Track.layers`.
    /// Invert pops the layer (layers are append-only, so the layer
    /// being inverted is always the last one). The phrase remains in
    /// the pool — monotonic-pool invariant.
    AppendLayer {
        track_id: TrackId,
        phrase: Phrase,
        layer: Layer,
    },
    /// Change a layer's gain. Apply replaces the gain; invert
    /// restores `from`.
    SetLayerGain {
        track_id: TrackId,
        layer_index: u16,
        from: f32,
        to: f32,
    },
    /// Change a layer's mute state. Apply sets `muted = to`; invert
    /// restores `from`.
    SetLayerMute {
        track_id: TrackId,
        layer_index: u16,
        from: bool,
        to: bool,
    },

    /// Change a track's playback mode (Sum vs SelectOne). This is the
    /// load-bearing edit for switching between the looper-pedal
    /// profile and the Deeler profile per-track.
    SetTrackPlaybackMode {
        track_id: TrackId,
        from: PlaybackMode,
        to: PlaybackMode,
    },
    /// Pick the currently-active layer in `SelectOne` mode (the
    /// Deeler variation-picking gesture). No-op on `Sum`-mode tracks.
    /// `from` and `to` are the active-layer indices before and after.
    SelectActiveLayer {
        track_id: TrackId,
        from: Option<u16>,
        to: Option<u16>,
    },
}

impl Edit {
    /// Apply this edit forward to the given session.
    pub fn apply(&self, session: &mut Session) {
        match self {
            Edit::Genesis => {}
            Edit::SetBpm { to, .. } => session.bpm = *to,
            Edit::SetTimeSignature { to, .. } => session.time_signature = *to,
            Edit::SetBarsPerPhrase { to, .. } => session.bars_per_phrase = *to,
            Edit::SetMasterClock { to, .. } => session.master_clock_enabled = *to,
            Edit::SetCountInBars { to, .. } => session.count_in_bars = *to,
            Edit::AddTrack { track } => session.tracks.push(track.clone()),
            Edit::RenameTrack { track_id, to, .. } => {
                if let Some(t) = session.track_mut(*track_id) {
                    t.name = to.clone();
                }
            }
            Edit::SetTrackColor { track_id, to, .. } => {
                if let Some(t) = session.track_mut(*track_id) {
                    t.color = *to;
                }
            }
            Edit::ArmTrack { track_id, to, .. } => {
                if let Some(t) = session.track_mut(*track_id) {
                    t.armed = *to;
                }
            }
            Edit::MuteTrack { track_id, to, .. } => {
                if let Some(t) = session.track_mut(*track_id) {
                    t.muted = *to;
                }
            }
            Edit::AppendLayer {
                track_id,
                phrase,
                layer,
            } => {
                session
                    .phrases
                    .entry(phrase.id)
                    .or_insert_with(|| phrase.clone());
                if let Some(t) = session.track_mut(*track_id) {
                    t.layers.push(*layer);
                }
            }
            Edit::SetLayerGain {
                track_id,
                layer_index,
                to,
                ..
            } => {
                if let Some(t) = session.track_mut(*track_id) {
                    if let Some(layer) = t.layers.get_mut(*layer_index as usize) {
                        layer.gain = *to;
                    }
                }
            }
            Edit::SetLayerMute {
                track_id,
                layer_index,
                to,
                ..
            } => {
                if let Some(t) = session.track_mut(*track_id) {
                    if let Some(layer) = t.layers.get_mut(*layer_index as usize) {
                        layer.muted = *to;
                    }
                }
            }
            Edit::SetTrackPlaybackMode { track_id, to, .. } => {
                if let Some(t) = session.track_mut(*track_id) {
                    t.playback_mode = *to;
                }
            }
            Edit::SelectActiveLayer { track_id, to, .. } => {
                if let Some(t) = session.track_mut(*track_id) {
                    if let PlaybackMode::SelectOne { active } = &mut t.playback_mode {
                        *active = *to;
                    }
                    // No-op on Sum-mode tracks (documented).
                }
            }
        }
    }

    /// Invert this edit, restoring the session to its pre-apply state.
    pub fn invert(&self, session: &mut Session) {
        match self {
            Edit::Genesis => {}
            Edit::SetBpm { from, .. } => session.bpm = *from,
            Edit::SetTimeSignature { from, .. } => session.time_signature = *from,
            Edit::SetBarsPerPhrase { from, .. } => session.bars_per_phrase = *from,
            Edit::SetMasterClock { from, .. } => session.master_clock_enabled = *from,
            Edit::SetCountInBars { from, .. } => session.count_in_bars = *from,
            Edit::AddTrack { track } => {
                if let Some(index) = session.tracks.iter().rposition(|t| t.id == track.id) {
                    session.tracks.remove(index);
                }
            }
            Edit::RenameTrack { track_id, from, .. } => {
                if let Some(t) = session.track_mut(*track_id) {
                    t.name = from.clone();
                }
            }
            Edit::SetTrackColor { track_id, from, .. } => {
                if let Some(t) = session.track_mut(*track_id) {
                    t.color = *from;
                }
            }
            Edit::ArmTrack { track_id, from, .. } => {
                if let Some(t) = session.track_mut(*track_id) {
                    t.armed = *from;
                }
            }
            Edit::MuteTrack { track_id, from, .. } => {
                if let Some(t) = session.track_mut(*track_id) {
                    t.muted = *from;
                }
            }
            Edit::AppendLayer { track_id, .. } => {
                // Layers are append-only; the layer being undone is
                // always the most recent one pushed onto this track.
                if let Some(t) = session.track_mut(*track_id) {
                    t.layers.pop();
                }
                // The phrase stays in the pool (monotonic-pool invariant).
            }
            Edit::SetLayerGain {
                track_id,
                layer_index,
                from,
                ..
            } => {
                if let Some(t) = session.track_mut(*track_id) {
                    if let Some(layer) = t.layers.get_mut(*layer_index as usize) {
                        layer.gain = *from;
                    }
                }
            }
            Edit::SetLayerMute {
                track_id,
                layer_index,
                from,
                ..
            } => {
                if let Some(t) = session.track_mut(*track_id) {
                    if let Some(layer) = t.layers.get_mut(*layer_index as usize) {
                        layer.muted = *from;
                    }
                }
            }
            Edit::SetTrackPlaybackMode { track_id, from, .. } => {
                if let Some(t) = session.track_mut(*track_id) {
                    t.playback_mode = *from;
                }
            }
            Edit::SelectActiveLayer { track_id, from, .. } => {
                if let Some(t) = session.track_mut(*track_id) {
                    if let PlaybackMode::SelectOne { active } = &mut t.playback_mode {
                        *active = *from;
                    }
                }
            }
        }
    }
}

/// A node in the history graph.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HistoryNode {
    pub id: NodeId,
    /// `None` only for the root (Genesis) node.
    pub parent: Option<NodeId>,
    pub edit: Edit,
    /// Milliseconds since Unix epoch when this edit was committed.
    /// 0 is acceptable in tests where wall-clock time doesn't matter.
    pub timestamp_ms: u64,
}

/// Errors specific to history operations.
#[derive(Debug, PartialEq, Eq)]
pub enum HistoryError {
    /// Requested checkout target is absent from the history graph.
    UnknownNode(NodeId),
    /// The designated root is absent.
    MissingRoot(NodeId),
    /// The active head is absent.
    MissingHead(NodeId),
    /// A map entry disagrees with its embedded identity.
    KeyMismatch { key: NodeId, id: NodeId },
    /// The designated root has a parent or is not Genesis.
    InvalidRoot(NodeId),
    /// A node other than the designated root is parentless.
    UnexpectedRoot(NodeId),
    /// Genesis is reserved for the designated root.
    UnexpectedGenesis(NodeId),
    /// A parent walk revisits a node instead of reaching the root.
    Cycle(NodeId),
    /// Incoming graph belongs to a different session history.
    DifferentRoot { local: NodeId, incoming: NodeId },
    /// Two graphs contain different payloads under one globally unique node id.
    ConflictingNode(NodeId),
    /// A retained node points to a parent absent from its history.
    MissingParent { node: NodeId, parent: NodeId },
}

impl std::fmt::Display for HistoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownNode(id) => write!(f, "history checkout target {id} is unknown"),
            Self::MissingRoot(id) => write!(f, "history root {id} is missing"),
            Self::MissingHead(id) => write!(f, "history head {id} is missing"),
            Self::KeyMismatch { key, id } => write!(f, "history key {key} disagrees with node {id}"),
            Self::InvalidRoot(id) => write!(f, "history root {id} must be parentless Genesis"),
            Self::UnexpectedRoot(id) => write!(f, "history node {id} is an additional root"),
            Self::UnexpectedGenesis(id) => write!(f, "history node {id} is a non-root Genesis"),
            Self::Cycle(id) => write!(f, "history contains a cycle at {id}"),
            Self::DifferentRoot { local, incoming } => {
                write!(
                    f,
                    "cannot integrate histories with roots {local} and {incoming}"
                )
            }
            Self::ConflictingNode(id) => write!(f, "history node {id} conflicts with local data"),
            Self::MissingParent { node, parent } => {
                write!(f, "history node {node} references missing parent {parent}")
            }
        }
    }
}

impl std::error::Error for HistoryError {}

/// The history graph plus current head pointer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct History {
    pub root: NodeId,
    pub head: NodeId,
    /// All live nodes. `BTreeMap` for deterministic serialization order.
    pub nodes: BTreeMap<NodeId, HistoryNode>,
}

impl History {
    /// New history with a Genesis root node.
    pub fn new() -> Self {
        let root = NodeId::new();
        let mut nodes = BTreeMap::new();
        nodes.insert(
            root,
            HistoryNode {
                id: root,
                parent: None,
                edit: Edit::Genesis,
                timestamp_ms: 0,
            },
        );
        Self {
            root,
            head: root,
            nodes,
        }
    }

    /// Validate the complete retained graph, including inactive branches.
    /// Every node must reach the single parentless Genesis root. Walks are
    /// iterative and memoized, so hostile depth cannot overflow the stack.
    pub fn validate(&self) -> Result<(), HistoryError> {
        let root = self
            .nodes
            .get(&self.root)
            .ok_or(HistoryError::MissingRoot(self.root))?;
        if !self.nodes.contains_key(&self.head) {
            return Err(HistoryError::MissingHead(self.head));
        }
        if root.parent.is_some() || !matches!(root.edit, Edit::Genesis) {
            return Err(HistoryError::InvalidRoot(self.root));
        }
        for (key, node) in &self.nodes {
            if *key != node.id {
                return Err(HistoryError::KeyMismatch {
                    key: *key,
                    id: node.id,
                });
            }
            if let Some(parent) = node.parent {
                if !self.nodes.contains_key(&parent) {
                    return Err(HistoryError::MissingParent { node: *key, parent });
                }
            } else if *key != self.root {
                return Err(HistoryError::UnexpectedRoot(*key));
            }
        }
        let mut rooted = BTreeSet::from([self.root]);
        for start in self.nodes.keys().copied() {
            let mut path = BTreeSet::new();
            let mut current = start;
            while !rooted.contains(&current) {
                if !path.insert(current) {
                    return Err(HistoryError::Cycle(current));
                }
                // Presence and the sole parentless node were checked above.
                current = self.nodes[&current]
                    .parent
                    .ok_or(HistoryError::UnexpectedRoot(current))?;
            }
            rooted.extend(path);
        }
        for (id, node) in &self.nodes {
            if *id != self.root && matches!(node.edit, Edit::Genesis) {
                return Err(HistoryError::UnexpectedGenesis(*id));
            }
        }
        Ok(())
    }

    /// Commit an edit. Applies it to the session, appends a new node pointing
    /// at the current head, and advances head. Existing descendants stay in the
    /// graph, so a commit from an earlier checkout creates a retained branch.
    pub fn commit(&mut self, edit: Edit, session: &mut Session, timestamp_ms: u64) -> NodeId {
        edit.apply(session);
        let id = NodeId::new();
        self.nodes.insert(
            id,
            HistoryNode {
                id,
                parent: Some(self.head),
                edit,
                timestamp_ms,
            },
        );
        self.head = id;
        id
    }

    /// Move the head through the two heads' lowest common ancestor. Validate
    /// both ancestry paths before changing the session; unrelated retained
    /// branches are checked by full admission and integration validation.
    pub fn checkout(&mut self, target: NodeId, session: &mut Session) -> Result<(), HistoryError> {
        let root = self.nodes.get(&self.root).ok_or(HistoryError::MissingRoot(self.root))?;
        if root.id != self.root {
            return Err(HistoryError::KeyMismatch { key: self.root, id: root.id });
        }
        if root.parent.is_some() || !matches!(root.edit, Edit::Genesis) {
            return Err(HistoryError::InvalidRoot(self.root));
        }
        if !self.nodes.contains_key(&self.head) {
            return Err(HistoryError::MissingHead(self.head));
        }
        if !self.nodes.contains_key(&target) {
            return Err(HistoryError::UnknownNode(target));
        }
        let head_chain = self.ancestor_chain(self.head)?;
        // A same-head request still checks every ancestor for public mutations.
        if self.head == target {
            return Ok(());
        }
        let target_chain = self.ancestor_chain(target)?;
        let shared = head_chain.iter().rev().zip(target_chain.iter().rev())
            .take_while(|(head, target)| head.id == target.id)
            .count();
        let head_index = head_chain.len() - shared;
        let target_index = target_chain.len() - shared;
        // Both paths terminate at the designated root, so shared is nonzero.
        for node in &head_chain[..head_index] {
            node.edit.invert(session);
        }
        for node in target_chain[..target_index].iter().rev() {
            node.edit.apply(session);
        }
        self.head = target;
        Ok(())
    }

    /// Check a path to the designated root, retaining nodes for edit application.
    /// A path cannot contain more nodes than the map: exceeding that bound
    /// proves a cycle without allocating a per-walk visited set.
    fn ancestor_chain(&self, start: NodeId) -> Result<Vec<&HistoryNode>, HistoryError> {
        let mut chain: Vec<&HistoryNode> = Vec::new();
        let mut current = start;
        loop {
            let node = self.nodes.get(&current).ok_or_else(|| {
                match chain.last() {
                    Some(previous) => HistoryError::MissingParent { node: previous.id, parent: current },
                    None => HistoryError::UnknownNode(current),
                }
            })?;
            if chain.len() == self.nodes.len() {
                return Err(HistoryError::Cycle(current));
            }
            if node.id != current {
                return Err(HistoryError::KeyMismatch { key: current, id: node.id });
            }
            if current != self.root && matches!(node.edit, Edit::Genesis) {
                return Err(HistoryError::UnexpectedGenesis(current));
            }
            chain.push(node);
            match node.parent {
                Some(parent) => current = parent,
                None if current == self.root => return Ok(chain),
                None => return Err(HistoryError::UnexpectedRoot(current)),
            }
        }
    }

    /// Integrate nodes with the same root without moving the local head.
    /// Incoming fragments may omit ancestors already present locally. Validate
    /// the complete temporary union before publishing it; refusal is atomic.
    /// Durable projects and handoffs separately require full valid snapshots.
    /// This is graph union, not semantic reconciliation of concurrent edits.
    pub fn integrate(&mut self, incoming: &History) -> Result<usize, HistoryError> {
        if self.root != incoming.root {
            return Err(HistoryError::DifferentRoot {
                local: self.root,
                incoming: incoming.root,
            });
        }
        for (id, incoming_node) in &incoming.nodes {
            if let Some(local_node) = self.nodes.get(id) {
                if local_node != incoming_node {
                    return Err(HistoryError::ConflictingNode(*id));
                }
            }
        }
        let mut combined = self.clone();
        combined.nodes.extend(incoming.nodes.clone());
        combined.validate()?;
        if !combined.nodes.contains_key(&incoming.head) {
            return Err(HistoryError::MissingHead(incoming.head));
        }
        let added = combined.nodes.len() - self.nodes.len();
        *self = combined;
        Ok(added)
    }

    /// Whether there's an edit to undo (head is past the root).
    pub fn can_undo(&self) -> bool {
        self.nodes.get(&self.head).and_then(|n| n.parent).is_some()
    }

    /// Whether there's an edit to redo (head has one or more child nodes).
    pub fn can_redo(&self) -> bool {
        self.nodes.values().any(|n| n.parent == Some(self.head))
    }

    /// Undo the last edit: move head to its parent, inverting the edit
    /// against `session`. Returns `false` (no-op) if already at the
    /// root.
    pub fn undo(&mut self, session: &mut Session) -> bool {
        let Some(parent) = self.nodes.get(&self.head).and_then(|n| n.parent) else {
            return false;
        };
        self.checkout(parent, session).is_ok()
    }

    /// Redo: move head to the first child in deterministic `NodeId` order.
    /// Hosts that expose branch choice should call [`Self::checkout`] with the
    /// chosen child instead.
    pub fn redo(&mut self, session: &mut Session) -> bool {
        let Some(child) = self
            .nodes
            .values()
            .find(|n| n.parent == Some(self.head))
            .map(|n| n.id)
        else {
            return false;
        };
        self.checkout(child, session).is_ok()
    }
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::MediaRef;

    fn malformed_histories() -> Vec<(&'static str, History)> {
        let mut session = Session::new_default();
        let mut base = History::new();
        let first = base.commit(Edit::SetBpm { from: 120.0, to: 90.0 }, &mut session, 1);
        let second = base.commit(Edit::SetBpm { from: 90.0, to: 80.0 }, &mut session, 2);
        let mut cases = Vec::new();
        for case in ["self cycle", "two node cycle", "missing root", "missing head",
            "root parent", "root edit", "extra root", "key mismatch", "missing parent",
            "non-root genesis", "disconnected cycle"] {
            let mut h = base.clone();
            match case {
                "self cycle" => h.nodes.get_mut(&second).unwrap().parent = Some(second),
                "two node cycle" => h.nodes.get_mut(&first).unwrap().parent = Some(second),
                "missing root" => h.root = NodeId::new(),
                "missing head" => h.head = NodeId::new(),
                "root parent" => h.nodes.get_mut(&h.root).unwrap().parent = Some(second),
                "root edit" => h.nodes.get_mut(&h.root).unwrap().edit = Edit::SetBpm { from: 120.0, to: 1.0 },
                "extra root" => h.nodes.get_mut(&first).unwrap().parent = None,
                "key mismatch" => h.nodes.get_mut(&first).unwrap().id = NodeId::new(),
                "missing parent" => h.nodes.get_mut(&first).unwrap().parent = Some(NodeId::new()),
                "non-root genesis" => h.nodes.get_mut(&first).unwrap().edit = Edit::Genesis,
                "disconnected cycle" => {
                    let id = NodeId::new();
                    h.nodes.insert(id, HistoryNode { id, parent: Some(id),
                        edit: Edit::SetBpm { from: 120.0, to: 1.0 }, timestamp_ms: 3 });
                }
                _ => unreachable!(),
            }
            cases.push((case, h));
        }
        cases
    }

    #[test]
    fn malformed_topology_is_refused_before_checkout_or_integration_mutation() {
        for (case, mut malformed) in malformed_histories() {
            assert!(malformed.validate().is_err(), "{case}");
            let bundle = crate::ProjectBundle::new(Session::new_default(), malformed.clone());
            let mut bytes = Vec::new();
            ciborium::into_writer(&bundle, &mut bytes).unwrap();
            assert!(matches!(crate::ProjectBundle::from_bytes(&bytes),
                Err(crate::PersistenceError::History(_))), "{case}");
            let before = malformed.clone();
            let mut session = Session::new_default();
            let before_session = session.clone();
            if case == "disconnected cycle" {
                // Unrelated corruption is refused by admission/integration;
                // checkout validates only the paths it will traverse.
                malformed.checkout(malformed.head, &mut session).unwrap();
                let cyclic = malformed.nodes.values()
                    .find(|node| node.parent == Some(node.id)).unwrap().id;
                assert!(malformed.checkout(cyclic, &mut session).is_err());
                malformed.head = cyclic;
                assert!(malformed.checkout(cyclic, &mut session).is_err());
                malformed.head = before.head;
            } else {
                assert!(malformed.checkout(malformed.head, &mut session).is_err(), "{case}");
                assert!(malformed.checkout(malformed.root, &mut session).is_err(), "{case}");
            }
            assert_eq!(session, before_session, "{case}");
            assert_eq!(malformed, before, "{case}");
            // Same root exercises admission rather than DifferentRoot refusal.
            let mut local = History {
                root: malformed.root,
                head: malformed.root,
                nodes: BTreeMap::from([(malformed.root, HistoryNode {
                    id: malformed.root,
                    parent: None,
                    edit: Edit::Genesis,
                    timestamp_ms: 0,
                })]),
            };
            let local_before = local.clone();
            assert!(local.integrate(&malformed).is_err(), "{case}");
            assert_eq!(local, local_before, "{case}");
            assert!(malformed.integrate(&local).is_err(), "{case}");
            assert_eq!(malformed, before, "{case}");
        }
    }

    #[test]
    fn valid_deep_history_and_retained_branches_are_rooted() {
        let mut h = History::new();
        let mut s = Session::new_default();
        for i in 0..2048 {
            h.commit(Edit::SetBpm { from: s.bpm, to: 90.0 }, &mut s, i);
        }
        let retained = h.head;
        h.checkout(h.root, &mut s).unwrap();
        h.commit(Edit::SetBpm { from: s.bpm, to: 80.0 }, &mut s, 2049);
        h.validate().unwrap();
        h.checkout(retained, &mut s).unwrap();
        assert_eq!(s.bpm, 90.0);
        assert_eq!(h.nodes.len(), 2050);
    }

    #[test]
    fn new_history_has_only_root() {
        let h = History::new();
        assert_eq!(h.head, h.root);
        assert_eq!(h.nodes.len(), 1);
        assert!(matches!(h.nodes[&h.root].edit, Edit::Genesis));
        assert_eq!(h.nodes[&h.root].parent, None);
    }

    #[test]
    fn commit_advances_head_and_applies() {
        let mut s = Session::new_default();
        let mut h = History::new();
        let n1 = h.commit(
            Edit::SetBpm {
                from: s.bpm,
                to: 90.0,
            },
            &mut s,
            0,
        );
        assert_eq!(h.head, n1);
        assert_eq!(s.bpm, 90.0);
        assert_eq!(h.nodes.len(), 2);
    }

    #[test]
    fn undo_redo_round_trips() {
        let mut s = Session::new_default();
        let mut h = History::new();
        assert!(!h.can_undo() && !h.can_redo());

        h.commit(
            Edit::SetBpm {
                from: 120.0,
                to: 90.0,
            },
            &mut s,
            0,
        );
        assert_eq!(s.bpm, 90.0);
        assert!(h.can_undo() && !h.can_redo());

        assert!(h.undo(&mut s));
        assert_eq!(s.bpm, 120.0);
        assert!(!h.can_undo() && h.can_redo());

        assert!(h.redo(&mut s));
        assert_eq!(s.bpm, 90.0);

        // Undo back to root, then a further undo is a no-op.
        assert!(h.undo(&mut s));
        assert_eq!(s.bpm, 120.0);
        assert!(!h.undo(&mut s)); // already at root
        assert!(h.can_redo()); // but the edit is still redoable
    }

    #[test]
    fn checkout_back_inverts() {
        let mut s = Session::new_default();
        let mut h = History::new();
        h.commit(
            Edit::SetBpm {
                from: 120.0,
                to: 90.0,
            },
            &mut s,
            0,
        );
        assert_eq!(s.bpm, 90.0);
        h.checkout(h.root, &mut s).unwrap();
        assert_eq!(s.bpm, 120.0);
        assert_eq!(h.head, h.root);
    }

    #[test]
    fn checkout_forward_applies() {
        let mut s = Session::new_default();
        let mut h = History::new();
        let n1 = h.commit(
            Edit::SetBpm {
                from: 120.0,
                to: 90.0,
            },
            &mut s,
            0,
        );
        h.checkout(h.root, &mut s).unwrap();
        assert_eq!(s.bpm, 120.0);
        h.checkout(n1, &mut s).unwrap();
        assert_eq!(s.bpm, 90.0);
    }

    #[test]
    fn commit_after_checkout_back_retains_future_branch() {
        let mut s = Session::new_default();
        let mut h = History::new();
        let _n1 = h.commit(
            Edit::SetBpm {
                from: 120.0,
                to: 90.0,
            },
            &mut s,
            0,
        );
        let n2 = h.commit(
            Edit::SetBpm {
                from: 90.0,
                to: 60.0,
            },
            &mut s,
            0,
        );
        assert_eq!(h.nodes.len(), 3);

        h.checkout(h.root, &mut s).unwrap();
        let _n3 = h.commit(
            Edit::SetBpm {
                from: 120.0,
                to: 100.0,
            },
            &mut s,
            0,
        );

        assert!(h.nodes.contains_key(&n2));
        assert_eq!(s.bpm, 100.0);
    }

    #[test]
    fn checkout_moves_between_retained_sibling_branches() {
        let mut s = Session::new_default();
        let mut h = History::new();
        let root = h.root;
        let first = h.commit(
            Edit::SetBpm {
                from: 120.0,
                to: 90.0,
            },
            &mut s,
            1,
        );
        h.checkout(root, &mut s).unwrap();
        let second = h.commit(
            Edit::SetBpm {
                from: 120.0,
                to: 100.0,
            },
            &mut s,
            2,
        );

        h.checkout(first, &mut s).unwrap();
        assert_eq!(s.bpm, 90.0);
        h.checkout(second, &mut s).unwrap();
        assert_eq!(s.bpm, 100.0);
        assert_eq!(h.nodes.len(), 3);
    }

    #[test]
    fn integrate_retains_remote_branch_without_moving_the_local_head() {
        let mut shared_session = Session::new_default();
        let mut local = History::new();
        let shared = local.commit(
            Edit::SetBpm {
                from: 120.0,
                to: 110.0,
            },
            &mut shared_session,
            1,
        );
        let mut remote_session = shared_session.clone();
        let mut remote = local.clone();
        let local_head = local.commit(
            Edit::SetBpm {
                from: 110.0,
                to: 90.0,
            },
            &mut shared_session,
            2,
        );
        let remote_head = remote.commit(
            Edit::SetBpm {
                from: 110.0,
                to: 100.0,
            },
            &mut remote_session,
            3,
        );

        assert_eq!(local.integrate(&remote).unwrap(), 1);
        assert_eq!(local.head, local_head);
        assert!(local.nodes.contains_key(&shared));
        assert!(local.nodes.contains_key(&remote_head));
        local.checkout(remote_head, &mut shared_session).unwrap();
        assert_eq!(shared_session.bpm, 100.0);
    }

    #[test]
    fn integrate_accepts_fragments_using_local_ancestors_without_moving_head() {
        let mut session = Session::new_default();
        let mut local = History::new();
        local.commit(Edit::SetBpm { from: 120.0, to: 110.0 }, &mut session, 1);
        let local_head = local.head;
        let mut remote = local.clone();
        let mut remote_session = session.clone();
        let first = remote.commit(Edit::SetBpm { from: 110.0, to: 90.0 }, &mut remote_session, 2);
        let second = remote.commit(Edit::SetBpm { from: 90.0, to: 80.0 }, &mut remote_session, 3);
        remote.nodes.retain(|id, _| *id == first || *id == second);
        assert!(remote.validate().is_err());
        // A graph fragment is not a valid durable snapshot.
        assert!(crate::ProjectBundle::new(remote_session, remote.clone()).to_bytes().is_err());
        assert_eq!(local.integrate(&remote).unwrap(), 2);
        assert_eq!(local.head, local_head);
        assert_eq!(session.bpm, 110.0);
        local.validate().unwrap();
        assert_eq!(local.integrate(&remote).unwrap(), 0);
        local.checkout(second, &mut session).unwrap();
        assert_eq!(session.bpm, 80.0);
        local.checkout(local_head, &mut session).unwrap();
        assert_eq!(session.bpm, 110.0);
    }

    #[test]
    fn invalid_fragment_union_is_atomic_even_with_a_valid_new_node() {
        for case in ["self cycle", "two node cycle", "missing parent", "extra root",
            "key mismatch", "missing head", "conflict"] {
            let mut local = History::new();
            let before = local.clone();
            let good = NodeId::new();
            let bad = NodeId::new();
            let mut fragment = History { root: local.root, head: good, nodes: BTreeMap::new() };
            fragment.nodes.insert(good, HistoryNode { id: good, parent: Some(local.root),
                edit: Edit::SetBpm { from: 120.0, to: 90.0 }, timestamp_ms: 1 });
            fragment.nodes.insert(bad, HistoryNode { id: bad, parent: Some(good),
                edit: Edit::SetBpm { from: 90.0, to: 80.0 }, timestamp_ms: 2 });
            match case {
                "self cycle" => fragment.nodes.get_mut(&bad).unwrap().parent = Some(bad),
                "two node cycle" => fragment.nodes.get_mut(&good).unwrap().parent = Some(bad),
                "missing parent" => fragment.nodes.get_mut(&bad).unwrap().parent = Some(NodeId::new()),
                "extra root" => fragment.nodes.get_mut(&bad).unwrap().parent = None,
                "key mismatch" => fragment.nodes.get_mut(&bad).unwrap().id = NodeId::new(),
                "missing head" => fragment.head = NodeId::new(),
                "conflict" => {
                    let mut root = local.nodes[&local.root].clone();
                    root.timestamp_ms = 99;
                    fragment.nodes.insert(local.root, root);
                }
                _ => unreachable!(),
            }
            assert!(local.integrate(&fragment).is_err(), "{case}");
            assert_eq!(local, before, "{case}");
        }
    }

    #[test]
    fn integrate_rejects_an_incompatible_graph() {
        let mut local = History::new();
        let other_root = History::new();
        assert!(matches!(
            local.integrate(&other_root),
            Err(HistoryError::DifferentRoot { .. })
        ));

        let mut conflicting = local.clone();
        conflicting
            .nodes
            .get_mut(&conflicting.root)
            .unwrap()
            .timestamp_ms = 1;
        assert!(matches!(
            local.integrate(&conflicting),
            Err(HistoryError::ConflictingNode(_))
        ));

        let mut missing_parent = local.clone();
        let orphan = NodeId::new();
        let absent_parent = NodeId::new();
        missing_parent.nodes.insert(
            orphan,
            HistoryNode {
                id: orphan,
                parent: Some(absent_parent),
                edit: Edit::Genesis,
                timestamp_ms: 0,
            },
        );
        assert!(matches!(
            local.integrate(&missing_parent),
            Err(HistoryError::MissingParent {
                node,
                parent
            }) if node == orphan && parent == absent_parent
        ));
    }

    #[test]
    fn append_layer_round_trip() {
        let mut s = Session::new_default();
        let mut h = History::new();
        let track_id = s.tracks[0].id;
        let phrase = Phrase::new(MediaRef([1; 32]), 4, 120.0, 1000);
        let phrase_id = phrase.id;
        let layer = Layer::new(phrase_id);

        let n1 = h.commit(
            Edit::AppendLayer {
                track_id,
                phrase,
                layer,
            },
            &mut s,
            0,
        );

        assert_eq!(s.tracks[0].layers.len(), 1);
        assert_eq!(s.tracks[0].layers[0].phrase_id, phrase_id);
        assert!(s.phrases.contains_key(&phrase_id));

        // Step back: layer popped, phrase stays in pool.
        h.checkout(h.root, &mut s).unwrap();
        assert!(s.tracks[0].layers.is_empty());
        assert!(
            s.phrases.contains_key(&phrase_id),
            "phrase should remain in the pool across undo"
        );

        // Step forward: layer pushed back.
        h.checkout(n1, &mut s).unwrap();
        assert_eq!(s.tracks[0].layers.len(), 1);
        assert_eq!(s.tracks[0].layers[0].phrase_id, phrase_id);
    }

    #[test]
    fn two_appends_then_mute_one() {
        let mut s = Session::new_default();
        let mut h = History::new();
        let track_id = s.tracks[0].id;

        let phrase_a = Phrase::new(MediaRef([1; 32]), 4, 120.0, 1000);
        let layer_a = Layer::new(phrase_a.id);
        h.commit(
            Edit::AppendLayer {
                track_id,
                phrase: phrase_a,
                layer: layer_a,
            },
            &mut s,
            0,
        );

        let phrase_b = Phrase::new(MediaRef([2; 32]), 4, 120.0, 2000);
        let layer_b = Layer::new(phrase_b.id);
        h.commit(
            Edit::AppendLayer {
                track_id,
                phrase: phrase_b,
                layer: layer_b,
            },
            &mut s,
            0,
        );

        assert_eq!(s.tracks[0].layers.len(), 2);

        // Mute layer 0 (the first one).
        h.commit(
            Edit::SetLayerMute {
                track_id,
                layer_index: 0,
                from: false,
                to: true,
            },
            &mut s,
            0,
        );
        assert!(s.tracks[0].layers[0].muted);
        assert!(!s.tracks[0].layers[1].muted);

        // Undo the mute.
        h.checkout(
            h.nodes
                .values()
                .find(|n| {
                    matches!(
                        n.edit,
                        Edit::AppendLayer { ref phrase, .. } if phrase.captured_at_ms == 2000
                    )
                })
                .unwrap()
                .id,
            &mut s,
        )
        .unwrap();
        assert!(!s.tracks[0].layers[0].muted);
    }

    #[test]
    fn set_layer_gain_round_trip() {
        let mut s = Session::new_default();
        let mut h = History::new();
        let track_id = s.tracks[0].id;

        let phrase = Phrase::new(MediaRef([1; 32]), 4, 120.0, 1000);
        let layer = Layer::new(phrase.id);
        h.commit(
            Edit::AppendLayer {
                track_id,
                phrase,
                layer,
            },
            &mut s,
            0,
        );

        let after_append = h.head;

        h.commit(
            Edit::SetLayerGain {
                track_id,
                layer_index: 0,
                from: 1.0,
                to: 0.5,
            },
            &mut s,
            0,
        );
        assert_eq!(s.tracks[0].layers[0].gain, 0.5);

        h.checkout(after_append, &mut s).unwrap();
        assert_eq!(s.tracks[0].layers[0].gain, 1.0);
    }

    #[test]
    fn set_track_playback_mode_round_trip() {
        let mut s = Session::new_default();
        let mut h = History::new();
        let track_id = s.tracks[0].id;
        assert_eq!(s.tracks[0].playback_mode, PlaybackMode::Sum);

        let after = h.commit(
            Edit::SetTrackPlaybackMode {
                track_id,
                from: PlaybackMode::Sum,
                to: PlaybackMode::SelectOne { active: None },
            },
            &mut s,
            0,
        );
        assert_eq!(
            s.tracks[0].playback_mode,
            PlaybackMode::SelectOne { active: None }
        );

        // Undo: back to Sum.
        h.checkout(h.root, &mut s).unwrap();
        assert_eq!(s.tracks[0].playback_mode, PlaybackMode::Sum);

        // Redo: back to SelectOne.
        h.checkout(after, &mut s).unwrap();
        assert_eq!(
            s.tracks[0].playback_mode,
            PlaybackMode::SelectOne { active: None }
        );
    }

    #[test]
    fn add_track_round_trip() {
        let mut s = Session::new_default();
        let mut h = History::new();
        let track = Track::new_with_mode(
            "track 5",
            TrackColor::from_palette_index(4),
            s.default_playback_mode,
        );
        let id = track.id;

        let added = h.commit(Edit::AddTrack { track }, &mut s, 0);
        assert_eq!(s.tracks.len(), 5);
        assert_eq!(s.tracks.last().map(|t| t.id), Some(id));

        h.checkout(h.root, &mut s).unwrap();
        assert_eq!(s.tracks.len(), 4);
        assert!(s.track(id).is_none());

        h.checkout(added, &mut s).unwrap();
        assert_eq!(s.tracks.len(), 5);
        assert_eq!(s.tracks.last().map(|t| t.id), Some(id));
    }

    #[test]
    fn select_active_layer_round_trip() {
        let mut s = Session::new_default();
        let mut h = History::new();
        let track_id = s.tracks[0].id;

        // Switch the track into SelectOne mode first.
        h.commit(
            Edit::SetTrackPlaybackMode {
                track_id,
                from: PlaybackMode::Sum,
                to: PlaybackMode::SelectOne { active: None },
            },
            &mut s,
            0,
        );
        let before_select = h.head;

        let after_select = h.commit(
            Edit::SelectActiveLayer {
                track_id,
                from: None,
                to: Some(2),
            },
            &mut s,
            0,
        );
        assert_eq!(
            s.tracks[0].playback_mode,
            PlaybackMode::SelectOne { active: Some(2) }
        );

        // Undo: back to None.
        h.checkout(before_select, &mut s).unwrap();
        assert_eq!(
            s.tracks[0].playback_mode,
            PlaybackMode::SelectOne { active: None }
        );

        // Redo: back to Some(2).
        h.checkout(after_select, &mut s).unwrap();
        assert_eq!(
            s.tracks[0].playback_mode,
            PlaybackMode::SelectOne { active: Some(2) }
        );
    }

    #[test]
    fn select_active_layer_on_sum_track_is_noop() {
        // SelectActiveLayer applied to a Sum-mode track does nothing
        // (documented behavior); the track stays in Sum mode.
        let mut s = Session::new_default();
        let mut h = History::new();
        let track_id = s.tracks[0].id;

        h.commit(
            Edit::SelectActiveLayer {
                track_id,
                from: None,
                to: Some(2),
            },
            &mut s,
            0,
        );
        assert_eq!(s.tracks[0].playback_mode, PlaybackMode::Sum);
    }

    #[test]
    fn unknown_target_yields_unknown_node() {
        let mut s = Session::new_default();
        let mut h = History::new();
        h.commit(
            Edit::SetBpm {
                from: 120.0,
                to: 90.0,
            },
            &mut s,
            0,
        );
        let bogus = NodeId::new();
        assert_eq!(
            h.checkout(bogus, &mut s),
            Err(HistoryError::UnknownNode(bogus))
        );
    }
}
