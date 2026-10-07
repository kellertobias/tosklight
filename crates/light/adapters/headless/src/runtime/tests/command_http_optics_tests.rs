//! TL-558: typed Focus and Zoom through the actual v2 values route and the UND key.
use super::*;
use light_core::programming::{ProgrammingOwner, ScalarIntent, ZoomIntent};

fn field(degrees: f32) -> light_core::AttributeValue {
    light_core::AttributeValue::Zoom(Arc::new(ZoomIntent {
        opening_degrees: ScalarIntent::Value(degrees),
        convention: light_core::OpeningConvention::Field,
    }))
}

fn intent(
    request: &str,
    revision: u64,
    target: serde_json::Value,
    attribute: &str,
    operation: serde_json::Value,
) -> serde_json::Value {
    let mut action = serde_json::json!({
        "type": "apply_intent",
        "attribute": attribute,
        "operation": operation,
        "timing": {"fade": false}
    });
    action
        .as_object_mut()
        .unwrap()
        .extend(target.as_object().unwrap().clone());
    serde_json::json!({
        "request_id": request,
        "expected_revision": revision,
        "expected_capture_mode_revision": 0,
        "action": action,
    })
}

fn relative(component: &str, delta: f32) -> serde_json::Value {
    serde_json::json!({"type": "component_edits", "edits": [{
        "kind": "scalar",
        "component": {"kind": component},
        "operation": {"kind": "relative", "value": delta}
    }]})
}

#[tokio::test]
async fn focus_and_zoom_edit_group_step_release_and_undo_over_the_v2_values_route() {
    let scenario = CommandHttpScenario::new().await;
    let fixture = scenario.install_direct_fixture();
    let fixture_target = serde_json::json!({"fixture_ids": [fixture.0]});
    let group_target = serde_json::json!({"group_id": "1"});
    let stored = |owner: ProgrammingOwner| {
        scenario
            .state
            .programming
            .get(scenario.session.id)
            .unwrap()
            .values
            .iter()
            .find(|v| v.fixture_id == fixture && v.attribute == owner.key())
            .map(|v| v.value.clone())
    };
    let revision = || scenario.state.programming.normal_values_revision();
    let send = |body: serde_json::Value| scenario.values_action(body);

    let zoom_wire = serde_json::json!({"type": "absolute_set", "value": {"kind": "zoom", "value": {
        "opening_degrees": {"kind": "value", "value": 20.0}, "convention": "field"
    }}});
    let response = send(intent(
        "zoom-set",
        revision(),
        fixture_target.clone(),
        "zoom",
        zoom_wire,
    ))
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let focus_set =
        serde_json::json!({"type": "absolute_set", "value": {"kind": "normalized", "value": 0.4}});
    let response = send(intent(
        "focus-set",
        revision(),
        fixture_target.clone(),
        "focus",
        focus_set,
    ))
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(stored(ProgrammingOwner::Zoom), Some(field(20.)));

    // Relative degrees on the fixture keep the convention and leave Focus alone.
    let response = send(intent(
        "zoom-rel",
        revision(),
        fixture_target.clone(),
        "zoom",
        relative("zoom", 10.),
    ))
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(stored(ProgrammingOwner::Zoom), Some(field(30.)));
    assert_eq!(
        stored(ProgrammingOwner::Focus),
        Some(light_core::AttributeValue::Normalized(0.4))
    );

    // A Group component step writes the Group's own Zoom; a unitless normalized step is refused.
    let response = send(intent(
        "group-zoom",
        revision(),
        group_target.clone(),
        "zoom",
        relative("zoom", 5.),
    ))
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = json(response).await;
    let group = &body["projection"]["group_values"][0];
    assert_eq!(
        (group["group_id"].as_str(), group["attribute"].as_str()),
        (Some("1"), Some("zoom"))
    );
    let before = revision();
    let refused = send(intent(
        "group-zoom-normalized",
        revision(),
        group_target,
        "zoom",
        serde_json::json!({"type": "relative_step", "delta": 0.1}),
    ))
    .await;
    assert!(refused.status().is_client_error(), "{}", refused.status());
    assert_eq!(revision(), before);

    // Per-owner release, then UND restores exactly that Zoom.
    let release = serde_json::json!({
        "request_id": "release-zoom",
        "expected_revision": revision(),
        "expected_capture_mode_revision": 0,
        "action": {"type": "release_fixture", "fixture_id": fixture.0, "attribute": "zoom"}
    });
    assert_eq!(send(release).await.status(), StatusCode::OK);
    assert_eq!(stored(ProgrammingOwner::Zoom), None);
    assert_eq!(
        stored(ProgrammingOwner::Focus),
        Some(light_core::AttributeValue::Normalized(0.4))
    );
    let undone = scenario
        .press_key(&scenario.token, "UND", "undo-zoom-release")
        .await;
    assert_eq!(undone.status(), StatusCode::OK);
    assert_eq!(stored(ProgrammingOwner::Zoom), Some(field(30.)));
    assert_eq!(
        stored(ProgrammingOwner::Focus),
        Some(light_core::AttributeValue::Normalized(0.4))
    );
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn recorded_cues_store_focus_and_zoom_as_independent_typed_changes_in_the_show() {
    let scenario = CommandHttpScenario::new().await;
    let show_id = scenario.create_and_open_show("Optics cue recording").await;
    let fixture = scenario.install_direct_fixture();
    let programming = &scenario.state.programming;
    let session = scenario.session.id;
    programming.set(
        session,
        fixture,
        ProgrammingOwner::Focus.key(),
        light_core::AttributeValue::Normalized(0.4),
    );
    programming.set(session, fixture, ProgrammingOwner::Zoom.key(), field(20.));
    let record = |request: &str, cue: f64, operation: &str| {
        let mut body = cue_record_request(
            request,
            serde_json::json!({"kind": "pool", "playback_number": 27}),
            Some(cue),
            "current_capture",
            "hold",
        );
        body["operation"] = operation.into();
        body
    };
    let send = |body: serde_json::Value| {
        scenario.cue_recording_action(
            &show_id,
            Some(&scenario.token),
            Some(active_show_revision(&scenario)),
            body,
        )
    };
    let change = |cue: usize, owner: ProgrammingOwner| {
        // Read back from the persisted show file, not from in-memory state.
        stored_cue_list(&scenario, 27).2.cues[cue]
            .changes
            .iter()
            .find(|change| change.fixture_id == fixture && change.attribute == owner.key())
            .and_then(|change| change.value.clone())
    };

    let response = send(record("record-cue-1", 1.0, "overwrite")).await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "{}",
        json(response).await
    );
    assert_eq!(change(0, ProgrammingOwner::Zoom), Some(field(20.)));
    assert_eq!(
        change(0, ProgrammingOwner::Focus),
        Some(light_core::AttributeValue::Normalized(0.4))
    );

    // Merge only a new Zoom into cue 1: the stored Focus is untouched.
    assert!(programming.programmers().release_fixture_attribute(
        session,
        fixture,
        &ProgrammingOwner::Focus.key()
    ));
    programming.set(session, fixture, ProgrammingOwner::Zoom.key(), field(35.));
    let response = send(record("merge-cue-1", 1.0, "merge")).await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "{}",
        json(response).await
    );
    assert_eq!(change(0, ProgrammingOwner::Zoom), Some(field(35.)));
    assert_eq!(
        change(0, ProgrammingOwner::Focus),
        Some(light_core::AttributeValue::Normalized(0.4))
    );

    // A new cue records only the owner the programmer holds.
    let response = send(record("record-cue-2", 2.0, "overwrite")).await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "{}",
        json(response).await
    );
    assert_eq!(change(1, ProgrammingOwner::Zoom), Some(field(35.)));
    assert_eq!(change(1, ProgrammingOwner::Focus), None);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}
