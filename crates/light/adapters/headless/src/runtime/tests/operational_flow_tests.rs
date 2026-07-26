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
                    &self.state,
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
            .oneshot(open_show_request(&self.token, &self.first_id))
            .await
            .unwrap();
        assert_eq!(opened.status(), StatusCode::OK);
        assert_eq!(self.state.output.snapshot().fixtures.len(), 1);
        let patch = self
            .app
            .clone()
            .oneshot(
                Request::get("/api/v2/patch")
                    .header(
                        header::AUTHORIZATION,
                        format!("Bearer {}", self.token),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(json(patch).await["fixtures"].as_array().unwrap().len(), 1);
    }

    async fn exercise_output_and_programmer(&self) {
        let response = self
            .app
            .clone()
            .oneshot(
                Request::post("/api/v2/output/dmx-overrides")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(
                        header::AUTHORIZATION,
                        format!("Bearer {}", self.token),
                    )
                    .body(Body::from(
                        r#"{"request_id":"dmx-test-1","universe":1,"address":1,"value":200}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            self.state.output.dmx_override(1, 1),
            Some(200)
        );
        let dmx = self
            .app
            .clone()
            .oneshot(Request::get("/api/v2/output/dmx").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(json(dmx).await["overrides"].as_array().unwrap().len(), 1);
        let session = authenticate_token(&self.state, &self.token).unwrap();
        assert_eq!(
            execute_programmer_command(&self.state, &session, "FIXTURE 1 AT 50 TIME 0").unwrap(),
            1
        );
        assert_eq!(self.rendered_intensity(), 128);
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
            self.state.programming.get(session.id).unwrap().selected,
            vec![self.fixture_id]
        );
    }

    fn rendered_intensity(&self) -> u8 {
        self.state
            .output.render(RenderOptions::default())
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
                &self.state,
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
        apply_command_preset(&self.state, &session, "1.1", &[self.fixture_id]).unwrap();
        let programmer = self.state.programming.get(session.id).unwrap();
        assert_eq!(programmer.values[0].fade_millis, Some(3_000));
        assert_eq!(programmer.values[0].value.normalized(), Some(0.75));
        assert_eq!(
            self.state.programming.get(session.id).unwrap().values[0].fade_millis,
            Some(3_000)
        );
        let go = self
            .app
            .clone()
            .oneshot(
                Request::post("/api/v2/playback-actions")
                    .header(
                        header::AUTHORIZATION,
                        format!("Bearer {}", self.token),
                    )
                    .header(header::CONTENT_TYPE, "application/json")
                    .header("x-tosk-show", &self.first_id)
                    .header("x-tosk-desk", session.desk.id.to_string())
                    .body(Body::from(
                        serde_json::json!({
                            "request_id":"operational-flow-go",
                            "address":{"kind":"cue_list","cue_list_id":self.cue_list_id.0},
                            "action":{"type":"go","pressed":true},
                            "surface":"physical"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(go.status(), StatusCode::OK);
        let playback = self
            .app
            .clone()
            .oneshot(
                Request::get("/api/v2/playback-overview")
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
        for path in ["/api/v2/diagnostics", "/api/v2/readiness"] {
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
                Request::get(format!("/api/v2/shows/{}/download", self.first_id))
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
            .oneshot(open_show_with_transition_request(
                &self.token,
                second_id,
                "timed_fade",
                Some(100),
            ))
            .await
            .unwrap();
        assert_eq!(opened.status(), StatusCode::OK);
        let rollback = self
            .app
            .clone()
            .oneshot(rollback_show_request(&self.token))
            .await
            .unwrap();
        assert_eq!(rollback.status(), StatusCode::OK);
        assert_eq!(
            self.state.active_show.current().as_ref().unwrap().id.0.to_string(),
            self.first_id
        );
        self.disconnect_and_clear_programmer().await;
    }

    async fn disconnect_and_clear_programmer(&self) {
        let disconnected = self
            .app
            .clone()
            .oneshot(
                Request::delete(format!("/api/v2/sessions/{}", self.session_id))
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
        assert!(!self.state.programming.get(session_id).unwrap().connected);
        let (second_token, _) = login(&self.app, "Operator").await;
        let cleared = self
            .app
            .clone()
            .oneshot(
                Request::post(format!("/api/v2/programmers/{}/clear", self.session_id))
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
