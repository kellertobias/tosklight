use std::sync::Arc;
use std::sync::atomic::Ordering;

use axum::http::StatusCode;
use media_domain::{CommandSource, Timestamp};
use serde_json::{Value, json};

use crate::routes::bench::{Bench, bench, get, post, send};

async fn edit(bench: &Bench, path: &str, body: Value) -> (StatusCode, Value) {
    send(
        &bench.router,
        post(format!("/api/v2/automation{path}"), &body.to_string()),
    )
    .await
}

async fn playlist(bench: &Bench) -> String {
    let (status, body) = edit(
        bench,
        "/playlists/create",
        json!({"requestId":"create-playlist", "name":"Marketing", "layers":[1,2]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["configuration"]["playlists"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[tokio::test]
async fn create_assigns_identity_and_retries_return_the_original_result() {
    let bench = bench();
    let id = playlist(&bench).await;
    let (status, replay) = edit(
        &bench,
        "/playlists/create",
        json!({"requestId":"create-playlist", "name":"Duplicate"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(replay["configuration"]["playlists"][0]["id"], id);
    assert_eq!(replay["configuration"]["playlists"][0]["name"], "Marketing");
    assert!(
        !replay["configuration"]["playlists"][0]["enabled"]
            .as_bool()
            .unwrap()
    );
    assert_eq!(bench.configuration.load().automation.playlists.len(), 1);
    assert_eq!(bench.stored.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn partial_playlist_edits_preserve_unmentioned_fields_and_unknown_fields_are_tolerated() {
    let bench = bench();
    let id = playlist(&bench).await;
    let (status, body) = edit(
        &bench,
        &format!("/playlists/{id}/update"),
        json!({"requestId":"priority", "priority":15,"futureField":true}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let playlist = &body["configuration"]["playlists"][0];
    assert_eq!(playlist["name"], "Marketing");
    assert_eq!(playlist["layers"], json!([1, 2]));
    assert_eq!(playlist["priority"], 15);
    assert_eq!(bench.applied.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn invalid_entries_and_layer_pairs_are_not_stored() {
    let bench = bench();
    let id = playlist(&bench).await;
    for (request, fields) in [
        ("same-layer", json!({"layers":[1,1]})),
        ("unavailable", json!({"layers":[1,3]})),
        (
            "zero-plays",
            json!({"entries":[{"id":uuid::Uuid::new_v4(),"assetId":uuid::Uuid::new_v4(),"plays":0,"holdMillis":1000,"fadeMillis":0}]}),
        ),
    ] {
        let mut body = fields;
        body["requestId"] = request.into();
        let (status, error) = edit(&bench, &format!("/playlists/{id}/update"), body).await;
        // A playlist has no output binding until referenced, so pair3 is accepted as reusable
        // eight-layer intent. Binding it to this two-layer output below must be refused.
        if request == "unavailable" {
            assert_eq!(status, StatusCode::OK);
        } else {
            assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");
        }
    }
    let (status, _) = edit(
        &bench,
        &format!("/outputs/{}/update", bench.output),
        json!({"requestId":"bind-invalid-pair","fallbackPlaylistId":id}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(bench.configuration.load().automation.outputs.len(), 0);
    assert_eq!(bench.stored.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn failed_persistence_never_publishes_the_edit() {
    let bench = bench();
    let id = playlist(&bench).await;
    bench.refuse.store(true, Ordering::SeqCst);
    let (status, error) = edit(
        &bench,
        &format!("/playlists/{id}/update"),
        json!({"requestId":"disk-failed", "name":"Lost"}),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(error["code"], "configuration-not-written");
    assert_eq!(
        bench.configuration.load().automation.playlists[0].name,
        "Marketing"
    );
    assert_eq!(bench.stored.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn referenced_playlist_deletion_requires_reassignment_and_null_clears_fallback() {
    let bench = bench();
    let id = playlist(&bench).await;
    let path = format!("/outputs/{}/update", bench.output);
    assert_eq!(
        edit(
            &bench,
            &path,
            json!({"requestId":"fallback", "fallbackPlaylistId":id})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        edit(
            &bench,
            &format!("/playlists/{id}/delete"),
            json!({"requestId":"in-use"})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        edit(
            &bench,
            &path,
            json!({"requestId":"clear", "fallbackPlaylistId":null})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert!(
        bench.configuration.load().automation.outputs[0]
            .fallback_playlist_id
            .is_none()
    );
    assert_eq!(
        edit(
            &bench,
            &format!("/playlists/{id}/delete"),
            json!({"requestId":"delete"})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert!(bench.configuration.load().automation.playlists.is_empty());
}

#[tokio::test]
async fn event_edits_are_partial_and_deletion_releases_playlist_reference() {
    let bench = bench();
    let id = playlist(&bench).await;
    let (status, body) = edit(&bench, "/events/create", json!({"requestId":"create-event", "name":"Monday", "outputId":bench.output.to_string(),"playlistId":id,
        "trigger":{"kind":"weekly","weekdays":[0],"startTime":"10:00","endTime":"12:00","endDayOffset":0,"firstDate":null,"lastDate":null}})).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let event = body["configuration"]["events"][0]["id"].as_str().unwrap();
    let (status, body) = edit(
        &bench,
        &format!("/events/{event}/update"),
        json!({"requestId":"rename-event","name":"Weekly campaign"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["configuration"]["events"][0]["trigger"]["kind"],
        "weekly"
    );
    assert_eq!(
        edit(
            &bench,
            &format!("/playlists/{id}/delete"),
            json!({"requestId":"event-in-use"})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        edit(
            &bench,
            &format!("/events/{event}/delete"),
            json!({"requestId":"delete-event"})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        edit(
            &bench,
            &format!("/playlists/{id}/delete"),
            json!({"requestId":"delete-after-event"})
        )
        .await
        .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn live_enable_actions_have_no_edit_identity_and_stop_is_idempotent() {
    let bench = bench();
    let path = format!("/outputs/{}/enabled", bench.output);
    let (status, body) = edit(&bench, &path, json!({"enabled":true})).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(bench.configuration.load().automation.outputs[0].enabled);
    let (status, _) = edit(&bench, &path, json!({"enabled":false})).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!bench.configuration.load().automation.outputs[0].enabled);
    assert_eq!(
        edit(&bench, &path, json!({"enabled":false})).await.0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn apply_now_is_no_store_and_refuses_manual_or_dmx_control() {
    use tower::ServiceExt as _;
    let bench = bench();
    let path = format!("/api/v2/automation/outputs/{}/apply-now", bench.output);
    assert_eq!(
        send(&bench.router, get(path.clone())).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        edit(
            &bench,
            &format!("/outputs/{}/enabled", bench.output),
            json!({"enabled":true})
        )
        .await
        .0,
        StatusCode::OK
    );
    let response = bench
        .router
        .clone()
        .oneshot(get(path.clone()))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert!(
        bench
            .api
            .diagnostics
            .automation
            .apply_now
            .lock()
            .unwrap()
            .remove(&bench.output)
    );
    let mut state = (**bench.state.load()).clone();
    state.outputs[0].ownership.web_takeover = true;
    bench.state.store(Arc::new(state.clone()));
    assert_eq!(
        send(&bench.router, get(path.clone())).await.0,
        StatusCode::CONFLICT
    );
    state.outputs[0].ownership.web_takeover = false;
    state.outputs[0]
        .ownership
        .observe_dmx(CommandSource::ArtNet, Timestamp::ZERO);
    bench.state.store(Arc::new(state));
    assert_eq!(send(&bench.router, get(path)).await.0, StatusCode::CONFLICT);
    assert!(
        bench
            .api
            .diagnostics
            .automation
            .apply_now
            .lock()
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn snapshot_contains_configuration_and_runtime_and_edits_require_identity() {
    let bench = bench();
    let (status, body) = send(&bench.router, get("/api/v2/automation".into())).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["configuration"]["playlists"], json!([]));
    assert!(body["timeZone"].is_string());
    assert!(body["status"].is_array());
    assert_eq!(
        edit(
            &bench,
            "/playlists/create",
            json!({"requestId":"", "name":"Lost"})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert!(bench.stored.lock().unwrap().is_empty());
}

#[tokio::test]
async fn entry_transform_defaults_round_trip_and_invalid_edits_leave_saved_framing_intact() {
    let bench = bench();
    let id = playlist(&bench).await;
    let path = format!("/playlists/{id}/update");
    let mut entry = json!({"id":uuid::Uuid::new_v4(),"assetId":uuid::Uuid::new_v4(),"plays":1,"holdMillis":1000,"fadeMillis":0});
    let (status, response) = edit(
        &bench,
        &path,
        json!({"requestId":"legacy-step","entries":[entry.clone()]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(
        response["configuration"]["playlists"][0]["entries"][0]["transform"],
        json!({"scaleX":1.0,"scaleY":1.0,"positionX":0.0,"positionY":0.0,"rotation":0.0})
    );
    entry["transform"] =
        json!({"scaleX":0.5,"scaleY":0.75,"positionX":-0.4,"positionY":0.2,"rotation":90.0});
    let (status, response) = edit(
        &bench,
        &path,
        json!({"requestId":"authored-step","entries":[entry.clone()]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(
        response["configuration"]["playlists"][0]["entries"][0]["transform"],
        entry["transform"]
    );
    let saved = bench.configuration.load().automation.playlists[0].entries[0].transform;
    entry["transform"]["rotation"] = json!(361.0);
    let (status, response) = edit(
        &bench,
        &path,
        json!({"requestId":"bad-step","entries":[entry]}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(response["code"], "automation-invalid");
    assert!(response["message"].as_str().unwrap().contains("rotation"));
    assert_eq!(
        bench.configuration.load().automation.playlists[0].entries[0].transform,
        saved
    );
    assert_eq!(bench.stored.lock().unwrap().len(), 3);
}
