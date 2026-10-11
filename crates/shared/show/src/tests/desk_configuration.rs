use super::temporary;
use crate::{
    DeskStore, FixedScreenFixtureColumn, FixedScreenFixtureCompactMode,
    FixedScreenFixtureIncludedHeads, FixedScreenFixtureOrder, FixedScreenPane, FixedScreenSide,
    FixedScreenStageRenderQuality, PlaybackSelection, ProgrammerControlSurfaceConfiguration,
    ScreenConfiguration, ScreenContent,
};
use light_core::ShowId;
use rusqlite::Connection;
use std::fs;
use uuid::Uuid;

#[test]
fn hardware_illumination_rejects_legacy_desks_and_persists_without_layout_changes() {
    let path = temporary("retired-desk-hardware");
    let connection = Connection::open(&path).unwrap();
    connection.execute_batch("CREATE TABLE schema_info(version INTEGER NOT NULL); INSERT INTO schema_info VALUES(16); CREATE TABLE settings(key TEXT PRIMARY KEY,value TEXT NOT NULL); INSERT INTO settings VALUES('marker','preserve');").unwrap();
    drop(connection);
    let original = fs::read(&path).unwrap();
    let rejection = crate::inspect_desk_playback_contract(&path)
        .unwrap()
        .unwrap();
    assert!(rejection.message.contains("separate fresh desk"));
    assert!(DeskStore::open(&path).is_err());
    assert_eq!(fs::read(&path).unwrap(), original);
    let _ = fs::remove_file(path);
}

#[test]
fn hardware_illumination_new_desk_and_old_json_default_to_full_white() {
    let path = temporary("hardware-illumination-new");
    let store = DeskStore::open(&path).unwrap();
    let desk = store.desk().unwrap();
    let legacy: crate::ControlDesk = serde_json::from_value(
        serde_json::json!({"id":desk.id,"name":desk.name,"columns":8,"rows":1,"buttons":3,"surface_id":"main"}),
    )
    .unwrap();
    assert_eq!(desk, legacy);
    assert_eq!(
        (
            desk.hardware_led_brightness,
            desk.hardware_gooseneck_brightness,
            desk.hardware_gooseneck_color
        ),
        (100, 100, 100)
    );
    drop(store);
    fs::remove_file(path).unwrap();
}

#[test]
fn screens_persist_central_surface_references() {
    let path = temporary("screens");
    let store = DeskStore::open(&path).unwrap();
    let id = Uuid::new_v4();
    let screen = ScreenConfiguration {
        not_editable: false,
        id,
        name: "Wing".into(),
        layout: serde_json::json!({"desks":[],"activeDeskId":""}),
        show_dock: false,
        show_playbacks: true,
        show_page_controls: false,
        show_programmer: false,
        show_compact_encoders: false,
        desired_open: true,
        display_id: Some("display".into()),
        bounds: Some(serde_json::json!({"x":1,"y":2,"width":800,"height":600})),
        fullscreen: true,
        surface_id: "main".into(),
        content: ScreenContent::Desktop,
    };
    store.put_screen(uuid::Uuid::nil(), screen).unwrap();
    let restored = store.screen(id).unwrap().unwrap();
    assert_eq!(restored.surface_id, "main");
    drop(store);
    let _ = fs::remove_file(path);
}

#[test]
fn screen_requires_registered_playback_surface() {
    let path = temporary("screen-validation");
    let store = DeskStore::open(&path).unwrap();
    let invalid = ScreenConfiguration {
        not_editable: false,
        id: Uuid::new_v4(),
        name: "Bad".into(),
        layout: serde_json::json!({}),
        show_dock: true,
        show_playbacks: true,
        show_page_controls: true,
        show_programmer: false,
        show_compact_encoders: false,
        desired_open: false,
        display_id: None,
        bounds: None,
        fullscreen: false,
        surface_id: "missing".into(),
        content: ScreenContent::Desktop,
    };
    assert!(store.put_screen(uuid::Uuid::nil(), invalid).is_err());
    drop(store);
    let _ = fs::remove_file(path);
}

#[test]
fn client_history_migrates_unknown_rows_reuses_identity_and_recreates_removed_defaults() {
    let path = temporary("client-history");
    let client_id = Uuid::new_v4();
    let (legacy_id, first_connected_at) = {
        let store = DeskStore::open(&path).unwrap();
        let legacy = store.desk().unwrap();
        let before = store.client_desks().unwrap();
        assert_eq!(before.len(), 1);
        assert_eq!(before[0].client_id, None);
        assert_eq!(before[0].last_connected_at, None);

        let resolved = store.resolve_client_desk(client_id).unwrap();
        assert_eq!(resolved.id, legacy.id);
        let connected = store.client_desks().unwrap();
        assert_eq!(connected.len(), 1);
        assert_eq!(connected[0].client_id, Some(client_id));
        (legacy.id, connected[0].last_connected_at.clone().unwrap())
    };

    let mut reopened = DeskStore::open(&path).unwrap();
    let same = reopened.resolve_client_desk(client_id).unwrap();
    assert_eq!(same.id, legacy_id);
    let history = reopened.client_desks().unwrap();
    assert_eq!(history.len(), 1);
    assert!(history[0].last_connected_at.as_deref() >= Some(first_connected_at.as_str()));
    // Forgetting the window leaves the desk it stood at. Connecting again is the same desk.
    assert!(reopened.remove_client(client_id).unwrap());
    let reconnected = reopened.resolve_client_desk(client_id).unwrap();
    assert_eq!(reconnected.id, legacy_id);
    assert_eq!(reopened.client_desks().unwrap().len(), 1);
    drop(reopened);
    let _ = fs::remove_file(path);
}

#[test]
fn collapsing_superseded_desks_cleans_only_their_own_installation_state() {
    let path = temporary("desk-collapse");
    let store = DeskStore::open(&path).unwrap();
    // Two desk records, as an installation from before the collapse holds. Connecting clients no
    // longer produce them — a client is its own record now — so they are made directly.
    // The collapse keeps the first desk by name, which is the one clients already resolve to.
    let retained = store.insert_legacy_desk("A kept desk").unwrap();
    let removed = store.insert_legacy_desk("Z superseded wing").unwrap();
    let show_id = ShowId::new();
    store
        .set_selected_playback(
            removed.id,
            show_id,
            Some(PlaybackSelection {
                page: 99,
                position: 23,
            }),
        )
        .unwrap();
    store
        .set_setting(
            &format!("desk_lock:{}", removed.id),
            r#"{"locked":false,"message":"wing"}"#,
        )
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
        show_page_controls: true,
        show_programmer: false,
        show_compact_encoders: false,
        desired_open: false,
        display_id: None,
        bounds: None,
        fullscreen: false,
        surface_id: "main".into(),
        content: ScreenContent::Desktop,
    };
    store.put_screen(uuid::Uuid::nil(), screen.clone()).unwrap();

    // Reopening runs the collapse: the first desk by name is the one clients resolve to.
    drop(store);
    let store = DeskStore::open(&path).unwrap();
    assert_eq!(store.desk().unwrap(), retained.clone());
    let retained_screen = store.screen(screen.id).unwrap().unwrap();
    assert_eq!(retained_screen.id, screen.id);
    assert_eq!(retained_screen.name, screen.name);
    assert_eq!(
        store.setting(&format!("desk_lock:{}", removed.id)).unwrap(),
        None
    );
    assert_eq!(
        store.setting("desk_lock").unwrap().as_deref(),
        Some(r#"{"locked":false,"message":"wing"}"#),
        "the only per-desk lock becomes the desk's one lock"
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
fn legacy_screens_are_rejected_to_desktop_content() {
    let path = temporary("retired-desk-legacy");
    let connection = Connection::open(&path).unwrap();
    connection.execute_batch("CREATE TABLE schema_info(version INTEGER NOT NULL); INSERT INTO schema_info VALUES(16); CREATE TABLE settings(key TEXT PRIMARY KEY,value TEXT NOT NULL); INSERT INTO settings VALUES('marker','preserve');").unwrap();
    drop(connection);
    let original = fs::read(&path).unwrap();
    let rejection = crate::inspect_desk_playback_contract(&path)
        .unwrap()
        .unwrap();
    assert!(rejection.message.contains("separate fresh desk"));
    assert!(DeskStore::open(&path).is_err());
    assert_eq!(fs::read(&path).unwrap(), original);
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
        .put_screen(
            uuid::Uuid::nil(),
            ScreenConfiguration {
                not_editable: false,
                id: Uuid::new_v4(),
                name: "Programmer wing".into(),
                layout: serde_json::json!({}),
                show_dock: false,
                show_playbacks: false,
                show_page_controls: false,
                show_programmer: false,
                show_compact_encoders: false,
                desired_open: true,
                display_id: None,
                bounds: None,
                fullscreen: false,
                surface_id: "main".into(),
                content: ScreenContent::Desktop,
            },
        )
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
                visible_encoders: 7,
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
        .put_screen(
            uuid::Uuid::nil(),
            ScreenConfiguration {
                not_editable: false,
                id,
                name: "Fixed fixtures".into(),
                layout: serde_json::json!({"desks":[],"activeDeskId":"preserved"}),
                show_dock: true,
                show_playbacks: true,
                show_page_controls: true,
                show_programmer: false,
                show_compact_encoders: false,
                desired_open: true,
                display_id: None,
                bounds: None,
                fullscreen: false,
                surface_id: "main".into(),
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
            },
        )
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
        show_page_controls: true,
        show_programmer: false,
        show_compact_encoders: false,
        desired_open: true,
        display_id: None,
        bounds: None,
        fullscreen: false,
        surface_id: "main".into(),
        content: ScreenContent::FixedSidePane {
            pane: FixedScreenPane::Cues {
                cue_list_id: Uuid::new_v4().to_string(),
            },
            side: FixedScreenSide::Right,
            width_percent: 25,
        },
    };

    let saved = store.put_screen(uuid::Uuid::nil(), base.clone()).unwrap();
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
    assert!(store.put_screen(uuid::Uuid::nil(), invalid).is_err());
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
        .put_screen(
            uuid::Uuid::nil(),
            ScreenConfiguration {
                not_editable: false,
                id: Uuid::new_v4(),
                name: "Side controls".into(),
                layout: serde_json::json!({"desks":[],"activeDeskId":"main"}),
                show_dock: true,
                show_playbacks: true,
                show_page_controls: true,
                show_programmer: false,
                show_compact_encoders: false,
                desired_open: true,
                display_id: None,
                bounds: None,
                fullscreen: false,
                surface_id: "main".into(),
                content: ScreenContent::FixedSidePane {
                    pane: FixedScreenPane::Cues {
                        cue_list_id: String::new(),
                    },
                    side: FixedScreenSide::Left,
                    width_percent: 22,
                },
            },
        )
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
        show_page_controls: true,
        show_programmer: false,
        show_compact_encoders: false,
        desired_open: true,
        display_id: None,
        bounds: None,
        fullscreen: false,
        surface_id: "main".into(),
        content: ScreenContent::ControlSurface,
    };

    let control = store.put_screen(uuid::Uuid::nil(), base.clone()).unwrap();
    assert!(!control.show_dock);
    assert_eq!(control.content, ScreenContent::ControlSurface);
    let empty = store
        .put_screen(
            uuid::Uuid::nil(),
            ScreenConfiguration {
                id: Uuid::new_v4(),
                content: ScreenContent::ControlSurface,
                ..base
            },
        )
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
        show_page_controls: true,
        show_programmer: false,
        show_compact_encoders: false,
        desired_open: false,
        display_id: None,
        bounds: None,
        fullscreen: false,
        surface_id: "main".into(),
        content: ScreenContent::Desktop,
    };
    assert!(
        store
            .put_screen(
                uuid::Uuid::nil(),
                ScreenConfiguration {
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
                }
            )
            .is_err()
    );
    assert!(
        store
            .put_screen(
                uuid::Uuid::nil(),
                ScreenConfiguration {
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
                }
            )
            .is_err()
    );
    assert!(
        store
            .put_screen(
                uuid::Uuid::nil(),
                ScreenConfiguration {
                    content: ScreenContent::FixedPane {
                        pane: FixedScreenPane::Cues {
                            cue_list_id: String::new(),
                        },
                    },
                    ..base
                }
            )
            .is_ok()
    );
    drop(store);
    let _ = fs::remove_file(path);
}

#[test]
fn selected_playback_is_persisted_per_show_and_survives_reopening() {
    let path = temporary("selected-playback");
    let first_show = ShowId::new();
    let second_show = ShowId::new();
    let desk_id = {
        let store = DeskStore::open(&path).unwrap();
        let desk = store.desk().unwrap();
        store
            .set_selected_playback(
                desk.id,
                first_show,
                Some(PlaybackSelection {
                    page: 1,
                    position: 17,
                }),
            )
            .unwrap();
        store
            .set_selected_playback(
                desk.id,
                second_show,
                Some(PlaybackSelection {
                    page: 99,
                    position: 23,
                }),
            )
            .unwrap();
        desk.id
    };
    let store = DeskStore::open(&path).unwrap();
    assert_eq!(
        store.selected_playback(desk_id, first_show).unwrap(),
        Some(PlaybackSelection {
            page: 1,
            position: 17
        })
    );
    assert_eq!(
        store.selected_playback(desk_id, second_show).unwrap(),
        Some(PlaybackSelection {
            page: 99,
            position: 23
        })
    );
    assert_eq!(
        store.selected_playback(desk_id, ShowId::new()).unwrap(),
        None
    );
    store
        .set_selected_playback(desk_id, first_show, None)
        .unwrap();
    assert_eq!(store.selected_playback(desk_id, first_show).unwrap(), None);
    drop(store);
    let _ = fs::remove_file(path);
}

#[test]
fn a_desk_saved_with_an_osc_alias_is_rejected_without_rewriting_configuration() {
    let path = temporary("retired-desk-a");
    let connection = Connection::open(&path).unwrap();
    connection.execute_batch("CREATE TABLE schema_info(version INTEGER NOT NULL); INSERT INTO schema_info VALUES(16); CREATE TABLE settings(key TEXT PRIMARY KEY,value TEXT NOT NULL); INSERT INTO settings VALUES('marker','preserve');").unwrap();
    drop(connection);
    let original = fs::read(&path).unwrap();
    let rejection = crate::inspect_desk_playback_contract(&path)
        .unwrap()
        .unwrap();
    assert!(rejection.message.contains("separate fresh desk"));
    assert!(DeskStore::open(&path).is_err());
    assert_eq!(fs::read(&path).unwrap(), original);
    let _ = fs::remove_file(path);
}

#[test]
fn a_locked_desk_lock_survives_the_collapse_into_one_lock() {
    let path = temporary("desk-lock-collapse");
    {
        let store = DeskStore::open(&path).unwrap();
        store
            .set_setting(
                &format!("desk_lock:{}", Uuid::new_v4()),
                r#"{"locked":false,"message":"unlocked wing"}"#,
            )
            .unwrap();
        store
            .set_setting(
                &format!("desk_lock:{}", Uuid::new_v4()),
                r#"{"locked":true,"message":"front of house"}"#,
            )
            .unwrap();
    }

    let store = DeskStore::open(&path).unwrap();

    // A desk that was locked must not come back unlocked.
    assert_eq!(
        store.setting("desk_lock").unwrap().as_deref(),
        Some(r#"{"locked":true,"message":"front of house"}"#)
    );
    assert!(
        store.settings_with_prefix("desk_lock:").unwrap().is_empty(),
        "the per-desk keys are gone, so a stale one cannot reappear"
    );
    drop(store);
    let _ = fs::remove_file(path);
}

#[test]
fn an_installation_that_already_holds_one_lock_keeps_it() {
    let path = temporary("desk-lock-kept");
    {
        let store = DeskStore::open(&path).unwrap();
        store
            .set_setting("desk_lock", r#"{"locked":true,"message":"kept"}"#)
            .unwrap();
        store
            .set_setting(
                &format!("desk_lock:{}", Uuid::new_v4()),
                r#"{"locked":false,"message":"superseded"}"#,
            )
            .unwrap();
    }

    let store = DeskStore::open(&path).unwrap();

    assert_eq!(
        store.setting("desk_lock").unwrap().as_deref(),
        Some(r#"{"locked":true,"message":"kept"}"#)
    );
    assert!(store.settings_with_prefix("desk_lock:").unwrap().is_empty());
    drop(store);
    let _ = fs::remove_file(path);
}

#[test]
fn five_encoder_count_migrates_preserving_existing_count() {
    let path = temporary("five-encoder-migration");
    let store = DeskStore::open(&path).unwrap();
    store
        .put_programmer_control_surface(ProgrammerControlSurfaceConfiguration {
            owner_screen_id: None,
            visible_encoders: 4,
        })
        .unwrap();
    store.conn.execute_batch("ALTER TABLE programmer_control_surface RENAME TO programmer_control_surface_new; CREATE TABLE programmer_control_surface(singleton INTEGER PRIMARY KEY CHECK(singleton=1),owner_screen_id TEXT,visible_encoders INTEGER NOT NULL DEFAULT 6 CHECK(visible_encoders IN(4,6))); INSERT INTO programmer_control_surface SELECT * FROM programmer_control_surface_new; DROP TABLE programmer_control_surface_new;").unwrap();
    drop(store);
    let store = DeskStore::open(&path).unwrap();
    assert_eq!(
        store.programmer_control_surface().unwrap().visible_encoders,
        4
    );
    store
        .put_programmer_control_surface(ProgrammerControlSurfaceConfiguration {
            owner_screen_id: None,
            visible_encoders: 5,
        })
        .unwrap();
    drop(store);
    let store = DeskStore::open(&path).unwrap();
    assert_eq!(
        store.programmer_control_surface().unwrap().visible_encoders,
        5
    );
    assert!(
        store
            .put_programmer_control_surface(ProgrammerControlSurfaceConfiguration {
                owner_screen_id: None,
                visible_encoders: 7
            })
            .is_err()
    );
    drop(store);
    fs::remove_file(path).unwrap();
}

#[test]
fn compact_encoder_flags_migrate_canonical_surfaces_and_preserve_sparse_updates() {
    let path = temporary("canonical-compact-encoders");
    let store = DeskStore::open(&path).unwrap();
    let desk = store.desk().unwrap();
    let screen = store
        .put_screen(
            desk.id,
            ScreenConfiguration {
                id: Uuid::new_v4(),
                name: "Canonical screen".into(),
                layout: serde_json::json!({"desks":[],"activeDeskId":"main"}),
                show_dock: true,
                show_playbacks: true,
                surface_id: "main".into(),
                show_page_controls: true,
                show_programmer: false,
                show_compact_encoders: false,
                desired_open: false,
                display_id: None,
                bounds: None,
                fullscreen: false,
                content: ScreenContent::Desktop,
                not_editable: false,
            },
        )
        .unwrap();
    store.conn.execute_batch("ALTER TABLE control_desks DROP COLUMN show_compact_encoders; ALTER TABLE screens DROP COLUMN show_compact_encoders;").unwrap();
    drop(store);
    let store = DeskStore::open(&path).unwrap();
    let current = store.desk().unwrap();
    assert_eq!(current.surface_id, "main");
    assert!(!current.show_compact_encoders);
    let mut restored = store.screen(screen.id).unwrap().unwrap();
    assert_eq!(restored.surface_id, screen.surface_id);
    assert!(!restored.show_compact_encoders);
    restored.show_compact_encoders = true;
    store.put_screen(desk.id, restored.clone()).unwrap();
    let compact = store
        .update_desk_with_screen_settings(
            desk.id,
            &current.name,
            current.columns,
            current.rows,
            current.buttons,
            current.surface_id.clone(),
            Some([0, 42, 100]),
            Some(true),
        )
        .unwrap();
    let renamed = store
        .update_desk(
            desk.id,
            "Renamed",
            compact.columns,
            compact.rows,
            compact.buttons,
            compact.surface_id.clone(),
        )
        .unwrap();
    assert!(renamed.show_compact_encoders);
    assert_eq!(renamed.hardware_gooseneck_brightness, 42);
    assert_eq!(renamed.surface_id, current.surface_id);
    drop(store);
    let store = DeskStore::open(&path).unwrap();
    assert_eq!(store.desk().unwrap(), renamed);
    assert_eq!(
        serde_json::to_value(store.screen(screen.id).unwrap().unwrap()).unwrap(),
        serde_json::to_value(restored).unwrap()
    );
    drop(store);
    fs::remove_file(path).unwrap();
}
