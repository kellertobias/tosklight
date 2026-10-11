//! Committed Dynamic output must replace pre-tick feedback for every assignment alias.
use super::*;
use light_application::{DynamicPlaybackRuntimeState, PlaybackRuntimeIdentity};
use light_core::FixtureId;

pub(super) async fn desk() -> (AppState, PathBuf, Router, String, Session, Uuid, FixtureId) {
    let (state, dir) = test_state_with_clock(Arc::new(ManualClock::new(
        chrono::DateTime::from_timestamp_millis(1_000_000).unwrap(),
    )));
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    open_playback_test_show(&app, &token).await;
    let session = session_for_token(&state, &token);
    let fixture = FixtureId::new();
    let mut dynamic = command_test_dynamic(Uuid::new_v4(), 1);
    dynamic.target_binding = light_dynamics::DynamicTargetBinding::FrozenTargets {
        targets: vec![fixture],
    };
    let id = dynamic.id;
    let target = light_playback::PlaybackTarget::Dynamic {
        assignment: light_playback::DynamicPlaybackAssignment {
            dynamic: light_dynamics::DynamicReference {
                dynamic_id: Some(id),
                last_known_pool_number: 1,
                embedded_fallback: light_dynamics::DynamicDefinitionSnapshot {
                    definition: Arc::new(dynamic.clone()),
                },
            },
            revision: 1,
            target_scope: None,
            fader_mode: light_playback::DynamicPlaybackFaderMode::SizeAndMaster,
            priority: 0,
            activation_override: None,
            resume_policy: light_playback::DynamicPlaybackResumePolicy::FollowDynamic,
            local_speed_multiplier: light_dynamics::Rational::ONE,
            learned_duration_millis: None,
            crossfade_non_intensity: false,
            auto_off_at_zero: false,
            auto_off_flash_release: false,
            auto_off_full_control: true,
        },
    };
    state
        .output
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![operational_fixture(fixture)].into(),
            dynamics: vec![dynamic].into(),
            playback_pages: vec![light_playback::PlaybackPage {
                number: 1,
                name: "Aliases".into(),
                assignments: HashMap::from([
                    (1, playback_test_definition(1, target.clone())),
                    (2, playback_test_definition(2, target.clone())),
                    (101, playback_test_definition(101, target)),
                ]),
            }]
            .into(),
            ..Default::default()
        })
        .unwrap();
    (state, dir, app, token, session, id, fixture)
}

pub(super) fn render(state: &AppState) {
    state
        .output
        .render_with_playback_events(
            &state.active_show.output_projection(),
            &state.playback.render_capability(),
            state.output.render_options(),
        )
        .unwrap();
}

fn assert_alias_feedback(state: &AppState, cursor: u64, expected: DynamicPlaybackRuntimeState) {
    let changes = playback_runtime_events(state, cursor);
    for identity in [
        PlaybackRuntimeIdentity::Playback(light_playback::PlaybackAddress::new(1, 1).unwrap()),
        PlaybackRuntimeIdentity::Playback(light_playback::PlaybackAddress::new(1, 2).unwrap()),
        PlaybackRuntimeIdentity::Playback(light_playback::PlaybackAddress::new(1, 101).unwrap()),
    ] {
        let projection = changes
            .iter()
            .rev()
            .find_map(|event| {
                let light_application::ApplicationEvent::Playback(
                    light_application::PlaybackEvent::RuntimeChanged(change),
                ) = &event.payload
                else {
                    return None;
                };
                (change.projection.identity == identity).then_some(&change.projection)
            })
            .unwrap_or_else(|| panic!("missing settled feedback for {identity:?}"));
        let light_application::PlaybackTargetProjection::Dynamic {
            runtime: Some(runtime),
            ..
        } = &projection.target
        else {
            panic!("expected Dynamic feedback");
        };
        assert_eq!(runtime.state, expected, "settled alias {identity:?}");
    }
}

#[tokio::test]
async fn committed_dynamic_start_replaces_pre_tick_feedback_for_physical_and_virtual_aliases() {
    let (state, dir, app, token, session, _, _) = desk().await;
    let response = post_action(
        &app,
        Some(&token),
        session.desk.id,
        serde_json::json!({
            "address":{"kind":"playback","page":1,"position":2},
            "control_source":{"surface_id":"main","control_id":1}, "action":{"type":"on","pressed":true}
        }),
    )
    .await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "{}",
        json(response).await
    );
    let cursor = state.events.latest_sequence();
    render(&state);
    assert_alias_feedback(&state, cursor, DynamicPlaybackRuntimeState::Active);
    let cursor = state.events.latest_sequence();
    render(&state);
    assert!(
        playback_runtime_events(&state, cursor).is_empty(),
        "unchanged frame must not repeatedly publish runtime feedback"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn full_control_auto_off_replaces_feedback_for_every_shared_dynamic_alias() {
    let (state, dir, app, token, session, id, fixture) = desk().await;
    state
        .dynamics
        .start(
            &operator_action_context(&session, light_application::ActionSource::Http),
            light_application::DynamicStartCommand {
                dynamic_id: id,
                targets: vec![fixture],
                overrides: light_dynamics::DynamicInstanceOverrides {
                    size: 1.0,
                    speed_multiplier: light_dynamics::Rational::ONE,
                    phase_offset_degrees: 0.0,
                },
                timing: Default::default(),
                undo_group: None,
            },
            &crate::runtime::dynamics_adapter::ServerDynamicsPorts {
                state: &state,
                session: &session,
            },
        )
        .unwrap();
    render(&state);
    let response = post_action(
        &app,
        Some(&token),
        session.desk.id,
        serde_json::json!({
            "address":{"kind":"playback","page":1,"position":2},
            "control_source":{"surface_id":"main","control_id":1}, "action":{"type":"on","pressed":true}
        }),
    )
    .await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "{}",
        json(response).await
    );
    let cursor = state.events.latest_sequence();
    render(&state);
    assert!(
        !state
            .output
            .active_dynamic_playback(light_playback::PlaybackAddress::new(1, 2).unwrap())
            .unwrap()
            .enabled,
        "persistent Programmer must auto-off the fully controlled direct Playback"
    );
    assert_alias_feedback(&state, cursor, DynamicPlaybackRuntimeState::Off);
    std::fs::remove_dir_all(dir).unwrap();
}
