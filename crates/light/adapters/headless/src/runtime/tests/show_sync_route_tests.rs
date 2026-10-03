//! `POST /api/v2/show-sync/transactions`: field compare-and-set, partial commit, exactly-once.
use super::show_patch_route_tests::{install_patch_route_profile, post_patch_update};
use super::show_sync_support::*;
use super::*;

fn annotation(text: &str, colour: &str) -> serde_json::Value {
    serde_json::json!({"text": text, "colour": colour, "position": {"x": 1.0, "y": 2.0}})
}

#[tokio::test]
async fn a_transaction_commits_atomically_and_reports_every_object_it_wrote() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Architect").await;
    let show_id = open_sync_show(&app, &token, "Sync accept").await;
    let association = Uuid::new_v4();
    let before = active_revision(&state);
    let body = sync_body(
        "gesture-1",
        association,
        &show_id,
        serde_json::json!([
            {"type": "create_object", "kind": "cad_annotation", "id": "a1", "body": annotation("FOH", "red")},
            {"type": "create_object", "kind": "rig_attachment", "id": "r1", "body": {"truss": "t1", "offset": 0.5}},
            {"type": "set_metadata", "key": "architect.venue", "base": null, "value": "Hall A"}
        ]),
    );
    let response = post_sync(&app, &token, &body).await;
    assert_eq!(response.status(), StatusCode::OK);
    let outcome = json(response).await;
    assert_eq!(outcome["status"], "accepted");
    assert_eq!(outcome["replayed"], false);
    assert_eq!(
        outcome["show_revision"],
        before + 1,
        "one gesture is one commit"
    );
    assert_eq!(outcome["applied"].as_array().unwrap().len(), 2);
    assert_eq!(outcome["metadata"], serde_json::json!(["architect.venue"]));
    let stored = read_object(&app, &token, &show_id, "cad_annotation", "a1").await;
    assert_eq!(stored["object"]["body"], annotation("FOH", "red"));
    let entry = state.active_show.current().unwrap();
    assert_eq!(
        ShowStore::open(&entry.path)
            .unwrap()
            .metadata_value("architect.venue")
            .unwrap()
            .as_deref(),
        Some("Hall A")
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn independent_fields_merge_and_a_same_field_edit_keeps_the_desk_value() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show_id = open_sync_show(&app, &token, "Sync merge").await;
    let association = Uuid::new_v4();
    let created = post_sync(
        &app,
        &token,
        &sync_body(
            "create-layer",
            association,
            &show_id,
            serde_json::json!([{"type": "create_object", "kind": "patch_layer", "id": "truss",
                "body": {"id": "truss", "name": "Truss", "order": 1}}]),
        ),
    )
    .await;
    assert_eq!(created.status(), StatusCode::OK);
    let revision = json(created).await["applied"][0]["revision"]
        .as_u64()
        .unwrap();
    // The desk renames the layer through its own last-write-wins intent route.
    let desk_edit = app
        .clone()
        .oneshot(
            Request::post("/api/v2/patch/layers/truss/update")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({"request_id": "desk-rename", "action": {"type": "save",
                        "expected_revision": revision, "layer": {"name": "Front Truss", "order": 1}}})
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(desk_edit.status(), StatusCode::OK);
    // Architect, offline from the old base, reorders the layer and renames it too.
    let response = post_sync(
        &app,
        &token,
        &sync_body(
            "offline-edit",
            association,
            &show_id,
            serde_json::json!([{"type": "update_object", "kind": "patch_layer", "id": "truss",
            "fields": [
                {"path": "/order", "base": 1, "value": 4},
                {"path": "/name", "base": "Truss", "value": "Back Truss"}
            ]}]),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let outcome = json(response).await;
    assert_eq!(outcome["status"], "conflicted");
    assert_eq!(
        outcome["applied"][0]["id"], "truss",
        "the free field committed"
    );
    let conflict = &outcome["conflicts"][0];
    assert_eq!(conflict["path"], "/name");
    assert_eq!(conflict["base"], "Truss");
    assert_eq!(conflict["mine"], "Back Truss");
    assert_eq!(conflict["theirs"], "Front Truss");
    assert_eq!(conflict["reason"], "field_changed");
    let stored = read_object(&app, &token, &show_id, "patch_layer", "truss").await;
    assert_eq!(stored["object"]["body"]["name"], "Front Truss");
    assert_eq!(stored["object"]["body"]["order"], 4);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn two_architects_bound_to_one_show_merge_without_a_lock() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Architect").await;
    let show_id = open_sync_show(&app, &token, "Sync two architects").await;
    let (first, second) = (Uuid::new_v4(), Uuid::new_v4());
    let create = sync_body(
        "seed",
        first,
        &show_id,
        serde_json::json!([{"type": "create_object", "kind": "cad_annotation", "id": "a1",
            "body": annotation("FOH", "red")}]),
    );
    assert_eq!(
        post_sync(&app, &token, &create).await.status(),
        StatusCode::OK
    );
    let text = sync_body(
        "same-id",
        first,
        &show_id,
        serde_json::json!([{"type": "update_object", "kind": "cad_annotation", "id": "a1",
            "fields": [{"path": "/text", "base": "FOH", "value": "Front of house"}]}]),
    );
    let colour = sync_body(
        "same-id",
        second,
        &show_id,
        serde_json::json!([{"type": "update_object", "kind": "cad_annotation", "id": "a1",
            "fields": [{"path": "/colour", "base": "red", "value": "blue"}]}]),
    );
    assert_eq!(
        json(post_sync(&app, &token, &text).await).await["status"],
        "accepted"
    );
    let outcome = json(post_sync(&app, &token, &colour).await).await;
    assert_eq!(
        outcome["status"], "accepted",
        "identities are scoped per association"
    );
    assert_eq!(outcome["replayed"], false);
    let stored = read_object(&app, &token, &show_id, "cad_annotation", "a1").await;
    assert_eq!(stored["object"]["body"]["text"], "Front of house");
    assert_eq!(stored["object"]["body"]["colour"], "blue");
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn deleting_a_changed_object_or_editing_a_deleted_one_conflicts() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Architect").await;
    let show_id = open_sync_show(&app, &token, "Sync delete").await;
    let association = Uuid::new_v4();
    let create = sync_body(
        "seed",
        association,
        &show_id,
        serde_json::json!([{"type": "create_object", "kind": "fixture_note", "id": "n1",
            "body": {"text": "a"}}]),
    );
    let revision = json(post_sync(&app, &token, &create).await).await["applied"][0]["revision"]
        .as_u64()
        .unwrap();
    let edit = sync_body(
        "edit",
        association,
        &show_id,
        serde_json::json!([{"type": "update_object", "kind": "fixture_note", "id": "n1",
            "fields": [{"path": "/text", "base": "a", "value": "b"}]}]),
    );
    assert_eq!(
        json(post_sync(&app, &token, &edit).await).await["status"],
        "accepted"
    );
    let stale_delete = sync_body(
        "stale-delete",
        association,
        &show_id,
        serde_json::json!([{"type": "delete_object", "kind": "fixture_note", "id": "n1",
            "base_object_revision": revision}]),
    );
    let outcome = json(post_sync(&app, &token, &stale_delete).await).await;
    assert_eq!(outcome["conflicts"][0]["reason"], "object_modified");
    assert_eq!(outcome["conflicts"][0]["theirs"]["text"], "b");
    let current =
        read_object(&app, &token, &show_id, "fixture_note", "n1").await["object"]["revision"]
            .as_u64()
            .unwrap();
    let delete = sync_body(
        "delete",
        association,
        &show_id,
        serde_json::json!([{"type": "delete_object", "kind": "fixture_note", "id": "n1",
            "base_object_revision": current}]),
    );
    let deleted = json(post_sync(&app, &token, &delete).await).await;
    assert_eq!(deleted["applied"][0]["deleted"], true);
    let orphan = sync_body(
        "orphan-edit",
        association,
        &show_id,
        serde_json::json!([{"type": "update_object", "kind": "fixture_note", "id": "n1",
            "fields": [{"path": "/text", "base": "b", "value": "c"}]}]),
    );
    let outcome = json(post_sync(&app, &token, &orphan).await).await;
    assert_eq!(outcome["conflicts"][0]["reason"], "object_deleted");
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn a_transaction_for_another_show_or_desk_is_refused_without_writing() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Architect").await;
    let show_id = open_sync_show(&app, &token, "Sync active").await;
    let other = create_show(&app, &token, "Sync inactive").await;
    let other_id = other["id"].as_str().unwrap();
    let before = active_revision(&state);
    let operations = serde_json::json!([{"type": "create_object", "kind": "cad_annotation",
        "id": "a1", "body": annotation("x", "y")}]);
    let wrong = post_sync(
        &app,
        &token,
        &sync_body("wrong-show", Uuid::new_v4(), other_id, operations.clone()),
    )
    .await;
    assert_eq!(wrong.status(), StatusCode::CONFLICT);
    let wrong = json(wrong).await;
    assert_eq!(wrong["kind"], "show_not_active");
    assert_eq!(wrong["active_show_id"], show_id.as_str());
    let mut foreign = sync_body("wrong-desk", Uuid::new_v4(), &show_id, operations);
    foreign["origin"]["desk_identity"] = serde_json::json!(Uuid::new_v4());
    let foreign = post_sync(&app, &token, &foreign).await;
    assert_eq!(foreign.status(), StatusCode::CONFLICT);
    assert_eq!(json(foreign).await["kind"], "desk_mismatch");
    assert_eq!(active_revision(&state), before, "nothing was written");
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn desk_only_kinds_unknown_metadata_and_malformed_fields_are_refused() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Architect").await;
    let show_id = open_sync_show(&app, &token, "Sync unknown").await;
    let before = active_revision(&state);
    for (request_id, operations) in [
        (
            "cue-list",
            serde_json::json!([{"type": "create_object", "kind": "cue_list", "id": "c",
            "body": {}}]),
        ),
        (
            "show-name",
            serde_json::json!([{"type": "set_metadata", "key": "name",
            "value": "Renamed"}]),
        ),
        (
            "pointer",
            serde_json::json!([{"type": "create_object", "kind": "fixture_note",
            "id": "n", "body": {"text": "a"}}, {"type": "update_object",
            "kind": "fixture_note", "id": "n", "fields": [{"path": "text", "value": "b"}]}]),
        ),
        ("empty", serde_json::json!([])),
    ] {
        let response = post_sync(
            &app,
            &token,
            &sync_body(request_id, Uuid::new_v4(), &show_id, operations),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{request_id}");
        assert_eq!(json(response).await["kind"], "invalid");
    }
    assert_eq!(active_revision(&state), before);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn patch_fields_merge_through_the_patch_capability() {
    let (state, data_dir) = test_state();
    let (profile_id, mode_id) = install_patch_route_profile(&state);
    let app = router(state.clone());
    let (token, _) = login(&app, "Architect").await;
    let show_id = open_sync_show(&app, &token, "Sync patch").await;
    let association = Uuid::new_v4();
    let fixture_id = Uuid::new_v4();
    let fixture = serde_json::json!({
        "fixture_id": fixture_id, "fixture_number": 1, "virtual_fixture_number": null,
        "name": "Spot 1", "profile_id": profile_id, "profile_revision": 1, "mode_id": mode_id,
        "split_patches": [{"split": 1, "universe": 1, "address": 1}], "layer_id": "default",
        "direct_control": null, "location": {"x": 0, "y": 0, "z": 0},
        "rotation": {"x": 0.0, "y": 0.0, "z": 0.0}, "multipatch": [],
        "move_in_black_enabled": true, "move_in_black_delay_millis": 0,
        "highlight_overrides": []
    });
    let created = post_sync(
        &app,
        &token,
        &sync_body(
            "patch",
            association,
            &show_id,
            serde_json::json!([{"type": "create_patch_fixture", "fixture": fixture}]),
        ),
    )
    .await;
    assert_eq!(created.status(), StatusCode::OK);
    let created = json(created).await;
    assert_eq!(created["applied"][0]["kind"], "patched_fixture");
    let fixture_revision = created["applied"][0]["revision"].as_u64().unwrap();
    // The desk moves the fixture along X through its own sparse update route.
    let moved = post_patch_update(
        &app,
        &token,
        &show_id,
        &fixture_id.to_string(),
        serde_json::json!({"request_id": "desk-move", "expected_fixture_revision": fixture_revision,
            "expected_patch_revision": created["patch_revision"],
            "expected_show_revision": created["show_revision"], "multipatch_instance_id": null,
            "action": "set_location_axis", "axis": "x", "millimetres": 500}),
    )
    .await;
    assert_eq!(moved.status(), StatusCode::OK);
    let outcome = json(
        post_sync(
            &app,
            &token,
            &sync_body(
                "architect-move",
                association,
                &show_id,
                serde_json::json!([{"type": "update_patch_fixture", "fixture_id": fixture_id,
                "fields": [
                    {"path": "/location/x", "base": 0, "value": 250},
                    {"path": "/location/z", "base": 0, "value": 6000},
                    {"path": "/name", "base": "Spot 1", "value": "FOH Spot"}
                ]}]),
            ),
        )
        .await,
    )
    .await;
    assert_eq!(outcome["status"], "conflicted");
    assert_eq!(outcome["conflicts"][0]["path"], "/location/x");
    assert_eq!(outcome["conflicts"][0]["theirs"], 500);
    let patch = json(
        app.clone()
            .oneshot(
                Request::get("/api/v2/patch")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let stored = &patch["fixtures"][0];
    assert_eq!(
        stored["location"],
        serde_json::json!({"x": 500, "y": 0, "z": 6000})
    );
    assert_eq!(stored["name"], "FOH Spot");
    let _ = std::fs::remove_dir_all(data_dir);
}
