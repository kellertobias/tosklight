impl OperationalScenario {
    async fn seed_and_open_show(&self) {
        for (object_type, object_id, body) in [
            (
                "patched_fixture",
                "dimmer",
                serde_json::to_value(operational_fixture(self.fixture_id)).unwrap(),
            ),
            (
                "cue_list",
                "main",
                serde_json::to_value(operational_cue_list(self.cue_list_id, self.fixture_id))
                    .unwrap(),
            ),
            (
                "route",
                "sacn",
                serde_json::to_value(operational_route()).unwrap(),
            ),
        ] {
            assert_eq!(
                put_show_object(
                    &self.app,
                    &self.token,
                    &self.first_id,
                    object_type,
                    object_id,
                    body,
                )
                .await
                .status(),
                StatusCode::OK
            );
        }
        let opened = self
            .app
            .clone()
            .oneshot(
                Request::post(format!("/api/v1/shows/{}/open", self.first_id))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(
                        header::AUTHORIZATION,
                        format!("Bearer {}", self.token),
                    )
                    .body(Body::from(r#"{"transition":"hold_current"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(opened.status(), StatusCode::OK);
        assert_eq!(self.state.engine.snapshot().fixtures.len(), 1);
        let patch = self
            .app
            .clone()
            .oneshot(Request::get("/api/v1/patch").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(json(patch).await["fixtures"].as_array().unwrap().len(), 1);
    }

    async fn exercise_output_and_programmer(&self) {
        let response = self
            .app
            .clone()
            .oneshot(
                Request::put("/api/v1/dmx/override")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(
                        header::AUTHORIZATION,
                        format!("Bearer {}", self.token),
                    )
                    .body(Body::from(r#"{"universe":1,"address":1,"value":200}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            self.state.output_control.lock().raw_overrides.get(&(1, 1)),
            Some(&200)
        );
        let dmx = self
            .app
            .clone()
            .oneshot(Request::get("/api/v1/dmx").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(json(dmx).await["overrides"].as_array().unwrap().len(), 1);
        let response = self
            .app
            .clone()
            .oneshot(
                Request::post("/api/v1/programmer/set")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(
                        header::AUTHORIZATION,
                        format!("Bearer {}", self.token),
                    )
                    .body(Body::from(
                        serde_json::json!({
                            "fixture_id": self.fixture_id,
                            "attribute": "intensity",
                            "value": 0.5
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(self.rendered_intensity(), 128);
        let session = authenticate_token(&self.state, &self.token).unwrap();
        assert_eq!(
            execute_programmer_command(&self.state, &session, "FIXTURE 1").unwrap(),
            1
        );
        assert_eq!(
            execute_programmer_command(&self.state, &session, "FIXTURE 1 AT 25 TIME 0").unwrap(),
            1
        );
        assert_eq!(self.rendered_intensity(), 64);
        assert_eq!(
            self.state.programmers.get(session.id).unwrap().selected,
            vec![self.fixture_id]
        );
    }

    fn rendered_intensity(&self) -> u8 {
        self.state
            .engine
            .render(RenderOptions::default())
            .unwrap()
            .universes[&1][0]
    }

    async fn exercise_presets_and_playback(&self) {
        let session = authenticate_token(&self.state, &self.token).unwrap();
        let preset = light_programmer::Preset {
            name: "Three quarter".into(),
            family: light_programmer::PresetFamily::Intensity,
            number: 1,
            values: std::collections::HashMap::from([(
                self.fixture_id,
                std::collections::HashMap::from([(
                    light_core::AttributeKey::intensity(),
                    light_core::AttributeValue::Normalized(0.75),
                )]),
            )]),
            group_values: std::collections::HashMap::new(),
        };
        assert_eq!(
            put_show_object(
                &self.app,
                &self.token,
                &self.first_id,
                "preset",
                "1.1",
                serde_json::to_value(preset).unwrap(),
            )
            .await
            .status(),
            StatusCode::OK
        );
        let applied = dispatch_ws_command(
            &self.state,
            &session,
            WsCommand {
                protocol_version: 1,
                request_id: "preset-test".into(),
                session_id: session.id,
                expected_revision: None,
                command: "preset.apply".into(),
                payload: serde_json::json!({"family":"Intensity","number":1}),
            },
        );
        assert!(applied.ok);
        let programmer = self.state.programmers.get(session.id).unwrap();
        assert_eq!(programmer.values[0].fade_millis, Some(3_000));
        assert_eq!(programmer.values[0].value.normalized(), Some(0.75));
        apply_command_preset(&self.state, &session, "1.1", &[self.fixture_id]).unwrap();
        assert_eq!(
            self.state.programmers.get(session.id).unwrap().values[0].fade_millis,
            Some(3_000)
        );
        let go = self
            .app
            .clone()
            .oneshot(
                Request::post(format!("/api/v1/playbacks/{}/go", self.cue_list_id.0))
                    .header(
                        header::AUTHORIZATION,
                        format!("Bearer {}", self.token),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(go.status(), StatusCode::OK);
        let playback = self
            .app
            .clone()
            .oneshot(
                Request::get("/api/v1/playbacks")
                    .header(
                        header::AUTHORIZATION,
                        format!("Bearer {}", self.token),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(json(playback).await["active"].as_array().unwrap().len(), 1);
    }

    async fn verify_service_endpoints(&self) {
        for path in ["/api/v1/diagnostics", "/api/v1/readiness"] {
            let response = self
                .app
                .clone()
                .oneshot(
                    Request::get(path)
                        .header(
                            header::AUTHORIZATION,
                            format!("Bearer {}", self.token),
                        )
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
        }
        let download = self
            .app
            .clone()
            .oneshot(
                Request::get(format!("/api/v1/shows/{}/download", self.first_id))
                    .header(
                        header::AUTHORIZATION,
                        format!("Bearer {}", self.token),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(download.status(), StatusCode::OK);
        assert_eq!(
            download.headers()[header::CONTENT_TYPE],
            "application/vnd.light.show"
        );
    }

    async fn exercise_show_and_session_lifecycle(&self) {
        let second = create_show(&self.app, &self.token, "Second").await;
        let second_id = second["id"].as_str().unwrap();
        let opened = self
            .app
            .clone()
            .oneshot(
                Request::post(format!("/api/v1/shows/{second_id}/open"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(
                        header::AUTHORIZATION,
                        format!("Bearer {}", self.token),
                    )
                    .body(Body::from(
                        r#"{"transition":"timed_fade","transition_millis":100}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(opened.status(), StatusCode::OK);
        let rollback = self
            .app
            .clone()
            .oneshot(
                Request::post("/api/v1/shows/rollback")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(
                        header::AUTHORIZATION,
                        format!("Bearer {}", self.token),
                    )
                    .body(Body::from(r#"{"transition":"hold_current"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(rollback.status(), StatusCode::OK);
        assert_eq!(
            self.state.active_show.read().as_ref().unwrap().id.0.to_string(),
            self.first_id
        );
        self.verify_show_deletions(second_id).await;
        self.disconnect_and_clear_programmer().await;
    }

    async fn verify_show_deletions(&self, second_id: &str) {
        for (show_id, expected) in [
            (second_id, StatusCode::NO_CONTENT),
            (&self.first_id, StatusCode::CONFLICT),
        ] {
            let response = self
                .app
                .clone()
                .oneshot(
                    Request::delete(format!("/api/v1/shows/{show_id}"))
                        .header(
                            header::AUTHORIZATION,
                            format!("Bearer {}", self.token),
                        )
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), expected);
        }
    }

    async fn disconnect_and_clear_programmer(&self) {
        let disconnected = self
            .app
            .clone()
            .oneshot(
                Request::delete(format!("/api/v1/sessions/{}", self.session_id))
                    .header(
                        header::AUTHORIZATION,
                        format!("Bearer {}", self.token),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(disconnected.status(), StatusCode::NO_CONTENT);
        let session_id = SessionId(Uuid::parse_str(&self.session_id).unwrap());
        assert!(!self.state.programmers.get(session_id).unwrap().connected);
        let (second_token, _) = login(&self.app, "Operator").await;
        let cleared = self
            .app
            .clone()
            .oneshot(
                Request::post(format!("/api/v1/programmers/{}/clear", self.session_id))
                    .header(
                        header::AUTHORIZATION,
                        format!("Bearer {second_token}"),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(cleared.status(), StatusCode::NO_CONTENT);
    }
}

#[tokio::test]
async fn operational_show_programmer_playback_and_rollback_flow() {
    let scenario = OperationalScenario::new().await;
    scenario.seed_and_open_show().await;
    scenario.exercise_output_and_programmer().await;
    scenario.exercise_presets_and_playback().await;
    scenario.verify_service_endpoints().await;
    scenario.exercise_show_and_session_lifecycle().await;
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}
