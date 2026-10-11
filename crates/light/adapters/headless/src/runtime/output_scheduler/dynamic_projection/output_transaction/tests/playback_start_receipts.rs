//! A rejected final render must not acknowledge a speculative Playback start.
use super::*;
use light_engine::{EnginePlaybackCommand as Command, PlaybackAction as Action};
use light_playback::DynamicPlaybackStartOutcome as Outcome;

#[test]
fn failed_final_render_preserves_pending_start_until_successful_retry() {
    let fixture = FixtureId::new();
    let mut definition = definition();
    definition.target_binding = DynamicTargetBinding::FrozenTargets {
        targets: vec![fixture],
    };
    let playback = serde_json::from_value(serde_json::json!({
        "name": "Receipt", "xfade_millis": 0,
        "target": {"type": "dynamic", "assignment": {
            "auto_off_full_control": false,
            "dynamic": {"dynamic_id": definition.id, "last_known_pool_number": 1,
                "embedded_fallback": {"definition": definition}}
        }}
    }))
    .unwrap();
    let clock = Arc::new(light_core::ManualClock::new(
        chrono::DateTime::from_timestamp_millis(1_000_000).unwrap(),
    ));
    let engine = Engine::new(ProgrammerRegistry::with_clock(clock));
    engine
        .replace_snapshot(light_engine::EngineSnapshot {
            dynamics: vec![definition.clone()].into(),
            playback_pages: vec![light_playback::PlaybackPage {
                number: 1,
                name: "Main".into(),
                assignments: [(1, playback)].into(),
            }]
            .into(),
            ..Default::default()
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
    let incarnation = engine.active_dynamic_playbacks()[0].start_state.incarnation;
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition.clone()]).unwrap();
    let dynamics = Mutex::new(runtime);
    let dynamic_snapshot = crate::runtime::DynamicSnapshotPublication::new(engine.snapshot());
    let origins = SharedDynamicSourceOrigins::default();
    let groups = Mutex::new(std::array::from_fn(|_| {
        light_control::speed::SpeedGroupController::new(120.0, Default::default()).unwrap()
    }));
    let rate = AtomicU16::new(40);
    let cache = ProgrammerReconciliationCache::default();
    let frame = engine.prepare_output_frame(Default::default());
    let failed = dynamic_output_frame(
        &engine,
        &frame,
        None,
        &[],
        &dynamics,
        &dynamic_snapshot,
        &origins,
        &groups,
        &rate,
        &cache,
        &LiveFamilyAdapters::default(),
        legacy(|_| Err::<(), _>(EngineError::Invalid("rejected final render".into()))),
    );
    assert!(failed.is_err());
    assert!(dynamics.lock().snapshot().instances.is_empty());
    assert_eq!(
        engine.active_dynamic_playbacks()[0].start_state.outcome,
        Outcome::Pending
    );
    let retry = engine.prepare_output_frame(Default::default());
    let committed = dynamic_output_frame(
        &engine,
        &retry,
        None,
        &[],
        &dynamics,
        &dynamic_snapshot,
        &origins,
        &groups,
        &rate,
        &cache,
        &LiveFamilyAdapters::default(),
        legacy(|batches| engine.render_prepared(&retry, batches)),
    )
    .unwrap();
    assert_eq!(committed.start_changes, vec![definition.id]);
    let active = &engine.active_dynamic_playbacks()[0];
    assert_eq!(active.start_state.incarnation, incarnation);
    assert_eq!(active.start_state.outcome, Outcome::Accepted);
    assert!(!committed.runtime.instances.is_empty());
    let previous = active.clone();
    engine
        .execute_playback(Command::Playback {
            address: light_playback::PlaybackAddress::new(1, 1).unwrap(),
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
    let reactivated = engine.active_dynamic_playbacks()[0].clone();
    assert_eq!(
        previous, reactivated,
        "durable inputs remain equal at the same clock instant"
    );
    assert_ne!(
        previous.start_state.incarnation,
        reactivated.start_state.incarnation
    );
    assert_eq!(reactivated.start_state.outcome, Outcome::Pending);
    let next = engine.prepare_output_frame(Default::default());
    let committed = dynamic_output_frame(
        &engine,
        &next,
        None,
        &[],
        &dynamics,
        &dynamic_snapshot,
        &origins,
        &groups,
        &rate,
        &cache,
        &LiveFamilyAdapters::default(),
        legacy(|batches| engine.render_prepared(&next, batches)),
    )
    .unwrap();
    assert_eq!(committed.start_changes, vec![definition.id]);
    assert_eq!(
        engine.active_dynamic_playbacks()[0].start_state.outcome,
        Outcome::Accepted
    );
}

#[test]
fn true_warm_start_rejection_publishes_failed_for_all_aliases_once() {
    use crate::runtime::output_scheduler::dynamic_reconciliation::reconcile_dynamic_playbacks_with_sources;
    use light_application::{
        ApplicationEvent, DynamicPlaybackRuntimeState, PlaybackEvent, PlaybackShowScope,
        PlaybackTargetProjection,
    };
    let fixture = FixtureId::new();
    let mut definition = definition();
    definition.target_binding = DynamicTargetBinding::FrozenTargets {
        targets: vec![fixture],
    };
    let playback = || {
        serde_json::from_value(serde_json::json!({
            "name": "Rejected receipt", "xfade_millis": 0,
            "target": {"type": "dynamic", "assignment": {
                "auto_off_full_control": false,
                "dynamic": {"dynamic_id": definition.id, "last_known_pool_number": 1,
                    "embedded_fallback": {"definition": definition}}
            }}
        }))
        .unwrap()
    };
    let engine = Engine::new(ProgrammerRegistry::default());
    engine
        .replace_snapshot(light_engine::EngineSnapshot {
            dynamics: vec![definition.clone()].into(),
            playback_pages: vec![
                light_playback::PlaybackPage {
                    number: 1,
                    name: "Main aliases".into(),
                    assignments: HashMap::from([(1, playback()), (2, playback())]),
                },
                light_playback::PlaybackPage {
                    number: 3,
                    name: "Pane alias".into(),
                    assignments: HashMap::from([(401, playback())]),
                },
            ]
            .into(),
            ..Default::default()
        })
        .unwrap();
    engine
        .execute_playback(Command::Playback {
            address: light_playback::PlaybackAddress::new(1, 2).unwrap(),
            activation_origin: None,
            control_source: light_playback::PlaybackControlSource {
                surface_id: "main".into(),
                control_id: 2,
            },
            action: Action::On,
        })
        .unwrap();
    let mut active = engine.active_dynamic_playbacks();
    // Inject an invalid captured controller value at the reconciliation boundary. Saved rows
    // reject non-finite input earlier; this exercises an actual unexpected warm runtime failure.
    active[0].size = f32::NAN;
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition.clone()]).unwrap();
    let snapshot = engine.snapshot();
    let (_, receipts) =
        reconcile_dynamic_playbacks_with_sources(&mut runtime, 100, &snapshot, &active, |_| {});
    assert!(matches!(&receipts[0].outcome, Outcome::Rejected(_)));
    let changed = acknowledge_dynamic_starts(&engine, &receipts);
    let scope = PlaybackShowScope {
        show_id: Uuid::nil(),
        show_revision: snapshot.revision,
    };
    let events = crate::runtime::playback_service::projection::settled_dynamic_playback_events(
        &engine,
        scope,
        &snapshot,
        &runtime.snapshot(),
        &[],
        &[],
        &changed,
        25,
    );
    assert_eq!(events.len(), 3);
    for event in events {
        let ApplicationEvent::Playback(PlaybackEvent::RuntimeChanged(change)) = event.payload
        else {
            panic!();
        };
        let PlaybackTargetProjection::Dynamic {
            runtime: Some(runtime),
            ..
        } = change.projection.target
        else {
            panic!();
        };
        assert_eq!(runtime.state, DynamicPlaybackRuntimeState::Failed);
        assert!(
            runtime.warning.is_some(),
            "actual rejection has actionable diagnostic"
        );
    }
    let unchanged = acknowledge_dynamic_starts(&engine, &receipts);
    assert!(unchanged.is_empty());
    assert!(
        crate::runtime::playback_service::projection::settled_dynamic_playback_events(
            &engine,
            scope,
            &snapshot,
            &runtime.snapshot(),
            &[],
            &[],
            &unchanged,
            25,
        )
        .is_empty()
    );
}
