use super::*;
use light_output::OutputRoute;
use std::{
    sync::atomic::{AtomicUsize, Ordering as AtomicOrdering},
    sync::mpsc,
    thread,
};

fn rendered(revision: u64) -> RenderedSemanticFrame {
    RenderedSemanticFrame::untraced(
        RenderResult {
            tracking: Arc::default(),
            source_snapshot: Arc::new(light_engine::EngineSnapshot {
                revision,
                ..Default::default()
            }),
            generation: revision,
            sampled_at: chrono::Utc::now(),
            points: Arc::new(light_engine::Pooled::default()),
            mounts: Arc::new(light_engine::Pooled::default()),
            physical: Arc::new(light_engine::Pooled::default()),
            universes: light_engine::Pooled::default(),
            resolved_values: light_engine::FrameValues::empty(),
            profile_visualization_values: Arc::new(light_engine::Pooled::default()),
            patched_slots: light_engine::Pooled::default(),
            revision,
            routes: Arc::<[OutputRoute]>::from([]),
            automatic_playback_transitions: Vec::new(),
        },
        RenderOptions::default(),
    )
}

fn scope(show_id: Uuid) -> VisualizationScope {
    VisualizationScope {
        show_id: Some(show_id),
    }
}

fn snapshot(
    scope: VisualizationScope,
    dynamic_stack: Vec<light_wire::v2::visualization::VisualizationDynamicStackEntry>,
) -> VisualizationLaneSnapshot {
    VisualizationLaneSnapshot {
        scope,
        revision: 10,
        generated_at: "2026-07-27T00:00:00Z".into(),
        grand_master: 1.0,
        blackout: false,
        preload: false,
        values: Vec::new(),
        dynamic_stack,
        profile_output_values: Vec::new(),
    }
}

fn dynamic_stack_entry() -> light_wire::v2::visualization::VisualizationDynamicStackEntry {
    serde_json::from_value(serde_json::json!({
        "fixture_id": Uuid::new_v4(),
        "attribute": "intensity",
        "entry_type": "dynamic",
        "priority": 0,
        "changed_at_millis": 1,
        "source": "Dynamic",
        "dynamic_id": null,
        "pool_number": 1,
        "name": "Pulse",
        "runtime_instance_id": null,
        "controller_id": null,
        "lane_id": null,
        "size": 1.0,
        "activation_mix": 1.0,
        "paused": false,
        "hidden": false,
        "pending": false,
        "winning": true,
        "value": null,
        "resolved_value": null
    }))
    .expect("test Dynamic stack entry is valid")
}

#[test]
fn retains_only_the_latest_complete_frame() {
    let hub = VisualizationFrameHub::default();
    let scope = scope(Uuid::new_v4());
    hub.publish(&rendered(10), scope, None);
    hub.publish(&rendered(20), scope, None);
    hub.publish(&rendered(30), scope, None);

    let latest = hub.latest().expect("a frame was published");
    assert_eq!(latest.sequence, 3);
    assert_eq!(latest.show_revision, 30);
    assert_eq!(latest.scope, scope);
}

#[test]
fn source_sidecar_is_shared_with_values_and_released_with_the_last_frame_reader() {
    let hub = VisualizationFrameHub::default();
    let scope = scope(Uuid::new_v4());
    let sources = Arc::new(FrameDynamicSources {
        sample_boundary: None,
        runtime: Default::default(),
        samples: Vec::new(),
        origins: Arc::default(),
        programmer_values: Arc::default(),
        cue_values: Arc::from([]),
        ordinary: None,
        change_lead_start: None,
    });
    let weak = Arc::downgrade(&sources);
    let mut first = rendered(10);
    first.dynamics = Some(sources);
    hub.publish(&first, scope, None);
    let retained = hub.latest().unwrap();
    assert!(Arc::ptr_eq(&first.rendered.mounts, &retained.mounts));
    assert!(Arc::ptr_eq(
        first.dynamics.as_ref().unwrap(),
        retained.dynamics.as_ref().unwrap(),
    ));
    drop(first);
    hub.publish(&rendered(20), scope, None);
    let latest = hub.latest().unwrap();
    assert_eq!(latest.sequence, 2);
    assert!(
        latest.dynamics.is_none(),
        "untraced frames must not inherit old sources"
    );
    assert!(
        weak.upgrade().is_some(),
        "an existing reader retains the old source pair"
    );
    assert_eq!(retained.show_revision, 10);
    drop(retained);
    assert!(
        weak.upgrade().is_none(),
        "the hub must not accumulate source history"
    );
}

#[test]
fn shares_one_projection_for_the_same_lane_and_source_frame() {
    let hub = VisualizationFrameHub::default();
    let scope = scope(Uuid::new_v4());
    hub.publish(&rendered(10), scope, None);
    let source = hub.latest().unwrap();
    let builds = AtomicUsize::new(0);
    let build = |_| {
        builds.fetch_add(1, AtomicOrdering::Relaxed);
        Ok(VisualizationLaneSnapshot {
            scope,
            revision: 10,
            generated_at: "2026-07-27T00:00:00Z".into(),
            grand_master: 1.0,
            blackout: false,
            preload: false,
            values: Vec::new(),
            dynamic_stack: Vec::new(),
            profile_output_values: Vec::new(),
        })
    };

    let first = hub
        .projection(
            VisualizationProjectionKey::Normal {
                include_dynamic_stack: false,
                complete_values: false,
            },
            &source,
            build,
        )
        .unwrap();
    let second = hub
        .projection(
            VisualizationProjectionKey::Normal {
                include_dynamic_stack: false,
                complete_values: false,
            },
            &source,
            build,
        )
        .unwrap();

    assert!(Arc::ptr_eq(&first, &second));
    assert!(first.previous_source_sequence.is_none());
    assert_eq!(builds.load(AtomicOrdering::Relaxed), 1);
    assert_eq!(hub.metrics().projections, 1);
}

#[test]
fn dynamic_stack_refresh_is_capped_at_fixture_sheet_cadence() {
    let hub = VisualizationFrameHub::default();
    let scope = scope(Uuid::new_v4());
    let key = VisualizationProjectionKey::Normal {
        include_dynamic_stack: true,
        complete_values: false,
    };
    hub.publish(&rendered(10), scope, None);
    let first_source = hub.latest().unwrap();
    hub.projection(key, &first_source, |refresh| {
        assert!(refresh);
        Ok(snapshot(scope, vec![dynamic_stack_entry()]))
    })
    .unwrap();

    hub.publish(&rendered(10), scope, None);
    let second_source = hub.latest().unwrap();
    let second = hub
        .projection(key, &second_source, |refresh| {
            assert!(!refresh);
            Ok(snapshot(scope, Vec::new()))
        })
        .unwrap();
    assert_eq!(second.snapshot.dynamic_stack.len(), 1);
    assert!(second.delta.dynamic_stack.is_none());

    std::thread::sleep(DYNAMIC_STACK_PUBLICATION_INTERVAL);
    hub.publish(&rendered(10), scope, None);
    let third_source = hub.latest().unwrap();
    let third = hub
        .projection(key, &third_source, |refresh| {
            assert!(refresh);
            Ok(snapshot(scope, Vec::new()))
        })
        .unwrap();
    assert_eq!(third.delta.dynamic_stack, Some(Vec::new()));
}

#[test]
fn sampler_publishes_one_shared_source_only_while_subscribed() {
    let hub = VisualizationFrameHub::default();
    let scope = scope(Uuid::new_v4());
    hub.publish(&rendered(10), scope, None);
    hub.sample_latest();
    assert!(hub.sampled().is_none());

    hub.change_subscribers(VisualizationLane::Normal, 1);
    hub.sample_latest();
    assert_eq!(hub.sampled().unwrap().sequence, 1);

    hub.publish(&rendered(20), scope, None);
    assert_eq!(hub.sampled().unwrap().sequence, 1);
    hub.sample_latest();
    assert_eq!(hub.sampled().unwrap().sequence, 2);
}

#[tokio::test(start_paused = true)]
async fn sampler_parks_without_subscribers_and_resumes_on_first_claim() {
    let hub = Arc::new(VisualizationFrameHub::default());
    let cancellation = CancellationToken::new();
    let sampler = tokio::spawn(Arc::clone(&hub).run_sampler(cancellation.clone()));
    let scope = scope(Uuid::new_v4());
    hub.publish(&rendered(10), scope, None);
    tokio::time::advance(Duration::from_secs(1)).await;
    assert!(hub.sampled().is_none());

    hub.change_subscribers(VisualizationLane::Normal, 1);
    tokio::task::yield_now().await;
    assert_eq!(hub.sampled().unwrap().sequence, 1);

    hub.change_subscribers(VisualizationLane::Normal, -1);
    hub.publish(&rendered(20), scope, None);
    tokio::time::advance(Duration::from_secs(1)).await;
    assert_eq!(hub.sampled().unwrap().sequence, 1);

    cancellation.cancel();
    sampler.await.unwrap().unwrap();
}

#[tokio::test(start_paused = true)]
async fn sampler_notifies_waiters_on_each_new_shared_sample() {
    let hub = Arc::new(VisualizationFrameHub::default());
    let cancellation = CancellationToken::new();
    let sampler = tokio::spawn(Arc::clone(&hub).run_sampler(cancellation.clone()));
    let scope = scope(Uuid::new_v4());
    hub.publish(&rendered(10), scope, None);

    let first_waiter = {
        let hub = Arc::clone(&hub);
        tokio::spawn(async move { hub.wait_for_sample_after(0).await.sequence })
    };
    hub.change_subscribers(VisualizationLane::Normal, 1);
    assert_eq!(first_waiter.await.unwrap(), 1);

    let second_waiter = {
        let hub = Arc::clone(&hub);
        tokio::spawn(async move { hub.wait_for_sample_after(1).await.sequence })
    };
    hub.publish(&rendered(20), scope, None);
    tokio::task::yield_now().await;
    assert!(!second_waiter.is_finished());

    tokio::time::advance(VISUALIZATION_SOURCE_SAMPLE_INTERVAL).await;
    assert_eq!(second_waiter.await.unwrap(), 2);

    hub.change_subscribers(VisualizationLane::Normal, -1);
    cancellation.cancel();
    sampler.await.unwrap().unwrap();
}

#[tokio::test(start_paused = true)]
async fn sampler_uses_the_next_source_when_a_cadence_deadline_precedes_publication() {
    let hub = Arc::new(VisualizationFrameHub::default());
    let cancellation = CancellationToken::new();
    let sampler = tokio::spawn(Arc::clone(&hub).run_sampler(cancellation.clone()));
    let scope = scope(Uuid::new_v4());
    hub.publish(&rendered(10), scope, None);
    hub.change_subscribers(VisualizationLane::Normal, 1);
    assert_eq!(hub.wait_for_sample_after(0).await.sequence, 1);
    tokio::task::yield_now().await;

    let next_waiter = {
        let hub = Arc::clone(&hub);
        tokio::spawn(async move { hub.wait_for_sample_after(1).await.sequence })
    };
    tokio::time::advance(VISUALIZATION_SOURCE_SAMPLE_INTERVAL).await;
    tokio::task::yield_now().await;
    assert!(!next_waiter.is_finished());

    hub.publish(&rendered(20), scope, None);
    assert_eq!(next_waiter.await.unwrap(), 2);

    hub.change_subscribers(VisualizationLane::Normal, -1);
    cancellation.cancel();
    sampler.await.unwrap().unwrap();
}

#[tokio::test(start_paused = true)]
async fn reactivated_sampler_starts_a_fresh_shared_cadence() {
    let hub = Arc::new(VisualizationFrameHub::default());
    let cancellation = CancellationToken::new();
    let sampler = tokio::spawn(Arc::clone(&hub).run_sampler(cancellation.clone()));
    let scope = scope(Uuid::new_v4());
    hub.publish(&rendered(10), scope, None);
    hub.change_subscribers(VisualizationLane::Normal, 1);
    assert_eq!(hub.wait_for_sample_after(0).await.sequence, 1);
    tokio::task::yield_now().await;

    tokio::time::advance(VISUALIZATION_SOURCE_SAMPLE_INTERVAL / 2).await;
    hub.change_subscribers(VisualizationLane::Normal, -1);
    hub.publish(&rendered(20), scope, None);
    hub.change_subscribers(VisualizationLane::Normal, 1);
    assert_eq!(hub.wait_for_sample_after(1).await.sequence, 2);
    tokio::task::yield_now().await;

    hub.publish(&rendered(30), scope, None);
    let next_waiter = {
        let hub = Arc::clone(&hub);
        tokio::spawn(async move { hub.wait_for_sample_after(2).await.sequence })
    };
    tokio::time::advance(VISUALIZATION_SOURCE_SAMPLE_INTERVAL - Duration::from_millis(1)).await;
    tokio::task::yield_now().await;
    assert!(!next_waiter.is_finished());
    tokio::time::advance(Duration::from_millis(1)).await;
    assert_eq!(next_waiter.await.unwrap(), 3);

    hub.change_subscribers(VisualizationLane::Normal, -1);
    cancellation.cancel();
    sampler.await.unwrap().unwrap();
}

#[test]
fn preload_projection_uses_its_own_authoritative_source_identity_and_timestamp() {
    let hub = VisualizationFrameHub::default();
    let scope = scope(Uuid::new_v4());
    hub.publish(&rendered(10), scope, None);
    let source = hub.latest().unwrap();
    let stale_timestamp = "2020-01-01T00:00:00Z";

    let projection = hub
        .projection(
            VisualizationProjectionKey::Preload {
                session_id: Uuid::new_v4(),
                include_dynamic_stack: false,
                complete_values: false,
            },
            &source,
            |_| {
                Ok(VisualizationLaneSnapshot {
                    scope,
                    revision: 10,
                    generated_at: stale_timestamp.into(),
                    grand_master: 1.0,
                    blackout: false,
                    preload: true,
                    values: Vec::new(),
                    dynamic_stack: Vec::new(),
                    profile_output_values: Vec::new(),
                })
            },
        )
        .unwrap();

    assert_eq!(projection.lane_source_sequence, 1);
    assert_ne!(projection.snapshot.generated_at, stale_timestamp);
    assert_eq!(
        projection.snapshot.generated_at,
        chrono::DateTime::<chrono::Utc>::from(projection.source_generated_at).to_rfc3339()
    );
}

#[test]
fn subscriber_metrics_return_to_zero() {
    let hub = VisualizationFrameHub::default();
    hub.change_subscribers(VisualizationLane::Normal, 1);
    hub.change_subscribers(VisualizationLane::Preload, 1);
    assert_eq!(hub.metrics().normal_subscribers, 1);
    assert_eq!(hub.metrics().preload_subscribers, 1);
    hub.change_subscribers(VisualizationLane::Normal, -1);
    hub.change_subscribers(VisualizationLane::Preload, -1);
    assert_eq!(hub.metrics().normal_subscribers, 0);
    assert_eq!(hub.metrics().preload_subscribers, 0);
}

#[test]
fn final_projection_claim_releases_session_specific_cache() {
    let hub = VisualizationFrameHub::default();
    let key = VisualizationProjectionKey::Preload {
        session_id: Uuid::new_v4(),
        include_dynamic_stack: false,
        complete_values: false,
    };
    let scope = scope(Uuid::new_v4());
    hub.change_projection_claim(key, 1);
    hub.publish(&rendered(10), scope, None);
    let source = hub.latest().unwrap();
    let builds = AtomicUsize::new(0);
    let mut build = |_| {
        builds.fetch_add(1, AtomicOrdering::Relaxed);
        Ok(VisualizationLaneSnapshot {
            scope,
            revision: 10,
            generated_at: "2026-07-27T00:00:00Z".into(),
            grand_master: 1.0,
            blackout: false,
            preload: true,
            values: Vec::new(),
            dynamic_stack: Vec::new(),
            profile_output_values: Vec::new(),
        })
    };
    hub.projection(key, &source, &mut build).unwrap();
    hub.projection(key, &source, &mut build).unwrap();
    assert_eq!(builds.load(AtomicOrdering::Relaxed), 1);

    hub.change_projection_claim(key, -1);
    hub.change_projection_claim(key, 1);
    hub.projection(key, &source, &mut build).unwrap();
    assert_eq!(builds.load(AtomicOrdering::Relaxed), 2);
}

#[test]
fn removed_values_have_deterministic_fixture_and_attribute_order() {
    let scope = scope(Uuid::new_v4());
    let fixture_a = Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap();
    let fixture_b = Uuid::parse_str("22222222-2222-4222-8222-222222222222").unwrap();
    let previous = VisualizationLaneSnapshot {
        scope,
        revision: 1,
        generated_at: "2026-07-27T00:00:00Z".into(),
        grand_master: 1.0,
        blackout: false,
        preload: false,
        values: vec![
            VisualizationValue {
                fixture_id: fixture_b,
                attribute: "tilt".into(),
                value: ProgrammingPreloadAttributeValue::Normalized(0.5),
            },
            VisualizationValue {
                fixture_id: fixture_a,
                attribute: "pan".into(),
                value: ProgrammingPreloadAttributeValue::Normalized(0.5),
            },
            VisualizationValue {
                fixture_id: fixture_a,
                attribute: "intensity".into(),
                value: ProgrammingPreloadAttributeValue::Normalized(0.5),
            },
        ],
        dynamic_stack: Vec::new(),
        profile_output_values: Vec::new(),
    };
    let current = VisualizationLaneSnapshot {
        values: Vec::new(),
        revision: 2,
        ..previous.clone()
    };

    let delta = lane_delta(Some(&previous), &current);

    assert_eq!(
        delta.removed_values,
        vec![
            VisualizationValueKey {
                fixture_id: fixture_a,
                attribute: "intensity".into(),
            },
            VisualizationValueKey {
                fixture_id: fixture_a,
                attribute: "pan".into(),
            },
            VisualizationValueKey {
                fixture_id: fixture_b,
                attribute: "tilt".into(),
            },
        ]
    );
}

#[test]
fn projection_work_cannot_block_latest_frame_publication() {
    let hub = Arc::new(VisualizationFrameHub::default());
    let scope = scope(Uuid::new_v4());
    hub.publish(&rendered(10), scope, None);
    let source = hub.latest().unwrap();
    let (projection_started_tx, projection_started_rx) = mpsc::channel();
    let (release_projection_tx, release_projection_rx) = mpsc::channel();
    let projection_hub = Arc::clone(&hub);
    let projection = thread::spawn(move || {
        projection_hub
            .projection(
                VisualizationProjectionKey::Normal {
                    include_dynamic_stack: false,
                    complete_values: false,
                },
                &source,
                |_| {
                    projection_started_tx.send(()).unwrap();
                    release_projection_rx.recv().unwrap();
                    Ok(VisualizationLaneSnapshot {
                        scope,
                        revision: 10,
                        generated_at: "2026-07-27T00:00:00Z".into(),
                        grand_master: 1.0,
                        blackout: false,
                        preload: false,
                        values: Vec::new(),
                        dynamic_stack: Vec::new(),
                        profile_output_values: Vec::new(),
                    })
                },
            )
            .unwrap();
    });
    projection_started_rx.recv().unwrap();

    let (published_tx, published_rx) = mpsc::channel();
    let publisher_hub = Arc::clone(&hub);
    thread::spawn(move || {
        publisher_hub.publish(&rendered(20), scope, None);
        published_tx.send(()).unwrap();
    });
    published_rx
        .recv_timeout(Duration::from_millis(100))
        .expect("publication must not wait for projection work");
    assert_eq!(hub.latest().unwrap().show_revision, 20);

    release_projection_tx.send(()).unwrap();
    projection.join().unwrap();
}
