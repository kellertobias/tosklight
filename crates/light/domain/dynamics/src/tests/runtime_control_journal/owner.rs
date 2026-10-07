//! TL-585: a surviving controller's operational owner and priority are refreshed by a recorded
//! control. Retained branches replay the same change; nothing else about the instance moves.
use super::*;

fn without_owner_metadata(mut snapshot: DynamicRuntimeSnapshot) -> DynamicRuntimeSnapshot {
    for controller in snapshot
        .instances
        .iter_mut()
        .flat_map(|instance| &mut instance.controllers)
    {
        controller.source = DynamicControllerSource::physical_playback(0);
        controller.priority = 0;
    }
    snapshot
}

fn paused_current_branches() -> (DynamicRuntime, DynamicRuntime, DynamicController, Uuid) {
    let target = FixtureId::new();
    let mut owner = controller(2301, 3, false);
    owner.source = DynamicControllerSource::physical_playback(1);
    let mut current = lane();
    for point in &mut current.legacy_mut().unwrap().keyframes.points {
        point.source = ScalarSource::Current;
    }
    let mut definition = definition(current);
    definition.default_activation = ActivationPolicy::JoinSyncNow;
    definition.speed = DynamicSpeed::SpeedGroup {
        group: SpeedGroup::A,
        beats_per_cycle: Rational::ONE,
    };
    definition.target_binding = DynamicTargetBinding::FrozenTargets {
        targets: vec![target],
    };
    let mut live = installed(&definition);
    let id = live
        .start(start_request(
            definition.id,
            owner.clone(),
            target,
            0,
            false,
        ))
        .unwrap();
    let mut pending = live.fork_for_pending_preview();
    let transports = [DynamicSpeedTransport {
        effective_bpm: 60.,
        phase_origin_millis: 0,
        phase_reference_millis: 100,
        beat_phase: 0.1,
        phase_advancing: true,
    }; 5];
    live.sample_all_addressed(100, 10, &transports, &Sources { current: 0.2 }, None);
    pending.sample_all_addressed(100, 10, &transports, &Sources { current: 0.8 }, None);
    (live, pending, owner, id)
}

#[test]
fn owner_refresh_records_replays_and_keeps_instance_clock_and_held_current() {
    let (mut live, mut pending, owner, id) = paused_current_branches();
    let mut journal = journal();
    let mut cursor = journal.cursor();
    let mut scratch = DynamicOutputFrameScratch::default();
    let pause = DynamicControl::Pause {
        controller: owner.id,
        paused: true,
        resume: None,
    };
    journal
        .execute(&mut live, &mut scratch, vec![timed(150, pause)])
        .unwrap();
    let batch = journal.read(cursor).unwrap();
    let anchor = pending.committed_sample_boundary();
    replay_dynamic_controls(&mut pending, &mut scratch, &mut cursor, &batch, anchor).unwrap();
    let live_before = live.snapshot();
    let pending_before = pending.snapshot();
    assert!(live_before.instances[0].paused_at_millis.is_some());

    let moved = DynamicControllerSource::virtual_playback(2, 1301);
    let refresh = DynamicControl::Owner {
        controller: owner.id,
        source: moved.clone(),
        priority: 9,
    };
    let outcomes = journal
        .execute(&mut live, &mut scratch, vec![timed(200, refresh.clone())])
        .unwrap();
    assert_eq!(outcomes[0].instance_id, Some(id));
    assert!(outcomes[0].changed);
    let batch = journal.read(cursor).unwrap();
    assert_eq!(batch.len(), 1);
    assert_eq!(batch.operation_times().collect::<Vec<_>>(), vec![200]);
    let anchor = pending.committed_sample_boundary();
    replay_dynamic_controls(&mut pending, &mut scratch, &mut cursor, &batch, anchor).unwrap();

    for (runtime, before) in [(&live, &live_before), (&pending, &pending_before)] {
        let (instance, controller) = runtime.controller(owner.id).unwrap();
        assert_eq!(instance, id);
        assert_eq!(controller.source, moved);
        assert_eq!(controller.priority, 9);
        assert!(controller.paused);
        // Instance identity, activation, paused clock, phase map, held Current and every
        // other retained field are byte-for-byte unchanged.
        assert_eq!(
            without_owner_metadata(runtime.snapshot()),
            without_owner_metadata(before.clone())
        );
    }
    let (live_now, pending_now) = (live.snapshot(), pending.snapshot());
    assert!(!live_now.instances[0].synchronized_hold_values.is_empty());
    assert_ne!(
        live_now.instances[0].expression_tape, pending_now.instances[0].expression_tape,
        "each retained branch keeps its own sampled Current"
    );

    // An identical refresh is not a change and records nothing.
    let unchanged = journal
        .execute(&mut live, &mut scratch, vec![timed(210, refresh)])
        .unwrap();
    assert!(!unchanged[0].changed);
    assert!(journal.read(cursor).unwrap().is_empty());
}

#[test]
fn owner_refresh_rejects_changing_the_runtime_owner_kind_atomically() {
    let (mut live, _, owner, _) = paused_current_branches();
    let before = live.snapshot();
    assert_eq!(
        live.update_controller_owner(
            owner.id,
            DynamicControllerSource::Programmer {
                programmer_id: Uuid::new_v4(),
                instance_link: None,
            },
            4,
        ),
        Err(DynamicRuntimeError::InvalidController)
    );
    assert_eq!(
        live.update_controller_owner(
            Uuid::new_v4(),
            DynamicControllerSource::physical_playback(2),
            4
        ),
        Err(DynamicRuntimeError::MissingController)
    );
    assert_eq!(live.snapshot(), before);
}

#[test]
fn playback_owner_serialization_keeps_old_checkpoints_and_qualifies_virtual_pages() {
    let legacy: DynamicControllerSource =
        serde_json::from_value(serde_json::json!({"type": "playback", "playback_number": 7}))
            .unwrap();
    assert_eq!(legacy, DynamicControllerSource::physical_playback(7));
    assert_eq!(
        serde_json::to_value(&legacy).unwrap(),
        serde_json::json!({"type": "playback", "playback_number": 7})
    );
    let virtual_owner = DynamicControllerSource::virtual_playback(2, 1301);
    let encoded = serde_json::to_value(&virtual_owner).unwrap();
    assert_eq!(
        encoded,
        serde_json::json!({"type": "playback", "playback_number": 1301, "virtual_page": 2})
    );
    assert_eq!(
        serde_json::from_value::<DynamicControllerSource>(encoded).unwrap(),
        virtual_owner
    );
}

#[test]
fn owner_refresh_keeps_random_streams_and_their_future_decisions() {
    let mut random_lane = lane();
    random_lane.legacy_mut().unwrap().mode = DynamicLaneMode::Random;
    let group = Uuid::from_u128(2302);
    random_lane.random_group_id = Some(group);
    let mut definition = definition(random_lane);
    definition.random_groups.push(DynamicRandomGroup {
        id: group,
        seed: 17,
        range: DynamicRandomRange::LegacyScalar {
            low: source(0.0),
            high: source(1.0),
        },
        decision_interval_millis: 100,
        start_probability: 1.0,
        mean_duration_millis: 250,
        duration_spread_millis: 30,
        attack_ratio: 0.1,
        decay_ratio: 0.5,
    });
    let mut owner = controller(2303, 2, false);
    owner.source = DynamicControllerSource::physical_playback(4);
    let mut live = installed(&definition);
    let instance = live
        .start(start_request(
            definition.id,
            owner.clone(),
            FixtureId::new(),
            0,
            false,
        ))
        .unwrap();
    let sample = |runtime: &mut DynamicRuntime, at| {
        runtime
            .sample(instance, at, 1_000, 10, &Sources { current: 0.0 })
            .unwrap()
    };
    sample(&mut live, 150);
    sample(&mut live, 400);
    let before = live.snapshot();
    assert!(!before.instances[0].random_streams.is_empty());
    let mut untouched = live.fork_for_pending_preview();

    assert_eq!(
        live.update_controller_owner(owner.id, DynamicControllerSource::physical_playback(8), 6),
        Ok(true)
    );
    assert_eq!(
        without_owner_metadata(live.snapshot()),
        without_owner_metadata(before)
    );
    for at in [675, 900, 1_300] {
        let refreshed = sample(&mut live, at);
        let mut original = sample(&mut untouched, at);
        assert!(refreshed.iter().all(|sample| sample.priority == 6));
        original.iter_mut().for_each(|sample| sample.priority = 6);
        assert_eq!(
            refreshed, original,
            "Random decisions continue unchanged at {at}"
        );
        assert_eq!(
            without_owner_metadata(live.snapshot()),
            without_owner_metadata(untouched.snapshot())
        );
    }
}
