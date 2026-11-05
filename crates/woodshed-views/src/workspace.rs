//! Woodshed's small workspace model over Genet's reusable tab/split core.
//!
//! The shared component owns tab and split mechanics. Woodshed owns which product surfaces
//! occupy those tabs, the stable ids that identify them, and its snapshot/restore policy. The
//! panel contents are deliberately open lanes: they are existing Woodshed views, not documents
//! or a new page system.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use workbench::{
    ContentSource, SplitAxis, TabStack, Tile, TileBranch, TileEvent, TileId, TileTree, Workbench,
    WorkbenchEffect, WorkbenchOutcome,
};

/// A Woodshed surface placed in the shared workspace.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspacePanel {
    Overview,
    Practice,
    Set,
    Related,
    Arrangement,
    Settings,
}

impl WorkspacePanel {
    pub const ALL: [Self; 6] = [
        Self::Overview,
        Self::Practice,
        Self::Set,
        Self::Related,
        Self::Arrangement,
        Self::Settings,
    ];

    pub const fn tile_id(self) -> TileId {
        match self {
            Self::Overview => TileId(0x574f_4d45),
            Self::Practice => TileId(0x574f_5052),
            Self::Set => TileId(0x574f_5345),
            Self::Related => TileId(0x574f_5245),
            Self::Arrangement => TileId(0x574f_4152),
            Self::Settings => TileId(0x574f_5347),
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Practice => "Practice",
            Self::Set => "Set",
            Self::Related => "Related",
            Self::Arrangement => "Arrangement",
            Self::Settings => "Settings",
        }
    }

    fn lane(self) -> (&'static str, &'static str) {
        match self {
            Self::Overview => ("woodshed.overview", "overview"),
            Self::Practice => ("woodshed.practice", "stage"),
            Self::Set => ("woodshed.set", "rehearsal"),
            Self::Related => ("woodshed.related", "stage"),
            Self::Arrangement => ("woodshed.arrangement", "stage"),
            Self::Settings => ("woodshed.settings", "settings"),
        }
    }

    fn tile(self) -> Tile {
        let (kind, id) = self.lane();
        Tile {
            id: self.tile_id(),
            title: self.label().to_string(),
            content: ContentSource::Open {
                kind: kind.to_string(),
                id: id.to_string(),
            },
            accent: None,
        }
    }

    fn from_tile(tile: &Tile) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|panel| panel.tile_id() == tile.id)
    }

    /// A label this panel carried in earlier saved layouts. Overview was labelled "Mere"
    /// until 2026-11-04; restore admits that title and rewrites it to the current label.
    const fn former_label(self) -> Option<&'static str> {
        match self {
            Self::Overview => Some("Mere"),
            _ => None,
        }
    }

    fn validates_tile(self, tile: &Tile) -> bool {
        let expected = self.tile();
        tile.id == expected.id
            && (tile.title == expected.title || self.former_label() == Some(tile.title.as_str()))
            && tile.content == expected.content
            && tile.accent.is_none()
    }
}

/// Give every admitted panel its current label, so a former title is never saved again.
fn relabel_panels(tree: &mut TileTree) {
    match tree {
        TileTree::Stack(stack) => {
            for tile in &mut stack.tabs {
                if let Some(panel) = WorkspacePanel::from_tile(tile) {
                    tile.title = panel.label().to_string();
                }
            }
        },
        TileTree::Split { children, .. } => {
            for child in children {
                relabel_panels(&mut child.tree);
            }
        },
    }
}

/// A typed workspace command that Woodshed routes to the reusable reducer.
#[derive(Clone, Debug, PartialEq)]
pub enum WorkspaceEvent {
    Tile(TileEvent),
}

impl WorkspaceEvent {
    pub fn activate(panel: WorkspacePanel) -> Self {
        Self::Tile(TileEvent::Activated(panel.tile_id()))
    }
}

/// A Woodshed-owned host decision requested by a workspace gesture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceEffect {
    TearOut { panel: WorkspacePanel },
}

/// Result of one typed workspace event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceOutcome {
    Activated(WorkspacePanel),
    Changed,
    Unchanged,
    Effect(WorkspaceEffect),
}

/// Woodshed's snapshot of the shared component's presentation state.
///
/// This is intentionally a host type. It captures the current tree without claiming that the
/// shared component owns Woodshed session storage or its migration policy.
#[derive(Clone, Debug, PartialEq)]
pub struct WorkspaceSnapshot {
    tree: TileTree,
    active_panel: Option<WorkspacePanel>,
}

/// Errors at Woodshed's durable workspace boundary.
#[derive(Debug)]
pub enum WorkspaceSnapshotError {
    Encode(serde_json::Error),
    Decode(serde_json::Error),
    Invalid(String),
}

impl std::fmt::Display for WorkspaceSnapshotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Encode(error) => write!(f, "could not encode Woodshed workspace: {error}"),
            Self::Decode(error) => write!(f, "could not decode Woodshed workspace: {error}"),
            Self::Invalid(reason) => write!(f, "invalid Woodshed workspace: {reason}"),
        }
    }
}

impl std::error::Error for WorkspaceSnapshotError {}

#[derive(Deserialize, Serialize)]
struct WorkspaceSnapshotDto {
    tree: WorkspaceTreeDto,
    active_panel: Option<WorkspacePanel>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum WorkspaceTreeDto {
    Split {
        axis: WorkspaceAxisDto,
        children: Vec<WorkspaceBranchDto>,
    },
    Stack {
        tabs: Vec<WorkspaceTileDto>,
        active: usize,
    },
}

#[derive(Deserialize, Serialize)]
struct WorkspaceBranchDto {
    fraction: f32,
    tree: WorkspaceTreeDto,
}

#[derive(Deserialize, Serialize)]
struct WorkspaceTileDto {
    panel: WorkspacePanel,
    id: u64,
    title: String,
    kind: String,
    content_id: String,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum WorkspaceAxisDto {
    Row,
    Column,
}

/// The host-owned workspace around Woodshed's existing views.
#[derive(Clone, Debug, PartialEq)]
pub struct WoodshedWorkspace {
    workbench: Workbench,
    /// The panel Woodshed most recently routed. A split has several stack-local active indices,
    /// so that presentation state cannot be inferred from the first tree branch.
    active_panel: Option<WorkspacePanel>,
}

impl Default for WoodshedWorkspace {
    fn default() -> Self {
        Self::new()
    }
}

impl WoodshedWorkspace {
    /// Start with the existing surfaces and a session Overview, keeping Practice selected.
    pub fn new() -> Self {
        Self {
            workbench: Workbench::new(TileTree::stack(
                WorkspacePanel::ALL
                    .into_iter()
                    .map(WorkspacePanel::tile)
                    .collect(),
                1,
            )),
            active_panel: Some(WorkspacePanel::Practice),
        }
    }

    pub fn tree(&self) -> &TileTree {
        self.workbench.tree()
    }

    pub fn active_panel(&self) -> Option<WorkspacePanel> {
        self.active_panel
    }

    pub fn snapshot(&self) -> WorkspaceSnapshot {
        WorkspaceSnapshot {
            tree: self.tree().clone(),
            active_panel: self.active_panel,
        }
    }

    pub fn restore(snapshot: WorkspaceSnapshot) -> Self {
        Self {
            workbench: Workbench::new(snapshot.tree),
            active_panel: snapshot.active_panel,
        }
    }

    /// Serialize Woodshed's allowed workspace presentation state for the non-browser host.
    ///
    /// This DTO intentionally records only the known open lanes, their stable ids, split
    /// ratios, and active indices. It does not make the shared component responsible for
    /// Woodshed's session format or accept arbitrary host content on restore.
    pub fn to_snapshot_json(&self) -> Result<String, WorkspaceSnapshotError> {
        let dto = WorkspaceSnapshotDto::from_workspace(self)?;
        serde_json::to_string(&dto).map_err(WorkspaceSnapshotError::Encode)
    }

    /// Restore a Woodshed workspace from its durable host DTO.
    ///
    /// Unknown panels, altered ids or lanes, duplicate panels, malformed active indices, and
    /// non-canonical split fractions fail closed rather than creating a new product surface.
    pub fn from_snapshot_json(json: &str) -> Result<Self, WorkspaceSnapshotError> {
        let dto = serde_json::from_str::<WorkspaceSnapshotDto>(json)
            .map_err(WorkspaceSnapshotError::Decode)?;
        let mut panels = BTreeSet::new();
        let mut tree = dto.tree.into_tree(&mut panels, true)?;
        relabel_panels(&mut tree);
        if let Some(panel) = dto.active_panel {
            if !panels.contains(&panel.tile_id().0) {
                return Err(WorkspaceSnapshotError::Invalid(format!(
                    "active panel {} is not present in the workspace",
                    panel.label()
                )));
            }
        } else if !panels.is_empty() {
            return Err(WorkspaceSnapshotError::Invalid(
                "a non-empty workspace must name its active panel".to_string(),
            ));
        }
        Ok(Self {
            workbench: Workbench::new(tree),
            active_panel: dto.active_panel,
        })
    }

    /// Add an explicitly requested surface without migrating an older saved layout.
    /// Existing split fractions, tab order and local selections remain intact.
    pub fn ensure_panel(&mut self, panel: WorkspacePanel) {
        if self.tree().find(panel.tile_id()).is_some() {
            return;
        }
        fn append(tree: &mut TileTree, tile: &Tile) -> bool {
            match tree {
                TileTree::Stack(stack) => {
                    stack.tabs.push(tile.clone());
                    true
                },
                TileTree::Split { children, .. } => children
                    .iter_mut()
                    .any(|child| append(&mut child.tree, tile)),
            }
        }
        let mut tree = self.tree().clone();
        if !append(&mut tree, &panel.tile()) {
            tree = TileTree::Stack(TabStack {
                tabs: vec![panel.tile()],
                active: 0,
            });
        }
        self.workbench = Workbench::new(tree);
    }

    /// Apply a typed gesture, retaining actual tile custody until Woodshed accepts a tear-out.
    pub fn apply(&mut self, event: WorkspaceEvent) -> WorkspaceOutcome {
        let WorkspaceEvent::Tile(event) = event;
        if !workspace_event_is_canonical(&self.workbench, &event) {
            return WorkspaceOutcome::Unchanged;
        }
        // A split has several locally active stacks. The shared reducer correctly treats
        // re-selecting a visible tab as a tree no-op, but Woodshed still needs to route that
        // deliberate selection as the host's active surface.
        let activated_panel = match event {
            TileEvent::Activated(tile) => {
                self.tree().find(tile).and_then(WorkspacePanel::from_tile)
            },
            _ => None,
        };
        match self.workbench.apply(&event) {
            WorkbenchOutcome::Applied => match event {
                TileEvent::Activated(tile) => {
                    let panel = activated_panel
                        .or_else(|| self.tree().find(tile).and_then(WorkspacePanel::from_tile));
                    self.active_panel = panel;
                    panel.map_or(WorkspaceOutcome::Changed, WorkspaceOutcome::Activated)
                },
                TileEvent::Closed(tile) => {
                    if self
                        .active_panel
                        .is_some_and(|panel| panel.tile_id() == tile)
                    {
                        self.active_panel = first_active_panel(self.tree());
                    }
                    WorkspaceOutcome::Changed
                },
                TileEvent::Dragged { .. } | TileEvent::DividerMoved { .. } => {
                    WorkspaceOutcome::Changed
                },
            },
            WorkbenchOutcome::Unchanged => {
                activated_panel.map_or(WorkspaceOutcome::Unchanged, |panel| {
                    self.active_panel = Some(panel);
                    WorkspaceOutcome::Activated(panel)
                })
            },
            WorkbenchOutcome::Effect(WorkbenchEffect::TearOut { tile }) => self
                .tree()
                .find(tile)
                .and_then(WorkspacePanel::from_tile)
                .map_or(WorkspaceOutcome::Unchanged, |panel| {
                    WorkspaceOutcome::Effect(WorkspaceEffect::TearOut { panel })
                }),
        }
    }
}

impl WorkspaceSnapshotDto {
    fn from_workspace(workspace: &WoodshedWorkspace) -> Result<Self, WorkspaceSnapshotError> {
        let mut panels = BTreeSet::new();
        WorkspaceTreeDto::validate_tree(workspace.tree(), &mut panels, true)?;
        if let Some(panel) = workspace.active_panel {
            if !panels.contains(&panel.tile_id().0) {
                return Err(WorkspaceSnapshotError::Invalid(format!(
                    "active panel {} is not present in the workspace",
                    panel.label()
                )));
            }
        } else if !panels.is_empty() {
            return Err(WorkspaceSnapshotError::Invalid(
                "a non-empty workspace must name its active panel".to_string(),
            ));
        }
        Ok(Self {
            tree: WorkspaceTreeDto::from_tree(workspace.tree())?,
            active_panel: workspace.active_panel,
        })
    }
}

fn first_active_panel(tree: &TileTree) -> Option<WorkspacePanel> {
    match tree {
        TileTree::Stack(stack) => stack
            .tabs
            .get(stack.active)
            .and_then(WorkspacePanel::from_tile),
        TileTree::Split { children, .. } => children
            .iter()
            .find_map(|branch| first_active_panel(&branch.tree)),
    }
}

/// Reject divider requests that the shared reducer permits structurally but that cannot enter
/// Woodshed's durable presentation format. The component retains generic mechanics; this host
/// owns the canonical ratio policy required to save and restore its workspace.
fn workspace_event_is_canonical(workbench: &Workbench, event: &TileEvent) -> bool {
    let TileEvent::DividerMoved { split, fractions } = event else {
        return true;
    };
    workbench
        .tree()
        .fractions_at(split)
        .is_some_and(|existing| {
            existing.len() == fractions.len()
                && fractions
                    .iter()
                    .all(|fraction| fraction.is_finite() && *fraction > 0.0)
                && (fractions.iter().sum::<f32>() - 1.0).abs() <= 0.0001
        })
}

#[cfg(test)]
#[path = "workspace/tests.rs"]
mod tests;

#[path = "workspace/tree.rs"]
mod tree;
