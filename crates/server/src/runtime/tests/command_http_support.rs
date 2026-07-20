struct CommandHttpScenario {
    state: AppState,
    app: Router,
    token: String,
    session: Session,
    path: String,
    data_dir: PathBuf,
}

impl CommandHttpScenario {
    async fn new() -> Self {
        let (state, data_dir) = test_state();
        let app = router(state.clone());
        let (token, _) = login(&app, "Operator").await;
        let session = state
            .sessions
            .read()
            .values()
            .find(|session| session.token == token)
            .cloned()
            .unwrap();
        Self {
            path: format!("/api/v2/desks/{}/command-line", session.desk.id),
            state,
            app,
            token,
            session,
            data_dir,
        }
    }

    fn install_direct_fixture(&self) -> light_core::FixtureId {
        let fixture = schema_v2_direct_fixture().0;
        let fixture_id = fixture.fixture_id;
        self.state
            .engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![fixture],
                groups: vec![light_programmer::GroupDefinition {
                    id: "1".into(),
                    name: "Group 1".into(),
                    fixtures: vec![fixture_id],
                    ..Default::default()
                }],
                revision: 1,
                ..EngineSnapshot::default()
            })
            .unwrap();
        fixture_id
    }

    async fn get(&self) -> Response {
        self.app
            .clone()
            .oneshot(
                Request::get(&self.path)
                    .header(
                        header::AUTHORIZATION,
                        format!("Bearer {}", self.token),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    async fn put(&self, text: &str, revision: u64) -> Response {
        self.app
            .clone()
            .oneshot(
                Request::put(&self.path)
                    .header(
                        header::AUTHORIZATION,
                        format!("Bearer {}", self.token),
                    )
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::IF_MATCH, revision.to_string())
                    .body(Body::from(
                        serde_json::json!({"text": text}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    async fn execute(&self, request_id: &str, command: Option<&str>) -> Response {
        let mut request = serde_json::json!({"request_id": request_id});
        if let Some(command) = command {
            request["command"] = command.into();
        }
        self.app
            .clone()
            .oneshot(
                Request::post(format!("{}/execute", self.path))
                    .header(
                        header::AUTHORIZATION,
                        format!("Bearer {}", self.token),
                    )
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(request.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    async fn interaction_snapshot(&self) -> Response {
        self.interaction_snapshot_for(self.session.desk.id).await
    }

    async fn interaction_snapshot_for(&self, desk_id: Uuid) -> Response {
        self.app
            .clone()
            .oneshot(
                Request::get(format!(
                    "/api/v2/desks/{}/programming-interaction/snapshot",
                    desk_id
                ))
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {}", self.token),
                )
                .body(Body::empty())
                .unwrap(),
            )
            .await
            .unwrap()
    }

    async fn values_snapshot(&self) -> Response {
        self.values_snapshot_for(self.session.user.id.0, Some(&self.token))
            .await
    }

    async fn values_snapshot_for(&self, user_id: Uuid, token: Option<&str>) -> Response {
        let mut request = Request::get(format!(
            "/api/v2/users/{user_id}/programmer-values/snapshot"
        ));
        if let Some(token) = token {
            request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        self.app
            .clone()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap()
    }

    async fn capture_mode_snapshot(&self) -> Response {
        self.capture_mode_snapshot_for(self.session.user.id.0, Some(&self.token))
            .await
    }

    async fn capture_mode_snapshot_for(&self, user_id: Uuid, token: Option<&str>) -> Response {
        let mut request = Request::get(format!(
            "/api/v2/users/{user_id}/programmer-capture-mode/snapshot"
        ));
        if let Some(token) = token {
            request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        self.app
            .clone()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap()
    }

    async fn lifecycle_snapshot(&self, token: Option<&str>) -> Response {
        let mut request = Request::get("/api/v2/programmer-lifecycle/snapshot");
        if let Some(token) = token {
            request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        self.app
            .clone()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap()
    }

    async fn values_action(&self, input: serde_json::Value) -> Response {
        self.values_action_for(self.session.user.id.0, &self.token, input)
            .await
    }

    async fn values_action_for(
        &self,
        user_id: Uuid,
        token: &str,
        input: serde_json::Value,
    ) -> Response {
        self.app
            .clone()
            .oneshot(
                Request::post(format!(
                    "/api/v2/users/{user_id}/programmer-values/actions"
                ))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(input.to_string()))
                .unwrap(),
            )
            .await
            .unwrap()
    }

    async fn preload_values_snapshot(&self) -> Response {
        self.preload_values_snapshot_for(self.session.user.id.0, Some(&self.token))
            .await
    }

    async fn preload_values_snapshot_for(&self, user_id: Uuid, token: Option<&str>) -> Response {
        let mut request = Request::get(format!(
            "/api/v2/users/{user_id}/programmer-preload-values/snapshot"
        ));
        if let Some(token) = token {
            request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        self.app
            .clone()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap()
    }

    async fn preload_values_action(&self, input: serde_json::Value) -> Response {
        self.preload_values_action_for(self.session.user.id.0, &self.token, input)
            .await
    }

    async fn preload_values_action_for(
        &self,
        user_id: Uuid,
        token: &str,
        input: serde_json::Value,
    ) -> Response {
        self.app
            .clone()
            .oneshot(
                Request::post(format!(
                    "/api/v2/users/{user_id}/programmer-preload-values/actions"
                ))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(input.to_string()))
                .unwrap(),
            )
            .await
            .unwrap()
    }

    async fn press_key(&self, token: &str, key: &str, request_id: &str) -> Response {
        self.app
            .clone()
            .oneshot(
                Request::post(format!("{}/keys", self.path))
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "key": key,
                            "phase": "press",
                            "request_id": request_id,
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    async fn selection_action(&self, input: serde_json::Value) -> Response {
        self.selection_action_for(self.session.desk.id, input).await
    }

    async fn selection_action_for(&self, desk_id: Uuid, input: serde_json::Value) -> Response {
        self.raw_selection_action(desk_id, Body::from(input.to_string()))
            .await
    }

    async fn raw_selection_action(&self, desk_id: Uuid, body: Body) -> Response {
        self.app
            .clone()
            .oneshot(
                Request::post(format!(
                    "/api/v2/desks/{}/programming-selection/actions",
                    desk_id
                ))
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {}", self.token),
                )
                .header(header::CONTENT_TYPE, "application/json")
                .body(body)
                .unwrap(),
            )
            .await
            .unwrap()
    }

    async fn create_and_open_show(&self, name: &str) -> String {
        let show = create_show(&self.app, &self.token, name).await;
        let show_id = show["id"].as_str().unwrap().to_owned();
        let response = self
            .app
            .clone()
            .oneshot(
                Request::post(format!("/api/v1/shows/{show_id}/open"))
                    .header(
                        header::AUTHORIZATION,
                        format!("Bearer {}", self.token),
                    )
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"transition":"hold_current"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        show_id
    }

    async fn put_active_object(
        &self,
        show_id: &str,
        kind: &str,
        object_id: &str,
        expected_revision: u64,
        body: serde_json::Value,
    ) -> Response {
        self.app
            .clone()
            .oneshot(
                Request::put(format!(
                    "/api/v1/shows/{show_id}/objects/{kind}/{object_id}"
                ))
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {}", self.token),
                )
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::IF_MATCH, expected_revision.to_string())
                .body(Body::from(body.to_string()))
                .unwrap(),
            )
            .await
            .unwrap()
    }

    fn history_len(&self) -> usize {
        self.state
            .command_history
            .lock()
            .get(&self.session.desk.id)
            .unwrap()
            .len()
    }
}
