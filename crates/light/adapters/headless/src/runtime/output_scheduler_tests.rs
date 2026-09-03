use super::*;
use light_control::FrameRate;
use light_core::{AttributeKey, FixtureId};
use light_dynamics::{
    DynamicController, DynamicControllerSource, DynamicInstanceSnapshot, DynamicRuntimeSample,
    DynamicRuntimeSnapshot,
};
use light_output::{DeliveryMode, OutputRoute};
use light_programmer::ProgrammerRegistry;
use uuid::Uuid;

#[test]
fn frame_selection_applies_overrides_and_reuses_held_frames() {
    let mut control = OutputControl::default();
    control.raw_overrides.insert((1, 2), 77);
    let live = output_frames(
        &mut control,
        light_engine::Pooled::unpooled(frames_with_value(1, 10)),
    );
    assert_eq!(live[&1][0], 10);
    assert_eq!(live[&1][1], 77);

    control.hold = true;
    let held = output_frames(
        &mut control,
        light_engine::Pooled::unpooled(frames_with_value(1, 99)),
    );
    assert_eq!(held, live);
}

#[test]
fn held_output_reuses_one_coherent_route_frame_and_slot_snapshot() {
    let mut control = OutputControl::default();
    let live_route = OutputRoute {
        target: Default::default(),
        protocol: Protocol::ArtNet,
        logical_universe: 1,
        destination_universe: 1,
        delivery_mode: Some(DeliveryMode::Broadcast),
        destination: None,
        enabled: true,
        minimum_slots: 1,
    };
    let live = output_payload(
        &mut control,
        Arc::from([live_route.clone()]),
        light_engine::Pooled::unpooled(frames_with_value(1, 10)),
        light_engine::Pooled::unpooled(HashMap::from([(1, 24)])),
    );

    control.hold = true;
    let held = output_payload(
        &mut control,
        Arc::from([]),
        light_engine::Pooled::unpooled(frames_with_value(2, 99)),
        light_engine::Pooled::unpooled(HashMap::from([(2, 512)])),
    );

    assert_eq!(held.0.as_ref(), &[live_route]);
    assert_eq!(held.1, live.1);
    assert_eq!(*held.2, HashMap::from([(1, 24)]));
}

#[test]
fn timecode_frame_uses_the_nominal_frame_rate() {
    let timecode = SmpteTimecode {
        hours: 1,
        minutes: 2,
        seconds: 3,
        frames: 4,
        rate: FrameRate::Fps25,
        source: "test".into(),
        received_at: chrono::Utc::now(),
    };
    assert_eq!(timecode_frame(&timecode), 93_079);
}

#[tokio::test]
async fn scheduler_gate_waits_for_start_and_cancels_without_deadlock() {
    let cancellation = CancellationToken::new();
    let (start, ready) = tokio::sync::oneshot::channel();
    let waiting = tokio::spawn({
        let cancellation = cancellation.clone();
        async move { await_start(ready, &cancellation).await }
    });
    tokio::task::yield_now().await;
    assert!(!waiting.is_finished());
    start.send(()).unwrap();
    assert!(waiting.await.unwrap());

    let (_start, ready) = tokio::sync::oneshot::channel();
    cancellation.cancel();
    assert!(!await_start(ready, &cancellation).await);
}

#[tokio::test]
async fn scheduler_gate_closes_when_the_start_owner_is_dropped() {
    let cancellation = CancellationToken::new();
    let (start, ready) = tokio::sync::oneshot::channel();
    drop(start);

    let allowed = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        await_start(ready, &cancellation),
    )
    .await
    .expect("a dropped start owner must not leave the scheduler gated");

    assert!(!allowed);
}

#[test]
fn dynamic_playback_full_control_requires_complete_persistent_coverage_and_honors_opt_out() {
    let engine = Engine::new(ProgrammerRegistry::default());
    let source_controller = Uuid::new_v4();
    let covering_controller = Uuid::new_v4();
    let first_target = FixtureId::new();
    let second_target = FixtureId::new();
    let source_samples = vec![
        dynamic_sample(source_controller, first_target, "intensity", 10, 100),
        dynamic_sample(source_controller, second_target, "pan", 10, 100),
    ];
    let persistent_cover = vec![
        dynamic_sample(covering_controller, first_target, "intensity", 11, 101),
        dynamic_sample(covering_controller, second_target, "pan", 11, 101),
    ];
    let runtime = runtime_with_controllers(vec![
        dynamic_controller(
            source_controller,
            DynamicControllerSource::Playback { playback_number: 1 },
            10,
            100,
        ),
        dynamic_controller(
            covering_controller,
            DynamicControllerSource::Programmer {
                programmer_id: Uuid::new_v4(),
            },
            11,
            101,
        ),
    ]);
    let default_on = HashMap::from([(source_controller, dynamic_playback_control(1, true, false))]);

    let mut completely_covered = source_samples.clone();
    completely_covered.extend(persistent_cover.clone());
    assert_eq!(
        fully_controlled_dynamic_playbacks(
            &engine,
            &completely_covered,
            &default_on,
            &runtime,
            &[],
            &[],
            engine.application_time(),
        ),
        vec![light_playback::PlaybackIdentity::physical(1).unwrap()],
        "the default-on setting turns the source off only after every address is covered",
    );

    let opt_out = HashMap::from([(source_controller, dynamic_playback_control(1, false, false))]);
    assert!(
        fully_controlled_dynamic_playbacks(
            &engine,
            &completely_covered,
            &opt_out,
            &runtime,
            &[],
            &[],
            engine.application_time(),
        )
        .is_empty()
    );

    let mut partially_covered = source_samples.clone();
    partially_covered.push(persistent_cover[0].clone());
    assert!(
        fully_controlled_dynamic_playbacks(
            &engine,
            &partially_covered,
            &default_on,
            &runtime,
            &[],
            &[],
            engine.application_time(),
        )
        .is_empty()
    );
}

#[test]
fn temporary_dynamic_playback_coverage_never_triggers_full_control_auto_off() {
    let engine = Engine::new(ProgrammerRegistry::default());
    let source_controller = Uuid::new_v4();
    let temporary_controller = Uuid::new_v4();
    let target = FixtureId::new();
    let samples = vec![
        dynamic_sample(source_controller, target, "intensity", 10, 100),
        dynamic_sample(temporary_controller, target, "intensity", 11, 101),
    ];
    let runtime = runtime_with_controllers(vec![
        dynamic_controller(
            source_controller,
            DynamicControllerSource::Playback { playback_number: 1 },
            10,
            100,
        ),
        dynamic_controller(
            temporary_controller,
            DynamicControllerSource::Playback { playback_number: 2 },
            11,
            101,
        ),
    ]);
    let controls = HashMap::from([
        (source_controller, dynamic_playback_control(1, true, false)),
        (
            temporary_controller,
            dynamic_playback_control(2, true, true),
        ),
    ]);

    assert!(
        fully_controlled_dynamic_playbacks(
            &engine,
            &samples,
            &controls,
            &runtime,
            &[],
            &[],
            engine.application_time(),
        )
        .is_empty()
    );
}

#[test]
fn scheduler_emits_typed_dynamic_runtime_boundaries() {
    let controller_id = Uuid::new_v4();
    let controller = dynamic_controller(
        controller_id,
        DynamicControllerSource::Programmer {
            programmer_id: Uuid::new_v4(),
        },
        10,
        100,
    );
    let before_start = DynamicRuntimeSnapshot::default();
    let after_start = runtime_with_controllers(vec![controller.clone()]);
    assert_eq!(
        dynamic_event_kinds(dynamic_transition_events(&before_start, &after_start, 100,)),
        vec![
            light_application::DynamicRuntimeEventKind::InstanceStarted,
            light_application::DynamicRuntimeEventKind::InstanceActive,
        ],
    );

    let mut before_pending = after_start.clone();
    before_pending.instances[0].pending_until_millis = Some(200);
    let mut after_active = before_pending.clone();
    after_active.instances[0].pending_until_millis = None;
    assert_eq!(
        dynamic_event_kinds(dynamic_transition_events(
            &before_pending,
            &after_active,
            200,
        )),
        vec![light_application::DynamicRuntimeEventKind::InstanceActive],
    );

    let mut before_release = after_active.clone();
    before_release.instances[0].controller_transitions.push(
        light_dynamics::DynamicControllerTransitionSnapshot {
            controller_id,
            release_started_at_millis: Some(200),
            ..Default::default()
        },
    );
    let mut after_release = before_release.clone();
    after_release.instances[0].controllers.clear();
    after_release.instances[0].controller_transitions.clear();
    assert_eq!(
        dynamic_event_kinds(dynamic_transition_events(
            &before_release,
            &after_release,
            300,
        )),
        vec![light_application::DynamicRuntimeEventKind::TransitionCompleted],
    );

    let mut paused = after_active.clone();
    paused.global_paused = true;
    assert_eq!(
        dynamic_event_kinds(dynamic_transition_events(&after_active, &paused, 400)),
        vec![light_application::DynamicRuntimeEventKind::Paused],
    );
    assert_eq!(
        dynamic_event_kinds(dynamic_transition_events(&paused, &after_active, 500)),
        vec![light_application::DynamicRuntimeEventKind::Resumed],
    );
}

fn dynamic_event_kinds(
    events: Vec<light_application::EventDraft>,
) -> Vec<light_application::DynamicRuntimeEventKind> {
    events
        .into_iter()
        .filter_map(|event| match event.payload {
            light_application::ApplicationEvent::Output(
                light_application::OutputEvent::DynamicRuntimeChanged(change),
            ) => Some(change.kind),
            _ => None,
        })
        .collect()
}

fn dynamic_playback_control(
    playback_number: u16,
    auto_off_full_control: bool,
    temporary_only: bool,
) -> DynamicPlaybackControl {
    DynamicPlaybackControl {
        identity: light_playback::PlaybackIdentity::physical(playback_number).unwrap(),
        master: 1.0,
        crossfade_non_intensity: false,
        auto_off_full_control,
        temporary_only,
    }
}

fn dynamic_sample(
    controller_id: Uuid,
    target: FixtureId,
    attribute: &str,
    priority: i16,
    activated_at_millis: u64,
) -> DynamicRuntimeSample {
    DynamicRuntimeSample {
        instance_id: Uuid::new_v4(),
        controller_id,
        target,
        lane_id: Uuid::new_v4(),
        attribute: AttributeKey(attribute.into()),
        value: 0.5,
        priority,
        activated_at_millis,
        activation_mix: 1.0,
        address: None,
    }
}

fn dynamic_controller(
    id: Uuid,
    source: DynamicControllerSource,
    priority: i16,
    activated_at_millis: u64,
) -> DynamicController {
    DynamicController {
        id,
        source,
        priority,
        activated_at_millis,
        size: 1.0,
        speed_multiplier: 1.0,
        phase_offset_degrees: 0.0,
        paused: false,
    }
}

fn runtime_with_controllers(controllers: Vec<DynamicController>) -> DynamicRuntimeSnapshot {
    DynamicRuntimeSnapshot {
        global_paused: false,
        instances: vec![DynamicInstanceSnapshot {
            id: Uuid::new_v4(),
            definition: serde_json::from_value(serde_json::json!({
                "id": Uuid::new_v4(),
                "pool_number": 1,
                "revision": 1,
                "name": "Coverage test",
                "color": null,
                "icon": null,
                "target_binding": {"type": "targetless"},
                "lanes": [],
                "random_groups": [],
                "phase": {
                    "ordering": {"type": "selection"},
                    "offset_degrees": 0.0,
                    "span_degrees": 360.0,
                    "block_size": 1,
                    "repeats": 1,
                    "wings": false,
                    "anchors_degrees": []
                },
                "speed": {"type": "fixed", "duration_millis": 1000},
                "default_activation": "start_now"
            }))
            .unwrap(),
            targets: Vec::new(),
            phase_by_target: Vec::new(),
            phase_by_lane_target: Vec::new(),
            controllers,
            controller_transitions: Vec::new(),
            started_at_millis: 0,
            paused_at_millis: None,
            paused_elapsed_millis: 0,
            activation_policy: light_dynamics::ActivationPolicy::StartNow,
            pending_until_millis: None,
            speed_paused_at_millis: None,
            speed_paused_elapsed_millis: 0,
            random_streams: Vec::new(),
            completed: false,
            synchronized_hold_elapsed_millis: None,
            last_synchronized_elapsed_millis: None,
            synchronized_resume_transition: None,
            last_sample_values: Vec::new(),
            synchronized_hold_values: Vec::new(),
        }],
    }
}

fn frames_with_value(universe: Universe, value: u8) -> HashMap<Universe, DmxFrame> {
    let mut frame = [0; light_output::DMX_SLOTS];
    frame[0] = value;
    HashMap::from([(universe, frame)])
}

#[test]
fn healthy_usb_delivery_isolated_from_total_network_failure() {
    let outcome =
        combined_delivery_result(std::io::Result::Err(std::io::Error::other("udp")), 1).unwrap();
    assert_eq!(outcome, 1);
}

#[test]
fn total_delivery_failure_remains_visible_when_usb_accepts_nothing() {
    let error = combined_delivery_result(std::io::Result::Err(std::io::Error::other("udp")), 0)
        .unwrap_err();
    assert_eq!(error.to_string(), "udp");
}
