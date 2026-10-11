use super::*;

struct UpdateRouteScenario {
    state: AppState,
    app: Router,
    session: Session,
    show_id: light_core::ShowId,
    data_dir: PathBuf,
    group_id: String,
    first: light_core::FixtureId,
    added: light_core::FixtureId,
}

impl UpdateRouteScenario {
    fn new() -> Self {
        let (state, data_dir) = test_state();
        let mut desk = test_control_desk();
        desk.id = Uuid::new_v4();
        let session = Session {
            capability: light_core::SurfaceCapability::Programming,
            id: SessionId::new(),
            token: "programming-update-v2".into(),
            connected: true,
            desk,
        };
        state.programming.start(session.id);
        attach_session_command_context(&state, &session);
        state.sessions.insert_session(session.clone());

        let first = light_core::FixtureId::new();
        let added = light_core::FixtureId::new();
        let group_id = "front".to_owned();
        state.programming.select_expression(
            session.id,
            vec![first, added],
            light_programmer::SelectionExpression::LiveGroup {
                group_id: group_id.clone(),
                rule: light_programmer::SelectionRule::All,
            },
        );
        let show_path = data_dir.join("shows/programming-update-v2.show");
        let show_id = initialise_show(&show_path, "Programming Update v2").unwrap();
        state.active_show.replace_current(Some(ShowEntry {
            is_base_show: false,
            id: show_id,
            name: "Programming Update v2".into(),
            path: show_path.display().to_string(),
            revision: 0,
            updated_at: String::new(),
            created_at: None,
            last_loaded_at: None,
            revision_copy: None,
        }));
        let mut body = serde_json::to_value(light_programmer::GroupDefinition {
            id: group_id.clone(),
            name: "Front".into(),
            fixtures: vec![first],
            ..Default::default()
        })
        .unwrap();
        body["future_group"] = serde_json::json!({"retained":true});
        ShowStore::open(&show_path)
            .unwrap()
            .put_object("group", &group_id, &body, 0)
            .unwrap();
        let app = router(state.clone());
        Self {
            state,
            app,
            session,
            show_id,
            data_dir,
            group_id,
            first,
            added,
        }
    }

    fn revision(&self) -> u64 {
        self.store().portable_revision().unwrap().value()
    }

    fn store(&self) -> ShowStore {
        let entry = self.state.active_show.current().clone().unwrap();
        ShowStore::open(entry.path).unwrap()
    }

    fn target(&self) -> serde_json::Value {
        serde_json::json!({"type":"group","object_id":self.group_id})
    }

    async fn post(
        &self,
        operation: &str,
        body: serde_json::Value,
        revision: Option<u64>,
    ) -> Response {
        let mut request = Request::post(format!("/api/v2/programming-update/{operation}"))
            .header("x-tosk-show", self.show_id.0.to_string())
            .header(
                header::AUTHORIZATION,
                format!("Bearer {}", self.session.token),
            )
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(revision) = revision {
            request = request.header(header::IF_MATCH, revision.to_string());
        }
        self.app
            .clone()
            .oneshot(request.body(Body::from(body.to_string())).unwrap())
            .await
            .unwrap()
    }

    fn cleanup(self) {
        let _ = std::fs::remove_dir_all(self.data_dir);
    }
}

#[tokio::test]
async fn v2_update_locked_reads_and_settings_keep_exact_scope() {
    let scenario = UpdateRouteScenario::new();
    let revision = scenario.revision();
    scenario
        .state
        .installation
        .update_configuration(|configuration| {
            configuration.update_settings = update::UpdateSettings {
                other_target_modes: HashMap::from([(
                    "future".into(),
                    update::ExistingContentMode::AddNew,
                )]),
                ..Default::default()
            };
        });
    write_desk_lock(
        &scenario.state,
        &DeskLockConfiguration {
            locked: true,
            ..Default::default()
        },
    )
    .unwrap();
    let cursor = scenario.state.events.latest_sequence();

    let preview = scenario
        .post(
            "preview",
            serde_json::json!({
                "request_id":"preview-locked",
                "target":scenario.target(),
                "mode":{"target_type":"existing_content","mode":"add_new"}
            }),
            None,
        )
        .await;
    assert_eq!(preview.status(), StatusCode::OK);
    assert_eq!(preview.headers()[header::ETAG], format!("\"{revision}\""));
    let preview = json(preview).await;
    assert_eq!(preview["show_id"], scenario.show_id.0.to_string());
    assert_eq!(preview["object"]["object_id"], scenario.group_id);
    assert_eq!(preview["preview"]["items"].as_array().unwrap().len(), 2);

    let targets = scenario
        .post(
            "targets",
            serde_json::json!({"request_id":"targets-locked","filter":"show_all_active"}),
            None,
        )
        .await;
    assert_eq!(targets.status(), StatusCode::OK);
    assert_eq!(targets.headers()[header::ETAG], format!("\"{revision}\""));
    let targets = json(targets).await;
    assert_eq!(targets["targets"].as_array().unwrap().len(), 1);
    assert_eq!(
        targets["targets"][0]["object"]["object_id"],
        scenario.group_id
    );
    assert_eq!(
        targets["targets"][0]["programmer_revision"]
            .as_str()
            .unwrap()
            .len(),
        64
    );
    assert_eq!(scenario.state.events.latest_sequence(), cursor);
    assert_eq!(scenario.revision(), revision);

    let blocked_action = scenario
        .post(
            "actions",
            direct_group_action(&scenario, "locked-action", "add_new"),
            Some(revision),
        )
        .await;
    assert_eq!(blocked_action.status(), StatusCode::CONFLICT);
    assert_eq!(json(blocked_action).await["kind"], "conflict");
    assert_eq!(scenario.state.events.latest_sequence(), cursor);

    let settings_path = "/api/v2/programming-update/settings".to_owned();
    let settings = scenario
        .app
        .clone()
        .oneshot(
            Request::get(&settings_path)
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {}", scenario.session.token),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(settings.status(), StatusCode::OK);
    assert!(
        json(settings).await["settings"]
            .get("other_target_modes")
            .is_none()
    );

    let blocked = put_settings_request(&scenario, &settings_path).await;
    assert_eq!(blocked.status(), StatusCode::CONFLICT);
    let blocked = json(blocked).await;
    assert_eq!(blocked["kind"], "conflict", "{blocked}");
    // The route named a desk while there could be several. A caller still sending one addresses
    // nothing, and the settings it would have scoped are the desk's own.
    let retired = scenario
        .app
        .clone()
        .oneshot(
            Request::get(format!(
                "/api/v2/desks/{}/programming-update/settings",
                Uuid::new_v4()
            ))
            .header(
                header::AUTHORIZATION,
                format!("Bearer {}", scenario.session.token),
            )
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(retired.status(), StatusCode::NOT_FOUND);

    write_desk_lock(&scenario.state, &DeskLockConfiguration::default()).unwrap();
    let saved = put_settings_request(&scenario, &settings_path).await;
    assert_eq!(saved.status(), StatusCode::OK);
    assert_eq!(json(saved).await["settings"]["cue_mode"], "existing_only");
    assert_eq!(
        scenario
            .state
            .installation
            .configuration()
            .update_settings
            .other_target_modes["future"],
        update::ExistingContentMode::AddNew
    );
    scenario.cleanup();
}

#[tokio::test]
async fn v2_update_action_is_lossless_replay_safe_and_one_event() {
    let scenario = UpdateRouteScenario::new();
    let initial_revision = scenario.revision();
    let cursor = scenario.state.events.latest_sequence();
    let action = direct_group_action(&scenario, "group-add", "add_new");

    let changed = scenario
        .post("actions", action.clone(), Some(initial_revision))
        .await;
    assert_eq!(changed.status(), StatusCode::OK);
    let changed_etag = changed.headers()[header::ETAG].clone();
    let changed = json(changed).await;
    assert_eq!(changed["status"], "changed");
    assert_eq!(changed["replayed"], false);
    assert_eq!(changed["summary"]["changed_count"], 1);
    assert_eq!(
        changed["projection"]["body"]["future_group"]["retained"],
        true
    );
    let show_filter = light_application::EventFilter::default().with_object(
        light_application::EventObject::show_storage_object(
            scenario.show_id,
            "group",
            &scenario.group_id,
        ),
    );
    let light_application::EventReplay::Events(show_events) =
        scenario.state.events.replay(cursor, &show_filter)
    else {
        panic!("the focused Programming update Show event must remain replayable")
    };
    assert_eq!(show_events.len(), 1);
    assert_eq!(changed["event_sequence"], show_events[0].sequence);
    let committed_revision = changed["show_revision"].as_u64().unwrap();
    assert_eq!(changed_etag, format!("\"{committed_revision}\""));

    let replay = scenario
        .post("actions", action, Some(initial_revision))
        .await;
    assert_eq!(replay.status(), StatusCode::OK);
    let replay = json(replay).await;
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["event_sequence"], show_events[0].sequence);
    let light_application::EventReplay::Events(replayed_show_events) =
        scenario.state.events.replay(cursor, &show_filter)
    else {
        panic!("the focused Programming update Show event must remain replayable")
    };
    assert_eq!(replayed_show_events.len(), 1);

    let collision = scenario
        .post(
            "actions",
            direct_group_action(&scenario, "group-add", "update_existing"),
            Some(initial_revision),
        )
        .await;
    assert_eq!(collision.status(), StatusCode::CONFLICT);
    assert!(
        json(collision).await["error"]
            .as_str()
            .unwrap()
            .contains("different")
    );

    let no_op = scenario
        .post(
            "actions",
            direct_group_action(&scenario, "group-no-op", "add_new"),
            Some(committed_revision),
        )
        .await;
    assert_eq!(no_op.status(), StatusCode::BAD_REQUEST);
    assert_eq!(json(no_op).await["kind"], "invalid");
    let light_application::EventReplay::Events(show_events_after_no_op) =
        scenario.state.events.replay(cursor, &show_filter)
    else {
        panic!("the focused Programming update Show event must remain replayable")
    };
    assert_eq!(show_events_after_no_op.len(), 1);

    let stale_show = scenario
        .post(
            "actions",
            direct_group_action(&scenario, "group-stale", "update_existing"),
            Some(initial_revision),
        )
        .await;
    assert_eq!(stale_show.status(), StatusCode::CONFLICT);
    assert_eq!(
        stale_show.headers()[header::ETAG],
        format!("\"{committed_revision}\"")
    );
    let stale_show = json(stale_show).await;
    assert_eq!(stale_show["current_show_revision"], committed_revision);
    let light_application::EventReplay::Events(show_events_after_conflict) =
        scenario.state.events.replay(cursor, &show_filter)
    else {
        panic!("the focused Programming update Show event must remain replayable")
    };
    assert_eq!(show_events_after_conflict.len(), 1);

    let stored = scenario
        .store()
        .objects("group")
        .unwrap()
        .into_iter()
        .find(|object| object.id == scenario.group_id)
        .unwrap();
    assert_eq!(stored.body["future_group"]["retained"], true);
    let group = serde_json::from_value::<light_programmer::GroupDefinition>(stored.body).unwrap();
    assert_eq!(group.fixtures, vec![scenario.first, scenario.added]);
    scenario.cleanup();
}

async fn put_settings_request(scenario: &UpdateRouteScenario, path: &str) -> Response {
    scenario
        .app
        .clone()
        .oneshot(
            Request::post(path)
                .header(header::CONTENT_TYPE, "application/json")
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {}", scenario.session.token),
                )
                .body(Body::from(
                    serde_json::json!({
                        "request_id":"settings-update",
                        "settings":{
                            "cue_mode":"existing_only",
                            "preset_mode":"add_new",
                            "group_mode":"add_new",
                            "show_update_modal_on_touch":false,
                            "record_default":"smart",
                            "update_default":"smart"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
}

fn direct_group_action(
    scenario: &UpdateRouteScenario,
    request_id: &str,
    mode: &str,
) -> serde_json::Value {
    serde_json::json!({
        "request_id":request_id,
        "action":{
            "type":"apply_direct",
            "target":scenario.target(),
            "mode":{"target_type":"existing_content","mode":mode}
        }
    })
}

#[tokio::test]
async fn v2_update_waits_before_programming_gates_under_ordinary_writer_contention() {
    let scenario = UpdateRouteScenario::new();
    let initial_revision = scenario.revision();
    let writer = scenario.state.active_show.acquire().await;
    let action = direct_group_action(&scenario, "update-writer-wait", "add_new");
    let mut request = Box::pin(scenario.post("actions", action.clone(), Some(initial_revision)));
    assert!(
        tokio::time::timeout(Duration::from_millis(50), request.as_mut())
            .await
            .is_err(),
        "ordinary writer contention must wait instead of returning a false show-changing conflict"
    );
    let state = scenario.state.clone();
    let session = scenario.session.clone();
    let mut probe = tokio::task::spawn_blocking(move || {
        let context = light_application::ActionContext::operator(
            session.desk.id,
            session.id.0,
            light_application::ActionSource::Http,
        );
        let ports = command_http::ServerProgrammingPorts::new(
            &state,
            &session,
            "update-admission-probe",
            false,
        );
        state
            .programming
            .run_external_interaction(&context, &ports, || ())
    });
    let free_programming_gate = tokio::time::timeout(Duration::from_millis(250), &mut probe).await;
    if free_programming_gate.is_err() {
        drop(writer);
        let _ = request.await;
        let _ = probe.await;
        panic!("an Update waiting for show admission must not own the Programmer/desk gate");
    }
    free_programming_gate.unwrap().unwrap().unwrap();
    assert_eq!(scenario.revision(), initial_revision);
    drop(writer);
    let response = request.await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json(response).await["replayed"], false);
    let stored = scenario
        .store()
        .object("group", &scenario.group_id)
        .unwrap()
        .unwrap();
    let group = serde_json::from_value::<light_programmer::GroupDefinition>(stored.body).unwrap();
    assert_eq!(group.fixtures, vec![scenario.first, scenario.added]);
    let replay = scenario
        .post("actions", action, Some(initial_revision))
        .await;
    assert_eq!(replay.status(), StatusCode::OK);
    assert_eq!(json(replay).await["replayed"], true);
    scenario.cleanup();
}

#[tokio::test]
async fn v2_update_rejects_cold_replacement_including_same_id_before_capture() {
    for same_id in [true, false] {
        let mut scenario = UpdateRouteScenario::new();
        let current = scenario.state.active_show.current().clone().unwrap();
        let entry = scenario
            .state
            .installation
            .upsert_show(&current.name, &current.path, false)
            .unwrap();
        ActiveShowRepository::open(&entry.path)
            .unwrap()
            .set_identity(entry.id, &entry.name, None)
            .unwrap();
        scenario
            .state
            .active_show
            .replace_current(Some(entry.clone()));
        scenario.show_id = entry.id;
        let revision = scenario.revision();
        let action = direct_group_action(&scenario, "queued-cold-update", "add_new");
        let epoch = scenario.state.integrations.osc_playback_epoch();
        let pause = scenario.state.active_show.http_lifecycle_probe();
        pause.arm();
        let app = scenario.app.clone();
        let token = scenario.session.token.clone();
        let id = scenario.show_id;
        let body = action.clone();
        let queued = tokio::spawn(async move {
            app.oneshot(
                Request::post("/api/v2/programming-update/actions")
                    .header("x-tosk-show", id.0.to_string())
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::IF_MATCH, revision.to_string())
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap()
        });
        let observing = pause.clone();
        tokio::task::spawn_blocking(move || observing.wait_until_started())
            .await
            .unwrap();
        let destination = if same_id {
            entry.id.0.to_string()
        } else {
            create_show(
                &scenario.app,
                &scenario.session.token,
                "Other Update destination",
            )
            .await["id"]
                .as_str()
                .unwrap()
                .to_owned()
        };
        let opened = scenario
            .app
            .clone()
            .oneshot(open_show_request(&scenario.session.token, &destination))
            .await
            .unwrap();
        // Cold installation may canonically materialize the Group source. The
        // refused queued Update must preserve the accepted replacement exactly.
        let before = ShowStore::open(&entry.path)
            .unwrap()
            .object("group", &scenario.group_id)
            .unwrap()
            .unwrap()
            .body;
        assert_eq!(before["future_group"]["retained"], true);
        assert_eq!(before["fixtures"], serde_json::json!([scenario.first]));
        pause.release();
        assert_eq!(opened.status(), StatusCode::OK, "{}", json(opened).await);
        assert!(
            scenario.state.integrations.osc_playback_epoch() > epoch,
            "the real accepted HTTP cold install must invalidate the captured runtime"
        );
        let rejected = queued.await.unwrap();
        assert_eq!(rejected.status(), StatusCode::CONFLICT);
        assert!(
            json(rejected).await["error"]
                .as_str()
                .unwrap()
                .contains("runtime was replaced")
        );
        assert_eq!(
            ShowStore::open(&entry.path)
                .unwrap()
                .object("group", &scenario.group_id)
                .unwrap()
                .unwrap()
                .body,
            before
        );
        if same_id {
            // A refused queued action did not occupy the edit replay identity.
            scenario.state.programming.select_expression(
                scenario.session.id,
                vec![scenario.first, scenario.added],
                light_programmer::SelectionExpression::LiveGroup {
                    group_id: scenario.group_id.clone(),
                    rule: light_programmer::SelectionRule::All,
                },
            );
            let fresh = scenario
                .post("actions", action, Some(scenario.revision()))
                .await;
            assert_eq!(fresh.status(), StatusCode::OK, "{}", json(fresh).await);
        } else {
            assert!(scenario.state.active_show.current().unwrap().id != scenario.show_id);
            assert!(
                scenario
                    .store()
                    .object("group", &scenario.group_id)
                    .unwrap()
                    .is_none()
            );
        }
        scenario.cleanup();
    }
}

#[tokio::test]
async fn v2_update_preserves_preview_fingerprint_checks_after_admission_wait() {
    let scenario = UpdateRouteScenario::new();
    let revision = scenario.revision();
    let preview = scenario
        .post(
            "preview",
            serde_json::json!({
                "request_id":"preview-before-writer", "target":scenario.target(),
                "mode":{"target_type":"existing_content","mode":"add_new"}
            }),
            None,
        )
        .await;
    assert_eq!(preview.status(), StatusCode::OK);
    let preview = json(preview).await;
    let action = serde_json::json!({"request_id":"confirmed-waiting-update","action":{
        "type":"confirm_preview", "target":scenario.target(),
        "mode":{"target_type":"existing_content","mode":"add_new"},
        "expected_object_revision":preview["object"]["object_revision"],
        "expected_programmer_revision":preview["programmer_revision"]
    }});
    let writer = scenario.state.active_show.acquire().await;
    let mut queued = Box::pin(scenario.post("actions", action, Some(revision)));
    assert!(
        tokio::time::timeout(Duration::from_millis(50), queued.as_mut())
            .await
            .is_err()
    );
    scenario.state.programming.select_expression(
        scenario.session.id,
        vec![scenario.first],
        light_programmer::SelectionExpression::LiveGroup {
            group_id: scenario.group_id.clone(),
            rule: light_programmer::SelectionRule::All,
        },
    );
    drop(writer);
    let rejected = queued.await;
    assert_eq!(rejected.status(), StatusCode::CONFLICT);
    assert!(
        json(rejected).await["error"]
            .as_str()
            .unwrap()
            .contains("programmer content changed")
    );
    assert_eq!(scenario.revision(), revision);
    scenario.cleanup();
}

#[tokio::test]
async fn v2_color_preset_update_all_waits_and_retains_semantic_group_color() {
    let scenario = UpdateRouteScenario::new();
    let color = |rgb: [f32; 3]| {
        light_core::AttributeValue::ColorProgram(
            light_core::programming::ColorProgram::Semantic {
                intent: light_core::programming::ColorIntent {
                    base_xyz: light_fixture::srgb_to_xyz(rgb[0], rgb[1], rgb[2]),
                    recipe: light_core::programming::VirtualColorRecipe {
                        rgb,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            }
            .into(),
        )
    };
    let mut preset = light_programmer::Preset {
        number: 1,
        instance_id: Some(Uuid::new_v4()),
        family: light_programmer::PresetFamily::Color,
        group_values: HashMap::from([(
            scenario.group_id.clone(),
            HashMap::from([(light_core::AttributeKey::color(), color([0.0, 0.0, 1.0]))]),
        )]),
        ..Default::default()
    };
    preset.consolidate_universal_color();
    let instance = preset.instance_id;
    scenario
        .store()
        .put_object("preset", "2.1", &serde_json::to_value(&preset).unwrap(), 0)
        .unwrap();
    let red = color([1.0, 0.0, 0.0]);
    assert!(scenario.state.programming.set_group(
        scenario.session.id,
        scenario.group_id.clone(),
        light_core::AttributeKey::color(),
        red.clone()
    ));
    let revision = scenario.revision();
    let action = serde_json::json!({"request_id":"color-update-all-wait","action":{
        "type":"apply_direct", "target":{"type":"preset","object_id":"2.1"},
        "mode":{"target_type":"existing_content","mode":"add_new"}
    }});
    let writer = scenario.state.active_show.acquire().await;
    let mut queued = Box::pin(scenario.post("actions", action, Some(revision)));
    assert!(
        tokio::time::timeout(Duration::from_millis(50), queued.as_mut())
            .await
            .is_err()
    );
    drop(writer);
    let response = queued.await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "{}",
        json(response).await
    );
    let stored: light_programmer::Preset = serde_json::from_value(
        scenario
            .store()
            .object("preset", "2.1")
            .unwrap()
            .unwrap()
            .body,
    )
    .unwrap();
    assert_eq!(stored.instance_id, instance);
    assert_eq!(
        stored
            .universal_values
            .get(&light_core::AttributeKey::color()),
        Some(&red)
    );
    assert!(stored.values.is_empty());
    assert!(stored.group_values.is_empty());
    scenario.cleanup();
}

#[tokio::test(flavor = "current_thread")]
async fn v2_update_tolerates_unknown_nested_request_fields_but_rejects_malformed_known_fields() {
    let log = UpdateRequestLog::default();
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_writer(log.clone())
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    let scenario = UpdateRouteScenario::new();
    let target = serde_json::json!({"type":"group", "object_id":scenario.group_id,
        "future_target":{"retained":true}});
    let mode = serde_json::json!({"target_type":"existing_content", "mode":"add_new",
        "future_mode":true});
    let preview = scenario
        .post(
            "preview",
            serde_json::json!({
                "request_id":"extras-preview", "target":target, "mode":mode,
                "programmer":{"forged":true}, "future_wrapper":true
            }),
            None,
        )
        .await;
    assert_eq!(preview.status(), StatusCode::OK, "{}", json(preview).await);
    let targets = scenario
        .post(
            "targets",
            serde_json::json!({
                "request_id":"extras-targets", "filter":"show_all_active", "future_wrapper":true
            }),
            None,
        )
        .await;
    assert_eq!(targets.status(), StatusCode::OK, "{}", json(targets).await);
    let revision = scenario.revision();
    for (label, bad_target, bad_mode) in [
        (
            "target",
            serde_json::json!({"type":"group","object_id":17}),
            mode.clone(),
        ),
        (
            "mode",
            target.clone(),
            serde_json::json!({"target_type":"existing_content","mode":"unknown"}),
        ),
        (
            "family",
            target.clone(),
            serde_json::json!({"target_type":"cue","mode":"add_new"}),
        ),
    ] {
        let rejected = scenario
            .post(
                "actions",
                serde_json::json!({
                    "request_id":format!("malformed-{label}"), "action":{"type":"apply_direct",
                        "target":bad_target,"mode":bad_mode}
                }),
                Some(revision),
            )
            .await;
        assert!(
            rejected.status().is_client_error(),
            "{label}: {}",
            json(rejected).await
        );
        assert_eq!(scenario.revision(), revision);
    }
    let accepted = scenario
        .post(
            "actions",
            serde_json::json!({
                "request_id":"extras-action", "future_wrapper":true,
                "action":{"type":"apply_direct", "target":target, "mode":mode,
                    "expected_show_revision":"ignored", "future_action":true}
            }),
            Some(revision),
        )
        .await;
    assert_eq!(
        accepted.status(),
        StatusCode::OK,
        "{}",
        json(accepted).await
    );
    let body = scenario
        .store()
        .object("group", &scenario.group_id)
        .unwrap()
        .unwrap()
        .body;
    assert_eq!(
        body["fixtures"],
        serde_json::json!([scenario.first, scenario.added])
    );
    assert_eq!(body["future_group"]["retained"], true);
    let output = String::from_utf8(log.0.lock().unwrap().clone()).unwrap();
    for route in ["preview", "targets", "actions"] {
        assert!(
            output.contains(&format!("/api/v2/programming-update/{route}")),
            "{output}"
        );
    }
    for path in [
        "future_wrapper",
        "target.future_target",
        "mode.future_mode",
        "action.future_action",
        "action.target.future_target",
        "action.mode.future_mode",
    ] {
        assert!(output.contains(path), "missing {path}: {output}");
    }
    assert!(!output.contains("forged"));
    scenario.cleanup();
}

#[derive(Clone, Default)]
struct UpdateRequestLog(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);
struct UpdateRequestLogWriter(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);
impl std::io::Write for UpdateRequestLogWriter {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buffer);
        Ok(buffer.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for UpdateRequestLog {
    type Writer = UpdateRequestLogWriter;
    fn make_writer(&'a self) -> Self::Writer {
        UpdateRequestLogWriter(self.0.clone())
    }
}
