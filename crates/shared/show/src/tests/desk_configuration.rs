use super::temporary;
use crate::{DeskStore, PlaybackSurfaceLayout, PlaybackSurfaceRow, ScreenConfiguration};
use light_core::ShowId;
use std::fs;
use uuid::Uuid;

#[test]
fn screens_persist_and_keep_independent_pages_per_show() {
    let path = temporary("screens");
    let store = DeskStore::open(&path).unwrap();
    let show = ShowId::new();
    let id = Uuid::new_v4();
    let screen = ScreenConfiguration {
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
        desired_open: false,
        display_id: None,
        bounds: None,
        fullscreen: false,
        playback_layout: None,
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
        desired_open: false,
        display_id: None,
        bounds: None,
        fullscreen: false,
        playback_layout: None,
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
