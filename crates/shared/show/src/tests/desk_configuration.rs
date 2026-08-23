use super::temporary;
use crate::{
    DeskStore, FixedScreenFixtureColumn, FixedScreenFixtureCompactMode,
    FixedScreenFixtureIncludedHeads, FixedScreenFixtureOrder, FixedScreenPane, FixedScreenSide,
    FixedScreenStageRenderQuality, PlaybackSurfaceLayout, PlaybackSurfaceRow,
    ProgrammerControlSurfaceConfiguration, ScreenConfiguration, ScreenContent,
};
use light_core::ShowId;
use rusqlite::Connection;
use std::fs;
use uuid::Uuid;

#[test]
fn screens_persist_and_keep_independent_pages_per_show() {
    let path = temporary("screens");
    let store = DeskStore::open(&path).unwrap();
    let show = ShowId::new();
    let id = Uuid::new_v4();
    let screen = ScreenConfiguration {
        not_editable: false,
        id,
        name: "Wing".into(),
        layout: serde_json::json!({"desks":[],"activeDeskId":""}),
        show_dock: false,
        show_playbacks: true,
        playback_count: 12,
        playback_rows: 2,
        first_playback_slot: 20,
        page_mode: "independent".into(),
        show_page_controls: false,
        show_programmer: false,
        desired_open: true,
        display_id: Some("display".into()),
        bounds: Some(serde_json::json!({"x":1,"y":2,"width":800,"height":600})),
        fullscreen: true,
        playback_layout: Some(PlaybackSurfaceLayout {
            playbacks_per_row: 6,
            rows: vec![
                PlaybackSurfaceRow {
                    first_playback_slot: 20,
                    has_fader: false,
                    button_count: 1,
                },
                PlaybackSurfaceRow {
                    first_playback_slot: 40,
                    has_fader: true,
                    button_count: 3,
                },
            ],
        }),
        content: ScreenContent::Desktop,
    };
    store.put_screen(screen).unwrap();
    store.set_screen_page(id, show, 7).unwrap();
    let restored = store.screen(id).unwrap().unwrap();
    assert_eq!(restored.first_playback_slot, 20);
    assert_eq!(restored.playback_count, 12);
    assert_eq!(
        restored.playback_layout.unwrap().rows[1].first_playback_slot,
        40
    );
    assert_eq!(store.screen_page(id, show).unwrap(), 7);
    drop(store);
    let _ = fs::remove_file(path);
}

#[test]
fn screen_playback_range_must_fit_page_slots() {
    let path = temporary("screen-validation");
    let store = DeskStore::open(&path).unwrap();
    let invalid = ScreenConfiguration {
        not_editable: false,
        id: Uuid::new_v4(),
        name: "Bad".into(),
        layout: serde_json::json!({}),
        show_dock: true,
        show_playbacks: true,
        playback_count: 9,
        playback_rows: 1,
        first_playback_slot: 120,
        page_mode: "follow_main".into(),
        show_page_controls: true,
        show_programmer: false,
        desired_open: false,
        display_id: None,
        bounds: None,
        fullscreen: false,
        playback_layout: None,
        content: ScreenContent::Desktop,
    };
    assert!(store.put_screen(invalid).is_err());
    drop(store);
    let _ = fs::remove_file(path);
}

#[test]
fn control_desks_have_unique_aliases_and_per_show_pages() {
    let path = temporary("control-desks");
    let desk = DeskStore::open(&path).unwrap();
    let control = desk.add_desk("Front", "front-desk").unwrap();
    assert!(desk.add_desk("Other", "front-desk").is_err());
    let first = ShowId::new();
    let second = ShowId::new();
    desk.set_desk_page(control.id, first, 12).unwrap();
    assert_eq!(desk.desk_page(control.id, first).unwrap(), 12);
    assert_eq!(desk.desk_page(control.id, second).unwrap(), 1);
    assert!(desk.set_desk_page(control.id, first, 128).is_err());
    drop(desk);
    let _ = fs::remove_file(path);
}

#[test]
fn client_history_migrates_unknown_rows_reuses_identity_and_recreates_removed_defaults() {
    let path = temporary("client-history");
    let client_id = Uuid::new_v4();
    let (legacy_id, first_connected_at) = {
        let store = DeskStore::open(&path).unwrap();
        let legacy = store.add_desk("Legacy wing", "legacy-wing").unwrap();
        let before = store.client_desks().unwrap();
        assert_eq!(before.len(), 1);
        assert_eq!(before[0].client_id, None);
        assert_eq!(before[0].last_connected_at, None);

        let resolved = store
            .resolve_client_desk(client_id, Some(legacy.id))
            .unwrap();
        assert_eq!(resolved.id, legacy.id);
        let connected = store.client_desks().unwrap();
        assert_eq!(connected.len(), 1);
        assert_eq!(connected[0].client_id, Some(client_id));
        (legacy.id, connected[0].last_connected_at.clone().unwrap())
    };

    let mut reopened = DeskStore::open(&path).unwrap();
    let same = reopened.resolve_client_desk(client_id, None).unwrap();
    assert_eq!(same.id, legacy_id);
    let history = reopened.client_desks().unwrap();
    assert_eq!(history.len(), 1);
    assert!(history[0].last_connected_at.as_deref() >= Some(first_connected_at.as_str()));
    assert!(reopened.remove_client_desk(legacy_id).unwrap());

    let recreated = reopened
        .resolve_client_desk(client_id, Some(legacy_id))
        .unwrap();
    assert_ne!(recreated.id, legacy_id);
    assert_eq!(
        (recreated.columns, recreated.rows, recreated.buttons),
        (8, 1, 3)
    );
    assert_eq!(recreated.playback_layout, None);
    assert_eq!(reopened.client_desks().unwrap().len(), 1);
    drop(reopened);
    let _ = fs::remove_file(path);
}

#[test]
fn removing_a_client_cleans_only_its_desk_owned_installation_state() {
    let path = temporary("client-removal");
    let mut store = DeskStore::open(&path).unwrap();
    let removed_client = Uuid::new_v4();
    let retained_client = Uuid::new_v4();
    let removed = store.resolve_client_desk(removed_client, None).unwrap();
    let retained = store.resolve_client_desk(retained_client, None).unwrap();
    let show_id = ShowId::new();
    store.set_desk_page(removed.id, show_id, 17).unwrap();
    store
        .set_selected_playback(removed.id, show_id, Some(23))
        .unwrap();
    store
        .set_setting(&format!("desk_lock:{}", removed.id), "locked")
        .unwrap();
    store
        .set_setting(
            "server_configuration",
            &serde_json::json!({
                "update_settings_by_desk": {
                    (removed.id.to_string()): { "mode": "all" },
                    (retained.id.to_string()): { "mode": "tracked" }
                },
                "matter_enabled": true
            })
            .to_string(),
        )
        .unwrap();
    store
        .set_setting(
            &format!("virtual_playback_exclusion_zones:{}", show_id.0),
            &serde_json::json!({
                (removed.id.to_string()): [{ "id": "old" }],
                (retained.id.to_string()): [{ "id": "keep" }]
            })
            .to_string(),
        )
        .unwrap();
    let screen = ScreenConfiguration {
        not_editable: false,
        id: Uuid::new_v4(),
        name: "Shared optional screen".into(),
        layout: serde_json::json!({"desks":[],"activeDeskId":""}),
        show_dock: true,
        show_playbacks: true,
        playback_count: 8,
        playback_rows: 1,
        first_playback_slot: 1,
        page_mode: "follow_main".into(),
        show_page_controls: true,
        show_programmer: false,
        desired_open: false,
        display_id: None,
        bounds: None,
        fullscreen: false,
        playback_layout: None,
        content: ScreenContent::Desktop,
    };
    store.put_screen(screen.clone()).unwrap();

    assert!(store.remove_client_desk(removed.id).unwrap());
    assert!(store.control_desk(removed.id).unwrap().is_none());
    assert_eq!(
        store.control_desk(retained.id).unwrap(),
        Some(retained.clone())
    );
    let retained_screen = store.screen(screen.id).unwrap().unwrap();
    assert_eq!(retained_screen.id, screen.id);
    assert_eq!(retained_screen.name, screen.name);
    assert!(
        store
            .users()
            .unwrap()
            .iter()
            .any(|user| user.name == "Operator")
    );
    assert_eq!(
        store.setting(&format!("desk_lock:{}", removed.id)).unwrap(),
        None
    );
    let configuration: serde_json::Value =
        serde_json::from_str(&store.setting("server_configuration").unwrap().unwrap()).unwrap();
    assert!(configuration["matter_enabled"].as_bool().unwrap());
    assert!(
        configuration["update_settings_by_desk"]
            .get(removed.id.to_string())
            .is_none()
    );
    assert!(
        configuration["update_settings_by_desk"]
            .get(retained.id.to_string())
            .is_some()
    );
    let zones: serde_json::Value = serde_json::from_str(
        &store
            .setting(&format!("virtual_playback_exclusion_zones:{}", show_id.0))
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert!(zones.get(removed.id.to_string()).is_none());
    assert!(zones.get(retained.id.to_string()).is_some());
    drop(store);
    let _ = fs::remove_file(path);
}

#[test]
fn legacy_screens_migrate_to_desktop_content() {
    let path = temporary("legacy-screen-content");
    {
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                r#"CREATE TABLE schema_info(version INTEGER NOT NULL);
                INSERT INTO schema_info(version) VALUES(9);
                CREATE TABLE screens(
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    layout_json TEXT NOT NULL,
                    show_dock INTEGER NOT NULL,
                    show_playbacks INTEGER NOT NULL,
                    playback_count INTEGER NOT NULL,
                    playback_rows INTEGER NOT NULL,
                    first_playback_slot INTEGER NOT NULL,
                    page_mode TEXT NOT NULL,
                    show_page_controls INTEGER NOT NULL,
                    desired_open INTEGER NOT NULL,
                    display_id TEXT,
                    bounds_json TEXT,
                    fullscreen INTEGER NOT NULL,
                    playback_layout_json TEXT
                );
                INSERT INTO screens VALUES(
                    '00000000-0000-0000-0000-000000000025',
                    'Legacy external screen',
                    '{"desks":[],"activeDeskId":"legacy"}',
                    1,1,8,1,1,'follow_main',1,0,NULL,NULL,0,NULL
                );"#,
            )
            .unwrap();
    }
    let store = DeskStore::open(&path).unwrap();
    let screen = store.screens().unwrap().remove(0);
    assert_eq!(screen.content, ScreenContent::Desktop);
    assert!(screen.show_dock);
    assert_eq!(
        store.programmer_control_surface().unwrap(),
        ProgrammerControlSurfaceConfiguration {
            owner_screen_id: None,
            visible_encoders: 6,
        }
    );
    let version: i64 = store
        .conn
        .query_row("SELECT version FROM schema_info", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, 11);
    drop(store);
    let _ = fs::remove_file(path);
}

#[test]
fn programmer_control_surface_defaults_to_main_with_six_encoders() {
    let path = temporary("programmer-control-default");
    let store = DeskStore::open(&path).unwrap();
    assert_eq!(
        store.programmer_control_surface().unwrap(),
        ProgrammerControlSurfaceConfiguration {
            owner_screen_id: None,
            visible_encoders: 6,
        }
    );
    drop(store);
    let _ = fs::remove_file(path);
}

#[test]
fn programmer_control_surface_persists_one_screen_owner_and_reassigns_deleted_owner_to_main() {
    let path = temporary("programmer-control-owner");
    let store = DeskStore::open(&path).unwrap();
    let screen = store
        .put_screen(ScreenConfiguration {
            not_editable: false,
            id: Uuid::new_v4(),
            name: "Programmer wing".into(),
            layout: serde_json::json!({}),
            show_dock: false,
            show_playbacks: false,
            playback_count: 8,
            playback_rows: 1,
            first_playback_slot: 1,
            page_mode: "follow_main".into(),
            show_page_controls: false,
            show_programmer: false,
            desired_open: true,
            display_id: None,
            bounds: None,
            fullscreen: false,
            playback_layout: None,
            content: ScreenContent::Desktop,
        })
        .unwrap();
    assert_eq!(
        store
            .put_programmer_control_surface(ProgrammerControlSurfaceConfiguration {
                owner_screen_id: Some(screen.id),
                visible_encoders: 4,
            })
            .unwrap()
            .owner_screen_id,
        Some(screen.id)
    );
    store.delete_screen(screen.id).unwrap();
    assert_eq!(
        store.programmer_control_surface().unwrap(),
        ProgrammerControlSurfaceConfiguration {
            owner_screen_id: None,
            visible_encoders: 4,
        }
    );
    drop(store);
    let _ = fs::remove_file(path);
}

#[test]
fn programmer_control_surface_rejects_unknown_owners_and_unsupported_widths() {
    let path = temporary("programmer-control-validation");
    let store = DeskStore::open(&path).unwrap();
    assert!(
        store
            .put_programmer_control_surface(ProgrammerControlSurfaceConfiguration {
                owner_screen_id: Some(Uuid::new_v4()),
                visible_encoders: 6,
            })
            .is_err()
    );
    assert!(
        store
            .put_programmer_control_surface(ProgrammerControlSurfaceConfiguration {
                owner_screen_id: None,
                visible_encoders: 5,
            })
            .is_err()
    );
    drop(store);
    let _ = fs::remove_file(path);
}

#[test]
fn fixed_screen_content_round_trips_and_forces_dock_off() {
    let path = temporary("fixed-screen-content");
    let store = DeskStore::open(&path).unwrap();
    let id = Uuid::new_v4();
    let cue_list_id = Uuid::new_v4();
    let saved = store
        .put_screen(ScreenConfiguration {
            not_editable: false,
            id,
            name: "Fixed fixtures".into(),
            layout: serde_json::json!({"desks":[],"activeDeskId":"preserved"}),
            show_dock: true,
            show_playbacks: true,
            playback_count: 8,
            playback_rows: 1,
            first_playback_slot: 1,
            page_mode: "follow_main".into(),
            show_page_controls: true,
            show_programmer: false,
            desired_open: true,
            display_id: None,
            bounds: None,
            fullscreen: false,
            playback_layout: None,
            content: ScreenContent::FixedPane {
                pane: FixedScreenPane::FixtureSheet {
                    included_heads: FixedScreenFixtureIncludedHeads::NoSubHeads,
                    order: FixedScreenFixtureOrder::Active,
                    active_only: true,
                    compact_mode: FixedScreenFixtureCompactMode::TextOnly,
                    cue_list_id: Some(cue_list_id),
                    columns: vec![
                        FixedScreenFixtureColumn::Id,
                        FixedScreenFixtureColumn::Name,
                        FixedScreenFixtureColumn::Intensity,
                    ],
                    show_type: false,
                    show_group_shortcuts: true,
                },
            },
        })
        .unwrap();
    assert!(!saved.show_dock);
    assert_eq!(
        saved.content,
        ScreenContent::FixedPane {
            pane: FixedScreenPane::FixtureSheet {
                included_heads: FixedScreenFixtureIncludedHeads::NoSubHeads,
                order: FixedScreenFixtureOrder::Active,
                active_only: true,
                compact_mode: FixedScreenFixtureCompactMode::TextOnly,
                cue_list_id: Some(cue_list_id),
                columns: vec![
                    FixedScreenFixtureColumn::Id,
                    FixedScreenFixtureColumn::Name,
                    FixedScreenFixtureColumn::Intensity,
                ],
                show_type: false,
                show_group_shortcuts: true,
            },
        }
    );
    assert_eq!(
        store.screen(id).unwrap().unwrap().layout["activeDeskId"],
        "preserved"
    );
    drop(store);
    let _ = fs::remove_file(path);
}

#[test]
fn fixed_side_pane_round_trips_with_pixel_width_and_keeps_dock() {
    let path = temporary("fixed-side-pane");
    let store = DeskStore::open(&path).unwrap();
    let id = Uuid::new_v4();
    let base = ScreenConfiguration {
        not_editable: false,
        id,
        name: "Side fixtures".into(),
        layout: serde_json::json!({"desks":[],"activeDeskId":"main"}),
        show_dock: true,
        show_playbacks: true,
        playback_count: 8,
        playback_rows: 1,
        first_playback_slot: 1,
        page_mode: "follow_main".into(),
        show_page_controls: true,
        show_programmer: false,
        desired_open: true,
        display_id: None,
        bounds: None,
        fullscreen: false,
        playback_layout: None,
        content: ScreenContent::FixedSidePane {
            pane: FixedScreenPane::Cues {
                cue_list_id: Uuid::new_v4().to_string(),
            },
            side: FixedScreenSide::Right,
            width_percent: 25,
        },
    };

    let saved = store.put_screen(base.clone()).unwrap();
    assert!(!saved.show_dock);
    assert_eq!(saved.content, base.content);

    let invalid = ScreenConfiguration {
        id: Uuid::new_v4(),
        content: ScreenContent::FixedSidePane {
            pane: FixedScreenPane::Cues {
                cue_list_id: Uuid::new_v4().to_string(),
            },
            side: FixedScreenSide::Left,
            width_percent: 4,
        },
        ..base
    };
    assert!(store.put_screen(invalid).is_err());
    drop(store);
    let _ = fs::remove_file(path);
}

#[test]
fn fixed_side_pane_reads_legacy_base_discriminator_as_a_plain_side_pane() {
    let content: ScreenContent = serde_json::from_value(serde_json::json!({
        "type": "fixed_side_pane",
        "pane": { "type": "cues", "cue_list_id": "" },
        "side": "left",
        "width_px": 420,
        "base": "control_surface"
    }))
    .unwrap();

    assert_eq!(
        content,
        ScreenContent::FixedSidePane {
            pane: FixedScreenPane::Cues {
                cue_list_id: String::new(),
            },
            side: FixedScreenSide::Left,
            width_percent: 22,
        }
    );
}

#[test]
fn legacy_empty_screen_content_reads_as_controls_only() {
    let content: ScreenContent =
        serde_json::from_value(serde_json::json!({ "type": "none" })).unwrap();

    assert_eq!(content, ScreenContent::ControlSurface);
}

#[test]
fn fixed_side_pane_turns_desktop_dock_off() {
    let path = temporary("fixed-side-control-base");
    let store = DeskStore::open(&path).unwrap();
    let saved = store
        .put_screen(ScreenConfiguration {
            not_editable: false,
            id: Uuid::new_v4(),
            name: "Side controls".into(),
            layout: serde_json::json!({"desks":[],"activeDeskId":"main"}),
            show_dock: true,
            show_playbacks: true,
            playback_count: 8,
            playback_rows: 1,
            first_playback_slot: 1,
            page_mode: "follow_main".into(),
            show_page_controls: true,
            show_programmer: false,
            desired_open: true,
            display_id: None,
            bounds: None,
            fullscreen: false,
            playback_layout: None,
            content: ScreenContent::FixedSidePane {
                pane: FixedScreenPane::Cues {
                    cue_list_id: String::new(),
                },
                side: FixedScreenSide::Left,
                width_percent: 22,
            },
        })
        .unwrap();

    assert!(!saved.show_dock);
    drop(store);
    let _ = fs::remove_file(path);
}

#[test]
fn control_and_empty_screen_content_round_trip_without_desktop_dock() {
    let path = temporary("utility-screen-content");
    let store = DeskStore::open(&path).unwrap();
    let base = ScreenConfiguration {
        not_editable: false,
        id: Uuid::new_v4(),
        name: "Utility".into(),
        layout: serde_json::json!({"desks":[],"activeDeskId":"main"}),
        show_dock: true,
        show_playbacks: true,
        playback_count: 8,
        playback_rows: 1,
        first_playback_slot: 1,
        page_mode: "follow_main".into(),
        show_page_controls: true,
        show_programmer: false,
        desired_open: true,
        display_id: None,
        bounds: None,
        fullscreen: false,
        playback_layout: None,
        content: ScreenContent::ControlSurface,
    };

    let control = store.put_screen(base.clone()).unwrap();
    assert!(!control.show_dock);
    assert_eq!(control.content, ScreenContent::ControlSurface);
    let empty = store
        .put_screen(ScreenConfiguration {
            id: Uuid::new_v4(),
            content: ScreenContent::ControlSurface,
            ..base
        })
        .unwrap();
    assert!(!empty.show_dock);
    assert_eq!(empty.content, ScreenContent::ControlSurface);
    drop(store);
    let _ = fs::remove_file(path);
}

#[test]
fn fixed_screen_content_rejects_invalid_display_settings_without_resolving_references() {
    let path = temporary("fixed-screen-validation");
    let store = DeskStore::open(&path).unwrap();
    let base = ScreenConfiguration {
        not_editable: false,
        id: Uuid::new_v4(),
        name: "Invalid fixed screen".into(),
        layout: serde_json::json!({}),
        show_dock: false,
        show_playbacks: true,
        playback_count: 8,
        playback_rows: 1,
        first_playback_slot: 1,
        page_mode: "follow_main".into(),
        show_page_controls: true,
        show_programmer: false,
        desired_open: false,
        display_id: None,
        bounds: None,
        fullscreen: false,
        playback_layout: None,
        content: ScreenContent::Desktop,
    };
    assert!(
        store
            .put_screen(ScreenConfiguration {
                content: ScreenContent::FixedPane {
                    pane: FixedScreenPane::FixtureSheet {
                        included_heads: FixedScreenFixtureIncludedHeads::All,
                        order: FixedScreenFixtureOrder::FixtureId,
                        active_only: false,
                        compact_mode: FixedScreenFixtureCompactMode::Off,
                        cue_list_id: Some(Uuid::new_v4()),
                        columns: vec![],
                        show_type: true,
                        show_group_shortcuts: false,
                    },
                },
                ..base.clone()
            })
            .is_err()
    );
    assert!(
        store
            .put_screen(ScreenConfiguration {
                content: ScreenContent::FixedPane {
                    pane: FixedScreenPane::Stage3d {
                        follow_preload: false,
                        show_floor_grid: true,
                        show_beam_guides: true,
                        render_quality: FixedScreenStageRenderQuality::Full,
                        environment_brightness: 1.1,
                    },
                },
                ..base.clone()
            })
            .is_err()
    );
    assert!(
        store
            .put_screen(ScreenConfiguration {
                content: ScreenContent::FixedPane {
                    pane: FixedScreenPane::Cues {
                        cue_list_id: String::new(),
                    },
                },
                ..base
            })
            .is_ok()
    );
    drop(store);
    let _ = fs::remove_file(path);
}

#[test]
fn selected_playback_is_persisted_per_desk_and_show() {
    let path = temporary("selected-playback");
    let first_show = ShowId::new();
    let second_show = ShowId::new();
    let (first_desk, second_desk) = {
        let store = DeskStore::open(&path).unwrap();
        let first = store.add_desk("Front", "front").unwrap();
        let second = store.add_desk("Backup", "backup").unwrap();
        store
            .set_selected_playback(first.id, first_show, Some(17))
            .unwrap();
        store
            .set_selected_playback(second.id, first_show, Some(23))
            .unwrap();
        (first.id, second.id)
    };
    let store = DeskStore::open(&path).unwrap();
    assert_eq!(
        store.selected_playback(first_desk, first_show).unwrap(),
        Some(17)
    );
    assert_eq!(
        store.selected_playback(second_desk, first_show).unwrap(),
        Some(23)
    );
    assert_eq!(
        store.selected_playback(first_desk, second_show).unwrap(),
        None
    );
    store
        .set_selected_playback(first_desk, first_show, None)
        .unwrap();
    assert_eq!(
        store.selected_playback(first_desk, first_show).unwrap(),
        None
    );
    drop(store);
    let _ = fs::remove_file(path);
}
