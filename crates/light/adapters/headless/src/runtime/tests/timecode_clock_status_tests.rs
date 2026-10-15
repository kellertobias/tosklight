use super::*;
use light_control::{
    FrameRate, SmpteTimecode, TimecodeRouter, TimecodeRouterConfig, TimecodeSourceSelection,
};

fn sample(source: &str, frames: u8) -> SmpteTimecode {
    SmpteTimecode {
        hours: 0,
        minutes: 0,
        seconds: 2,
        frames,
        rate: FrameRate::Fps25,
        source: source.into(),
        received_at: chrono::Utc::now(),
    }
}
fn configuration() -> TimecodeRouterConfig {
    TimecodeRouterConfig {
        selected_source: TimecodeSourceSelection::External {
            source: "artnet:127.0.0.1:2".into(),
        },
        desk_rate: FrameRate::FpsCustom(44),
        loss_timeout_millis: 0,
        ..Default::default()
    }
}

#[test]
fn clock_projection_coalesces_frames_but_not_loss_and_relock_edges() {
    let mut router = TimecodeRouter::default();
    router.configure(configuration());
    let mut publication = timecode_clock_status::ClockPublication::default();
    let now = Instant::now();
    let waiting = publication.event(&router, now).unwrap();
    assert_eq!(waiting.state, "waiting");
    assert!(router.ingest(sample("artnet:127.0.0.2:2", 13)).is_none());
    assert!(publication.event(&router, now).is_none());
    router.ingest(sample("artnet:127.0.0.1:2", 13));
    let locked = publication.event(&router, now).unwrap();
    assert_eq!(locked.current.as_deref(), Some("00:00:02:22"));
    assert_eq!(locked.state, "locked");
    assert_eq!(locked.source_rate.as_deref(), Some("25"));
    assert_eq!(locked.desk_rate, "44");
    assert!(locked.rate_warning.is_some());
    router.ingest(sample("artnet:127.0.0.1:2", 14));
    assert!(
        publication
            .event(&router, now + Duration::from_millis(1))
            .is_none()
    );
    let frame = publication
        .event(&router, now + Duration::from_millis(100))
        .unwrap();
    assert!(frame.revision > locked.revision);
    // Zero configured TTL makes the next real router poll a loss, without sleeping or changing its production clock.
    router.poll_loss();
    let lost = publication
        .event(&router, now + Duration::from_millis(101))
        .unwrap();
    assert_eq!(lost.state, "lost");
    assert!(lost.current.is_none());
    assert!(lost.active_source.is_none());
    router.ingest(sample("artnet:127.0.0.1:2", 13));
    let relocked = publication
        .event(&router, now + Duration::from_millis(102))
        .unwrap();
    assert_eq!(relocked.state, "locked");
    assert!(relocked.revision > lost.revision);
}

#[tokio::test]
async fn authenticated_clock_read_and_real_ingress_share_the_pushed_projection() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Clock status operator").await;
    let denied = app
        .clone()
        .oneshot(
            Request::get("/api/v2/timecode-clock")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
    state.output.configure_timecode(configuration());
    let read = |token: &str| {
        Request::get("/api/v2/timecode-clock")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap()
    };
    let response = app.clone().oneshot(read(&token)).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let waiting = json(response).await;
    assert_eq!(waiting["state"], "waiting");
    let lighting = state.output.snapshot();
    let before_events = state.events.latest_sequence();
    handle_control_event(
        &state,
        ControlEvent::Timecode(sample("artnet:127.0.0.1:2", 13)),
    );
    timecode_clock_status::publish(&state);
    let response = app.clone().oneshot(read(&token)).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let locked = json(response).await;
    let light_application::EventReplay::Events(events) = state
        .events
        .replay(before_events, &light_application::EventFilter::default())
    else {
        panic!("clock event retained");
    };
    assert_eq!(events.iter().filter(|event| matches!(&event.payload, light_application::ApplicationEvent::Desk(light_application::DeskEvent::TimecodeClockChanged { revision, current, active_source, .. }) if *revision == locked["revision"].as_u64().unwrap() && current.as_deref() == Some("00:00:02:22") && active_source.as_deref() == Some("artnet:127.0.0.1:2"))).count(), 1);
    assert_eq!(locked["current"], "00:00:02:22");
    assert_eq!(locked["active_source"], "artnet:127.0.0.1:2");
    assert_eq!(locked["source_rate"], "25");
    assert_eq!(locked["desk_rate"], "44");
    handle_control_event(
        &state,
        ControlEvent::Timecode(sample("artnet:127.0.0.2:2", 1)),
    );
    let refused = json(app.clone().oneshot(read(&token)).await.unwrap()).await;
    assert_eq!(refused, locked);
    assert!(
        Arc::ptr_eq(&lighting, &state.output.snapshot()),
        "clock observations do not reinstall the lighting engine"
    );
    let malformed = app
        .clone()
        .oneshot(
            Request::get("/api/v2/timecode-clock")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header("x-tosk-desk", "bad")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(malformed.status(), StatusCode::BAD_REQUEST);
    let foreign = app
        .oneshot(
            Request::get("/api/v2/timecode-clock")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header("x-tosk-show", Uuid::new_v4().to_string())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(foreign.status(), StatusCode::CONFLICT);
    let _ = std::fs::remove_dir_all(data_dir);
}
