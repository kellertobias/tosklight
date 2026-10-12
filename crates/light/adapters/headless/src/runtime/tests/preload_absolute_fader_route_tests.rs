use super::*;
use light_programmer::{
    NormalProgrammerValueMutation as Normal, PreloadProgrammerValueMutation as Pending,
};

#[tokio::test]
async fn preload_absolute_master_and_both_configured_faders_stay_live_without_changing_pending() {
    for form in ["master", "left", "right"] {
        let (state, data_dir) = test_state();
        let app = router(state.clone());
        let (token, _) = login(&app, "Operator").await;
        let session = session_for_token(&state, &token);
        open_playback_test_show(&app, &token).await;
        let fixture = install_playback_test_state(&state);
        let mut snapshot = (*state.output.snapshot()).clone();
        let definition = std::sync::Arc::make_mut(&mut snapshot.playback_pages)[0]
            .assignments
            .get_mut(&1)
            .unwrap();
        definition.footprint = light_playback::PlaybackFootprint::Wider {
            right_buttons: [light_playback::PlaybackButtonAction::None; 3],
            right_fader: light_playback::PlaybackFaderMode::Master,
        };
        state.output.replace_snapshot(snapshot).unwrap();
        set_pool_enabled(&state, 1, true);
        assert!(state.programming.programmers().apply_normal_values(
            session.id,
            &[Normal::SetFixture {
                fixture_id: fixture,
                attribute: light_core::AttributeKey::intensity(),
                value: light_core::AttributeValue::Normalized(0.2),
                timing: Default::default(),
            },]
        ));
        assert_preload_key(
            &app,
            &token,
            session.desk.id,
            "arm-live-fader",
            "preload_entered",
        )
        .await;
        assert!(state.programming.programmers().apply_preload_values(
            session.id,
            &[Pending::SetFixture {
                fixture_id: fixture,
                attribute: light_core::AttributeKey::intensity(),
                value: light_core::AttributeValue::Normalized(0.7),
                timing: Default::default(),
            },]
        ));
        let captured = post_action(
            &app,
            Some(&token),
            session.desk.id,
            action_request(
                "capture-go-before-fader",
                1,
                serde_json::json!({"type":"go","pressed":true}),
            ),
        )
        .await;
        assert_eq!(captured.status(), StatusCode::OK);
        assert_eq!(json(captured).await["outcome"]["status"], "captured");
        let programmer_before = state.programming.get(session.id).unwrap();
        assert!(programmer_before.blind);
        assert_eq!(programmer_before.values.len(), 1);
        assert_eq!(programmer_before.preload_pending.len(), 1);
        assert_eq!(programmer_before.preload_playback_pending.len(), 1);
        let queue_events_before = preload_queue_events(&state).len();
        for level in [0.25, 0.0, 1.0] {
            let action = absolute_action(form, level);
            let response = post_action(
                &app,
                Some(&token),
                session.desk.id,
                action_request(&format!("live-{form}-{level}"), 1, action),
            )
            .await;
            let status = response.status();
            let outcome = json(response).await;
            assert_eq!(status, StatusCode::OK, "{form}: {outcome}");
            assert_eq!(outcome["outcome"]["status"], "applied", "{outcome}");
            assert_eq!(outcome["projection"]["runtime"]["master"], level);
            assert_eq!(outcome["projection"]["runtime"]["enabled"], true);
            let after = state.programming.get(session.id).unwrap();
            assert_eq!(after.values, programmer_before.values);
            assert_eq!(after.preload_pending, programmer_before.preload_pending);
            assert_eq!(
                after.preload_playback_pending,
                programmer_before.preload_playback_pending
            );
            assert_eq!(after.blind, programmer_before.blind);
            assert_eq!(preload_queue_events(&state).len(), queue_events_before);
        }
        for invalid_level in [-0.1, 1.1] {
            let runtime_before = state.output.playback_runtime();
            let response = post_action(
                &app,
                Some(&token),
                session.desk.id,
                action_request(
                    &format!("invalid-{form}-{invalid_level}"),
                    1,
                    absolute_action(form, invalid_level),
                ),
            )
            .await;
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            assert_eq!(state.output.playback_runtime(), runtime_before);
            let after = state.programming.get(session.id).unwrap();
            assert_eq!(after.values, programmer_before.values);
            assert_eq!(after.preload_pending, programmer_before.preload_pending);
            assert_eq!(
                after.preload_playback_pending,
                programmer_before.preload_playback_pending
            );
        }
        std::fs::remove_dir_all(data_dir).unwrap();
    }
}

fn absolute_action(form: &str, level: f64) -> serde_json::Value {
    if form == "master" {
        serde_json::json!({"type":"master_absolute","value":level})
    } else {
        serde_json::json!({"type":"configured_fader_absolute", "number":if form == "left" {1} else {2}, "level":level})
    }
}
