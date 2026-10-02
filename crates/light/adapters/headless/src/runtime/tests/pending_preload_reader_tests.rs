//! TL-594 track B: Preload readers present the accepted Pending publication, with its own
//! identity, behind the family-adapter gate; the legacy path is unchanged with the gate off.
use super::*;
use crate::runtime::output_scheduler::{
    PendingAttemptTicket, PendingEpisodeIdentity, PendingEpisodeStatus, PendingLaneValues,
    PendingNativeReadout,
};
use crate::runtime::visualization_frame::RenderedSemanticFrame;
use light_core::programming::PROGRAMMING_CONTRACT_VERSION;
use light_core::{AttributeKey, AttributeValue, FixtureId};
use light_wire::v2::output_control::{
    OutputDmxSnapshot, OutputFrameIdentity, OutputNativeLane, OutputPreloadState,
};

const PATIENCE: Duration = Duration::from_secs(30);

async fn get_json(app: &Router, token: &str, uri: &str) -> serde_json::Value {
    let response = app
        .clone()
        .oneshot(
            Request::get(uri)
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK, "{uri}");
    json(response).await
}

async fn read_dmx(app: &Router, token: &str) -> (OutputDmxSnapshot, serde_json::Value) {
    let raw = get_json(app, token, "/api/v2/output/dmx?include_preload=true").await;
    (serde_json::from_value(raw.clone()).unwrap(), raw)
}

async fn read_visualization(app: &Router, token: &str) -> serde_json::Value {
    get_json(app, token, "/api/v2/output/visualization?preload=true").await
}

fn publish_live(state: &AppState) {
    let rendered = RenderedSemanticFrame::untraced(
        state.output.render(Default::default()).unwrap(),
        Default::default(),
    );
    state.output.render_frames_and_publish(
        &rendered,
        light_wire::v2::visualization::VisualizationScope {
            show_id: state.active_show.current().map(|show| show.id.0),
        },
    );
}

/// A desk with an active show, a desk Programmer and one published Live frame.
async fn desk(opted_in: bool) -> (AppState, PathBuf, Router, String) {
    let programmers = ProgrammerRegistry::default();
    let (state, data_dir) = if opted_in {
        test_state_with_family_adapters(programmers, None, PROGRAMMING_CONTRACT_VERSION)
    } else {
        test_state()
    };
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Pending readers").await;
    let show_id = Uuid::parse_str(show["id"].as_str().unwrap()).unwrap();
    let response = app
        .clone()
        .oneshot(open_show_request(&token, show_id))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(state.active_show.current().is_some());
    publish_live(&state);
    (state, data_dir, app, token)
}

/// A Pending lane and values no Live frame or fresh projection could produce.
fn sentinel_readout(
    identity: PendingEpisodeIdentity,
    ticket: u64,
    sentinel: FixtureId,
) -> PendingNativeReadout {
    let frame = OutputFrameIdentity {
        generation: 4_242,
        sequence: ticket,
        sampled_at: format!("2026-10-02T10:00:{:02}.000+00:00", ticket % 60),
        tracking: None,
    };
    let mut values = light_engine::ResolvedValues::default();
    values.insert(
        (sentinel, AttributeKey("tl594.sentinel".into())),
        AttributeValue::Normalized(0.125),
    );
    let mut profile = light_engine::ResolvedValues::default();
    profile.insert(
        (sentinel, AttributeKey("tl594.profile".into())),
        AttributeValue::Normalized(0.75),
    );
    PendingNativeReadout::for_tests(
        identity,
        PendingAttemptTicket::new(ticket),
        OutputNativeLane {
            show_id: Some(identity.show_id.0),
            revision: 77,
            frame: Some(frame),
            instances: Vec::new(),
            points: Vec::new(),
        },
        PendingLaneValues {
            values: Arc::new(values),
            profile: Arc::new(profile),
            grand_master: 0.5,
            blackout: false,
        },
    )
}

fn has_sentinel(entries: &serde_json::Value, sentinel: FixtureId) -> bool {
    entries
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["fixture_id"] == sentinel.0.to_string())
}

#[tokio::test]
async fn gated_preload_readers_present_the_published_pending_identity_never_a_fresh_projection() {
    let (state, data_dir, app, token) = desk(true).await;
    let show = state.active_show.current().unwrap().id;
    let programmer = state
        .programming
        .programmers()
        .programmer_id()
        .expect("the desk Programmer");
    let pending = state.programming.pending_episodes();
    assert!(pending.start_if_gated(&state));
    let executor = pending.executor().unwrap();
    assert!(executor.wait_for(PATIENCE, |progress, status| {
        progress.steps > 0 && *status == PendingEpisodeStatus::Idle
    }));

    let identity = PendingEpisodeIdentity {
        show_id: show,
        activation: Uuid::new_v4(),
        programmer,
        episode: Uuid::new_v4(),
    };
    let lease = executor.begin_test_episode(identity);
    let sentinel = FixtureId::new();
    for ticket in [7, 8] {
        let published =
            executor.publish_test_readout(&lease, sentinel_readout(identity, ticket, sentinel));
        let gate_ticket = executor.latest().unwrap().ticket().sequence();
        assert_eq!(gate_ticket, ticket);

        // Stage: the published lane byte for byte, stamped by the gate's ticket and episode.
        let (dmx, _) = read_dmx(&app, &token).await;
        let lane = dmx.preload.clone().expect("the published Pending lane");
        assert_eq!(&lane, published.lane());
        assert_eq!(lane.frame.as_ref().unwrap().sequence, gate_ticket);
        let status = dmx.preload_status.clone().unwrap();
        assert_eq!(status.state, OutputPreloadState::Published);
        assert_eq!(status.episode, Some(identity.episode));
        assert_ne!(lane.frame, dmx.frame, "never the Live stamp");
        assert!(dmx.native.is_some(), "Live is still read beside it");

        // Visualization snapshot: the publication's values and identity, not a projection.
        let snapshot = read_visualization(&app, &token).await;
        assert_eq!(snapshot["pending"]["state"], "published");
        assert_eq!(
            snapshot["pending"]["episode"],
            identity.episode.to_string().as_str()
        );
        assert_eq!(
            snapshot["pending"]["frame"]["sequence"].as_u64(),
            Some(gate_ticket)
        );
        assert_eq!(snapshot["revision"], 77);
        assert!(has_sentinel(&snapshot["values"], sentinel));
        assert!(has_sentinel(&snapshot["profile_output_values"], sentinel));
        assert_eq!(snapshot["values"].as_array().unwrap().len(), 1);
        assert!(snapshot.get("source_frame").is_none(), "no Live sequence");

        // Stream lane: the same content function the WebSocket Preload lane projects.
        let session = authenticate_token(&state, &token).unwrap();
        let source = state.output.latest_visualization_frame().unwrap();
        let lane: light_wire::v2::visualization::VisualizationLaneSnapshot = serde_json::from_value(
            crate::runtime::operator_api::visualization_snapshot_for_session_content_from_resolved(
                &state,
                &session,
                true,
                false,
                true,
                Some(&source),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(lane.preload);
        assert_eq!(lane.revision, 77);
        assert_eq!(lane.values.len(), 1);
        assert_eq!(lane.values[0].fixture_id, sentinel.0);
    }
    drop(executor);
    pending.stop(&state);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn gated_preload_readers_report_a_passive_status_and_never_fall_back_to_live() {
    let (state, data_dir, app, token) = desk(true).await;
    // Programmer Preload values a legacy projection would turn into a Preload lane.
    let session = authenticate_token(&state, &token).unwrap();
    assert!(state.programming.arm_preload(session.id, true));
    state.programming.set(
        session.id,
        FixtureId::new(),
        AttributeKey::intensity(),
        AttributeValue::Normalized(1.),
    );

    // Gated, worker not started: nothing published yet.
    let (dmx, _) = read_dmx(&app, &token).await;
    assert!(dmx.preload.is_none(), "no Live or fresh Preload lane");
    assert_eq!(
        dmx.preload_status.unwrap().state,
        OutputPreloadState::NotYetAvailable
    );
    assert!(dmx.native.is_some());
    let snapshot = read_visualization(&app, &token).await;
    assert_eq!(snapshot["pending"]["state"], "not_yet_available");
    assert!(snapshot["pending"]["frame"].is_null());
    assert_eq!(snapshot["values"], serde_json::json!([]));
    assert_eq!(snapshot["profile_output_values"], serde_json::json!([]));
    assert!(snapshot.get("source_frame").is_none());

    // An episode that has accepted nothing yet, or a readout of another show, is not presented.
    // (Preload is released first, so the idle worker never begins an episode of its own.)
    assert!(state.programming.release_preload(session.id));
    let pending = state.programming.pending_episodes();
    assert!(pending.start_if_gated(&state));
    let executor = pending.executor().unwrap();
    assert!(executor.wait_for(PATIENCE, |progress, status| {
        progress.steps > 0 && *status == PendingEpisodeStatus::Idle
    }));
    let (dmx, _) = read_dmx(&app, &token).await;
    assert!(dmx.preload.is_none());
    assert_eq!(dmx.preload_status.unwrap().state, OutputPreloadState::Idle);
    let identity = PendingEpisodeIdentity {
        show_id: light_core::ShowId(Uuid::new_v4()),
        activation: Uuid::new_v4(),
        programmer: state.programming.programmers().programmer_id().unwrap(),
        episode: Uuid::new_v4(),
    };
    let lease = executor.begin_test_episode(identity);
    let (dmx, _) = read_dmx(&app, &token).await;
    assert!(dmx.preload.is_none());
    assert_ne!(
        dmx.preload_status.unwrap().state,
        OutputPreloadState::Published
    );
    executor.publish_test_readout(&lease, sentinel_readout(identity, 1, FixtureId::new()));
    let (dmx, _) = read_dmx(&app, &token).await;
    assert!(
        dmx.preload.is_none(),
        "another show's Pending frame is not presented"
    );
    assert_eq!(
        dmx.preload_status.unwrap().state,
        OutputPreloadState::NotYetAvailable
    );
    drop(executor);
    pending.stop(&state);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn with_the_gate_off_the_preload_readers_keep_the_legacy_payloads() {
    let (state, data_dir, app, token) = desk(false).await;
    assert!(!crate::runtime::output_scheduler::PendingEpisodeResource::gated(&state));
    let (dmx, raw) = read_dmx(&app, &token).await;
    let keys = raw
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        keys,
        [
            "frame",
            "native",
            "native_protocol",
            "overrides",
            "points",
            "preload",
            "revision",
            "universes"
        ]
        .into_iter()
        .collect(),
        "no Pending status on the legacy payload"
    );
    let lane = dmx.preload.clone().expect("the legacy fresh Preload lane");
    assert!(lane.frame.is_none(), "the legacy lane is unstamped");
    assert_eq!(
        serde_json::to_vec(&raw).unwrap(),
        serde_json::to_vec(
            &serde_json::to_value(&OutputDmxSnapshot {
                preload: Some(lane),
                ..dmx.clone()
            })
            .unwrap()
        )
        .unwrap(),
        "the typed legacy snapshot serializes to the same bytes"
    );
    let snapshot = read_visualization(&app, &token).await;
    assert!(snapshot.get("pending").is_none());
    assert_eq!(snapshot["preload"], true);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn the_color_report_route_is_legacy_with_the_gate_off_and_accepted_frame_when_on() {
    for opted_in in [false, true] {
        let (state, data_dir, app, token) = desk(opted_in).await;
        let report = get_json(&app, &token, "/api/v2/color-intent/report").await;
        if opted_in {
            // The test Live frame above was published without the family path accepting it.
            assert_eq!(report["accepted_frame"]["state"], "not_yet_available");
            assert_eq!(report["heads"], serde_json::json!([]));
        } else {
            assert!(report.get("accepted_frame").is_none(), "legacy payload");
            assert_eq!(
                report["heads"].as_array().unwrap().len(),
                state
                    .output
                    .engine()
                    .color_intent_report(None)
                    .unwrap()
                    .len()
            );
        }
        let _ = std::fs::remove_dir_all(data_dir);
    }
}
