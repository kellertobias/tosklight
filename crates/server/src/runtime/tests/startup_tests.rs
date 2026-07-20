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
    *state.active_show.write() = Some(entry.clone());

    let _activation = state.activation_lock.clone().try_lock_owned().unwrap();
    let context = light_application::ActionContext::system(
        Uuid::nil(),
        light_application::ActionSource::Http,
    );
    let before_document = ShowStore::open(&entry.path)
        .unwrap()
        .portable_document()
        .unwrap();
    let before_runtime = state.engine.snapshot();
    let before_events = state.application_events.latest_sequence();
    let before_backups = startup_show_object_backup_count(&data_dir);
    let created = ensure_playback_page_for_advance(&state, &entry, 2, &context).unwrap();
    assert!(created.available());
    assert_eq!(created.event_sequence(), Some(before_events + 1));
    let after_create = ShowStore::open(&entry.path)
        .unwrap()
        .portable_document()
        .unwrap();
    assert_eq!(
        after_create.revision().value(),
        before_document.revision().value() + 1
    );
    assert_eq!(state.application_events.latest_sequence(), before_events + 1);
    assert_eq!(
        startup_show_object_backup_count(&data_dir),
        before_backups + 1
    );
    assert_eq!(state.engine.snapshot().revision, after_create.revision().value());
    assert!(!Arc::ptr_eq(&state.engine.snapshot(), &before_runtime));
    let missing = ensure_playback_page_for_advance(&state, &entry, 3, &context).unwrap();
    assert!(!missing.available());
    assert_eq!(missing.event_sequence(), None);
    assert_eq!(
        ShowStore::open(&entry.path)
            .unwrap()
            .portable_revision()
            .unwrap(),
        after_create.revision()
    );
    assert_eq!(state.application_events.latest_sequence(), before_events + 1);
    assert_eq!(
        startup_show_object_backup_count(&data_dir),
        before_backups + 1
    );
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

#[test]
fn restored_exclusion_normalization_emits_each_loser_once_and_is_idempotent() {
    let (state, data_dir) = test_state();
    assert!(state.network_output.is_none());
    let show = ShowEntry {
        id: light_core::ShowId::new(),
        name: "Restored exclusions".into(),
        path: data_dir.join("shows/restored-exclusions.show").display().to_string(),
        revision: 7,
        updated_at: String::new(),
        revision_copy: None,
    };
    *state.active_show.write() = Some(show.clone());
    let cue_list_id = light_core::CueListId::new();
    state
        .engine
        .replace_snapshot(restored_exclusion_snapshot(cue_list_id))
        .unwrap();
    let activated_at = "2026-01-01T00:00:00Z";
    state
        .engine
        .execute_playback(EnginePlaybackCommand::RestoreActive(vec![
            restored_exclusion_active(3, cue_list_id, activated_at),
            restored_exclusion_active(1, cue_list_id, activated_at),
            restored_exclusion_active(4, cue_list_id, activated_at),
            restored_exclusion_active(2, cue_list_id, activated_at),
        ]))
        .unwrap();
    let desk = state.desk.lock().add_desk("Restored", "restored").unwrap();
    state.desk.lock().set_desk_page(desk.id, show.id, 1).unwrap();
    let stored = VirtualPlaybackExclusionStore::from([(
        desk.id.to_string(),
        HashMap::from([(
            "surface".into(),
            vec![
                VirtualPlaybackExclusionZone {
                    id: "left".into(),
                    name: "Left".into(),
                    slots: vec![1, 2],
                },
                VirtualPlaybackExclusionZone {
                    id: "right".into(),
                    name: "Right".into(),
                    slots: vec![2, 3],
                },
            ],
        )]),
    )]);
    state
        .desk
        .lock()
        .set_setting(
            &virtual_playback_exclusion_setting(show.id),
            &serde_json::to_string(&stored).unwrap(),
        )
        .unwrap();

    let first = normalize_restored_virtual_playback_exclusions(&state).unwrap();

    assert_eq!(first.released_playbacks, vec![1, 2]);
    assert!(first.provenance_migrated);
    assert!(!first.persistence_pending);
    let light_application::EventReplay::Events(events) = state
        .application_events
        .replay(0, &light_application::EventFilter::default())
    else {
        panic!("expected retained restored-exclusion events");
    };
    assert_eq!(events.len(), 2);
    let numbers = events
        .iter()
        .map(|event| match &event.payload {
            light_application::ApplicationEvent::Playback(
                light_application::PlaybackEvent::RuntimeChanged(change),
            ) => {
                assert!(!change.projection.cue_list_runtime().unwrap().enabled);
                change.projection.playback_number.unwrap()
            }
            other => panic!("expected Playback runtime event, got {other:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(numbers, vec![1, 2]);
    assert_eq!(events[0].sequence, 1);
    assert_eq!(events[1].sequence, 2);
    assert_eq!(events[0].correlation_id, events[1].correlation_id);
    assert!(events[0].correlation_id.is_some());
    assert!(events.iter().all(|event| {
        event.source
            == light_application::EventSource::Action(light_application::ActionSource::System)
    }));

    let second = normalize_restored_virtual_playback_exclusions(&state).unwrap();

    assert!(second.released_playbacks.is_empty());
    assert!(!second.provenance_migrated);
    assert!(!second.persistence_pending);
    assert_eq!(state.application_events.latest_sequence(), 2);
    let enabled = state
        .engine
        .playback_runtime()
        .into_iter()
        .filter(|playback| playback.enabled)
        .filter_map(|playback| playback.playback_number)
        .collect::<HashSet<_>>();
    assert_eq!(enabled, HashSet::from([3, 4]));
    let persisted = state
        .desk
        .lock()
        .setting(&active_playbacks_setting(show.id))
        .unwrap()
        .unwrap();
    let persisted: Vec<light_playback::ActivePlayback> = serde_json::from_str(&persisted).unwrap();
    assert!(
        persisted
            .iter()
            .filter(|playback| playback.enabled)
            .all(|playback| playback.activation.is_some())
    );
    assert_eq!(
        persisted
            .into_iter()
            .filter(|playback| playback.enabled)
            .filter_map(|playback| playback.playback_number)
            .collect::<HashSet<_>>(),
        HashSet::from([3, 4])
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn restored_exclusions_replay_each_activation_with_its_own_desk_zones() {
    for configured_desk_activates_last in [false, true] {
        let (state, data_dir) = test_state();
        let show = ShowEntry {
            id: light_core::ShowId::new(),
            name: "Desk exact restored exclusions".into(),
            path: data_dir.join("shows/desk-exact.show").display().to_string(),
            revision: 1,
            updated_at: String::new(),
            revision_copy: None,
        };
        *state.active_show.write() = Some(show.clone());
        let cue_list_id = light_core::CueListId::new();
        state
            .engine
            .replace_snapshot(restored_exclusion_snapshot(cue_list_id))
            .unwrap();
        let configured = state
            .desk
            .lock()
            .add_desk("Configured restart desk", "configured-restart")
            .unwrap();
        let unconfigured = state
            .desk
            .lock()
            .add_desk("Unconfigured restart desk", "unconfigured-restart")
            .unwrap();
        for desk in [&configured, &unconfigured] {
            state
                .desk
                .lock()
                .set_desk_page(desk.id, show.id, 1)
                .unwrap();
        }
        store_restart_zone(&state, &show, configured.id);
        let (first, second) = if configured_desk_activates_last {
            ((2, unconfigured.id), (1, configured.id))
        } else {
            ((1, configured.id), (2, unconfigured.id))
        };
        state
            .engine
            .execute_playback(EnginePlaybackCommand::RestoreActive(vec![
                restored_exclusion_active_with_origin(first.0, cue_list_id, 1, first.1),
                restored_exclusion_active_with_origin(second.0, cue_list_id, 2, second.1),
            ]))
            .unwrap();
        persist_active_playbacks(&state).unwrap();
        if !configured_desk_activates_last {
            assert!(
                state
                    .desk
                    .lock()
                    .remove_client_desk(unconfigured.id)
                    .unwrap()
            );
        }

        let outcome = normalize_restored_virtual_playback_exclusions(&state).unwrap();

        let expected_released = if configured_desk_activates_last {
            vec![2]
        } else {
            Vec::new()
        };
        assert_eq!(outcome.released_playbacks, expected_released);
        assert!(!outcome.provenance_migrated);
        let enabled = state
            .engine
            .playback_runtime()
            .into_iter()
            .filter(|playback| playback.enabled)
            .filter_map(|playback| playback.playback_number)
            .collect::<HashSet<_>>();
        let expected_enabled = if configured_desk_activates_last {
            HashSet::from([1])
        } else {
            HashSet::from([1, 2])
        };
        assert_eq!(enabled, expected_enabled);
        let persisted = state
            .desk
            .lock()
            .setting(&active_playbacks_setting(show.id))
            .unwrap()
            .unwrap();
        assert!(persisted.contains("\"activation\""));
        assert!(
            !serde_json::to_string(&state.engine.playback_runtime())
                .unwrap()
                .contains("\"activation\"")
        );
        let second = normalize_restored_virtual_playback_exclusions(&state).unwrap();
        assert!(second.released_playbacks.is_empty());
        assert!(!second.provenance_migrated);
        assert!(!second.persistence_pending);
        let _ = std::fs::remove_dir_all(data_dir);
    }
}

#[test]
fn timed_preload_release_restart_keeps_the_original_activation_order() {
    let (state, data_dir) = test_state();
    let show = ShowEntry {
        id: light_core::ShowId::new(),
        name: "Timed Preload restart".into(),
        path: data_dir.join("shows/timed-preload.show").display().to_string(),
        revision: 1,
        updated_at: String::new(),
        revision_copy: None,
    };
    *state.active_show.write() = Some(show.clone());
    let cue_list_id = light_core::CueListId::new();
    state
        .engine
        .replace_snapshot(restored_exclusion_snapshot(cue_list_id))
        .unwrap();
    let desk = state
        .desk
        .lock()
        .add_desk("Timed Preload desk", "timed-preload")
        .unwrap();
    state
        .desk
        .lock()
        .set_desk_page(desk.id, show.id, 1)
        .unwrap();
    store_restart_zone(&state, &show, desk.id);

    let mut releasing = restored_exclusion_active_with_origin(1, cue_list_id, 1, desk.id);
    releasing.activation.as_mut().unwrap().at = "2026-01-01T00:00:01Z".parse().unwrap();
    releasing.activated_at = "2026-01-01T00:00:03Z".parse().unwrap();
    let mut later = restored_exclusion_active_with_origin(2, cue_list_id, 2, desk.id);
    later.activation = Some(light_playback::PlaybackActivationProvenance {
        ordinal: 2,
        at: "2026-01-01T00:00:02Z".parse().unwrap(),
        desk_id: None,
        surface: light_playback::PlaybackActivationSurface::Matter,
        exclusion_scope: light_playback::PlaybackExclusionScope::None,
    });
    state
        .engine
        .execute_playback(EnginePlaybackCommand::RestoreActive(vec![releasing, later]))
        .unwrap();
    let prepared = state
        .engine
        .prepare_playback_batch(
            &[light_engine::PlaybackBatchCommand {
                number: 1,
                action: light_engine::PlaybackBatchAction::Off,
                exclusion_zones: std::sync::Arc::default(),
                activation_origin: None,
            }],
            "2026-01-01T00:00:04Z".parse().unwrap(),
            1_000,
        )
        .unwrap();
    state
        .engine
        .install_prepared_playback_batch(prepared)
        .unwrap();
    let active_release = state
        .engine
        .playback_runtime()
        .into_iter()
        .find(|playback| playback.playback_number == Some(1))
        .unwrap();
    assert!(active_release.enabled);
    assert_eq!(active_release.activation.unwrap().ordinal, 1);
    persist_active_playbacks(&state).unwrap();
    let checkpoint = state
        .desk
        .lock()
        .setting(&active_playbacks_setting(show.id))
        .unwrap()
        .unwrap();
    let restored = serde_json::from_str(&checkpoint).unwrap();
    state
        .engine
        .execute_playback(EnginePlaybackCommand::RestoreActive(restored))
        .unwrap();

    let normalized = normalize_restored_virtual_playback_exclusions(&state).unwrap();
    assert!(!normalized.provenance_migrated);
    assert!(normalized.released_playbacks.is_empty());
    let enabled = state
        .engine
        .playback_runtime()
        .into_iter()
        .filter(|playback| playback.enabled)
        .filter_map(|playback| playback.playback_number)
        .collect::<HashSet<_>>();
    assert_eq!(enabled, HashSet::from([1, 2]));
    let _ = std::fs::remove_dir_all(data_dir);
}

fn store_restart_zone(state: &AppState, show: &ShowEntry, desk_id: Uuid) {
    let stored = VirtualPlaybackExclusionStore::from([(
        desk_id.to_string(),
        HashMap::from([(
            "restart-surface".into(),
            vec![VirtualPlaybackExclusionZone {
                id: "restart-zone".into(),
                name: "Restart zone".into(),
                slots: vec![1, 2],
            }],
        )]),
    )]);
    state
        .desk
        .lock()
        .set_setting(
            &virtual_playback_exclusion_setting(show.id),
            &serde_json::to_string(&stored).unwrap(),
        )
        .unwrap();
}

fn restored_exclusion_active_with_origin(
    number: u16,
    cue_list_id: light_core::CueListId,
    ordinal: u64,
    desk_id: Uuid,
) -> light_playback::ActivePlayback {
    let mut playback =
        restored_exclusion_active(number, cue_list_id, "2026-01-01T00:00:00Z");
    playback.activation = Some(light_playback::PlaybackActivationProvenance {
        ordinal,
        at: playback.activated_at,
        desk_id: Some(desk_id),
        surface: light_playback::PlaybackActivationSurface::Virtual,
        exclusion_scope: light_playback::PlaybackExclusionScope::OriginatingDesk,
    });
    playback
}

fn restored_exclusion_snapshot(cue_list_id: light_core::CueListId) -> EngineSnapshot {
    let cue_list = light_playback::CueList {
        id: cue_list_id,
        name: "Restored exclusion look".into(),
        priority: 0,
        mode: light_playback::CueListMode::Sequence,
        looped: false,
        chaser_step_millis: 1_000,
        speed_group: None,
        intensity_priority_mode: light_playback::IntensityPriorityMode::Htp,
        wrap_mode: Some(light_playback::WrapMode::Off),
        restart_mode: light_playback::RestartMode::FirstCue,
        force_cue_timing: false,
        disable_cue_timing: false,
        chaser_xfade_millis: 0,
        chaser_xfade_percent: Some(0),
        speed_multiplier: 1.0,
        cues: vec![light_playback::Cue::new(1.0)],
    };
    EngineSnapshot {
        revision: 7,
        cue_lists: vec![cue_list],
        playbacks: (1..=4)
            .map(|number| restored_exclusion_playback(number, cue_list_id))
            .collect(),
        playback_pages: vec![light_playback::PlaybackPage {
            number: 1,
            name: "Page 1".into(),
            slots: HashMap::from([(1, 1), (2, 2), (3, 3), (4, 4)]),
        }],
        ..EngineSnapshot::default()
    }
}

fn restored_exclusion_playback(
    number: u16,
    cue_list_id: light_core::CueListId,
) -> light_playback::PlaybackDefinition {
    let target = light_playback::PlaybackTarget::CueList { cue_list_id };
    light_playback::PlaybackDefinition {
        number,
        name: format!("Playback {number}"),
        buttons: light_playback::PlaybackDefinition::default_buttons(&target),
        target,
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
    }
}

fn restored_exclusion_active(
    number: u16,
    cue_list_id: light_core::CueListId,
    activated_at: &str,
) -> light_playback::ActivePlayback {
    serde_json::from_value(serde_json::json!({
        "playback_number": number,
        "cue_list_id": cue_list_id,
        "cue_index": 0,
        "previous_index": null,
        "paused": false,
        "activated_at": activated_at,
        "paused_at": null
    }))
    .expect("minimal restored Playback runtime must decode")
}

fn startup_show_object_backup_count(data_dir: &std::path::Path) -> usize {
    std::fs::read_dir(data_dir.join("backups"))
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().contains("show-object"))
        .count()
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
