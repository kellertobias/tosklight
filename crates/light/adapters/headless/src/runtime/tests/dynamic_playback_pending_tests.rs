use super::dynamic_playback_settled_feedback_tests::{desk, render};
use super::*;
use light_application::{
    DynamicPlaybackRuntimeState, PlaybackRuntimeIdentity, PlaybackTargetProjection,
};

fn projections(state: &AppState) -> Vec<light_application::PlaybackRuntimeProjection> {
    crate::runtime::playback_service::read_runtime_projections(
        state,
        &light_application::ActionContext::system(
            Uuid::nil(),
            light_application::ActionSource::Scheduler,
        ),
        &[
            PlaybackRuntimeIdentity::Playback(light_playback::PlaybackAddress::new(1, 1).unwrap()),
            PlaybackRuntimeIdentity::Playback(light_playback::PlaybackAddress::new(1, 2).unwrap()),
            PlaybackRuntimeIdentity::Playback(
                light_playback::PlaybackAddress::new(1, 101).unwrap(),
            ),
        ],
    )
    .unwrap()
}

fn assert_state(state: &AppState, expected: DynamicPlaybackRuntimeState) {
    for projection in projections(state) {
        let PlaybackTargetProjection::Dynamic {
            runtime: Some(runtime),
            ..
        } = projection.target
        else {
            panic!("expected Dynamic projection");
        };
        assert_eq!(runtime.state, expected, "alias {:?}", projection.identity);
        if expected == DynamicPlaybackRuntimeState::Pending {
            assert!(
                runtime.warning.is_none(),
                "unattempted start is not an error: {:?}",
                runtime.warning
            );
        }
    }
}

async fn on(app: &Router, token: &str, session: &Session) {
    let response=post_action(app,Some(token),session.desk.id,serde_json::json!({
        "address":{"kind":"playback","page":1,"position":2},
        "control_source":{"surface_id":"main","control_id":1},"action":{"type":"on","pressed":true}
    })).await;
    let status = response.status();
    let outcome = json(response).await;
    assert_eq!(status, StatusCode::OK, "{outcome}");
    assert_eq!(
        outcome["projection"]["runtime"]["state"], "pending",
        "{outcome}"
    );
    assert!(outcome["projection"]["runtime"]["warning"].is_null());
}

#[tokio::test]
async fn unattempted_on_is_pending_and_first_committed_output_settles_all_aliases() {
    let (state, dir, app, token, session, _, _) = desk().await;
    on(&app, &token, &session).await;
    assert_state(&state, DynamicPlaybackRuntimeState::Pending);
    render(&state);
    assert_state(&state, DynamicPlaybackRuntimeState::Active);
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn legacy_active_runtime_without_transient_state_waits_for_real_reconciliation() {
    let (state, dir, app, token, session, _, _) = desk().await;
    on(&app, &token, &session).await;
    let saved =
        serde_json::to_value(state.output.active_dynamic_playbacks_for_persistence()).unwrap();
    // This remains the old persisted shape: no attempt/outcome or activation-incarnation data.
    let row = saved[0].as_object().unwrap();
    assert!(!row.contains_key("start_state"));
    let legacy: Vec<light_playback::ActiveDynamicPlayback> = serde_json::from_value(saved).unwrap();
    state
        .output
        .execute_playback(light_engine::EnginePlaybackCommand::RestoreActiveDynamics(
            legacy,
        ))
        .unwrap();
    assert_state(&state, DynamicPlaybackRuntimeState::Pending);
    render(&state);
    assert_state(&state, DynamicPlaybackRuntimeState::Active);
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn same_clock_reactivation_rejects_old_receipt_and_accepted_disappearance_is_failed() {
    use light_engine::{EnginePlaybackCommand as Command, PlaybackAction as Action};
    use light_playback::DynamicPlaybackStartOutcome as Outcome;
    let (state, dir, app, token, session, id, _) = desk().await;
    on(&app, &token, &session).await;
    let engine = state.output.engine();
    let previous = engine.active_dynamic_playbacks()[0].clone();
    engine
        .execute_playback(Command::Playback {
            address: light_playback::PlaybackAddress::new(1, 2).unwrap(),
            activation_origin: None,
            control_source: light_playback::PlaybackControlSource {
                surface_id: "main".into(),
                control_id: 1,
            },
            action: Action::Off,
        })
        .unwrap();
    engine
        .execute_playback(Command::Playback {
            address: light_playback::PlaybackAddress::new(1, 1).unwrap(),
            activation_origin: None,
            control_source: light_playback::PlaybackControlSource {
                surface_id: "main".into(),
                control_id: 1,
            },
            action: Action::On,
        })
        .unwrap();
    let current = engine.active_dynamic_playbacks()[0].clone();
    assert_eq!(
        previous.activated_at, current.activated_at,
        "test must exercise same clock time"
    );
    assert_ne!(
        previous.start_state.incarnation,
        current.start_state.incarnation
    );
    assert!(!engine.acknowledge_dynamic_playback_start(
        id,
        previous.start_state.incarnation,
        Outcome::Rejected("stale prior activation".into())
    ));
    assert_state(&state, DynamicPlaybackRuntimeState::Pending);
    assert!(engine.acknowledge_dynamic_playback_start(
        id,
        current.start_state.incarnation,
        Outcome::Accepted
    ));
    assert_state(&state, DynamicPlaybackRuntimeState::Failed);
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn intentional_empty_scope_waits_and_recovers_without_reactivation() {
    let (state, dir, app, token, session, _, fixture) = desk().await;
    let mut snapshot = (*state.output.engine().snapshot()).clone();
    let mut definition = snapshot.dynamics[0].clone();
    definition.target_binding = light_dynamics::DynamicTargetBinding::LiveGroup {
        group_id: "receipt-empty".into(),
    };
    snapshot.dynamics = vec![definition].into();
    let group = light_programmer::GroupDefinition {
        id: "receipt-empty".into(),
        name: "Stored empty".into(),
        source: Some(light_programmer::GroupFixtureSource::Explicit {
            fixture_ids: vec![],
        }),
        ..Default::default()
    };
    snapshot.groups = vec![group.clone()].into();
    state.output.replace_snapshot(snapshot).unwrap();
    on(&app, &token, &session).await;
    let incarnation = state.output.engine().active_dynamic_playbacks()[0]
        .start_state
        .incarnation;
    render(&state);
    for projection in projections(&state) {
        let PlaybackTargetProjection::Dynamic {
            runtime: Some(runtime),
            ..
        } = projection.target
        else {
            panic!();
        };
        assert_eq!(runtime.state, DynamicPlaybackRuntimeState::Pending);
        assert!(
            runtime
                .warning
                .as_deref()
                .is_some_and(|warning| warning.contains("Target group receipt-empty is empty"))
        );
    }
    let mut snapshot = (*state.output.engine().snapshot()).clone();
    let mut populated = group;
    populated.source = Some(light_programmer::GroupFixtureSource::Explicit {
        fixture_ids: vec![fixture],
    });
    snapshot.groups = vec![populated.clone()].into();
    state.output.replace_snapshot(snapshot).unwrap();
    render(&state);
    assert_state(&state, DynamicPlaybackRuntimeState::Active);
    let mut missing = (*state.output.engine().snapshot()).clone();
    missing.groups = vec![].into();
    state.output.replace_snapshot(missing).unwrap();
    render(&state);
    assert_state(&state, DynamicPlaybackRuntimeState::Failed);
    let mut repaired = (*state.output.engine().snapshot()).clone();
    let mut empty = populated.clone();
    empty.source = Some(light_programmer::GroupFixtureSource::Explicit {
        fixture_ids: vec![],
    });
    repaired.groups = vec![empty].into();
    state.output.replace_snapshot(repaired).unwrap();
    render(&state);
    for projection in projections(&state) {
        let PlaybackTargetProjection::Dynamic {
            runtime: Some(runtime),
            ..
        } = projection.target
        else {
            panic!();
        };
        assert_eq!(runtime.state, DynamicPlaybackRuntimeState::Pending);
    }
    let mut repaired = (*state.output.engine().snapshot()).clone();
    repaired.groups = vec![populated].into();
    state.output.replace_snapshot(repaired).unwrap();
    render(&state);
    assert_state(&state, DynamicPlaybackRuntimeState::Active);
    assert_eq!(
        state.output.engine().active_dynamic_playbacks()[0]
            .start_state
            .incarnation,
        incarnation
    );
    std::fs::remove_dir_all(dir).unwrap();
}
