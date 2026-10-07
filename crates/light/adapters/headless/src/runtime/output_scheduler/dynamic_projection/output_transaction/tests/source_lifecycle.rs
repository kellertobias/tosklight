use super::*;

#[test]
fn group_membership_change_rebinds_sources_without_restarting_the_dynamic() {
    let clock = Arc::new(light_core::ManualClock::new(
        chrono::DateTime::from_timestamp_millis(1_000_000).unwrap(),
    ));
    let programmers = ProgrammerRegistry::with_clock(clock.clone());
    let session = light_core::SessionId::new();
    programmers.start(session);
    let first = FixtureId::new();
    let added = FixtureId::new();
    let mut definition = definition();
    definition.target_binding = DynamicTargetBinding::LiveGroup {
        group_id: "front".into(),
    };
    let group = |targets: Vec<FixtureId>| light_programmer::GroupDefinition {
        id: "front".into(),
        name: "Front".into(),
        source: Some(light_programmer::GroupFixtureSource::Explicit {
            fixture_ids: targets,
        }),
        ..Default::default()
    };
    let engine = Engine::new(programmers.clone());
    engine
        .replace_snapshot(light_engine::EngineSnapshot {
            dynamics: Arc::new(vec![definition.clone()]),
            groups: Arc::new(vec![group(vec![first])]),
            ..Default::default()
        })
        .unwrap();
    assert!(programmers.apply_dynamic_values(
        session,
        &[light_programmer::DynamicProgrammerValueMutation::Set {
            fixture_id: first,
            attribute: AttributeKey::intensity(),
            value: DynamicSemanticValue::DynamicOn {
                instance_link: Uuid::new_v4(),
                lane_id: definition.lanes[0].id,
                dynamic: DynamicReference {
                    dynamic_id: Some(definition.id),
                    last_known_pool_number: 1,
                    embedded_fallback: DynamicDefinitionSnapshot {
                        definition: Arc::new(definition.clone())
                    },
                },
                overrides: DynamicInstanceOverrides {
                    size: 1.0,
                    speed_multiplier: Rational::ONE,
                    phase_offset_degrees: 0.0
                },
                timing: Default::default(),
            },
        },],
        None
    ));
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition]).unwrap();
    let dynamics = Mutex::new(runtime);
    let dynamic_snapshot = crate::runtime::DynamicSnapshotPublication::new(engine.snapshot());
    let origins = SharedDynamicSourceOrigins::default();
    let groups = Mutex::new(std::array::from_fn(|_| {
        light_control::speed::SpeedGroupController::new(120.0, Default::default()).unwrap()
    }));
    let rate = AtomicU16::new(40);
    let cache = ProgrammerReconciliationCache::default();
    let render = |frame: &light_engine::PreparedOutputFrame| {
        dynamic_output_frame(
            &engine,
            frame,
            None,
            &[],
            &dynamics,
            &dynamic_snapshot,
            &origins,
            &groups,
            &rate,
            &cache,
            &LiveFamilyAdapters::default(),
            legacy(|batches| engine.render_prepared(frame, batches)),
        )
        .unwrap()
    };
    let frame = engine.prepare_output_frame(Default::default());
    let initial = render(&frame);
    assert_eq!(initial.samples.len(), 1);
    let original_instance = initial.runtime.instances[0].id;
    let started = initial.runtime.instances[0].started_at_millis;
    let occurrence = |sample: &DynamicRuntimeSample| {
        let mut ids = HashSet::new();
        sample
            .expression
            .visit_source_occurrences(&mut |id| {
                ids.insert(id);
            })
            .unwrap();
        assert_eq!(ids.len(), 1);
        *ids.iter().next().unwrap()
    };
    let original_origin = occurrence(&initial.samples[0]);
    let snapshot = engine.snapshot();
    let mut changed = (*snapshot).clone();
    changed.groups = Arc::new(vec![group(vec![first, added])]);
    engine.replace_snapshot(changed).unwrap();
    {
        let _runtime = dynamics.lock();
        dynamic_snapshot.installed(engine.snapshot());
    }
    clock.advance_millis(125);
    let next_frame = engine.prepare_output_frame(Default::default());
    assert!(Arc::ptr_eq(
        frame.dynamic_programmer_values(),
        next_frame.dynamic_programmer_values()
    ));
    assert!(Arc::ptr_eq(
        &snapshot.dynamics,
        &next_frame.snapshot().dynamics
    ));
    assert!(Arc::ptr_eq(
        &snapshot.dynamic_stage_positions,
        &next_frame.snapshot().dynamic_stage_positions
    ));
    let next = render(&next_frame);
    assert_eq!(next.samples.len(), 2);
    assert!(
        next.samples
            .iter()
            .all(|sample| sample.instance_id == original_instance)
    );
    assert_eq!(next.runtime.instances[0].started_at_millis, started);
    assert_eq!(
        occurrence(
            next.samples
                .iter()
                .find(|sample| sample.target == first)
                .unwrap()
        ),
        original_origin
    );
    let added_origin = occurrence(
        next.samples
            .iter()
            .find(|sample| sample.target == added)
            .unwrap(),
    );
    assert_ne!(added_origin, original_origin);
    assert_eq!(
        next.origins.get(added_origin).unwrap().origin,
        next.origins.get(original_origin).unwrap().origin
    );
    assert!(initial.origins.get(added_origin).is_none());
    assert!(
        next.samples
            .iter()
            .all(|sample| (sample.legacy().unwrap().value - 0.25).abs() < 0.001)
    );
    let unchanged = render(&engine.prepare_output_frame(Default::default()));
    assert!(Arc::ptr_eq(&next.origins, &unchanged.origins));
}
