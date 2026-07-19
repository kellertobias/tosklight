#[tokio::test]
async fn desk_lock_is_persisted_scoped_and_enforced_by_the_server() {
    let (state, data_dir) = test_state();
    let second = state.desk.lock().add_desk("Second", "second").unwrap();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let configure = app.clone().oneshot(Request::put("/api/v1/desk-lock").header(header::AUTHORIZATION, format!("Bearer {token}")).header(header::CONTENT_TYPE,"application/json").body(Body::from(r#"{"message":"Call the operator","wallpaper":null,"unlock_mode":"pin","pin":"1234"}"#)).unwrap()).await.unwrap();
    assert_eq!(configure.status(), StatusCode::OK);
    let lock = app
        .clone()
        .oneshot(
            Request::post("/api/v1/desk-lock/lock")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(lock.status(), StatusCode::OK);
    assert!(
        read_desk_lock(
            &state,
            state
                .sessions
                .read()
                .values()
                .find(|session| session.token == token)
                .unwrap()
                .desk
                .id
        )
        .locked
    );
    let desk_id = state
        .sessions
        .read()
        .values()
        .find(|session| session.token == token)
        .unwrap()
        .desk
        .id;
    let reopened = DeskStore::open(data_dir.join("desk.sqlite")).unwrap();
    let persisted: DeskLockConfiguration =
        serde_json::from_str(&reopened.setting(&desk_lock_key(desk_id)).unwrap().unwrap()).unwrap();
    assert!(
        persisted.locked,
        "a server restart must reopen the desk as locked"
    );
    let blocked = app
        .clone()
        .oneshot(
            Request::put("/api/v1/master")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"grand_master":0.5}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(blocked.status(), StatusCode::CONFLICT);
    let wrong = app
        .clone()
        .oneshot(
            Request::post("/api/v1/desk-lock/unlock")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"pin":"9999"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(wrong.status(), StatusCode::UNAUTHORIZED);

    let second_login = app
        .clone()
        .oneshot(
            Request::post("/api/v1/sessions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({"username":"Operator","desk_id":second.id}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let second_token = json(second_login).await["token"]
        .as_str()
        .unwrap()
        .to_owned();
    let unaffected = app
        .clone()
        .oneshot(
            Request::put("/api/v1/master")
                .header(header::AUTHORIZATION, format!("Bearer {second_token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"grand_master":0.5}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unaffected.status(), StatusCode::OK);

    let unlock = app
        .oneshot(
            Request::post("/api/v1/desk-lock/unlock")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"pin":"1234"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unlock.status(), StatusCode::OK);
    let stored = state
        .desk
        .lock()
        .setting(&desk_lock_key(
            state
                .sessions
                .read()
                .values()
                .find(|session| session.token == token)
                .unwrap()
                .desk
                .id,
        ))
        .unwrap()
        .unwrap();
    assert!(!stored.contains("1234"));
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn login_reuses_client_desk_when_remembered_desk_is_stale() {
    let (state, data_dir) = test_state();
    let app = router(state);
    let client_id = Uuid::new_v4();
    let login = |desk_id: Option<Uuid>| {
        let app = app.clone();
        async move {
            app.oneshot(
                    Request::post("/api/v1/sessions")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(
                            serde_json::json!({"username":"Operator","client_id":client_id,"desk_id":desk_id}).to_string(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap()
        }
    };
    let first = login(None).await;
    assert_eq!(first.status(), StatusCode::OK);
    let first_desk = json(first).await["desk"]["id"].clone();
    let second = login(Some(Uuid::new_v4())).await;
    assert_eq!(second.status(), StatusCode::OK);
    assert_eq!(json(second).await["desk"]["id"], first_desk);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn citp_thumbnail_api_uses_patched_parent_endpoint_and_cache() {
    use tokio::io::AsyncWriteExt;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (state, data_dir) = test_state();
    let fixture_id = light_core::FixtureId::new();
    state
        .engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![light_fixture::PatchedFixture {
                name: "Media Server".into(),
                layer_id: "default".into(),
                fixture_id,
                fixture_number: None,
                virtual_fixture_number: None,
                definition: light_fixture::FixtureDefinition {
                    schema_version: 1,
                    id: light_core::FixtureId::new(),
                    revision: 1,
                    manufacturer: "Test".into(),
                    device_type: "media server".into(),
                    name: "Media Server".into(),
                    model: "Media Server".into(),
                    mode: "2 layers".into(),
                    footprint: 1,
                    heads: vec![
                        light_fixture::LogicalHead {
                            index: 0,
                            name: "Master".into(),
                            shared: true,
                            parameters: vec![],
                        },
                        light_fixture::LogicalHead {
                            index: 1,
                            name: "Layer 1".into(),
                            shared: false,
                            parameters: vec![],
                        },
                    ],
                    color_calibration: None,
                    physical: Default::default(),
                    model_asset: None,
                    icon_asset: None,
                    hazardous: false,
                    direct_control_protocols: vec![light_fixture::DirectControlProtocol::Citp],
                    signal_loss_policy: light_fixture::SignalLossPolicy::HoldLast,
                    safe_values: std::collections::BTreeMap::new(),
                    profile_id: None,
                    mode_id: None,
                    profile_snapshot: None,
                },
                universe: Some(1),
                address: Some(1),
                split_patches: Vec::new(),
                direct_control: Some(light_fixture::DirectControlEndpoint {
                    protocol: light_fixture::DirectControlProtocol::Citp,
                    ip_address: address.ip(),
                    port: address.port(),
                }),
                location: Default::default(),
                rotation: Default::default(),
                logical_heads: vec![light_fixture::PatchedHead {
                    profile_head_id: None,
                    head_index: 1,
                    fixture_id: light_core::FixtureId::new(),
                }],
                move_in_black_enabled: true,
                move_in_black_delay_millis: 0,
                multipatch: vec![],
                highlight_overrides: Default::default(),
            }],
            revision: 1,
            ..EngineSnapshot::default()
        })
        .unwrap();
    let mock = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let cinf = read_citp_test_packet(&mut stream).await;
        assert_eq!(&cinf[22..26], b"CInf");
        let mut info = citp_test_packet(*b"SInf", &[]);
        info[6..8].copy_from_slice(&1_u16.to_le_bytes());
        stream.write_all(&info).await.unwrap();
        let request = read_citp_test_packet(&mut stream).await;
        assert_eq!(&request[22..26], b"GETh");
        let mut payload = vec![1, 0, 0, 0, 0, 7];
        payload.extend_from_slice(b"JPEG");
        payload.extend_from_slice(&2_u16.to_le_bytes());
        payload.extend_from_slice(&1_u16.to_le_bytes());
        payload.extend_from_slice(&3_u16.to_le_bytes());
        payload.extend_from_slice(&[1, 2, 3]);
        stream
            .write_all(&citp_test_packet(*b"EThn", &payload))
            .await
            .unwrap();
    });
    let app = router(state);
    let (token, _) = login(&app, "Operator").await;
    let refreshed = app
        .clone()
        .oneshot(
            Request::post(format!("/api/v1/media/{}/thumbnails/refresh", fixture_id.0))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"elements":[7],"width":64,"height":64}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(refreshed.status(), StatusCode::OK);
    let image = app
        .clone()
        .oneshot(
            Request::get(format!(
                "/api/v1/media/{}/thumbnail?element=7",
                fixture_id.0
            ))
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(image.status(), StatusCode::OK);
    assert_eq!(image.headers()[header::CONTENT_TYPE], "image/jpeg");
    assert_eq!(
        image
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .as_ref(),
        &[1, 2, 3]
    );
    let status = app
        .oneshot(
            Request::get("/api/v1/media")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = json(status).await;
    assert_eq!(status["fixtures"][0]["status"]["online"], true);
    assert_eq!(status["fixtures"][0]["layers"].as_array().unwrap().len(), 1);
    mock.await.unwrap();
    let _ = std::fs::remove_dir_all(data_dir);
}
