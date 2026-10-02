//! Accepted output values never borrow a newer observer's sources or control options.
use super::super::visualization_frame::{FrameDynamicSources, RenderedSemanticFrame};
use super::*;
use light_core::{AttributeKey, AttributeValue, FixtureId, MergeMode, TimedValue};
use light_engine::{ContributionBatch, ContributionSample, RenderOptions};
use light_wire::v2::visualization::VisualizationScope;

fn captured_frame(state: &AppState, fixture: FixtureId) -> RenderedSemanticFrame {
    state
        .output
        .replace_snapshot(EngineSnapshot {
            revision: 23,
            fixtures: vec![operational_fixture(fixture)].into(),
            ..Default::default()
        })
        .unwrap();
    let options = RenderOptions {
        grand_master: 0.6,
        ..Default::default()
    };
    let sample = ContributionSample::independent(TimedValue {
        fixture_id: fixture,
        attribute: AttributeKey::intensity(),
        value: AttributeValue::Normalized(0.25),
        priority: 100,
        changed_at: chrono::Utc::now(),
        programmer_order: 0,
        merge_mode: MergeMode::Ltp,
        fade: false,
        fade_millis: None,
        delay_millis: None,
    });
    let rendered = state
        .output
        .engine()
        .render_with_contribution_batches(options, &[ContributionBatch::new([sample])])
        .unwrap();
    RenderedSemanticFrame::untraced(rendered, options)
}

async fn session(state: &AppState) -> Session {
    let (token, _) = login(&router(state.clone()), "Operator").await;
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        format!("Bearer {token}").parse().unwrap(),
    );
    authenticate(state, &headers).unwrap()
}

#[tokio::test]
async fn accepted_untraced_frame_never_resamples_to_fill_missing_dynamic_evidence() {
    let (state, directory) = test_state();
    let fixture = FixtureId::new();
    let frame = captured_frame(&state, fixture);
    let scope = VisualizationScope {
        show_id: Some(Uuid::new_v4()),
    };
    state.output.render_frames_and_publish(&frame, scope);
    let source = state.output.latest_visualization_frame().unwrap();
    assert!(source.dynamics.is_none());
    let mut newer = (*state.output.snapshot()).clone();
    newer.revision = 94;
    state.output.replace_snapshot(newer).unwrap();
    let session = session(&state).await;
    for include_stack in [false, true, true] {
        let snapshot = visualization_snapshot_for_session_content_from_resolved(
            &state,
            &session,
            false,
            include_stack,
            false,
            Some(&source),
        )
        .unwrap();
        assert_eq!(snapshot["revision"], 23);
        assert_eq!(
            snapshot["scope"]["show_id"],
            scope.show_id.unwrap().to_string()
        );
        assert!((snapshot["grand_master"].as_f64().unwrap() - 0.6).abs() < 1e-6);
        assert_eq!(snapshot["dynamic_stack"], serde_json::json!([]));
        assert!(
            snapshot["values"].as_array().unwrap().iter().any(|value| {
                value["fixture_id"] == fixture.0.to_string()
                    && value["attribute"] == "intensity"
                    && value["value"] == serde_json::json!({"kind":"normalized", "value":0.25})
            }),
            "accepted value must survive without a Dynamic sidecar: {snapshot}"
        );
    }
    let _ = std::fs::remove_dir_all(directory);
}

#[tokio::test]
async fn accepted_stack_uses_its_captured_programmer_masks_even_when_live_rows_differ() {
    let (state, directory) = test_state();
    let fixture = FixtureId::new();
    let programmer = Uuid::new_v4();
    let mut frame = captured_frame(&state, fixture);
    frame.dynamics = Some(Arc::new(FrameDynamicSources {
        sample_boundary: None,
        runtime: Default::default(),
        samples: Vec::new(),
        origins: Arc::new(Default::default()),
        programmer_values: Arc::new(vec![(
            programmer,
            150,
            light_dynamics::DynamicAddressValue {
                fixture_id: fixture,
                attribute: AttributeKey::intensity(),
                value: light_dynamics::DynamicSemanticValue::Static {
                    value: AttributeValue::Normalized(0.4),
                    timing: Default::default(),
                },
                changed_at_millis: 1234,
                programmer_order: 7,
            },
        )]),
        cue_values: Arc::from([]),
        ordinary: None,
    }));
    state
        .output
        .render_frames_and_publish(&frame, VisualizationScope { show_id: None });
    let source = state.output.latest_visualization_frame().unwrap();
    let session = session(&state).await;
    assert!(state.output.dynamic_programmer_values().is_empty());
    for _ in 0..3 {
        let snapshot = visualization_snapshot_for_session_content_from_resolved(
            &state,
            &session,
            false,
            true,
            false,
            Some(&source),
        )
        .unwrap();
        let stack = snapshot["dynamic_stack"].as_array().unwrap();
        assert_eq!(stack.len(), 1);
        assert_eq!(stack[0]["source"], format!("Programmer {programmer}"));
        assert_eq!(stack[0]["changed_at_millis"], 1234);
        assert_eq!(
            stack[0]["value"],
            serde_json::json!(AttributeValue::Normalized(0.4))
        );
    }
    let _ = std::fs::remove_dir_all(directory);
}

#[tokio::test]
async fn accepted_stack_retains_captured_cue_rows_after_the_live_cue_is_released() {
    let clock = Arc::new(ManualClock::new(fixed_test_time()));
    let (state, directory) = test_state_with_clock(clock.clone());
    let session = session(&state).await;
    let fixture = FixtureId::new();
    let list_id = light_core::CueListId::new();
    let mut list = operational_cue_list(list_id, fixture);
    list.cues[0].changes.clear();
    let cue_id = list.cues[0].id;
    list.cues[0]
        .dynamic_changes
        .push(light_playback::CueDynamicChange {
            fixture_id: fixture,
            attribute: AttributeKey::intensity(),
            value: light_dynamics::DynamicSemanticValue::FixAt {
                value: 0.4,
                timing: Default::default(),
            },
            automatic_restore: false,
        });
    state
        .output
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![operational_fixture(fixture)].into(),
            cue_lists: vec![list].into(),
            ..Default::default()
        })
        .unwrap();
    state
        .output
        .execute_playback(EnginePlaybackCommand::CueList {
            id: list_id,
            action: light_engine::CueListPlaybackAction::GoAt(clock.now()),
        })
        .unwrap();
    let completed = state
        .output
        .render_with_playback_events(
            &state.active_show.output_projection(),
            &state.playback.render_capability(),
            RenderOptions::default(),
        )
        .unwrap();
    state
        .output
        .render_frames_and_publish(&completed, VisualizationScope { show_id: None });
    let source = state.output.latest_visualization_frame().unwrap();
    let captured = &source.dynamics.as_ref().unwrap().cue_values;
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].authored_cue_id, cue_id);
    assert_eq!(captured[0].current_cue_id, cue_id);

    clock.advance_millis(1_000);
    state
        .output
        .execute_playback(EnginePlaybackCommand::CueList {
            id: list_id,
            action: light_engine::CueListPlaybackAction::Release,
        })
        .unwrap();
    assert!(state.output.active_cue_dynamic_values().is_empty());
    for _ in 0..2 {
        let snapshot = visualization_snapshot_for_session_content_from_resolved(
            &state,
            &session,
            false,
            true,
            false,
            Some(&source),
        )
        .unwrap();
        let stack = snapshot["dynamic_stack"].as_array().unwrap();
        assert_eq!(stack.len(), 1);
        assert_eq!(stack[0]["source"], format!("Cue {cue_id}"));
        assert_eq!(stack[0]["entry_type"], "fix_at");
        assert_eq!(stack[0]["changed_at_millis"], captured[0].changed_at_millis);
        assert_eq!(
            stack[0]["value"],
            serde_json::json!(AttributeValue::Normalized(0.4))
        );
    }
    let _ = std::fs::remove_dir_all(directory);
}

#[tokio::test]
async fn accepted_pending_and_activation_state_uses_its_captured_sample_time() {
    let clock = Arc::new(ManualClock::new(fixed_test_time()));
    let (state, directory) = test_state_with_clock(clock.clone());
    let session = session(&state).await;
    let fixture = FixtureId::new();
    let mut definition = command_test_dynamic(Uuid::new_v4(), 1);
    definition.speed = light_dynamics::DynamicSpeed::SpeedGroup {
        group: light_dynamics::SpeedGroup::A,
        beats_per_cycle: light_dynamics::Rational::ONE,
    };
    definition.default_activation = light_dynamics::ActivationPolicy::NextBoundary;
    definition.activation_boundary = light_dynamics::ActivationBoundary::Bar;
    let definition = Arc::new(definition);
    state
        .output
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![operational_fixture(fixture)].into(),
            dynamics: vec![definition.as_ref().clone()].into(),
            ..Default::default()
        })
        .unwrap();
    assert!(state.programming.programmers().apply_dynamic_values(
        session.id,
        &[light_programmer::DynamicProgrammerValueMutation::Set {
            fixture_id: fixture,
            attribute: AttributeKey::intensity(),
            value: light_dynamics::DynamicSemanticValue::DynamicOn {
                instance_link: Uuid::new_v4(),
                lane_id: definition.lanes[0].id,
                dynamic: light_dynamics::DynamicReference {
                    dynamic_id: Some(definition.id),
                    last_known_pool_number: 1,
                    embedded_fallback: light_dynamics::DynamicDefinitionSnapshot {
                        definition: Arc::clone(&definition),
                    },
                },
                overrides: light_dynamics::DynamicInstanceOverrides {
                    size: 1.0,
                    speed_multiplier: light_dynamics::Rational::ONE,
                    phase_offset_degrees: 0.0,
                },
                timing: light_dynamics::DynamicValueTiming {
                    delay_millis: Some(200),
                    fade_millis: Some(1_000),
                },
            },
        },],
        None
    ));
    let completed = state
        .output
        .render_with_playback_events(
            &state.active_show.output_projection(),
            &state.playback.render_capability(),
            RenderOptions::default(),
        )
        .unwrap();
    state
        .output
        .render_frames_and_publish(&completed, VisualizationScope { show_id: None });
    let source = state.output.latest_visualization_frame().unwrap();
    let captured = source.dynamics.as_ref().unwrap();
    assert_eq!(captured.runtime.instances.len(), 1);
    let boundary = captured
        .sample_boundary
        .expect("published Dynamic history is anchored");
    assert_eq!(
        boundary.scope(),
        light_dynamics::DynamicSampleScope::WholeRuntime
    );
    let instance = &captured.runtime.instances[0];
    let captured_millis = u64::try_from(source.sampled_at.timestamp_millis()).unwrap();
    assert_eq!(boundary.sampled_at_millis(), captured_millis);
    assert!(instance.pending_until_millis.unwrap() > captured_millis);
    let transition = instance.controller_transitions[0];
    let expected_mix =
        super::super::dynamics_http::runtime_transition_mix(transition, captured_millis);
    assert_eq!(expected_mix, 0.0);

    clock.advance_millis(10_000);
    let newer = state
        .output
        .render_with_playback_events(
            &state.active_show.output_projection(),
            &state.playback.render_capability(),
            RenderOptions::default(),
        )
        .unwrap();
    assert_ne!(
        newer.dynamics.as_ref().unwrap().sample_boundary,
        Some(boundary)
    );
    assert_eq!(
        source.dynamics.as_ref().unwrap().sample_boundary,
        Some(boundary)
    );
    assert_eq!(
        super::super::dynamics_http::runtime_transition_mix(
            transition,
            u64::try_from(clock.now().timestamp_millis()).unwrap()
        ),
        1.0
    );
    for _ in 0..2 {
        let snapshot = visualization_snapshot_for_session_content_from_resolved(
            &state,
            &session,
            false,
            true,
            false,
            Some(&source),
        )
        .unwrap();
        let stack = snapshot["dynamic_stack"].as_array().unwrap();
        assert_eq!(stack.len(), 1);
        assert_eq!(stack[0]["entry_type"], "dynamic");
        assert_eq!(stack[0]["pending"], true);
        assert_eq!(stack[0]["activation_mix"], serde_json::json!(expected_mix));
    }
    let _ = std::fs::remove_dir_all(directory);
}
