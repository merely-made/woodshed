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
            Self::KeyMismatch { key, id } => {
                write!(f, "history key {key} disagrees with node {id}")
            },
            Self::InvalidRoot(id) => write!(f, "history root {id} must be parentless Genesis"),
            Self::UnexpectedRoot(id) => write!(f, "history node {id} is an additional root"),
            Self::UnexpectedGenesis(id) => write!(f, "history node {id} is a non-root Genesis"),
            Self::Cycle(id) => write!(f, "history contains a cycle at {id}"),
            Self::DifferentRoot { local, incoming } => {
                write!(
                    f,
                    "cannot integrate histories with roots {local} and {incoming}"
                )
            },
            Self::ConflictingNode(id) => write!(f, "history node {id} conflicts with local data"),
            Self::MissingParent { node, parent } => {
                write!(f, "history node {node} references missing parent {parent}")
            },
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
        let root = self
            .nodes
            .get(&self.root)
            .ok_or(HistoryError::MissingRoot(self.root))?;
        if root.id != self.root {
            return Err(HistoryError::KeyMismatch {
                key: self.root,
                id: root.id,
            });
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
        let shared = head_chain
            .iter()
            .rev()
            .zip(target_chain.iter().rev())
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
            let node = self.nodes.get(&current).ok_or_else(|| match chain.last() {
                Some(previous) => HistoryError::MissingParent {
                    node: previous.id,
                    parent: current,
                },
                None => HistoryError::UnknownNode(current),
            })?;
            if chain.len() == self.nodes.len() {
                return Err(HistoryError::Cycle(current));
            }
            if node.id != current {
                return Err(HistoryError::KeyMismatch {
                    key: current,
                    id: node.id,
                });
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
mod tests;

mod edit;
