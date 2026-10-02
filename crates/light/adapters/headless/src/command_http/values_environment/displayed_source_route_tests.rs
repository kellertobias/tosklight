//! TL-594 C2 route contract: `GET /api/v2/output/readouts` is authenticated, show-guarded,
//! typed, `no-store`, and leases the delivered source to the calling session only.
use super::*;
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
    response::Response,
};
use http_body_util::BodyExt;
use tower::ServiceExt;

async fn login(app: &Router) -> (String, SessionId) {
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v2/sessions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"username":"Operator"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let value: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    (
        value["token"].as_str().unwrap().into(),
        SessionId(Uuid::parse_str(value["session_id"].as_str().unwrap()).unwrap()),
    )
}

async fn get(app: &Router, token: Option<&str>, show: Option<&str>, uri: &str) -> Response {
    let mut request = Request::get(uri);
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    if let Some(show) = show {
        request = request.header("x-tosk-show", show);
    }
    app.clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap()
}

async fn snapshot(response: Response) -> OutputReadoutSnapshot {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).expect("the response is the typed readout snapshot")
}

#[tokio::test]
async fn the_readout_route_is_authenticated_show_guarded_typed_and_session_leased() {
    let (state, directory) = crate::runtime::tests::test_state();
    let fixture = mover();
    let owner = fixture.fixture_id;
    install(&state, fixture);
    let frame = render(&state, owner, 1., 0., Some(requested([1., 2., 3.])));
    publish(&state, &frame, None);
    let app = crate::runtime::http_router::build(state.clone());
    let (token, session) = login(&app).await;
    let uri = format!(
        "/api/v2/output/readouts?fixture_ids={0},{0}&future=1",
        owner.0
    );

    assert_eq!(
        get(&app, None, None, &uri).await.status(),
        StatusCode::UNAUTHORIZED
    );
    let other_show = Uuid::new_v4().to_string();
    assert_eq!(
        get(&app, Some(&token), Some(&other_show), &uri)
            .await
            .status(),
        StatusCode::CONFLICT,
        "the show-switch race is rejected"
    );
    assert_eq!(
        get(&app, Some(&token), Some("not-a-uuid"), &uri)
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        get(
            &app,
            Some(&token),
            None,
            "/api/v2/output/readouts?fixture_ids=nope"
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        get(
            &app,
            Some(&token),
            None,
            "/api/v2/output/readouts?lane=sideways"
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );

    let response = get(&app, Some(&token), None, &uri).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let readout = snapshot(response).await;
    assert_eq!(readout.lane, VisualizationLane::Normal);
    assert_eq!(readout.owners.len(), 2);
    assert_eq!(readout.owners[0].fixture_id, owner.0);
    assert!(readout.owners[0].requested.is_some());
    assert!(readout.owners[0].position.common.is_some());
    let lease = readout.lease.unwrap();
    assert!(
        adopt(&state, session, owner, false, Some(live(lease)))
            .displayed_source_hold
            .is_none(),
        "the route lease resolves for the calling session"
    );
    assert_eq!(
        adopt(&state, SessionId::new(), owner, false, Some(live(lease))).displayed_source_hold,
        HELD,
        "and for no other session"
    );

    let preload = get(
        &app,
        Some(&token),
        None,
        &format!(
            "/api/v2/output/readouts?lane=preload&fixture_ids={}",
            owner.0
        ),
    )
    .await;
    assert_eq!(preload.status(), StatusCode::OK);
    let preload = snapshot(preload).await;
    assert_eq!(
        preload.unavailable,
        Some(OutputReadoutUnavailable::NoAcceptedPreload)
    );
    assert_eq!((preload.lease, preload.frame), (None, None));
    let _ = std::fs::remove_dir_all(directory);
}
