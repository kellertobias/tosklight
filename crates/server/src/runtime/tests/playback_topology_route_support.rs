use super::*;

pub(super) struct TopologyScenario {
    pub state: AppState,
    pub app: Router,
    pub token: String,
    pub show_id: String,
    pub data_dir: PathBuf,
}

impl TopologyScenario {
    pub async fn new(name: &str) -> Self {
        let (state, data_dir) = test_state();
        let app = router(state.clone());
        let (token, _) = login(&app, "Operator").await;
        let show = create_show(&app, &token, name).await;
        let show_id = show["id"].as_str().unwrap().to_owned();
        open_topology_show(&app, &token, &show_id, None).await;
        Self {
            state,
            app,
            token,
            show_id,
            data_dir,
        }
    }

    pub fn show_revision(&self) -> u64 {
        show_revision(&self.state, &self.show_id)
    }

    pub async fn action(&self, revision: u64, body: serde_json::Value) -> Response {
        post_topology(
            &self.app,
            Some(&self.token),
            &self.show_id,
            Some(revision),
            body,
            None,
        )
        .await
    }

    pub fn document(&self) -> light_show::PortableShowDocument {
        show_document(&self.state, &self.show_id)
    }

    pub fn cleanup(self) {
        let _ = std::fs::remove_dir_all(self.data_dir);
    }
}

pub(super) async fn open_topology_show(
    app: &Router,
    token: &str,
    show_id: &str,
    desk_boundary: Option<&str>,
) {
    let mut request = Request::post(format!("/api/v1/shows/{show_id}/open"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {token}"));
    if let Some(value) = desk_boundary {
        request = request.header("x-light-desk-token", value);
    }
    let response = app
        .clone()
        .oneshot(
            request
                .body(Body::from(r#"{"transition":"hold_current"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

pub(super) async fn post_topology(
    app: &Router,
    token: Option<&str>,
    show_id: &str,
    expected_revision: Option<u64>,
    body: serde_json::Value,
    desk_boundary: Option<&str>,
) -> Response {
    let mut request = Request::post(format!("/api/v2/shows/{show_id}/playback-topology/actions"))
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    if let Some(revision) = expected_revision {
        request = request.header(header::IF_MATCH, revision.to_string());
    }
    if let Some(value) = desk_boundary {
        request = request.header("x-light-desk-token", value);
    }
    app.clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap()
}

pub(super) async fn login_on_desk(
    app: &Router,
    username: &str,
    desk_id: Option<Uuid>,
    desk_boundary: Option<&str>,
) -> serde_json::Value {
    let mut request =
        Request::post("/api/v1/sessions").header(header::CONTENT_TYPE, "application/json");
    if let Some(value) = desk_boundary {
        request = request.header("x-light-desk-token", value);
    }
    let response = app
        .clone()
        .oneshot(
            request
                .body(Body::from(
                    serde_json::json!({"username":username,"desk_id":desk_id}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    json(response).await
}

pub(super) async fn create_show_with_boundary(
    app: &Router,
    token: &str,
    name: &str,
    boundary: &str,
) -> serde_json::Value {
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v1/shows")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header("x-light-desk-token", boundary)
                .body(Body::from(
                    serde_json::json!({"name":name,"data_base64":null,"overwrite":false})
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    json(response).await
}

pub(super) fn configure_request(
    request_id: &str,
    page_revision: u64,
    playback_revision: u64,
) -> serde_json::Value {
    serde_json::json!({
        "request_id": request_id,
        "action": {
            "type": "configure_slot",
            "page": 1,
            "slot": 4,
            "expected_page_revision": page_revision,
            "expected_page_object_id": if page_revision == 0 { None } else { Some("1") },
            "expected_playback_revision": playback_revision,
            "expected_playback_object_id": if playback_revision == 0 { None } else { Some("1") },
            "playback": playback_body()
        }
    })
}

pub(super) fn clear_request(
    request_id: &str,
    page_revision: u64,
    playback_revision: u64,
) -> serde_json::Value {
    serde_json::json!({
        "request_id": request_id,
        "action": {
            "type": "clear_mapped_playback",
            "page": 1,
            "slot": 4,
            "expected_page_revision": page_revision,
            "expected_page_object_id": if page_revision == 0 { None } else { Some("1") },
            "expected_playback_revision": playback_revision,
            "expected_playback_object_id": if playback_revision == 0 { None } else { Some("1") }
        }
    })
}

pub(super) fn save_request(request_id: &str, expected_revision: u64) -> serde_json::Value {
    let cue_list = cue_list(request_id);
    let cue_list_id = cue_list.id.0;
    let mut body = serde_json::to_value(cue_list).unwrap();
    body["future_topology"] = serde_json::json!({"retained": true});
    serde_json::json!({
        "request_id": request_id,
        "action": {
            "type": "save_cue_list",
            "cue_list_id": cue_list_id,
            "expected_revision": expected_revision,
            "expected_object_id": if expected_revision == 0 {
                None
            } else {
                Some(cue_list_id.to_string())
            },
            "body": body
        }
    })
}

pub(super) fn projection_revision(value: &serde_json::Value, kind: &str) -> u64 {
    value["objects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|object| object["kind"] == kind)
        .unwrap()["object_revision"]
        .as_u64()
        .unwrap()
}

pub(super) fn show_events(
    state: &AppState,
    after: u64,
) -> Vec<Arc<light_application::EventEnvelope>> {
    let light_application::EventReplay::Events(events) = state
        .application_events
        .replay(after, &light_application::EventFilter::default())
    else {
        panic!("expected retained Playback topology events")
    };
    events
}

pub(super) fn assert_etag(response: &Response, revision: u64) {
    assert_eq!(
        response.headers().get(header::ETAG).unwrap(),
        format!("\"{revision}\"").as_str()
    );
}

fn show_revision(state: &AppState, show_id: &str) -> u64 {
    show_document(state, show_id).revision().value()
}

fn show_document(state: &AppState, show_id: &str) -> light_show::PortableShowDocument {
    let id = light_core::ShowId(Uuid::parse_str(show_id).unwrap());
    let entry = state.desk.lock().show(id).unwrap().unwrap();
    ShowStore::open(entry.path)
        .unwrap()
        .portable_document()
        .unwrap()
}

fn playback_body() -> serde_json::Value {
    serde_json::json!({
        "number": 999,
        "name": "House",
        "target": {"type": "grand_master"},
        "buttons": ["blackout", "pause_dynamics", "flash"],
        "button_count": 3,
        "fader": "master",
        "has_fader": true,
        "go_activates": true,
        "auto_off": true,
        "xfade_millis": 0,
        "color": "#20c997",
        "flash_release": "release_all",
        "protect_from_swap": false,
        "presentation_icon": null,
        "presentation_image": null
    })
}

fn cue_list(name: &str) -> light_playback::CueList {
    light_playback::CueList {
        id: light_core::CueListId::new(),
        name: name.into(),
        priority: 0,
        mode: light_playback::CueListMode::Sequence,
        looped: false,
        chaser_step_millis: 1_000,
        speed_group: None,
        intensity_priority_mode: light_playback::IntensityPriorityMode::Htp,
        wrap_mode: Some(light_playback::WrapMode::Off),
        restart_mode: light_playback::RestartMode::FirstCue,
        force_cue_timing: false,
        disable_cue_timing: false,
        chaser_xfade_millis: 0,
        chaser_xfade_percent: Some(0),
        speed_multiplier: 1.0,
        cues: vec![light_playback::Cue::new(1.0)],
    }
}
