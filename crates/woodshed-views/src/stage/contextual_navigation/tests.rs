use super::*;

fn instance_node(owner: WorkingSetId) -> OverviewNodeId {
    OverviewNodeId::Artifact(SessionArtifactId::WorkingSetInstance(owner))
}
fn two_sets_with_the_same_occurrence() -> (UiState, WorkingSetId, WorkingSetId, CardId, CardId) {
    let mut ui = UiState::new();
    ui.stage_current(None);
    let first = ui.working_sets.active_id;
    let first_card = ui.set.cards[0].id;
    let same_material = ui.set.clone();
    let second = ui
        .working_sets
        .create(&mut ui.set, "Same material", same_material);
    let second_card = ui.set.cards[0].id;
    assert_eq!(
        first_card, second_card,
        "the Set owner qualifies an occurrence even when Card IDs coincide"
    );
    (ui, first, second, first_card, second_card)
}
fn open_owner(ui: &mut UiState, owner: WorkingSetId) {
    ui.activate_workspace_panel(WorkspacePanel::Overview);
    ui.overview_focus = Some(instance_node(owner));
    ui.open_overview_node(instance_node(owner));
}

#[test]
fn up_and_down_move_between_overview_set_and_card_without_changing_runner_cursor() {
    let (mut ui, owner, _, card, _) = two_sets_with_the_same_occurrence();
    open_owner(&mut ui, owner);
    assert_eq!(ui.workspace.active_panel(), Some(WorkspacePanel::Set));
    let runner_cursor = ui.set.cursor_id();
    ui.remember_card_scope(card);
    ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Down));
    assert_eq!(
        ui.scope_navigation.current().unwrap().place,
        WoodshedScopePlace::CardOccurrence { owner, card }
    );
    assert_eq!(ui.workspace.active_panel(), Some(WorkspacePanel::Practice));
    assert_eq!(
        ui.set.cursor_id(),
        runner_cursor,
        "Card navigation must not seek the Set runner"
    );
    ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Up));
    assert_eq!(
        ui.scope_navigation.current().unwrap().place,
        WoodshedScopePlace::WorkingSet {
            owner,
            panel: WorkspacePanel::Set
        }
    );
    ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Up));
    assert_eq!(
        ui.scope_navigation.current().unwrap().place,
        WoodshedScopePlace::Overview
    );
    assert_eq!(ui.workspace.active_panel(), Some(WorkspacePanel::Overview));
    ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Down));
    assert_eq!(
        ui.scope_navigation.current().unwrap().place,
        WoodshedScopePlace::WorkingSet {
            owner,
            panel: WorkspacePanel::Set
        }
    );
    ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Down));
    assert_eq!(
        ui.scope_navigation.current().unwrap().place,
        WoodshedScopePlace::CardOccurrence { owner, card }
    );
}

#[test]
fn back_and_forward_revisit_owner_card_occurrences_independently_of_scope_depth() {
    let (mut ui, first, second, first_card, second_card) = two_sets_with_the_same_occurrence();
    open_owner(&mut ui, first);
    ui.remember_card_scope(first_card);
    ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Down));
    assert_eq!(
        ui.scope_navigation.current().unwrap().place,
        WoodshedScopePlace::CardOccurrence {
            owner: first,
            card: first_card
        }
    );
    assert!(ui.visit_scope(
        WoodshedScopePlace::CardOccurrence {
            owner: second,
            card: second_card
        },
        Some(WoodshedScopeAnchor::CardOccurrence {
            owner: second,
            card: second_card
        }),
    ));
    assert_eq!(ui.working_sets.active_id, second);
    ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Back));
    assert_eq!(ui.working_sets.active_id, first);
    assert_eq!(
        ui.scope_navigation.current().unwrap().place,
        WoodshedScopePlace::CardOccurrence {
            owner: first,
            card: first_card
        }
    );
    ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Forward));
    assert_eq!(ui.working_sets.active_id, second);
    assert_eq!(
        ui.scope_navigation.current().unwrap().place,
        WoodshedScopePlace::CardOccurrence {
            owner: second,
            card: second_card
        }
    );
}

#[test]
fn edits_made_to_an_inactive_set_survive_return_navigation() {
    let (mut ui, first, second, _, _) = two_sets_with_the_same_occurrence();
    open_owner(&mut ui, first);
    assert!(ui.visit_scope(
        WoodshedScopePlace::WorkingSet {
            owner: second,
            panel: WorkspacePanel::Set
        },
        None,
    ));
    let inactive = ui.working_sets.get_mut(first, &mut ui.set).unwrap();
    inactive.cards[0].label = "Edited while away".into();
    assert!(ui.visit_scope(
        WoodshedScopePlace::WorkingSet {
            owner: first,
            panel: WorkspacePanel::Set
        },
        None,
    ));
    assert_eq!(ui.working_sets.active_id, first);
    assert_eq!(ui.set.cards[0].label, "Edited while away");
}

#[test]
fn history_restores_the_original_practice_panel_and_graph_camera() {
    let (mut ui, owner, _, _, _) = two_sets_with_the_same_occurrence();
    open_owner(&mut ui, owner);
    ui.activate_workspace_panel(WorkspacePanel::Practice);
    ui.set_graph_viewport = cambium::GraphViewport {
        pan: (0.23, -0.17),
        zoom: 1.4,
    };

    assert!(ui.visit_scope(
        WoodshedScopePlace::Overview,
        Some(WoodshedScopeAnchor::OverviewNode(instance_node(owner))),
    ));
    ui.set_graph_viewport = cambium::GraphViewport::default();
    ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Back));

    assert_eq!(ui.workspace.active_panel(), Some(WorkspacePanel::Practice));
    assert_eq!(
        ui.set_graph_viewport,
        cambium::GraphViewport {
            pan: (0.23, -0.17),
            zoom: 1.4,
        }
    );
    assert_eq!(
        ui.scope_navigation.current().unwrap().place,
        WoodshedScopePlace::WorkingSet {
            owner,
            panel: WorkspacePanel::Practice,
        }
    );
}

#[test]
fn reopened_card_history_restores_the_exact_overview_camera_bookmark() {
    let (mut ui, first, second, first_card, second_card) = two_sets_with_the_same_occurrence();
    open_owner(&mut ui, first);
    ui.remember_card_scope(first_card);
    ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Down));
    ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Up));
    ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Up));
    ui.overview_viewport = cambium::GraphViewport {
        pan: (0.16, -0.12),
        zoom: 1.1,
    };
    assert!(ui.visit_scope(
        WoodshedScopePlace::WorkingSet {
            owner: second,
            panel: WorkspacePanel::Set,
        },
        Some(WoodshedScopeAnchor::OverviewNode(instance_node(second))),
    ));
    ui.remember_card_scope(second_card);
    ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Down));
    ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Up));
    ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Up));
    ui.overview_viewport = cambium::GraphViewport {
        pan: (0.31, -0.22),
        zoom: 1.25,
    };
    ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Down));
    ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Down));

    let persisted = ui.to_persisted();
    let mut reopened = UiState::new();
    assert!(
        reopened
            .apply_persisted(&persisted, ui.app_settings.clone())
            .is_none()
    );
    assert_eq!(reopened.working_sets.active_id, second);
    assert_eq!(
        reopened.workspace.active_panel(),
        Some(WorkspacePanel::Practice)
    );
    assert_eq!(reopened.overview_viewport.zoom, 1.25);

    reopened.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Back));
    reopened.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Back));
    assert_eq!(
        reopened.workspace.active_panel(),
        Some(WorkspacePanel::Overview)
    );
    assert_eq!(
        reopened.overview_viewport,
        cambium::GraphViewport {
            pan: (0.31, -0.22),
            zoom: 1.25,
        }
    );
    assert_eq!(
        reopened.scope_navigation.current().unwrap().place,
        WoodshedScopePlace::Overview
    );

    for _ in 0..10 {
        if reopened.scope_navigation.current().is_some_and(|visit| {
            visit.place == WoodshedScopePlace::Overview
                && visit.anchor == Some(WoodshedScopeAnchor::OverviewNode(instance_node(first)))
        }) {
            break;
        }
        reopened.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Back));
    }
    assert_eq!(
        reopened.workspace.active_panel(),
        Some(WorkspacePanel::Overview)
    );
    assert_eq!(
        reopened.scope_navigation.current().unwrap().anchor,
        Some(WoodshedScopeAnchor::OverviewNode(instance_node(first)))
    );
    assert_eq!(
        reopened.overview_viewport,
        cambium::GraphViewport {
            pan: (0.16, -0.12),
            zoom: 1.1,
        }
    );
    assert_eq!(
        reopened.scope_navigation.current().unwrap().place,
        WoodshedScopePlace::Overview
    );
}

#[test]
fn session_save_reopens_contextual_history_and_owner_qualified_card_route() {
    let (mut ui, first, second, first_card, second_card) = two_sets_with_the_same_occurrence();
    open_owner(&mut ui, first);
    ui.remember_card_scope(first_card);
    ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Down));
    assert!(ui.visit_scope(
        WoodshedScopePlace::CardOccurrence {
            owner: second,
            card: second_card,
        },
        Some(WoodshedScopeAnchor::CardOccurrence {
            owner: second,
            card: second_card,
        }),
    ));
    ui.set_graph_viewport = cambium::GraphViewport {
        pan: (0.31, -0.22),
        zoom: 1.6,
    };
    ui.music_example(false);
    ui.practice_music();
    assert_eq!(ui.music_editor.practice.owner(), Some(second));

    let saved = ui.to_persisted();
    assert!(saved.scope_navigation_json.is_some());
    let encoded = serde_json::to_string(&saved).unwrap();
    let reopened_session = woodshed_core::storage::decode_session(&encoded)
        .unwrap()
        .session;
    let mut reopened = UiState::new();
    assert!(
        reopened
            .apply_persisted(&reopened_session, ui.app_settings.clone())
            .is_none()
    );

    assert_eq!(reopened.working_sets.active_id, second);
    assert!(reopened.music_editor.practice.owner().is_none());
    assert_eq!(
        reopened.workspace.active_panel(),
        Some(WorkspacePanel::Practice)
    );
    assert_eq!(
        reopened.scope_navigation.current().unwrap().place,
        WoodshedScopePlace::CardOccurrence {
            owner: second,
            card: second_card,
        }
    );
    assert_eq!(
        reopened.set_graph_viewport,
        cambium::GraphViewport {
            pan: (0.31, -0.22),
            zoom: 1.6,
        }
    );
    reopened.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Back));
    assert_eq!(reopened.working_sets.active_id, first);
    assert_eq!(
        reopened.scope_navigation.current().unwrap().place,
        WoodshedScopePlace::CardOccurrence {
            owner: first,
            card: first_card,
        }
    );
}

#[test]
fn corrupt_stale_and_legacy_scope_snapshots_fail_closed() {
    let legacy = woodshed_core::storage::PersistedSession::default();
    assert!(legacy.scope_navigation_json.is_none());
    let mut reopened = UiState::new();
    reopened.apply_persisted(&legacy, Default::default());
    assert!(reopened.scope_navigation.current().is_none());
    assert!(reopened.scope_navigation_notice.is_none());

    let (mut ui, owner, _, card, _) = two_sets_with_the_same_occurrence();
    open_owner(&mut ui, owner);
    ui.remember_card_scope(card);
    ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Down));

    let mut corrupt = ui.to_persisted();
    corrupt.scope_navigation_json = Some("{".into());
    let mut reopened = UiState::new();
    reopened.apply_persisted(&corrupt, ui.app_settings.clone());
    assert!(reopened.scope_navigation.current().is_none());
    assert!(
        reopened
            .scope_navigation_notice
            .as_deref()
            .is_some_and(|notice| notice.contains("malformed"))
    );
    assert_eq!(
        reopened.to_persisted().scope_navigation_json.as_deref(),
        Some("{")
    );
    assert!(reopened.visit_scope(
        WoodshedScopePlace::WorkingSet {
            owner: ui.working_sets.active_id,
            panel: WorkspacePanel::Set,
        },
        None,
    ));
    assert!(reopened.scope_navigation_notice.is_none());
    assert_ne!(
        reopened.to_persisted().scope_navigation_json.as_deref(),
        Some("{")
    );

    let mut stale = ui.to_persisted();
    stale.set.cards.clear();
    let stale_navigation_json = stale.scope_navigation_json.clone().unwrap();
    let mut reopened = UiState::new();
    reopened.apply_persisted(&stale, ui.app_settings.clone());
    assert!(reopened.scope_navigation.current().is_none());
    assert_eq!(
        reopened.to_persisted().scope_navigation_json.as_deref(),
        Some(stale_navigation_json.as_str())
    );
    assert!(
        reopened
            .scope_navigation_notice
            .as_deref()
            .is_some_and(|notice| notice.contains("missing material"))
    );
}

#[test]
fn navigation_preserves_the_music_practice_owner_transport_and_capture_state() {
    let (mut ui, owner, other, _, _) = two_sets_with_the_same_occurrence();
    assert!(ui.working_sets.activate(owner, &mut ui.set));
    ui.music_example(false);
    ui.practice_music();
    let practice_owner = ui.music_editor.practice.owner();
    let practice_status = ui.music_editor.practice.status();
    let practice_epoch = ui.music_editor.practice.run_epoch();
    assert_eq!(practice_owner, Some(owner));
    ui.song_playing = true;
    ui.song_recording = true;
    ui.rehearsal_running = true;
    ui.rehearsal_owner = Some(owner);
    let transport = ui.transport.clone();
    let captures = ui.loop_captures.clone();
    let audio_requests = ui.audio_requests.clone();

    open_owner(&mut ui, other);

    assert_eq!(ui.working_sets.active_id, other);
    assert_eq!(ui.music_editor.practice.owner(), practice_owner);
    assert_eq!(ui.music_editor.practice.status(), practice_status);
    assert_eq!(ui.music_editor.practice.run_epoch(), practice_epoch);
    assert_eq!(ui.rehearsal_owner, Some(owner));
    assert!(ui.rehearsal_running);
    assert!(ui.song_playing && ui.song_recording);
    assert_eq!(ui.transport, transport);
    assert_eq!(ui.loop_captures, captures);
    assert_eq!(ui.audio_requests, audio_requests);
    assert_eq!(ui.workspace.active_panel(), Some(WorkspacePanel::Set));
    assert_ne!(ui.rehearsal_owner, Some(ui.working_sets.active_id));
    assert_eq!(ui.music_editor.practice.owner(), Some(owner));
}

#[test]
fn stale_history_destination_does_not_move_the_route_or_open_a_replacement() {
    let (mut ui, first, second, first_card, second_card) = two_sets_with_the_same_occurrence();
    open_owner(&mut ui, first);
    ui.remember_card_scope(first_card);
    ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Down));
    assert!(ui.visit_scope(
        WoodshedScopePlace::CardOccurrence {
            owner: second,
            card: second_card
        },
        Some(WoodshedScopeAnchor::CardOccurrence {
            owner: second,
            card: second_card
        }),
    ));
    let destination_owner = ui.working_sets.active_id;
    let current_place = ui.scope_navigation.current().unwrap().place.clone();
    ui.working_sets
        .get_mut(first, &mut ui.set)
        .unwrap()
        .cards
        .clear();
    ui.handle_scope_command(ScopeCommand::Navigate(ScopeDirection::Back));
    assert_eq!(ui.working_sets.active_id, destination_owner);
    assert_eq!(ui.scope_navigation.current().unwrap().place, current_place);
    assert!(
        matches!(current_place, WoodshedScopePlace::CardOccurrence { owner, .. } if owner == second)
    );
}
