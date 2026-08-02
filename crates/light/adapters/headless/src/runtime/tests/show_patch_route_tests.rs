use super::*;
use http_body_util::BodyExt;

#[tokio::test]
async fn v2_patch_route_resolves_ordered_placement_and_replays_authoritative_addresses() {
    let (state, data_dir) = test_state();
    let (profile_id, mode_id) = install_patch_route_profile(&state);
    let app = router(state);
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "V2 server placement").await;
    let show_id = show["id"].as_str().unwrap();
    open_show_for_patch_test(&app, &token, show_id).await;

    let mut request = valid_patch_request_for(profile_id, mode_id, "server-placement-route-test");
    let template = request["fixtures"][0].clone();
    let fixture_ids = [Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
    request["fixtures"] = serde_json::Value::Array(
        fixture_ids
            .iter()
            .enumerate()
            .map(|(index, fixture_id)| {
                let mut fixture = template.clone();
                fixture["fixture_id"] = serde_json::json!(fixture_id);
                fixture["fixture_number"] = serde_json::json!(index + 1);
                fixture
            })
            .collect(),
    );
    request["placements"] = serde_json::json!([{
        "fixture_ids": fixture_ids,
        "splits": [{
            "split": 1,
            "universe": 1,
            "address": 1,
            "mode": {
                "type": "operator_overrides",
                "overrides": [{
                    "fixture_id": fixture_ids[1],
                    "universe": 1,
                    "address": 50
                }],
                "future_mode_field": true
            },
            "future_split_field": "accepted"
        }],
        "future_placement_field": {"accepted": true}
    }]);
    request["future_request_field"] = serde_json::json!("accepted");

    let response = post_patch(&app, &token, show_id, Some(0), request.clone()).await;
    assert_eq!(response.status(), StatusCode::OK);
    let response = json(response).await;
    assert_eq!(
        response["fixtures"]
            .as_array()
            .unwrap()
            .iter()
            .map(|fixture| fixture["split_patches"][0]["address"].as_u64())
            .collect::<Vec<_>>(),
        vec![Some(1), Some(50), Some(3)]
    );

    let replay = post_patch(&app, &token, show_id, Some(0), request).await;
    assert_eq!(replay.status(), StatusCode::OK);
    let replay = json(replay).await;
    assert_eq!(replay["replayed"], true);
    assert_eq!(
        replay["fixtures"]
            .as_array()
            .unwrap()
            .iter()
            .map(|fixture| fixture["split_patches"][0]["address"].as_u64())
            .collect::<Vec<_>>(),
        vec![Some(1), Some(50), Some(3)]
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn v2_patch_policy_route_applies_one_sparse_idempotent_intent() {
    let (state, data_dir) = test_state();
    let (profile_id, mode_id) = install_patch_route_profile(&state);
    let app = router(state);
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Sparse fixture policy").await;
    let show_id = show["id"].as_str().unwrap();
    open_show_for_patch_test(&app, &token, show_id).await;
    let request = valid_patch_request_for(profile_id, mode_id, "policy-fixture");
    let fixture_id = request["fixtures"][0]["fixture_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let created = post_patch(&app, &token, show_id, Some(0), request).await;
    assert_eq!(created.status(), StatusCode::OK);
    let created = json(created).await;
    assert_eq!(created["fixtures"][0]["group_masters_enabled"], true);
    assert_eq!(created["fixtures"][0]["grand_master_enabled"], true);
    assert_eq!(created["fixtures"][0]["invert_pan"], false);
    assert_eq!(created["fixtures"][0]["invert_tilt"], false);

    let body = serde_json::json!({
        "request_id": "ignore-groups",
        "action": "set_group_masters",
        "controlled": false,
        "future_client_context": {"ignored": true}
    });
    let changed = post_patch_policy(&app, &token, show_id, &fixture_id, 1, body.clone()).await;
    let changed_status = changed.status();
    let changed = json(changed).await;
    assert_eq!(changed_status, StatusCode::OK, "{changed}");
    assert_eq!(changed["fixtures"][0]["group_masters_enabled"], false);
    assert_eq!(changed["fixtures"][0]["grand_master_enabled"], true);
    assert_eq!(changed["fixtures"][0]["invert_pan"], false);
    assert_eq!(changed["fixtures"][0]["invert_tilt"], false);

    let replay = post_patch_policy(&app, &token, show_id, &fixture_id, 1, body).await;
    let replay_status = replay.status();
    let replay = json(replay).await;
    assert_eq!(replay_status, StatusCode::OK, "{replay}");
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["patch_revision"], 2);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn v2_patch_update_route_mutates_the_exact_physical_instance_and_replays() {
    let (state, data_dir) = test_state();
    let (profile_id, mode_id) = install_patch_route_profile(&state);
    let app = router(state);
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Sparse physical fixture update").await;
    let show_id = show["id"].as_str().unwrap();
    open_show_for_patch_test(&app, &token, show_id).await;
    let mut request = valid_patch_request_for(profile_id, mode_id, "physical-update-seed");
    let fixture_id = request["fixtures"][0]["fixture_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let copy_id = Uuid::new_v4();
    request["fixtures"][0]["multipatch"] = serde_json::json!([{
        "id": copy_id,
        "name": "Exact copy",
        "split_patches": [{"split": 1, "universe": null, "address": null}],
        "location": {"x": 100, "y": 200, "z": 300},
        "rotation": {"x": 0.0, "y": 0.0, "z": 0.0}
    }]);
    let created = post_patch(&app, &token, show_id, Some(0), request).await;
    assert_eq!(created.status(), StatusCode::OK);
    let created = json(created).await;
    assert_eq!(created["fixtures"][0]["fixture_revision"], 1);

    let masters = serde_json::json!({
        "request_id": "paired-master-update",
        "expected_fixture_revision": 1,
        "expected_patch_revision": 1,
        "expected_show_revision": 2,
        "multipatch_instance_id": null,
        "action": "set_masters",
        "group_masters_enabled": false,
        "grand_master_enabled": true,
        "future_client_context": {"accepted": true}
    });
    let changed = post_patch_update(&app, &token, show_id, &fixture_id, masters.clone()).await;
    let changed_status = changed.status();
    let changed = json(changed).await;
    assert_eq!(changed_status, StatusCode::OK, "{changed}");
    assert_eq!(changed["fixtures"][0]["group_masters_enabled"], false);
    assert_eq!(changed["fixtures"][0]["grand_master_enabled"], true);
    assert_eq!(changed["fixtures"][0]["fixture_revision"], 2);
    assert_eq!(changed["patch_revision"], 2);

    let replay = post_patch_update(&app, &token, show_id, &fixture_id, masters).await;
    let replay_status = replay.status();
    let replay = json(replay).await;
    assert_eq!(replay_status, StatusCode::OK, "{replay}");
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["event_sequence"], changed["event_sequence"]);

    let copy_update = serde_json::json!({
        "request_id": "exact-copy-pan-tilt",
        "expected_fixture_revision": 2,
        "expected_patch_revision": 2,
        "expected_show_revision": 3,
        "multipatch_instance_id": copy_id,
        "action": "set_pan_tilt",
        "invert_pan": true,
        "invert_tilt": false
    });
    let copy_changed = post_patch_update(&app, &token, show_id, &fixture_id, copy_update).await;
    let copy_status = copy_changed.status();
    let copy_changed = json(copy_changed).await;
    assert_eq!(copy_status, StatusCode::OK, "{copy_changed}");
    assert_eq!(copy_changed["fixtures"][0]["invert_pan"], false);
    assert_eq!(
        copy_changed["fixtures"][0]["multipatch"][0]["invert_pan"],
        true
    );
    assert_eq!(
        copy_changed["fixtures"][0]["multipatch"][0]["location"],
        serde_json::json!({"x": 100, "y": 200, "z": 300})
    );

    let missing_copy = serde_json::json!({
        "request_id": "missing-copy-location",
        "expected_fixture_revision": 3,
        "expected_patch_revision": 3,
        "expected_show_revision": 4,
        "multipatch_instance_id": Uuid::new_v4(),
        "action": "set_location_axis",
        "axis": "x",
        "millimetres": 999
    });
    let missing = post_patch_update(&app, &token, show_id, &fixture_id, missing_copy).await;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        json(missing).await["error"],
        "multi-patch instance does not exist"
    );

    let stale_fixture = serde_json::json!({
        "request_id": "stale-fixture-update",
        "expected_fixture_revision": 2,
        "expected_patch_revision": 3,
        "expected_show_revision": 4,
        "multipatch_instance_id": null,
        "action": "set_bracket_angle",
        "degrees": 15.0
    });
    let stale = post_patch_update(&app, &token, show_id, &fixture_id, stale_fixture).await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    assert_eq!(json(stale).await["error"], "stale fixture revision");
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn whole_show_upload_preserves_distinct_root_and_multipatch_appearance() {
    let (state, data_dir) = test_state();
    let (profile_id, mode_id) = install_patch_route_profile(&state);
    let app = router(state);
    let (token, _) = login(&app, "Operator").await;
    let source = create_show(&app, &token, "Appearance upload source").await;
    let source_id = source["id"].as_str().unwrap();
    open_show_for_patch_test(&app, &token, source_id).await;

    let root_appearance = serde_json::json!({
        "light_source": {"type": "halogen"},
        "color_temperature_kelvin": 3200,
        "gel": {
            "type": "built_in",
            "catalog_id": "touring-gels",
            "entry_id": "deep-red",
            "embedded_fallback": {
                "number": "R1",
                "name": "Deep red",
                "display_srgb": "#D92838",
                "visualizer_srgb": "#C01020"
            }
        },
        "shaper_angles_degrees": [-30.0, 15.5, 0.0, 179.5]
    });
    let copy_appearance = serde_json::json!({
        "light_source": {"type": "other", "label": "Carbon arc"},
        "color_temperature_kelvin": 5600,
        "gel": {
            "type": "custom",
            "name": "Window blue",
            "color_srgb": "#80A0FF",
            "note": "Balcony copy only"
        },
        "shaper_angles_degrees": [1.0, 2.0, 3.0, 4.0]
    });
    let mut patch = valid_patch_request_for(profile_id, mode_id, "appearance-upload-seed");
    let fixture_id = patch["fixtures"][0]["fixture_id"].clone();
    let copy_id = Uuid::new_v4();
    patch["fixtures"][0]["installed_appearance"] = root_appearance.clone();
    patch["fixtures"][0]["multipatch"] = serde_json::json!([{
        "id": copy_id,
        "name": "Balcony copy",
        "split_patches": [{"split": 1, "universe": null, "address": null}],
        "location": {"x": 100, "y": 200, "z": 300},
        "rotation": {"x": 0.0, "y": 0.0, "z": 0.0},
        "installed_appearance": copy_appearance
    }]);
    let created = post_patch(&app, &token, source_id, Some(0), patch).await;
    assert_eq!(created.status(), StatusCode::OK);

    let download = app
        .clone()
        .oneshot(
            Request::get(format!("/api/v2/shows/{source_id}/download"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(download.status(), StatusCode::OK);
    let show_bytes = download.into_body().collect().await.unwrap().to_bytes();
    let uploaded = app
        .clone()
        .oneshot(show_action_request(
            &token,
            serde_json::json!({
                "type": "create",
                "name": "Appearance upload copy",
                "data_base64": STANDARD.encode(show_bytes),
                "overwrite": false
            }),
        ))
        .await
        .unwrap();
    assert_eq!(uploaded.status(), StatusCode::OK);
    let uploaded = show_action_result(json(uploaded).await, "show");
    let uploaded_id = uploaded["id"].as_str().unwrap();
    assert_ne!(uploaded_id, source_id, "the upload is an independent show");
    open_show_for_patch_test(&app, &token, uploaded_id).await;

    let imported = get_patch(&app, &token, uploaded_id).await;
    assert_eq!(imported.status(), StatusCode::OK);
    let imported = json(imported).await;
    assert_eq!(imported["fixtures"].as_array().unwrap().len(), 1);
    let fixture = &imported["fixtures"][0];
    assert_eq!(fixture["fixture_id"], fixture_id);
    assert_eq!(fixture["profile_id"], profile_id.to_string());
    assert_eq!(fixture["profile_revision"], 1);
    assert_eq!(fixture["mode_id"], mode_id.to_string());
    assert_eq!(fixture["installed_appearance"], root_appearance);
    assert_eq!(fixture["multipatch"].as_array().unwrap().len(), 1);
    assert_eq!(fixture["multipatch"][0]["id"], copy_id.to_string());
    assert_eq!(
        fixture["multipatch"][0]["installed_appearance"],
        copy_appearance
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn v2_patch_snapshot_authenticates_and_returns_the_patch_revision_etag() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let denied = app
        .clone()
        .oneshot(Request::get("/api/v2/patch").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
    let denied_body = json(denied).await;
    assert_eq!(denied_body["retryable"], false);
    assert!(denied_body["current_revision"].is_null());

    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "V2 Patch snapshot").await;
    let show_id = show["id"].as_str().unwrap();
    open_show_for_patch_test(&app, &token, show_id).await;
    let baseline_sequence = state.events.latest_sequence();

    let response = app
        .oneshot(
            Request::get("/api/v2/patch")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header("x-tosk-show", show_id)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::ETAG], "\"0\"");
    let snapshot = json(response).await;
    assert_eq!(snapshot["show_id"], show_id);
    assert_eq!(snapshot["show_revision"], 1);
    assert_eq!(snapshot["patch_revision"], 0);
    assert_eq!(snapshot["cursor"]["sequence"], baseline_sequence);
    assert_eq!(snapshot["fixtures"], serde_json::json!([]));
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn v2_patch_mutation_returns_typed_revision_conflicts() {
    let (state, data_dir) = test_state();
    let app = router(state);
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "V2 Patch conflict").await;
    let show_id = show["id"].as_str().unwrap();
    open_show_for_patch_test(&app, &token, show_id).await;

    let response = app
        .oneshot(
            Request::post("/api/v2/patch/fixtures")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header("x-tosk-show", show_id)
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::IF_MATCH, "1")
                .body(Body::from(valid_patch_request().to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(response.headers()[header::ETAG], "\"0\"");
    let error = json(response).await;
    assert_eq!(error["error"], "stale patch revision");
    assert_eq!(error["current_revision"], 0);
    assert_eq!(error["retryable"], false);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn v2_patch_requires_if_match_and_rejects_invalid_batches_without_side_effects() {
    let (state, data_dir) = test_state();
    let (profile_id, mode_id) = install_patch_route_profile(&state);
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "V2 Patch validation").await;
    let show_id = show["id"].as_str().unwrap();
    open_show_for_patch_test(&app, &token, show_id).await;
    let baseline_sequence = state.events.latest_sequence();

    let missing_precondition = post_patch(
        &app,
        &token,
        show_id,
        None,
        serde_json::json!({"request_id":"missing-if-match","remove_fixture_ids":[Uuid::new_v4()]}),
    )
    .await;
    assert_eq!(missing_precondition.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        json(missing_precondition).await["error"],
        "If-Match revision is required"
    );

    let invalid = post_patch(
        &app,
        &token,
        show_id,
        Some(0),
        serde_json::json!({"request_id":"empty-batch"}),
    )
    .await;
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    assert_eq!(json(invalid).await["retryable"], false);

    let unchanged = get_patch(&app, &token, show_id).await;
    assert_eq!(unchanged.headers()[header::ETAG], "\"0\"");
    let unchanged = json(unchanged).await;
    assert_eq!(unchanged["show_revision"], 1);
    assert_eq!(unchanged["patch_revision"], 0);
    assert_eq!(unchanged["cursor"]["sequence"], baseline_sequence);
    assert_eq!(state.events.latest_sequence(), baseline_sequence);

    let successful_request = valid_patch_request_for(profile_id, mode_id, "successful-route-test");
    let success = post_patch(&app, &token, show_id, Some(0), successful_request.clone()).await;
    assert_eq!(success.status(), StatusCode::OK);
    assert_eq!(success.headers()[header::ETAG], "\"1\"");
    let success = json(success).await;
    assert_eq!(success["changed"], true);
    assert_eq!(success["show_revision"], 2);
    assert_eq!(success["patch_revision"], 1);
    assert_eq!(success["event_sequence"], baseline_sequence + 1);
    assert_eq!(success["fixtures"].as_array().unwrap().len(), 1);
    assert_eq!(state.events.latest_sequence(), baseline_sequence + 1);

    let replay = post_patch(&app, &token, show_id, Some(0), successful_request).await;
    assert_eq!(replay.status(), StatusCode::OK);
    assert_eq!(replay.headers()[header::ETAG], "\"1\"");
    let replay = json(replay).await;
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["event_sequence"], baseline_sequence + 1);
    assert_eq!(state.events.latest_sequence(), baseline_sequence + 1);
    assert_eq!(patch_backup_count(&data_dir), 1);

    let committed_response = get_patch(&app, &token, show_id).await;
    assert_eq!(committed_response.headers()[header::ETAG], "\"1\"");
    let committed = json(committed_response).await;
    assert_eq!(committed["show_revision"], 2);
    assert_eq!(committed["patch_revision"], 1);
    assert_eq!(committed["cursor"]["sequence"], baseline_sequence + 1);
    assert_eq!(committed["fixtures"].as_array().unwrap().len(), 1);

    open_show_for_patch_test(&app, &token, show_id).await;
    assert_eq!(state.output.snapshot().fixtures.len(), 1);
    let reopened = json(get_patch(&app, &token, show_id).await).await;
    assert_eq!(reopened["show_revision"], 2);
    assert_eq!(reopened["patch_revision"], 1);
    assert_eq!(reopened["fixtures"].as_array().unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn v2_patch_revision_ignores_unrelated_group_mutations() {
    let (state, data_dir) = test_state();
    let (profile_id, mode_id) = install_patch_route_profile(&state);
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "V2 Patch independent revision").await;
    let show_id = show["id"].as_str().unwrap();
    open_show_for_patch_test(&app, &token, show_id).await;

    let unrelated = seed_show_object(
        &state,
        &token,
        show_id,
        "group",
        "unrelated",
        0,
        serde_json::json!({
            "name": "Unrelated Group",
            "fixtures": []
        }),
    )
    .await;
    assert_eq!(unrelated.status(), StatusCode::OK);
    let show_uuid = Uuid::parse_str(show_id).unwrap();
    let entry = state
        .installation
        .show(light_core::ShowId(show_uuid))
        .unwrap()
        .unwrap();
    let after_group = ShowStore::open(&entry.path)
        .unwrap()
        .portable_document()
        .unwrap();
    assert_eq!(after_group.revision().value(), 2);
    assert_eq!(after_group.patch_revision().value(), 0);
    let patch_after_group = get_patch(&app, &token, show_id).await;
    assert_eq!(patch_after_group.headers()[header::ETAG], "\"0\"");
    let patch_after_group = json(patch_after_group).await;
    assert_eq!(patch_after_group["show_revision"], 2);
    assert_eq!(patch_after_group["patch_revision"], 0);

    let response = post_patch(
        &app,
        &token,
        show_id,
        Some(0),
        valid_patch_request_for(profile_id, mode_id, "patch-after-group"),
    )
    .await;

    let status = response.status();
    let etag = response.headers().get(header::ETAG).cloned();
    let outcome = json(response).await;
    assert_eq!(status, StatusCode::OK, "{outcome}");
    assert_eq!(etag.as_ref().unwrap(), "\"1\"");
    assert_eq!(outcome["show_revision"], 3);
    assert_eq!(outcome["patch_revision"], 1);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn ordinary_http_and_patch_share_one_outer_lock_order_without_deadlock() {
    let (state, data_dir) = test_state();
    let (profile_id, mode_id) = install_patch_route_profile(&state);
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "V2 Patch lifecycle ordering").await;
    let show_id = show["id"].as_str().unwrap().to_owned();
    open_show_for_patch_test(&app, &token, &show_id).await;

    state.active_show.http_lifecycle_probe().arm();
    let group_state = state.clone();
    let group_token = token.clone();
    let group_show_id = show_id.clone();
    let group = tokio::spawn(async move {
        seed_show_object(
            &group_state,
            &group_token,
            &group_show_id,
            "group",
            "before-patch",
            0,
            serde_json::json!({
                "name":"Stored before Patch lifecycle",
                "fixtures":[]
            }),
        )
        .await
    });
    let ordinary_pause = state.active_show.http_lifecycle_probe();
    tokio::task::spawn_blocking(move || ordinary_pause.wait_until_started())
        .await
        .unwrap();

    state.active_show.patch_lifecycle_probe().arm();
    let patch_app = app.clone();
    let patch_token = token.clone();
    let patch_show_id = show_id.clone();
    let patch = tokio::spawn(async move {
        post_patch(
            &patch_app,
            &patch_token,
            &patch_show_id,
            Some(0),
            valid_patch_request_for(profile_id, mode_id, "ordered-lifecycle-race"),
        )
        .await
    });
    let patch_pause = state.active_show.patch_lifecycle_probe();
    tokio::task::spawn_blocking(move || patch_pause.wait_until_started())
        .await
        .unwrap();

    // Patch is paused immediately before taking activation. Because that outer lifecycle precedes
    // the application operation gate, the ordinary request must still be able to commit while
    // Patch remains paused. The former operation-then-activation order deadlocked at this point.
    state.active_show.http_lifecycle_probe().release();
    let group = tokio::time::timeout(Duration::from_secs(2), group).await;
    state.active_show.patch_lifecycle_probe().release();
    let group = group
        .expect("ordinary active-show HTTP mutation deadlocked with Patch")
        .unwrap();
    let patch = tokio::time::timeout(Duration::from_secs(2), patch)
        .await
        .expect("Patch deadlocked with an ordinary active-show HTTP mutation")
        .unwrap();
    assert_eq!(group.status(), StatusCode::OK);
    assert_eq!(patch.status(), StatusCode::OK);

    let document = ShowStore::open(
        &state
            .installation
            .show(light_core::ShowId(Uuid::parse_str(&show_id).unwrap()))
            .unwrap()
            .unwrap()
            .path,
    )
    .unwrap()
    .portable_document()
    .unwrap();
    assert_eq!(document.revision().value(), 3);
    assert!(document.object("group", "before-patch").is_some());
    assert_eq!(document.objects_of_kind("patched_fixture").count(), 1);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn paused_profile_resolution_releases_activation_for_an_http_show_mutation() {
    let (state, data_dir) = test_state();
    let (profile_id, mode_id) = install_patch_route_profile(&state);
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "V2 Patch planning concurrency").await;
    let show_id = show["id"].as_str().unwrap().to_owned();
    open_show_for_patch_test(&app, &token, &show_id).await;
    state.active_show.patch_profile_resolution_probe().arm();

    let patch_app = app.clone();
    let patch_token = token.clone();
    let patch_show_id = show_id.clone();
    let patch = tokio::spawn(async move {
        post_patch(
            &patch_app,
            &patch_token,
            &patch_show_id,
            Some(0),
            valid_patch_request_for(profile_id, mode_id, "paused-server-resolution"),
        )
        .await
    });
    let pause = state.active_show.patch_profile_resolution_probe();
    let started = tokio::task::spawn_blocking(move || pause.wait_until_started()).await;
    if started.is_err() {
        state.active_show.patch_profile_resolution_probe().release();
    }
    started.unwrap();

    let group = tokio::time::timeout(
        Duration::from_secs(2),
        seed_show_object(
            &state,
            &token,
            &show_id,
            "group",
            "during-resolution",
            0,
            serde_json::json!({
                "name":"Stored during Patch planning",
                "fixtures":[]
            }),
        ),
    )
    .await;
    state.active_show.patch_profile_resolution_probe().release();
    let patch = patch.await.unwrap();
    let group =
        group.expect("ordinary active-show HTTP mutation was blocked by Patch profile resolution");

    assert_eq!(group.status(), StatusCode::OK);
    assert_eq!(patch.status(), StatusCode::OK);
    let patch = json(patch).await;
    assert_eq!(patch["show_revision"], 3);
    assert_eq!(patch["patch_revision"], 1);
    let show_id = light_core::ShowId(Uuid::parse_str(&show_id).unwrap());
    let entry = state.installation.show(show_id).unwrap().unwrap();
    let document = ShowStore::open(&entry.path)
        .unwrap()
        .portable_document()
        .unwrap();
    assert!(document.object("group", "during-resolution").is_some());
    assert_eq!(document.objects_of_kind("patched_fixture").count(), 1);
    assert_eq!(state.output.snapshot().revision, 3);
    assert_eq!(state.output.snapshot().fixtures.len(), 1);
    let _ = std::fs::remove_dir_all(data_dir);
}

fn patch_backup_count(data_dir: &FsPath) -> usize {
    std::fs::read_dir(data_dir.join("backups"))
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().contains("-patch-"))
        .count()
}

async fn open_show_for_patch_test(app: &Router, token: &str, show_id: &str) {
    let response = app
        .clone()
        .oneshot(open_show_request(token, show_id))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

fn valid_patch_request() -> serde_json::Value {
    valid_patch_request_for(Uuid::new_v4(), Uuid::new_v4(), "stale-route-test")
}

fn valid_patch_request_for(profile_id: Uuid, mode_id: Uuid, request_id: &str) -> serde_json::Value {
    serde_json::json!({
        "request_id": request_id,
        "fixtures": [{
            "fixture_id": Uuid::new_v4(),
            "fixture_number": 1,
            "virtual_fixture_number": null,
            "name": "Route test",
            "profile_id": profile_id,
            "profile_revision": 1,
            "mode_id": mode_id,
            "split_patches": [{"split": 1, "universe": null, "address": null}],
            "layer_id": "default",
            "direct_control": null,
            "location": {"x": 0, "y": 0, "z": 0},
            "rotation": {"x": 0.0, "y": 0.0, "z": 0.0},
            "multipatch": [],
            "move_in_black_enabled": true,
            "move_in_black_delay_millis": 0,
            "highlight_overrides": []
        }]
    })
}

fn install_patch_route_profile(state: &AppState) -> (Uuid, Uuid) {
    let mut profile = light_fixture::FixtureProfile::blank();
    profile.manufacturer = "Route Test".into();
    profile.name = "Patch Fixture".into();
    profile.short_name = "Patch".into();
    let head_id = profile.modes[0].heads[0].id;
    profile.modes[0].splits[0].footprint = 1;
    profile.modes[0]
        .channels
        .push(light_fixture::FixtureChannel {
            id: Uuid::new_v4(),
            head_id,
            split: 1,
            fixture_attribute: light_core::AttributeKey("intensity".into()),
            attribute: light_core::AttributeKey("intensity".into()),
            canonical_transform: light_fixture::CanonicalTransform::Identity,
            resolution: light_fixture::ChannelResolution::U8,
            secondary_slots: vec![],
            default_raw: 0,
            highlight_raw: 255,
            physical_min: None,
            physical_max: None,
            unit: None,
            invert: false,
            snap: false,
            reacts_to_virtual_intensity: false,
            reacts_to_sequence_master: false,
            reacts_to_group_master: true,
            reacts_to_grand_master: true,
            behavior: light_fixture::ChannelBehavior::Controlled,
            functions: vec![],
        });
    let profile = state.installation.save_fixture_profile(profile, 0).unwrap();
    (profile.id.0, profile.modes[0].id)
}

async fn get_patch(app: &Router, token: &str, show_id: &str) -> Response {
    app.clone()
        .oneshot(
            Request::get("/api/v2/patch")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header("x-tosk-show", show_id)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn post_patch(
    app: &Router,
    token: &str,
    show_id: &str,
    patch_revision: Option<u64>,
    body: serde_json::Value,
) -> Response {
    let mut request = Request::post("/api/v2/patch/fixtures")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header("x-tosk-show", show_id)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(patch_revision) = patch_revision {
        request = request.header(header::IF_MATCH, patch_revision.to_string());
    }
    app.clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap()
}

async fn post_patch_policy(
    app: &Router,
    token: &str,
    show_id: &str,
    fixture_id: &str,
    patch_revision: u64,
    body: serde_json::Value,
) -> Response {
    app.clone()
        .oneshot(
            Request::post(format!("/api/v2/patch/fixtures/{fixture_id}/policy"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header("x-tosk-show", show_id)
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::IF_MATCH, patch_revision.to_string())
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn post_patch_update(
    app: &Router,
    token: &str,
    show_id: &str,
    fixture_id: &str,
    body: serde_json::Value,
) -> Response {
    app.clone()
        .oneshot(
            Request::post(format!("/api/v2/patch/fixtures/{fixture_id}/update"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header("x-tosk-show", show_id)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}
