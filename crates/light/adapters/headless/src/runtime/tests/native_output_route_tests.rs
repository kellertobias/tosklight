//! Final-native output observation and the desk-owned Preload lane.
use super::*;
use crate::runtime::visualization_frame::RenderedSemanticFrame;
use light_core::{AttributeKey, AttributeValue, FixtureId};
use light_wire::v2::output_control::OutputDmxSnapshot;

async fn read(app: &Router, token: &str) -> OutputDmxSnapshot {
    let response = app
        .clone()
        .oneshot(
            Request::get("/api/v2/output/dmx?include_preload=true")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    serde_json::from_value(json(response).await).unwrap()
}
fn publish(state: &AppState) {
    let rendered = RenderedSemanticFrame::untraced(
        state.output.render(Default::default()).unwrap(),
        Default::default(),
    );
    state.output.render_frames_and_publish(
        &rendered,
        light_wire::v2::visualization::VisualizationScope { show_id: None },
    );
}

#[tokio::test]
async fn point_reads_retain_the_published_generation_and_pose_until_the_next_output_frame() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../assets/fixture-library/tosklight--3d-point.toskfixture");
    let profile = light_fixture::read_fixture_package(&std::fs::read(path).unwrap()).unwrap();
    let point = FixtureId::new();
    let mut fixture = operational_fixture(point);
    fixture.definition = profile.resolved_definition(profile.modes[0].id).unwrap();
    fixture.logical_heads = profile.modes[0]
        .heads
        .iter()
        .enumerate()
        .filter(|(_, head)| !head.master_shared)
        .map(|(index, head)| light_fixture::PatchedHead {
            profile_head_id: Some(head.id),
            head_index: index as u16,
            fixture_id: FixtureId::new(),
        })
        .collect();
    fixture.universe = None;
    fixture.address = None;
    state
        .output
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    let tracked = |value| {
        light_engine::TrackedOverride::new(
            point,
            AttributeKey("point.position.x".into()),
            AttributeValue::Normalized(value),
        )
    };
    state.output.engine().set_tracked_overrides([tracked(0.55)]);
    let rendered = RenderedSemanticFrame::untraced(
        state.output.render(Default::default()).unwrap(),
        Default::default(),
    );
    assert!(!rendered.rendered.resolved_values.materialised_by_name());
    state.output.render_frames_and_publish(
        &rendered,
        light_wire::v2::visualization::VisualizationScope { show_id: None },
    );
    let source = state.output.dmx_snapshot().1.unwrap();

    // Tracking and the runtime generation advance without publishing output. A GET must
    // neither solve the newer Point nor label the older frame with the newer generation.
    state.output.engine().set_tracked_overrides([tracked(0.9)]);
    let mut changed = (*state.output.snapshot()).clone();
    Arc::make_mut(&mut changed.fixtures)[0].location.x = 9000;
    state.output.replace_snapshot(changed).unwrap();
    for _ in 0..3 {
        let response = app
            .clone()
            .oneshot(
                Request::get("/api/v2/output/dmx")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let snapshot: OutputDmxSnapshot = serde_json::from_value(json(response).await).unwrap();
        assert_eq!(snapshot.frame, Some(source.identity()));
        let native = snapshot.native.unwrap();
        assert_eq!(native.frame, snapshot.frame);
        assert_eq!(native.points, snapshot.points);
        assert_eq!(snapshot.points.len(), 1);
        assert!(
            (snapshot.points[0].offset_metres[0] - 10.0).abs() < 0.0001,
            "published pose {:?}; root value {:?}",
            snapshot.points[0],
            rendered
                .rendered
                .resolved_values
                .value(point, &AttributeKey("point.position.x".into()))
        );
        assert!(
            !source.values.materialised_by_name(),
            "a Point read must not materialize the show"
        );
    }

    let next = RenderedSemanticFrame::untraced(
        state.output.render(Default::default()).unwrap(),
        Default::default(),
    );
    assert_ne!(next.rendered.generation, rendered.rendered.generation);
    assert_eq!(next.rendered.points[0].origin_metres[0], 9.0);
    assert!((next.rendered.points[0].offset_metres[0] - 80.0).abs() < 0.0001);
    state.output.render_frames_and_publish(
        &next,
        light_wire::v2::visualization::VisualizationScope { show_id: None },
    );
    let published = state.output.dmx_snapshot().1.unwrap();
    assert!(published.sequence > source.sequence);
    assert_eq!(published.generation, next.rendered.generation);
    assert_eq!(published.sampled_at, next.rendered.sampled_at);
    assert!(
        (source.points[0].offset_metres[0] - 10.0).abs() < 0.0001,
        "retaining an older frame must protect its pooled Point buffer"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn native_observer_reads_real_live_overrides_and_unpatched_desk_preload_without_a_programmer()
{
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (operator_token, _) = login(&app, "Operator").await;
    let operator = authenticate_token(&state, &operator_token).unwrap();
    let observer = Session {
        id: SessionId::new(),
        token: "native-observer".into(),
        ..operator.clone()
    };
    state.sessions.insert_session(observer.clone());
    state.sessions.set_role(
        observer.id,
        light_wire::v2::runtime::RuntimeSessionRole::Visualizer,
    );
    assert!(state.programming.get(observer.id).is_none());
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../assets/fixture-library/cameo--root-par-6.toskfixture");
    let profile = light_fixture::read_fixture_package(&std::fs::read(path).unwrap()).unwrap();
    let mode = &profile.modes[0];
    let red = mode
        .channels
        .iter()
        .position(|c| c.attribute.0.as_ref() == "color.red")
        .unwrap();
    let uv = mode
        .channels
        .iter()
        .position(|c| c.attribute.0.as_ref() == "color.uv")
        .unwrap();
    let first = FixtureId::new();
    let other = FixtureId::new();
    let mut fixture = operational_fixture(first);
    fixture.definition = profile.resolved_definition(mode.id).unwrap();
    fixture.logical_heads.clear();
    fixture.universe = None;
    fixture.address = None;
    let mut patched = fixture.clone();
    patched.fixture_id = other;
    patched.universe = Some(1);
    patched.address = Some(1);
    state
        .output
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture, patched].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    for id in [first, other] {
        state.programming.set(
            operator.id,
            id,
            AttributeKey::intensity(),
            AttributeValue::Normalized(1.),
        );
        state.programming.set(
            operator.id,
            id,
            AttributeKey("color.red".into()),
            AttributeValue::Normalized(0.25),
        );
    }
    state.output.set_dmx_override(1, 1, Some(201));
    publish(&state);
    let live = read(&app, &observer.token).await;
    assert_eq!(live.native_protocol, 1);
    assert_eq!(live.universes[0].slots[0], 201);
    assert_eq!(
        live.native
            .as_ref()
            .unwrap()
            .instances
            .iter()
            .find(|i| i.fixture_id == first.0)
            .unwrap()
            .raw[red],
        64
    );
    assert!(live.preload.unwrap().instances.is_empty());
    assert!(state.programming.arm_preload(operator.id, true));
    // Explicit same-value intent must still own Preload, including when a live override differs.
    state.programming.set(
        operator.id,
        first,
        AttributeKey("color.red".into()),
        AttributeValue::Normalized(0.25),
    );
    state.programming.set(
        operator.id,
        first,
        AttributeKey("color.uv".into()),
        AttributeValue::RawDmxExact(197),
    );
    let preview = read(&app, &observer.token).await;
    let lane = preview.preload.unwrap();
    assert_eq!(
        lane.instances.len(),
        1,
        "an unrelated patched fixture remains Live"
    );
    assert_eq!(lane.instances[0].fixture_id, first.0);
    assert_eq!(lane.instances[0].raw[uv], 197);
    let owned = lane.instances[0].owned_channels.as_ref().unwrap();
    assert!(owned[red] && owned[uv]);
    assert_eq!(owned.iter().filter(|v| **v).count(), 2);
    assert_eq!(preview.universes[0].slots[0], 201);
    assert!(
        state.programming.get(observer.id).is_none(),
        "observing never registers an operator"
    );
    // Same-value pending Dynamic ownership is explicit, and cannot claim a second fixture just
    // because the live sample has advanced since the published source.
    let mut programmer = state.programming.get(operator.id).unwrap();
    programmer.preload_pending.clear();
    programmer.preload_active = Arc::new(Vec::new());
    programmer.preload_dynamic_pending = Arc::new(vec![light_dynamics::DynamicAddressValue {
        fixture_id: first,
        attribute: AttributeKey("color.red".into()),
        value: light_dynamics::DynamicSemanticValue::FixAt {
            value: 0.25,
            timing: Default::default(),
        },
        programmer_order: 1,
        changed_at_millis: 0,
    }]);
    state.programming.restore(programmer);
    // Simulate unrelated live progress after the retained output frame.
    state.programming.arm_preload(operator.id, false);
    state.programming.set(
        operator.id,
        other,
        AttributeKey("color.red".into()),
        AttributeValue::Normalized(0.75),
    );
    let dynamic_preview = read(&app, &observer.token).await.preload.unwrap();
    assert_eq!(dynamic_preview.instances.len(), 1);
    assert_eq!(dynamic_preview.instances[0].fixture_id, first.0);
    assert_eq!(
        dynamic_preview.instances[0]
            .owned_channels
            .as_ref()
            .unwrap()
            .iter()
            .filter(|v| **v)
            .count(),
        1
    );
    assert!(
        dynamic_preview.instances[0]
            .owned_channels
            .as_ref()
            .unwrap()[red]
    );
    for value in [
        light_dynamics::DynamicSemanticValue::DynamicOff {
            instance_link: Uuid::new_v4(),
            timing: Default::default(),
        },
        light_dynamics::DynamicSemanticValue::Release,
    ] {
        let mut programmer = state.programming.get(operator.id).unwrap();
        Arc::make_mut(&mut programmer.preload_dynamic_pending)[0].value = value;
        state.programming.restore(programmer);
        let lane = read(&app, &observer.token).await.preload.unwrap();
        assert_eq!(lane.instances.len(), 1);
        let owned = lane.instances[0].owned_channels.as_ref().unwrap();
        assert!(owned[red]);
        assert_eq!(owned.iter().filter(|v| **v).count(), 1);
    }
    let mut changed = (*state.output.snapshot()).clone();
    changed.revision = 2;
    state.output.replace_snapshot(changed).unwrap();
    let stale = read(&app, &observer.token).await;
    assert_eq!(stale.native_protocol, 1);
    assert!(
        stale.native.is_none() && stale.preload.is_none(),
        "retained output cannot be relabeled as a new generation"
    );
    publish(&state);
    assert!(read(&app, &observer.token).await.native.is_some());
    let _ = std::fs::remove_dir_all(data_dir);
}
