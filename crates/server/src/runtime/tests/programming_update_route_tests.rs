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
        let user = state.desk.lock().users().unwrap().remove(0);
        let mut desk = test_control_desk();
        desk.id = Uuid::new_v4();
        let session = Session {
            id: SessionId::new(),
            user: user.clone(),
            token: "programming-update-v2".into(),
            connected: true,
            desk,
        };
        state.programmers.start(session.id, user.id);
        attach_session_command_context(&state, &session);
        state.sessions.write().insert(session.id, session.clone());

        let first = light_core::FixtureId::new();
        let added = light_core::FixtureId::new();
        let group_id = "front".to_owned();
        state.programmers.select_expression(
            session.id,
            vec![first, added],
            light_programmer::SelectionExpression::LiveGroup {
                group_id: group_id.clone(),
                rule: light_programmer::SelectionRule::All,
            },
        );
        let show_path = data_dir.join("shows/programming-update-v2.show");
        let show_id = initialise_show(&show_path, "Programming Update v2").unwrap();
        *state.active_show.write() = Some(ShowEntry {
            id: show_id,
            name: "Programming Update v2".into(),
            path: show_path.display().to_string(),
            revision: 0,
            updated_at: String::new(),
            revision_copy: None,
        });
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
        let entry = self.state.active_show.read().clone().unwrap();
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
        let mut request = Request::post(format!(
            "/api/v2/shows/{}/programming-update/{operation}",
            self.show_id.0
        ))
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
        .configuration
        .write()
        .update_settings_by_desk
        .insert(
            scenario.session.desk.id,
            update::UpdateSettings {
                other_target_modes: HashMap::from([(
                    "future".into(),
                    update::ExistingContentMode::AddNew,
                )]),
                ..Default::default()
            },
        );
    write_desk_lock(
        &scenario.state,
        scenario.session.desk.id,
        &DeskLockConfiguration {
            locked: true,
            ..Default::default()
        },
    )
    .unwrap();
    let cursor = scenario.state.application_events.latest_sequence();

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
    assert_eq!(scenario.state.application_events.latest_sequence(), cursor);
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
    assert_eq!(scenario.state.application_events.latest_sequence(), cursor);

    let settings_path = format!(
        "/api/v2/desks/{}/programming-update/settings",
        scenario.session.desk.id
    );
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
    assert_eq!(json(blocked).await["kind"], "conflict");
    let foreign = format!(
        "/api/v2/desks/{}/programming-update/settings",
        Uuid::new_v4()
    );
    let foreign = scenario
        .app
        .clone()
        .oneshot(
            Request::get(foreign)
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {}", scenario.session.token),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(foreign.status(), StatusCode::FORBIDDEN);
    assert_eq!(json(foreign).await["kind"], "forbidden");

    write_desk_lock(
        &scenario.state,
        scenario.session.desk.id,
        &DeskLockConfiguration::default(),
    )
    .unwrap();
    let saved = put_settings_request(&scenario, &settings_path).await;
    assert_eq!(saved.status(), StatusCode::OK);
    assert_eq!(json(saved).await["settings"]["cue_mode"], "existing_only");
    assert_eq!(
        scenario.state.configuration.read().update_settings_by_desk[&scenario.session.desk.id]
            .other_target_modes["future"],
        update::ExistingContentMode::AddNew
    );
    scenario.cleanup();
}

#[tokio::test]
async fn v2_update_action_is_lossless_replay_safe_and_one_event() {
    let scenario = UpdateRouteScenario::new();
    let initial_revision = scenario.revision();
    let cursor = scenario.state.application_events.latest_sequence();
    let mut compatibility = scenario.state.events.subscribe();
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
    assert_eq!(changed["event_sequence"], cursor + 1);
    let committed_revision = changed["show_revision"].as_u64().unwrap();
    assert_eq!(changed_etag, format!("\"{committed_revision}\""));
    assert_eq!(
        scenario.state.application_events.latest_sequence(),
        cursor + 1
    );
    assert!(
        std::iter::from_fn(|| compatibility.try_recv().ok())
            .all(|event| event.kind != "show_object_changed")
    );

    let replay = scenario
        .post("actions", action, Some(initial_revision))
        .await;
    assert_eq!(replay.status(), StatusCode::OK);
    let replay = json(replay).await;
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["event_sequence"], cursor + 1);
    assert_eq!(
        scenario.state.application_events.latest_sequence(),
        cursor + 1
    );

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
    assert_eq!(
        scenario.state.application_events.latest_sequence(),
        cursor + 1
    );

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
    assert_eq!(
        scenario.state.application_events.latest_sequence(),
        cursor + 1
    );

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
            Request::put(path)
                .header(header::CONTENT_TYPE, "application/json")
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {}", scenario.session.token),
                )
                .body(Body::from(
                    serde_json::json!({
                        "cue_mode":"existing_only",
                        "preset_mode":"add_new",
                        "group_mode":"add_new",
                        "show_update_modal_on_touch":false
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
