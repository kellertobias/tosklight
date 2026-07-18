#[test]
fn startup_rebases_show_paths_after_the_desk_data_directory_moves() {
    let root = std::env::temp_dir().join(format!("light-show-rebase-{}", Uuid::new_v4()));
    let legacy = root.join("legacy");
    let current = root.join("current");
    std::fs::create_dir_all(legacy.join("shows")).unwrap();
    std::fs::create_dir_all(&current).unwrap();
    let old_path = legacy.join("shows").join("Default Stage Show.show");
    default_show::initialise(&old_path).unwrap();
    let desk = DeskStore::open(current.join("desk.sqlite")).unwrap();
    let entry = desk
        .upsert_show(default_show::name(), &old_path.display().to_string(), false)
        .unwrap();
    std::fs::rename(legacy.join("shows"), current.join("shows")).unwrap();

    rebase_desk_show_paths(&desk, &current).unwrap();

    let relocated = desk.show(entry.id).unwrap().unwrap();
    assert_eq!(
        FsPath::new(&relocated.path),
        current.join("shows").join("Default Stage Show.show")
    );
    validate_show_file(&relocated.path).unwrap();
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn clean_default_load_creates_a_pristine_copy_without_replacing_manual_changes() {
    let (state, data_dir) = test_state();
    let working = ensure_default_show_available(&state.desk.lock(), &data_dir).unwrap();
    let working_store = ShowStore::open(&working.path).unwrap();
    let hazer = working_store
        .objects("patched_fixture")
        .unwrap()
        .into_iter()
        .find(|object| object.body["name"] == "Stage Hazer")
        .unwrap();
    assert!(
        working_store
            .delete_object("patched_fixture", &hazer.id)
            .unwrap()
    );
    state.desk.lock().set_active_show(Some(working.id)).unwrap();
    *state.active_show.write() = Some(working.clone());
    state
        .engine
        .replace_snapshot(load_engine_snapshot(&working).unwrap())
        .unwrap();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;

    let response = app
        .oneshot(
            Request::post("/api/v1/shows/default/open")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"transition":"hold_current"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let opened = json(response).await;
    assert_eq!(opened["name"], "Default Stage Show Clean Copy");
    let clean_store = ShowStore::open(opened["path"].as_str().unwrap()).unwrap();
    let clean_fixtures = clean_store.objects("patched_fixture").unwrap();
    assert_eq!(clean_fixtures.len(), 49);
    assert!(
        clean_fixtures
            .iter()
            .any(|object| object.body["name"] == "Stage Hazer")
    );
    assert_eq!(
        ShowStore::open(&working.path)
            .unwrap()
            .objects("patched_fixture")
            .unwrap()
            .len(),
        48
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn command_history_is_desk_scoped_bounded_newest_first_and_redacted() {
    let (state, data_dir) = test_state();
    let user = state.desk.lock().users().unwrap().remove(0);
    let session = Session {
        id: SessionId::new(),
        user: user.clone(),
        token: "history-token".into(),
        connected: true,
        desk: test_control_desk(),
    };
    let other = Session {
        id: SessionId::new(),
        user,
        token: "other-history-token".into(),
        connected: true,
        desk: ControlDesk {
            id: Uuid::new_v4(),
            name: "Other desk".into(),
            osc_alias: "other-desk".into(),
            ..test_control_desk()
        },
    };
    state.sessions.write().insert(session.id, session.clone());
    state.sessions.write().insert(other.id, other.clone());
    for number in 0..54 {
        record_command_history(
            &state,
            &session,
            &format!("GROUP 1 AT {number}"),
            "accepted",
            "Accepted",
            "software",
            None,
        );
    }
    record_command_history(
        &state,
        &session,
        "LOGIN TOKEN super-secret-value",
        "rejected",
        "parser included super-secret-value",
        "software",
        None,
    );
    record_command_history(
        &state,
        &other,
        "GROUP 2 AT 50",
        "accepted",
        "Accepted",
        "osc",
        None,
    );

    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        "Bearer history-token".parse().unwrap(),
    );
    let Json(entries) = command_history(State(state), headers).await.unwrap();
    assert_eq!(entries.len(), COMMAND_HISTORY_LIMIT);
    assert_eq!(entries[0].command, "[REDACTED SENSITIVE COMMAND]");
    assert_eq!(entries[0].feedback, "Sensitive input omitted");
    assert_eq!(entries[49].command, "GROUP 1 AT 5");
    assert!(entries.iter().all(|entry| entry.desk_id == session.desk.id));
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn advancing_from_an_occupied_last_playback_page_creates_one_empty_page() {
    let (state, data_dir) = test_state();
    let show_path = data_dir.join("shows/page-advance.show");
    let show_id = initialise_show(&show_path, "Page advance").unwrap();
    let entry = ShowEntry {
        id: show_id,
        name: "Page advance".into(),
        path: show_path.display().to_string(),
        revision: 0,
        updated_at: String::new(),
        revision_copy: None,
    };
    let page = light_playback::PlaybackPage {
        number: 1,
        name: "Main".into(),
        slots: HashMap::from([(1, 1)]),
    };
    let playback = light_playback::PlaybackDefinition {
        number: 1,
        name: "Grand Master".into(),
        target: light_playback::PlaybackTarget::GrandMaster,
        buttons: light_playback::PlaybackDefinition::default_buttons(
            &light_playback::PlaybackTarget::GrandMaster,
        ),
        button_count: 3,
        fader: light_playback::PlaybackFaderMode::Master,
        has_fader: true,
        go_activates: true,
        auto_off: false,
        xfade_millis: 0,
        color: "#20c997".into(),
        flash_release: light_playback::FlashReleaseMode::ReleaseAll,
        protect_from_swap: false,
        presentation_icon: None,
        presentation_image: None,
    };
    let store = ShowStore::open(&entry.path).unwrap();
    store
        .put_object("playback", "1", &serde_json::to_value(playback).unwrap(), 0)
        .unwrap();
    store
        .put_object(
            "playback_page",
            "1",
            &serde_json::to_value(page).unwrap(),
            0,
        )
        .unwrap();
    state
        .engine
        .replace_snapshot(load_engine_snapshot(&entry).unwrap())
        .unwrap();

    assert!(ensure_playback_page_for_advance(&state, &entry, 2).unwrap());
    assert!(!ensure_playback_page_for_advance(&state, &entry, 3).unwrap());
    let pages = ShowStore::open(&entry.path)
        .unwrap()
        .objects("playback_page")
        .unwrap();
    let created = pages.iter().find(|object| object.id == "2").unwrap();
    assert_eq!(
        serde_json::from_value::<light_playback::PlaybackPage>(created.body.clone())
            .unwrap()
            .name,
        "Page 2"
    );
    assert_eq!(pages.len(), 2);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn production_router_does_not_expose_test_clock_controls() {
    let (state, data_dir) = test_state();
    let response = router(state)
        .oneshot(
            Request::post("/api/v1/test/clock/advance")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"millis":0}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let _ = std::fs::remove_dir_all(data_dir);
}
