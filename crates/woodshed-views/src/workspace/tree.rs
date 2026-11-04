use super::*;

impl WorkspaceTreeDto {
    pub(super) fn from_tree(tree: &TileTree) -> Result<Self, WorkspaceSnapshotError> {
        match tree {
            TileTree::Split { axis, children } => Ok(Self::Split {
                axis: match axis {
                    SplitAxis::Row => WorkspaceAxisDto::Row,
                    SplitAxis::Column => WorkspaceAxisDto::Column,
                },
                children: children
                    .iter()
                    .map(|branch| {
                        Ok(WorkspaceBranchDto {
                            fraction: branch.fraction,
                            tree: Self::from_tree(&branch.tree)?,
                        })
                    })
                    .collect::<Result<Vec<_>, WorkspaceSnapshotError>>()?,
            }),
            TileTree::Stack(stack) => Ok(Self::Stack {
                tabs: stack
                    .tabs
                    .iter()
                    .map(|tile| {
                        let panel = WorkspacePanel::from_tile(tile)
                            .expect("validated Woodshed workspace contains only known panels");
                        let ContentSource::Open { kind, id } = &tile.content else {
                            unreachable!("known Woodshed panels always use open lanes");
                        };
                        WorkspaceTileDto {
                            panel,
                            id: tile.id.0,
                            title: tile.title.clone(),
                            kind: kind.clone(),
                            content_id: id.clone(),
                        }
                    })
                    .collect(),
                active: stack.active,
            }),
        }
    }

    pub(super) fn validate_tree(
        tree: &TileTree,
        panels: &mut BTreeSet<u64>,
        is_root: bool,
    ) -> Result<(), WorkspaceSnapshotError> {
        match tree {
            TileTree::Split { children, .. } => {
                if children.len() < 2 {
                    return Err(WorkspaceSnapshotError::Invalid(
                        "a split must have at least two children".to_string(),
                    ));
                }
                let mut fraction_sum = 0.0;
                for child in children {
                    if !child.fraction.is_finite() || child.fraction <= 0.0 {
                        return Err(WorkspaceSnapshotError::Invalid(
                            "split fractions must be finite and positive".to_string(),
                        ));
                    }
                    fraction_sum += child.fraction;
                    Self::validate_tree(&child.tree, panels, false)?;
                }
                if (fraction_sum - 1.0).abs() > 0.0001 {
                    return Err(WorkspaceSnapshotError::Invalid(
                        "split fractions must sum to one".to_string(),
                    ));
                }
            },
            TileTree::Stack(stack) => {
                if stack.tabs.is_empty() && !is_root {
                    return Err(WorkspaceSnapshotError::Invalid(
                        "only the root stack may be empty".to_string(),
                    ));
                }
                if stack.active >= stack.tabs.len() && !stack.tabs.is_empty() {
                    return Err(WorkspaceSnapshotError::Invalid(
                        "active tab index is outside its stack".to_string(),
                    ));
                }
                if stack.tabs.is_empty() && stack.active != 0 {
                    return Err(WorkspaceSnapshotError::Invalid(
                        "an empty stack must have active index zero".to_string(),
                    ));
                }
                for tile in &stack.tabs {
                    let Some(panel) = WorkspacePanel::from_tile(tile) else {
                        return Err(WorkspaceSnapshotError::Invalid(
                            "an unregistered panel appears in the workspace".to_string(),
                        ));
                    };
                    if !panels.insert(tile.id.0) {
                        return Err(WorkspaceSnapshotError::Invalid(
                            "a panel appears more than once".to_string(),
                        ));
                    }
                    if !panel.validates_tile(tile) {
                        return Err(WorkspaceSnapshotError::Invalid(format!(
                            "{} does not match Woodshed's registered lane",
                            panel.label()
                        )));
                    }
                }
            },
        }
        Ok(())
    }

    pub(super) fn into_tree(
        self,
        panels: &mut BTreeSet<u64>,
        is_root: bool,
    ) -> Result<TileTree, WorkspaceSnapshotError> {
        match self {
            Self::Split { axis, children } => {
                if children.len() < 2 {
                    return Err(WorkspaceSnapshotError::Invalid(
                        "a split must have at least two children".to_string(),
                    ));
                }
                let mut fraction_sum = 0.0;
                let mut branches = Vec::with_capacity(children.len());
                for child in children {
                    if !child.fraction.is_finite() || child.fraction <= 0.0 {
                        return Err(WorkspaceSnapshotError::Invalid(
                            "split fractions must be finite and positive".to_string(),
                        ));
                    }
                    fraction_sum += child.fraction;
                    branches.push(TileBranch::new(
                        child.fraction,
                        child.tree.into_tree(panels, false)?,
                    ));
                }
                if (fraction_sum - 1.0).abs() > 0.0001 {
                    return Err(WorkspaceSnapshotError::Invalid(
                        "split fractions must sum to one".to_string(),
                    ));
                }
                Ok(TileTree::split(
                    match axis {
                        WorkspaceAxisDto::Row => SplitAxis::Row,
                        WorkspaceAxisDto::Column => SplitAxis::Column,
                    },
                    branches,
                ))
            },
            Self::Stack { tabs, active } => {
                if tabs.is_empty() && !is_root {
                    return Err(WorkspaceSnapshotError::Invalid(
                        "only the root stack may be empty".to_string(),
                    ));
                }
                if active >= tabs.len() && !tabs.is_empty() {
                    return Err(WorkspaceSnapshotError::Invalid(
                        "active tab index is outside its stack".to_string(),
                    ));
                }
                if tabs.is_empty() && active != 0 {
                    return Err(WorkspaceSnapshotError::Invalid(
                        "an empty stack must have active index zero".to_string(),
                    ));
                }
                let mut restored = Vec::with_capacity(tabs.len());
                for tile in tabs {
                    let candidate = Tile {
                        id: TileId(tile.id),
                        title: tile.title,
                        content: ContentSource::Open {
                            kind: tile.kind,
                            id: tile.content_id,
                        },
                        accent: None,
                    };
                    if !tile.panel.validates_tile(&candidate) {
                        return Err(WorkspaceSnapshotError::Invalid(format!(
                            "{} does not match Woodshed's registered lane",
                            tile.panel.label()
                        )));
                    }
                    if !panels.insert(candidate.id.0) {
                        return Err(WorkspaceSnapshotError::Invalid(
                            "a panel appears more than once".to_string(),
                        ));
                    }
                    restored.push(candidate);
                }
                Ok(TileTree::Stack(TabStack {
                    tabs: restored,
                    active,
                }))
            },
        }
    }
}
