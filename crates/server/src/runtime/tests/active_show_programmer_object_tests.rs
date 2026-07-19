use super::*;

#[tokio::test]
async fn active_group_and_preset_puts_install_the_exact_committed_candidate() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Programmer object boundary").await;
    let show_id = show["id"].as_str().unwrap();
    let show_uuid = Uuid::parse_str(show_id).unwrap();
    let entry = state
        .desk
        .lock()
        .show(light_core::ShowId(show_uuid))
        .unwrap()
        .unwrap();
    let first = light_core::FixtureId(Uuid::from_u128(101));
    let second = light_core::FixtureId(Uuid::from_u128(102));
    ShowStore::open(&entry.path)
        .unwrap()
        .put_object(
            "group",
            "7",
            &serde_json::json!({
                "id":"old",
                "name":"Before",
                "fixtures":[first],
                "future_server_field":{"retained":true}
            }),
            0,
        )
        .unwrap();
    open_show_for_test(&app, &token, show_id).await;
    let initial_group_revision = ShowStore::open(&entry.path)
        .unwrap()
        .portable_document()
        .unwrap()
        .object("group", "7")
        .unwrap()
        .revision();

    let updated = put_active_object(
        &app,
        &token,
        show_id,
        "group",
        "7",
        initial_group_revision,
        serde_json::json!({
            "name":"Ordered",
            "fixtures":[second,first],
            "future_client_field":"accepted"
        }),
    )
    .await;
    assert_eq!(updated.status(), StatusCode::OK);
    assert_eq!(
        updated.headers()[header::ETAG],
        format!("\"{}\"", initial_group_revision + 1)
    );

    let empty = put_active_object(
        &app,
        &token,
        show_id,
        "group",
        "8",
        0,
        serde_json::json!({"name":"Stored empty","fixtures":[]}),
    )
    .await;
    assert_eq!(empty.status(), StatusCode::OK);

    let preset = put_active_object(
        &app,
        &token,
        show_id,
        "preset",
        "2.3",
        0,
        serde_json::json!({
            "name":"Color three",
            "family":"Color",
            "number":3,
            "values":{},
            "group_values":{},
            "future_preset_field":42
        }),
    )
    .await;
    assert_eq!(preset.status(), StatusCode::OK);

    let before_store_sequence = state.application_events.latest_sequence();
    let stored_preset = app
        .clone()
        .oneshot(
            Request::post(format!("/api/v1/shows/{show_id}/presets/2.3/store"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::IF_MATCH, "1")
                .body(Body::from(
                    serde_json::json!({
                        "mode":"merge",
                        "preset":{
                            "name":"Merged color three",
                            "family":"Color",
                            "number":3,
                            "values":{},
                            "group_values":{},
                            "future_store_field":"accepted"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stored_preset.status(), StatusCode::OK);
    assert_eq!(stored_preset.headers()[header::ETAG], "\"2\"");
    let stored_preset_response = json(stored_preset).await;
    assert_eq!(
        stored_preset_response["event_sequence"],
        before_store_sequence + 1
    );

    let document = ShowStore::open(&entry.path)
        .unwrap()
        .portable_document()
        .unwrap();
    let stored = document.object("group", "7").unwrap();
    assert_eq!(stored.revision(), initial_group_revision + 1);
    assert_eq!(stored.body()["id"], "7");
    assert_eq!(
        stored.body()["fixtures"],
        serde_json::json!([second, first])
    );
    assert_eq!(
        stored.body()["future_server_field"],
        serde_json::json!({"retained":true})
    );
    assert_eq!(stored.body()["future_client_field"], "accepted");
    assert_eq!(
        document.object("group", "8").unwrap().body()["fixtures"],
        serde_json::json!([])
    );
    let stored_preset = document.object("preset", "2.3").unwrap();
    assert_eq!(stored_preset.revision(), 2);
    assert_eq!(stored_preset.body()["future_preset_field"], 42);
    assert_eq!(stored_preset.body()["future_store_field"], "accepted");
    let snapshot = state.engine.snapshot();
    assert_eq!(snapshot.revision, document.revision().value());
    assert_eq!(
        snapshot
            .groups
            .iter()
            .find(|group| group.id == "7")
            .unwrap()
            .fixtures,
        vec![second, first]
    );
    assert!(
        snapshot
            .groups
            .iter()
            .any(|group| group.id == "8" && group.fixtures.is_empty())
    );

    let revision_before_failures = document.revision();
    let invalid = put_active_object(
        &app,
        &token,
        show_id,
        "preset",
        "2.4",
        0,
        serde_json::json!({"name":"Wrong","family":"Position","number":4}),
    )
    .await;
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    let stale = put_active_object(
        &app,
        &token,
        show_id,
        "group",
        "7",
        initial_group_revision,
        serde_json::json!({"name":"Stale","fixtures":[]}),
    )
    .await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    let after_failures = ShowStore::open(&entry.path)
        .unwrap()
        .portable_document()
        .unwrap();
    assert_eq!(after_failures.revision(), revision_before_failures);
    assert_eq!(
        state.engine.snapshot().revision,
        revision_before_failures.value()
    );
    assert!(after_failures.object("preset", "2.4").is_none());

    let object_backups = std::fs::read_dir(data_dir.join("backups"))
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.contains("-show-object-"))
        })
        .count();
    assert_eq!(object_backups, 4);
    let audit = state.audit_events.lock();
    let changed = audit
        .iter()
        .filter(|event| event.kind == "show_object_changed")
        .count();
    assert_eq!(changed, 3);
    assert_eq!(
        audit
            .iter()
            .filter(|event| event.kind == "preset_stored")
            .count(),
        1
    );
    drop(audit);
    let _ = std::fs::remove_dir_all(data_dir);
}

async fn open_show_for_test(app: &Router, token: &str, show_id: &str) {
    let response = app
        .clone()
        .oneshot(
            Request::post(format!("/api/v1/shows/{show_id}/open"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::from(r#"{"transition":"hold_current"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

async fn put_active_object(
    app: &Router,
    token: &str,
    show_id: &str,
    kind: &str,
    object_id: &str,
    revision: u64,
    body: serde_json::Value,
) -> Response {
    app.clone()
        .oneshot(
            Request::put(format!(
                "/api/v1/shows/{show_id}/objects/{kind}/{object_id}"
            ))
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::IF_MATCH, revision.to_string())
            .body(Body::from(body.to_string()))
            .unwrap(),
        )
        .await
        .unwrap()
}

struct ActiveObjectScenario {
    state: AppState,
    data_dir: PathBuf,
    show_path: PathBuf,
    session: Session,
}

impl ActiveObjectScenario {
    fn new(name: &str, seed: impl FnOnce(&ShowStore)) -> Self {
        let (state, data_dir) = test_state();
        let user = state.desk.lock().users().unwrap().remove(0);
        let session = Session {
            id: SessionId::new(),
            user: user.clone(),
            token: format!("{}-operator", name.replace(' ', "-")),
            connected: true,
            desk: test_control_desk(),
        };
        state.programmers.start(session.id, user.id);
        attach_session_command_context(&state, &session);
        state.sessions.write().insert(session.id, session.clone());

        let show_path = data_dir.join("shows").join(format!(
            "{}-{}.show",
            name.replace(' ', "-"),
            Uuid::new_v4()
        ));
        let show_id = initialise_show(&show_path, name).unwrap();
        let store = ShowStore::open(&show_path).unwrap();
        seed(&store);
        let entry = ShowEntry {
            id: show_id,
            name: name.into(),
            path: show_path.display().to_string(),
            revision: 0,
            updated_at: String::new(),
            revision_copy: None,
        };
        *state.active_show.write() = Some(entry.clone());
        state
            .engine
            .replace_snapshot(load_engine_snapshot(&entry).unwrap())
            .unwrap();

        Self {
            state,
            data_dir,
            show_path,
            session,
        }
    }

    fn store(&self) -> ShowStore {
        ShowStore::open(&self.show_path).unwrap()
    }

    fn boundary(&self) -> MutationBoundary {
        MutationBoundary {
            show_revision: self.store().portable_document().unwrap().revision().value(),
            backup_count: show_object_backup_count(&self.data_dir),
            runtime: self.state.engine.snapshot(),
        }
    }

    fn assert_one_commit(&self, before: &MutationBoundary) {
        let document = self.store().portable_document().unwrap();
        assert_eq!(document.revision().value(), before.show_revision + 1);
        assert_eq!(
            show_object_backup_count(&self.data_dir),
            before.backup_count + 1
        );
        let runtime = self.state.engine.snapshot();
        assert_eq!(runtime.revision, document.revision().value());
        assert!(!std::sync::Arc::ptr_eq(&runtime, &before.runtime));
    }

    fn assert_unchanged(&self, before: &MutationBoundary) {
        assert_eq!(
            self.store().portable_document().unwrap().revision().value(),
            before.show_revision
        );
        assert_eq!(
            show_object_backup_count(&self.data_dir),
            before.backup_count
        );
        assert!(std::sync::Arc::ptr_eq(
            &self.state.engine.snapshot(),
            &before.runtime
        ));
    }
}

impl Drop for ActiveObjectScenario {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.data_dir);
    }
}

struct MutationBoundary {
    show_revision: u64,
    backup_count: usize,
    runtime: std::sync::Arc<EngineSnapshot>,
}

fn show_object_backup_count(data_dir: &std::path::Path) -> usize {
    std::fs::read_dir(data_dir.join("backups"))
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.contains("-show-object-"))
        })
        .count()
}

fn stored_object(store: &ShowStore, kind: &str, id: &str) -> Option<light_show::VersionedObject> {
    store
        .objects(kind)
        .unwrap()
        .into_iter()
        .find(|object| object.id == id)
}

fn preset_body(
    name: &str,
    family: light_programmer::PresetFamily,
    number: u32,
) -> serde_json::Value {
    serde_json::to_value(light_programmer::Preset {
        name: name.into(),
        family,
        number,
        ..Default::default()
    })
    .unwrap()
}

#[test]
fn record_and_delete_commands_each_cross_one_active_show_boundary() {
    let scenario = ActiveObjectScenario::new("Record delete boundary", |_| {});
    let fixtures = [light_core::FixtureId::new(), light_core::FixtureId::new()];
    scenario
        .state
        .programmers
        .select(scenario.session.id, fixtures);

    let before_group_record = scenario.boundary();
    let record_context =
        operator_action_context(&scenario.session, light_application::ActionSource::Osc)
            .with_request_id("record-group-71");
    assert_eq!(
        execute_programmer_command_from(
            &scenario.state,
            &scenario.session,
            "RECORD GROUP 71",
            &record_context,
        )
        .unwrap(),
        2
    );
    scenario.assert_one_commit(&before_group_record);
    assert_eq!(
        stored_object(&scenario.store(), "group", "71")
            .unwrap()
            .body["fixtures"],
        serde_json::json!(fixtures)
    );
    assert!(
        scenario
            .state
            .engine
            .snapshot()
            .groups
            .iter()
            .any(|group| group.id == "71" && group.fixtures == fixtures)
    );
    let light_application::EventReplay::Events(events) = scenario
        .state
        .application_events
        .replay(0, &light_application::EventFilter::default())
    else {
        panic!("expected a retained Group event");
    };
    assert_eq!(
        events.last().unwrap().source,
        light_application::EventSource::Action(light_application::ActionSource::Osc)
    );
    assert_eq!(
        events.last().unwrap().correlation_id,
        Some(record_context.correlation_id)
    );
    let backup_identity = format!("{}-record-group-71", record_context.correlation_id);
    assert!(
        std::fs::read_dir(scenario.data_dir.join("backups"))
            .unwrap()
            .filter_map(Result::ok)
            .any(|entry| entry
                .file_name()
                .to_string_lossy()
                .contains(&backup_identity)),
        "the show-object backup should retain the command correlation and request identity"
    );

    let before_group_delete = scenario.boundary();
    assert_eq!(
        execute_programmer_command(&scenario.state, &scenario.session, "DELETE GROUP 71").unwrap(),
        1
    );
    scenario.assert_one_commit(&before_group_delete);
    assert!(stored_object(&scenario.store(), "group", "71").is_none());
    assert!(
        scenario
            .state
            .engine
            .snapshot()
            .groups
            .iter()
            .all(|group| group.id != "71")
    );

    scenario.state.programmers.set(
        scenario.session.id,
        fixtures[0],
        light_core::AttributeKey("pan".into()),
        light_core::AttributeValue::Normalized(0.4),
    );
    let before_preset_record = scenario.boundary();
    assert_eq!(
        execute_programmer_command(&scenario.state, &scenario.session, "RECORD 0.7").unwrap(),
        1
    );
    scenario.assert_one_commit(&before_preset_record);
    assert!(stored_object(&scenario.store(), "preset", "0.7").is_some());

    let before_preset_delete = scenario.boundary();
    assert_eq!(
        execute_programmer_command(&scenario.state, &scenario.session, "DELETE 0.7").unwrap(),
        1
    );
    scenario.assert_one_commit(&before_preset_delete);
    assert!(stored_object(&scenario.store(), "preset", "0.7").is_none());
}

#[test]
fn preset_move_commits_destination_and_source_delete_atomically() {
    let scenario = ActiveObjectScenario::new("Preset move boundary", |store| {
        for (id, number, name) in [("2.1", 1, "Source"), ("2.2", 2, "Blocked source")] {
            store
                .put_object(
                    "preset",
                    id,
                    &preset_body(name, light_programmer::PresetFamily::Color, number),
                    0,
                )
                .unwrap();
        }
        store
            .put_object(
                "preset",
                "2.6",
                &preset_body("Occupied", light_programmer::PresetFamily::Color, 6),
                0,
            )
            .unwrap();
    });

    let before_move = scenario.boundary();
    assert_eq!(
        execute_programmer_command(&scenario.state, &scenario.session, "MOVE 2.1 AT 5").unwrap(),
        1
    );
    scenario.assert_one_commit(&before_move);
    let store = scenario.store();
    assert!(stored_object(&store, "preset", "2.1").is_none());
    let destination = stored_object(&store, "preset", "2.5").unwrap();
    assert_eq!(destination.body["name"], "Source");
    assert_eq!(destination.body["number"], 5);

    let before_conflict = scenario.boundary();
    let error = execute_programmer_command(&scenario.state, &scenario.session, "MOVE 2.2 AT 6")
        .unwrap_err();
    assert!(error.contains("already exists"));
    scenario.assert_unchanged(&before_conflict);
    let store = scenario.store();
    assert_eq!(
        stored_object(&store, "preset", "2.2").unwrap().body["name"],
        "Blocked source"
    );
    assert_eq!(
        stored_object(&store, "preset", "2.6").unwrap().body["name"],
        "Occupied"
    );

    let before_stale_batch = scenario.boundary();
    let show_id = scenario.state.active_show.read().as_ref().unwrap().id;
    let _activation = scenario
        .state
        .activation_lock
        .clone()
        .try_lock_owned()
        .unwrap();
    let stale_move = active_show_object_action(
        operator_action_context(&scenario.session, light_application::ActionSource::Http),
        show_id,
        vec![
            put_active_show_object(
                light_application::ActiveShowObjectKind::Preset,
                "2.8",
                0,
                preset_body("Must not survive", light_programmer::PresetFamily::Color, 8),
            ),
            delete_active_show_object(light_application::ActiveShowObjectKind::Preset, "2.2", 99),
        ],
    );
    let error = run_active_show_object_action(&scenario.state, stale_move).unwrap_err();
    assert_eq!(error.status, StatusCode::CONFLICT);
    scenario.assert_unchanged(&before_stale_batch);
    assert!(stored_object(&scenario.store(), "preset", "2.8").is_none());
    assert!(stored_object(&scenario.store(), "preset", "2.2").is_some());
}

#[test]
fn generated_presets_share_one_show_commit_backup_and_runtime_install() {
    let mut fixture = schema_v2_direct_fixture().0;
    let mode_id = fixture.definition.mode_id.unwrap();
    let mut profile = fixture.definition.profile_snapshot.take().unwrap();
    let channel = &mut profile.modes[0].channels[0];
    channel.functions.push(light_fixture::ChannelFunction {
        id: Uuid::new_v4(),
        name: "Open".into(),
        dmx_from: 128,
        dmx_to: 255,
        attribute: light_core::AttributeKey("gobo.1".into()),
        priority: 100,
        behavior: light_fixture::ChannelFunctionBehavior::Indexed {
            semantic_id: "gobo.open".into(),
            label: "Open".into(),
            raw_value: 200,
        },
    });
    fixture.definition = profile.resolved_definition(mode_id).unwrap();
    let fixture_id = fixture.fixture_id;
    let scenario = ActiveObjectScenario::new("Generated preset boundary", |store| {
        store
            .put_object(
                "patched_fixture",
                &fixture_id.0.to_string(),
                &serde_json::to_value(fixture).unwrap(),
                0,
            )
            .unwrap();
    });

    let before = scenario.boundary();
    let response = generate_profile_presets(&scenario.state, vec![fixture_id]).unwrap();
    assert_eq!(response["created"].as_array().unwrap().len(), 2);
    scenario.assert_one_commit(&before);
    let presets = scenario.store().objects("preset").unwrap();
    assert_eq!(presets.len(), 2);
    assert!(presets.iter().all(|preset| preset.revision == 1));
    assert_eq!(
        presets
            .iter()
            .map(
                |preset| preset.body["generated_from_fixture_profile"]["semantic_id"]
                    .as_str()
                    .unwrap()
            )
            .collect::<HashSet<_>>(),
        HashSet::from(["gobo.dots", "gobo.open"])
    );
}

#[tokio::test]
async fn active_preload_preset_uses_one_typed_show_boundary_and_returns_its_event_cursor() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Preload preset boundary").await;
    let show_id = show["id"].as_str().unwrap();
    let show_uuid = Uuid::parse_str(show_id).unwrap();
    let entry = state
        .desk
        .lock()
        .show(light_core::ShowId(show_uuid))
        .unwrap()
        .unwrap();
    let store = ShowStore::open(&entry.path).unwrap();
    store
        .put_object(
            "group",
            "1",
            &serde_json::json!({"id":"1","name":"Empty","fixtures":[]}),
            0,
        )
        .unwrap();
    store
        .put_object(
            "preset",
            "1.4",
            &serde_json::json!({
                "name":"Before",
                "family":"Intensity",
                "number":4,
                "values":{},
                "group_values":{},
                "future_extension":{"retained":true}
            }),
            0,
        )
        .unwrap();
    open_show_for_test(&app, &token, show_id).await;
    let session = authenticate_token(&state, &token).unwrap();
    assert!(state.programmers.set_preload_group(
        session.id,
        "1".into(),
        light_core::AttributeKey::intensity(),
        light_core::AttributeValue::Normalized(0.6),
    ));
    let before = state.engine.snapshot();
    let before_revision = store.portable_document().unwrap().revision().value();
    let before_backups = show_object_backup_count(&data_dir);
    let before_sequence = state.application_events.latest_sequence();

    let response = app
        .clone()
        .oneshot(
            Request::post(format!("/api/v1/shows/{show_id}/preload/store"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::IF_MATCH, "1")
                .body(Body::from(
                    serde_json::json!({
                        "target":"preset",
                        "target_id":"1.4",
                        "name":"From Preload",
                        "mode":"merge",
                        "family":"Intensity"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let response = json(response).await;
    assert_eq!(response["revision"], 2);
    assert_eq!(response["event_sequence"], before_sequence + 1);

    let document = store.portable_document().unwrap();
    assert_eq!(document.revision().value(), before_revision + 1);
    assert_eq!(show_object_backup_count(&data_dir), before_backups + 1);
    let preset = document.object("preset", "1.4").unwrap();
    assert_eq!(preset.revision(), 2);
    assert_eq!(preset.body()["future_extension"]["retained"], true);
    assert_eq!(preset.body()["name"], "From Preload");
    let runtime = state.engine.snapshot();
    assert_eq!(runtime.revision, document.revision().value());
    assert!(!std::sync::Arc::ptr_eq(&before, &runtime));

    let light_application::EventReplay::Events(events) = state
        .application_events
        .replay(before_sequence, &light_application::EventFilter::default())
    else {
        panic!("expected the authoritative Preload Preset event");
    };
    assert!(matches!(
        &events[0].payload,
        light_application::ApplicationEvent::Show(
            light_application::ShowEvent::ObjectsChanged(change)
        ) if change.changes.len() == 1
            && change.changes[0].kind == light_application::ActiveShowObjectKind::Preset
            && change.changes[0].object_id == "1.4"
    ));

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn group_and_preset_updates_each_install_the_exact_committed_revision() {
    let first = light_core::FixtureId::new();
    let added = light_core::FixtureId::new();
    let scenario = ActiveObjectScenario::new("Update boundary", |store| {
        store
            .put_object(
                "group",
                "81",
                &serde_json::json!({
                    "id":"81",
                    "name":"Update group",
                    "fixtures":[first],
                    "future_group_field":"retained"
                }),
                0,
            )
            .unwrap();
        let mut preset = preset_body("Update color", light_programmer::PresetFamily::Color, 9);
        preset["values"] = serde_json::json!({
            first.0.to_string(): {"color.red":{"kind":"normalized","value":0.2}}
        });
        preset["future_preset_field"] = serde_json::json!("retained");
        store.put_object("preset", "2.9", &preset, 0).unwrap();
    });
    scenario
        .state
        .configuration
        .write()
        .update_settings_by_desk
        .insert(
            scenario.session.desk.id,
            update::UpdateSettings {
                group_mode: update::ExistingContentMode::AddNew,
                preset_mode: update::ExistingContentMode::AddNew,
                ..Default::default()
            },
        );
    scenario
        .state
        .programmers
        .select(scenario.session.id, [first, added]);

    let before_group = scenario.boundary();
    assert_eq!(
        execute_programmer_command(&scenario.state, &scenario.session, "UPDATE GROUP 81").unwrap(),
        1
    );
    scenario.assert_one_commit(&before_group);
    let group = stored_object(&scenario.store(), "group", "81").unwrap();
    assert_eq!(group.body["fixtures"], serde_json::json!([first, added]));
    assert_eq!(group.body["future_group_field"], "retained");

    scenario.state.programmers.set(
        scenario.session.id,
        first,
        light_core::AttributeKey("color.blue".into()),
        light_core::AttributeValue::Normalized(0.8),
    );
    let before_preset = scenario.boundary();
    assert_eq!(
        execute_programmer_command(&scenario.state, &scenario.session, "UPDATE 2.9").unwrap(),
        1
    );
    scenario.assert_one_commit(&before_preset);
    let preset = stored_object(&scenario.store(), "preset", "2.9").unwrap();
    assert_eq!(preset.body["future_preset_field"], "retained");
    let preset: light_programmer::Preset = serde_json::from_value(preset.body).unwrap();
    assert_eq!(
        preset.values[&first][&light_core::AttributeKey("color.red".into())],
        light_core::AttributeValue::Normalized(0.2)
    );
    assert_eq!(
        preset.values[&first][&light_core::AttributeKey("color.blue".into())],
        light_core::AttributeValue::Normalized(0.8)
    );
}
