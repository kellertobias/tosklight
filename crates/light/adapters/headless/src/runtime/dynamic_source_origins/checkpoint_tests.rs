use super::*;
use light_core::{AttributeKey, AttributeValue};
use light_dynamics::{
    DynamicController, DynamicControllerSource, DynamicFamilyRepresentation,
    DynamicHeldSampleSnapshot, DynamicInstanceSnapshot, DynamicTransitionReason, DynamicValue,
    DynamicValueAddress,
};

fn authored(
    catalogue: &mut DynamicSourceOrigins,
    target: FixtureId,
) -> (DynamicSourceOccurrenceId, DynamicSourceBinding) {
    let origin = DynamicSourceOrigin::Programmer {
        programmer_id: ProgrammerId(Uuid::from_u128(77)),
        lane: DynamicProgrammerSourceLane::Live,
        instance_link: Uuid::new_v4(),
        changed_at_millis: 100,
        programmer_order: 3,
    };
    let key = DynamicSourceBinding::Authored {
        instance_id: Uuid::new_v4(),
        controller_id: origin.authored_controller_id().unwrap(),
        target,
        lane_id: Uuid::new_v4(),
    };
    (catalogue.bind(key, origin).unwrap(), key)
}

fn baseline(
    catalogue: &mut DynamicSourceOrigins,
    target: FixtureId,
    owner: ProgrammingOwner,
) -> DynamicSourceOccurrenceId {
    catalogue
        .bind(
            DynamicSourceBinding::StaticBaseline { target, owner },
            DynamicSourceOrigin::StaticBaseline {
                sources: vec![DynamicStaticSourceEntry {
                    source: DynamicStaticSource::Programmer {
                        programmer_id: ProgrammerId::new(),
                        lane: DynamicStaticProgrammerLane::Live,
                    },
                    changed_at: DateTime::from_timestamp(1, 456_123_789).unwrap(),
                    programmer_order: 1,
                    transition_ordinal: None,
                    authored_cue_id: None,
                    footprint: DynamicStaticFootprint::Whole,
                    role: DynamicStaticRole::Authored,
                    effective_fields: None,
                }],
            },
        )
        .unwrap()
}

fn leaf(id: Option<DynamicSourceOccurrenceId>) -> DynamicSampleExpression {
    DynamicSampleExpression::LegacyScalar {
        attribute: AttributeKey::intensity(),
        value: 0.5,
        occurrence: id,
        dependency_occurrence: None,
    }
}

fn runtime(
    key: DynamicSourceBinding,
    expression: DynamicSampleExpression,
) -> DynamicRuntimeSnapshot {
    let DynamicSourceBinding::Authored {
        instance_id,
        controller_id,
        target,
        lane_id,
    } = key
    else {
        panic!()
    };
    let pwm = serde_json::to_value(light_dynamics::PwmShape::default()).unwrap();
    let definition = serde_json::from_value(serde_json::json!({
        "id": Uuid::new_v4(), "pool_number":1, "revision":1, "name":"Origin checkpoint",
        "target_binding":{"type":"targetless"},
        "lanes":[{
            "id":lane_id, "attribute":"intensity", "mode":"keyframes",
            "keyframes":{"points":[
                {"position":0.0,"source":{"type":"value","value":0.0},"interpolation":"linear"},
                {"position":0.5,"source":{"type":"value","value":1.0},"interpolation":"linear"}
            ], "size":1.0},
            "max_min":{"minimum":{"type":"value","value":0.0},"maximum":{"type":"value","value":1.0},"function":"sinus","size":1.0,"pwm":pwm.clone()},
            "middle_amplitude":{"middle":{"type":"current"},"amplitude":0.5,"function":"sinus","size":1.0,"pwm":pwm},
            "speed_multiplier":{"numerator":1,"denominator":1},"width":1.0
        }],
        "phase":{"ordering":{"type":"selection"},"offset_degrees":0.0,"span_degrees":360.0,"block_size":1,"repeats":1,"wings":false,"anchors_degrees":[]},
        "speed":{"type":"fixed","duration_millis":1000}, "default_activation":"start_now"
    })).unwrap();
    let row = DynamicHeldSampleSnapshot {
        controller_id,
        target,
        lane_id,
        payload: DynamicHeldPayload::Expression { expression },
    };
    DynamicRuntimeSnapshot {
        global_paused: true,
        instances: vec![DynamicInstanceSnapshot {
            id: instance_id,
            definition,
            targets: vec![target],
            phase_by_target: vec![],
            phase_by_lane_target: vec![],
            controllers: vec![DynamicController {
                id: controller_id,
                source: DynamicControllerSource::Programmer {
                    programmer_id: Uuid::from_u128(77),
                    instance_link: None,
                },
                priority: 100,
                activated_at_millis: 100,
                size: 1.0,
                speed_multiplier: 1.0,
                phase_offset_degrees: 0.0,
                paused: false,
            }],
            lane_selections: vec![],
            controller_transitions: vec![],
            started_at_millis: 100,
            paused_at_millis: None,
            paused_elapsed_millis: 0,
            activation_policy: light_dynamics::ActivationPolicy::StartNow,
            pending_until_millis: None,
            speed_paused_at_millis: None,
            speed_paused_elapsed_millis: 0,
            random_streams: vec![],
            completed: false,
            synchronized_hold_elapsed_millis: Some(0),
            synchronized_hold_captured: true,
            last_synchronized_elapsed_millis: Some(0),
            synchronized_resume_transition: None,
            last_sample_values: vec![row.clone()],
            synchronized_hold_values: vec![row],
            expression_tape: None,
            preset_source_values: vec![],
        }],
    }
}

fn pack(snapshot: &mut DynamicRuntimeSnapshot) {
    let instance = &mut snapshot.instances[0];
    let expressions = instance
        .last_sample_values
        .iter()
        .chain(&instance.synchronized_hold_values)
        .map(|row| match &row.payload {
            DynamicHeldPayload::Expression { expression } => Arc::new(expression.clone()),
            _ => panic!(),
        })
        .collect::<Vec<_>>();
    let tape = Arc::new(RetainedExpressionTape::from_roots(&expressions).unwrap());
    for (row, root) in instance
        .last_sample_values
        .iter_mut()
        .chain(&mut instance.synchronized_hold_values)
        .zip(&tape.roots)
    {
        row.payload = DynamicHeldPayload::TapeRoot { tape_root: *root };
    }
    instance.expression_tape = Some(tape);
}

#[test]
fn checkpoint_roundtrip_validates_tree_and_tape_origins_together() {
    let mut origins = DynamicSourceOrigins::default();
    let (id, key) = authored(&mut origins, FixtureId::new());
    let mut snapshot = runtime(key, leaf(Some(id)));
    for packed in [false, true] {
        if packed {
            pack(&mut snapshot);
        }
        let checkpoint =
            DynamicRuntimeSourceCheckpoint::capture(snapshot.clone(), &origins).unwrap();
        let encoded = serde_json::to_vec(&checkpoint).unwrap();
        let decoded: DynamicRuntimeSourceCheckpoint = serde_json::from_slice(&encoded).unwrap();
        let (restored, catalogue) = decoded.restore().unwrap();
        assert_eq!(restored, snapshot);
        assert_eq!(catalogue.snapshot(), origins.snapshot());
        let mut missing = checkpoint;
        missing.origins = None;
        assert!(
            missing.restore().is_err(),
            "known occurrence IDs cannot degrade to Unknown"
        );
    }
}

#[test]
fn absent_origins_remain_unknown_for_legacy_numbers_and_unannotated_expressions() {
    let mut unused = DynamicSourceOrigins::default();
    let (_, key) = authored(&mut unused, FixtureId::new());
    let mut snapshot = runtime(key, leaf(None));
    let (restored, origins) = DynamicRuntimeSourceCheckpoint {
        runtime: snapshot.clone(),
        origins: None,
    }
    .restore()
    .unwrap();
    assert_eq!(restored, snapshot);
    assert!(origins.snapshot().records.is_empty());
    pack(&mut snapshot);
    Arc::make_mut(snapshot.instances[0].expression_tape.as_mut().unwrap()).version = 1;
    assert!(
        DynamicRuntimeSourceCheckpoint {
            runtime: snapshot.clone(),
            origins: None
        }
        .restore()
        .is_ok()
    );
    snapshot.instances[0].expression_tape = None;
    let instance = &mut snapshot.instances[0];
    for row in instance
        .last_sample_values
        .iter_mut()
        .chain(&mut instance.synchronized_hold_values)
    {
        row.payload = DynamicHeldPayload::Legacy { value: 0.5 };
    }
    assert!(
        DynamicRuntimeSourceCheckpoint {
            runtime: snapshot,
            origins: None
        }
        .restore()
        .is_ok()
    );
}

#[test]
fn checkpoint_rejects_foreign_target_but_preserves_original_historical_lane_and_source() {
    let mut origins = DynamicSourceOrigins::default();
    let target = FixtureId::new();
    let (old, old_key) = authored(&mut origins, target);
    let (new, new_key) = authored(&mut origins, target);
    assert_ne!(old_key, new_key);
    let expression = DynamicSampleExpression::Transition {
        from: Some(Arc::new(leaf(Some(old)))),
        to: Some(Arc::new(leaf(Some(new)))),
        progress: 0.25,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::new_v4(),
        },
    };
    let mut snapshot = runtime(new_key, expression);
    pack(&mut snapshot);
    assert!(
        origins.validate_runtime(&snapshot).is_ok(),
        "history must keep old lane/controller/instance provenance"
    );
    snapshot.instances[0].synchronized_hold_values[0].target = FixtureId::new();
    assert!(origins.validate_runtime(&snapshot).is_err());
}

#[test]
fn dependency_roles_and_typed_baseline_owner_are_validated() {
    let mut origins = DynamicSourceOrigins::default();
    let target = FixtureId::new();
    let (id, key) = authored(&mut origins, target);
    let focus = baseline(&mut origins, target, ProgrammingOwner::Focus);
    let color = baseline(&mut origins, target, ProgrammingOwner::Color);
    let address = Arc::new(DynamicValueAddress {
        representation: DynamicFamilyRepresentation::Focus,
        component: None,
    });
    let whole = |dependency| DynamicSampleExpression::Scale {
        address: address.clone(),
        base: DynamicValue::Family(AttributeValue::Normalized(0.1)),
        factor: 1.5,
        value: Arc::new(DynamicSampleExpression::Programming {
            address: address.clone(),
            value: DynamicValue::Family(AttributeValue::Normalized(0.7)),
            occurrence: Some(id),
            dependency_occurrence: None,
        }),
        baseline_occurrence: Some(dependency),
    };
    assert!(
        origins
            .validate_runtime(&runtime(key, whole(focus)))
            .is_ok()
    );
    assert!(
        origins
            .validate_runtime(&runtime(key, whole(color)))
            .is_err()
    );
    assert!(
        origins.validate_runtime(&runtime(key, whole(id))).is_err(),
        "authorship cannot substitute for baseline evidence"
    );
    assert!(
        origins
            .validate_runtime(&runtime(key, leaf(Some(focus))))
            .is_err(),
        "baseline evidence cannot become authorship"
    );
    let foreign = baseline(&mut origins, FixtureId::new(), ProgrammingOwner::Focus);
    assert!(
        origins
            .validate_runtime(&runtime(key, whole(foreign)))
            .is_err()
    );
}

#[test]
fn both_maps_and_nested_inactive_history_require_known_origins() {
    let mut origins = DynamicSourceOrigins::default();
    let (id, key) = authored(&mut origins, FixtureId::new());
    let unknown = DynamicSourceOccurrenceId::new(Uuid::new_v4()).unwrap();
    let mut snapshot = runtime(key, leaf(Some(id)));
    snapshot.instances[0].synchronized_hold_values[0].payload = DynamicHeldPayload::Expression {
        expression: DynamicSampleExpression::Transition {
            from: Some(Arc::new(leaf(Some(unknown)))),
            to: Some(Arc::new(leaf(Some(id)))),
            progress: 1.0,
            reason: DynamicTransitionReason::Resume {
                occurrence_id: Uuid::new_v4(),
            },
        },
    };
    assert!(
        origins.validate_runtime(&snapshot).is_err(),
        "stored inactive history can become participating after resume edits"
    );
    pack(&mut snapshot);
    assert!(origins.validate_runtime(&snapshot).is_err());
    let before = origins.snapshot();
    assert!(origins.prune_runtime(&snapshot).is_err());
    assert_eq!(origins.snapshot(), before);
}

#[test]
fn shared_tape_is_walked_once_across_many_rows_and_prune_retains_only_reachable_history() {
    let mut origins = DynamicSourceOrigins::default();
    let target = FixtureId::new();
    let (id, key) = authored(&mut origins, target);
    let (obsolete, obsolete_key) = authored(&mut origins, target);
    origins.unbind(&obsolete_key);
    let mut tape = RetainedExpressionTape::from_roots(&[Arc::new(leaf(Some(id)))]).unwrap();
    let mut root = tape.roots[0];
    for _ in 0..128 {
        let next = RetainedNodeId(tape.nodes.len() as u32);
        tape.nodes.push(RetainedExpressionNode::Transition {
            from: Some(root),
            to: Some(root),
            progress: 0.5,
            reason: DynamicTransitionReason::Resume {
                occurrence_id: Uuid::new_v4(),
            },
        });
        root = next;
    }
    tape.roots = vec![root];
    let unique_nodes = tape.nodes.len();
    let mut snapshot = runtime(key, leaf(Some(id)));
    let instance = &mut snapshot.instances[0];
    instance.expression_tape = Some(Arc::new(tape));
    let first = instance.last_sample_values[0].clone();
    let lane = instance.definition.lanes[0].clone();
    instance.definition.lanes.clear();
    instance.last_sample_values.clear();
    for _ in 0..256 {
        let mut lane = lane.clone();
        lane.id = Uuid::new_v4();
        instance.last_sample_values.push(DynamicHeldSampleSnapshot {
            lane_id: lane.id,
            payload: DynamicHeldPayload::TapeRoot { tape_root: root },
            ..first.clone()
        });
        instance.definition.lanes.push(lane);
    }
    instance.synchronized_hold_values = instance.last_sample_values.clone();
    let reachable = origins.runtime_reachability(&snapshot).unwrap();
    assert_eq!(
        reachable.visited_nodes, unique_nodes,
        "512 row references must not expand the shared diamond graph"
    );
    assert_eq!(reachable.ids, HashSet::from([id]));
    assert_eq!(origins.prune_runtime(&snapshot).unwrap(), 1);
    assert!(origins.get(obsolete).is_none());
    assert!(origins.get(id).is_some());
}

#[test]
fn held_tape_root_requires_its_matching_tape_and_declared_root() {
    let mut origins = DynamicSourceOrigins::default();
    let (id, key) = authored(&mut origins, FixtureId::new());
    let mut snapshot = runtime(key, leaf(Some(id)));
    pack(&mut snapshot);
    let retained = snapshot.instances[0].expression_tape.take();
    assert!(origins.validate_runtime(&snapshot).is_err());
    snapshot.instances[0].expression_tape = retained;
    Arc::make_mut(snapshot.instances[0].expression_tape.as_mut().unwrap())
        .roots
        .clear();
    assert!(origins.validate_runtime(&snapshot).is_err());
}

/// TL-613: producer-operation witnesses through the actual persisted checkpoint and the
/// restored-candidate path. Every origin comes from the real sampler.
mod operation_witnesses {
    use super::*;
    use crate::runtime::output_scheduler::prepare_restored_dynamic_candidate;
    use light_core::programming::{PositionIntent, TargetReference};
    use light_dynamics::{
        ActivationBoundary, ActivationPolicy, DynamicDefinition, DynamicKeyframe, DynamicLane,
        DynamicLaneBody, DynamicOperationCorrespondence, DynamicOperationSite,
        DynamicPhaseSpreadMode, DynamicPresetSourceBinding, DynamicRunMode, DynamicRuntime,
        DynamicSpatialMappingOverride, DynamicSpeed, DynamicStartRequest, DynamicTargetBinding,
        DynamicTargetScope, DynamicValueSource, DynamicValueSourceResolver, KeyframeConfiguration,
        PhaseDistribution, PhaseOrdering, ProgrammingLaneBody, ProgrammingLaneConfiguration,
        Rational, ScalarInterpolation, ScalarSourceResolver,
    };

    const HEADS: [FixtureId; 2] = [
        FixtureId(Uuid::from_u128(61_401)),
        FixtureId(Uuid::from_u128(61_402)),
    ];

    fn target(reference: TargetReference) -> DynamicValue {
        DynamicValue::Family(AttributeValue::Position(Arc::new(PositionIntent::target(
            reference,
            [1.0, 2.0, 3.0],
        ))))
    }

    struct Frame;
    impl ScalarSourceResolver for Frame {
        fn current(&self, _: FixtureId, _: &AttributeKey) -> Option<f32> {
            None
        }
        fn preset(&self, _: &str, _: FixtureId, _: &AttributeKey) -> Option<f32> {
            None
        }
    }
    impl DynamicValueSourceResolver for Frame {
        fn current(&self, _: FixtureId, _: &DynamicValueAddress) -> Option<DynamicValue> {
            Some(target(TargetReference::Origin))
        }
        fn preset(
            &self,
            _: &DynamicPresetSourceBinding,
            _: Uuid,
            _: FixtureId,
        ) -> Option<DynamicValue> {
            None
        }
    }

    /// Two heads, Origin -> Point keyframes (Required) at controller Size 2, paused forever.
    fn paused() -> (DynamicRuntime, Vec<light_dynamics::DynamicRuntimeSample>) {
        let keyframe = |position, reference| DynamicKeyframe {
            position,
            source: DynamicValueSource::Value {
                value: target(reference),
            },
            interpolation: ScalarInterpolation::Linear,
        };
        let definition = DynamicDefinition {
            id: Uuid::from_u128(61_410),
            pool_number: 1,
            revision: 1,
            name: "Required Size".into(),
            color: None,
            icon: None,
            target_binding: DynamicTargetBinding::Targetless,
            lanes: vec![DynamicLane {
                id: Uuid::from_u128(61_411),
                body: DynamicLaneBody::Programming(ProgrammingLaneBody {
                    address: DynamicValueAddress {
                        representation: DynamicFamilyRepresentation::Target { reference: None },
                        component: None,
                    },
                    configuration: ProgrammingLaneConfiguration::Keyframes(KeyframeConfiguration {
                        points: vec![
                            keyframe(0.0, TargetReference::Origin),
                            keyframe(
                                0.5,
                                TargetReference::Point {
                                    point_id: Uuid::from_u128(9),
                                },
                            ),
                        ],
                        size: 1.0,
                    }),
                }),
                speed_multiplier: Rational::ONE,
                width: 1.0,
                phase: None,
                random_group_id: None,
            }],
            random_groups: vec![],
            phase_spread_mode: DynamicPhaseSpreadMode::Uniform,
            spatial_mapping: DynamicSpatialMappingOverride::default(),
            phase: PhaseDistribution {
                ordering: PhaseOrdering::Selection,
                offset_degrees: 0.0,
                span_degrees: 36.0,
                block_size: 1,
                repeats: 1,
                wings: false,
                anchors_degrees: vec![],
            },
            speed: DynamicSpeed::Fixed {
                duration_millis: 1_000,
            },
            overall_speed_multiplier: Rational::ONE,
            run_mode: DynamicRunMode::Loop,
            default_activation: ActivationPolicy::StartNow,
            activation_boundary: ActivationBoundary::Beat,
        };
        let controller = DynamicController {
            id: Uuid::from_u128(61_412),
            source: DynamicControllerSource::physical_playback(1),
            priority: 1,
            activated_at_millis: 0,
            size: 2.0,
            speed_multiplier: 1.0,
            phase_offset_degrees: 0.0,
            paused: false,
        };
        let mut runtime = DynamicRuntime::default();
        runtime.install_definitions([definition.clone()]).unwrap();
        let instance = runtime
            .start(DynamicStartRequest {
                definition_id: definition.id,
                controller: controller.clone(),
                target_scope: DynamicTargetScope {
                    ordered_targets: HEADS.to_vec(),
                },
                stage_positions: HashMap::new(),
                inherited_spatial_mapping: None,
                now_millis: 0,
                activation_delay_millis: 0,
                activation_duration_millis: 0,
                activation_policy_override: Some(ActivationPolicy::JoinSyncNow),
                reuse_matching_targetless: false,
            })
            .unwrap();
        let sample = |runtime: &mut DynamicRuntime, at| {
            let mut samples = runtime
                .sample_programming(instance, at, 1_000, 10, &Frame, &Frame)
                .unwrap();
            samples.sort_by_key(|sample| sample.target.0);
            samples
        };
        let fresh = sample(&mut runtime, 250);
        runtime
            .set_controller_paused(instance, controller.id, true, 250)
            .unwrap();
        let held = sample(&mut runtime, 86_400_000);
        assert_eq!(held, fresh, "an indefinite pause keeps the ordinary result");
        (runtime, fresh)
    }

    fn rows(snapshot: &DynamicRuntimeSnapshot) -> Vec<(FixtureId, DynamicSampleExpression)> {
        let instance = &snapshot.instances[0];
        let tape = instance.expression_tape.as_ref().unwrap();
        assert_eq!(tape.operation_emission_count(), 1);
        instance
            .last_sample_values
            .iter()
            .chain(&instance.synchronized_hold_values)
            .map(|row| {
                let DynamicHeldPayload::TapeRoot { tape_root } = row.payload else {
                    panic!("tape root")
                };
                (
                    row.target,
                    DynamicSampleExpression::Retained {
                        tape: Arc::clone(tape),
                        root: tape_root,
                    },
                )
            })
            .collect()
    }

    #[test]
    fn paused_two_head_required_size_history_restores_shared_handles_through_candidate_path() {
        let (live, fresh) = paused();
        let stored = live.snapshot();
        let checkpoint =
            DynamicRuntimeSourceCheckpoint::capture(stored, &DynamicSourceOrigins::default())
                .unwrap();
        let checkpoint: DynamicRuntimeSourceCheckpoint =
            serde_json::from_str(&serde_json::to_string(&checkpoint).unwrap()).unwrap();
        let candidate = prepare_restored_dynamic_candidate(
            &light_engine::EngineSnapshot::default(),
            &DynamicRuntime::default(),
            checkpoint,
        )
        .unwrap();
        assert!(
            candidate.runtime.committed_sample_boundary().is_none(),
            "restore creates no sample-boundary or solver-frame authority"
        );
        let restored = rows(&candidate.runtime.snapshot());
        assert_eq!(restored.len(), 4, "two heads in both held maps");
        let reference = restored[0]
            .1
            .operation_provenance()
            .unwrap()
            .handles()
            .to_vec();
        for (head, row) in &restored {
            let original = &fresh.iter().find(|sample| sample.target == *head).unwrap();
            assert_eq!(row, &original.expression, "pre-checkpoint ordinary result");
            let provenance = row.operation_provenance().unwrap();
            assert!(provenance.is_complete());
            assert_eq!(provenance.handles().len(), 2);
            for handle in provenance.handles() {
                assert!(handle.is_historical());
                assert_eq!(handle.target(), *head);
                assert_eq!(handle.emission().targets(), HEADS);
                let same_site = reference
                    .iter()
                    .find(|other| other.site() == handle.site())
                    .unwrap();
                assert_eq!(
                    handle.correspondence(same_site),
                    DynamicOperationCorrespondence::Shared { historical: true }
                );
            }
            assert!(provenance.handles().iter().any(|handle| matches!(
                handle.site(),
                DynamicOperationSite::KeyframeTransition { segment_index: 0 }
            )));
        }
        // The live runtime and its objects are untouched by the detached candidate.
        let live_rows = rows(&live.snapshot());
        let live_handle = live_rows[0].1.operation_provenance().unwrap().handles()[0].clone();
        assert!(!live_handle.is_historical());
        assert!(matches!(
            live_handle.correspondence(&reference[0]),
            DynamicOperationCorrespondence::Uncorrelated(_)
        ));
    }

    #[test]
    fn malformed_operation_tables_reject_the_checkpoint_pair_atomically() {
        let (live, _) = paused();
        let valid = serde_json::to_value(
            DynamicRuntimeSourceCheckpoint::capture(
                live.snapshot(),
                &DynamicSourceOrigins::default(),
            )
            .unwrap(),
        )
        .unwrap();
        let base = live.fork_for_cold_install();
        let before = serde_json::to_string(&base.snapshot()).unwrap();
        for mutate in [
            |tape: &mut serde_json::Value| tape["operations"][0]["emission"] = 9.into(),
            |tape: &mut serde_json::Value| {
                tape["operations"][0]["target"] =
                    serde_json::to_value(FixtureId(Uuid::from_u128(1))).unwrap()
            },
            |tape: &mut serde_json::Value| {
                tape["emissions"][0]["controller"]["id"] =
                    serde_json::to_value(Uuid::from_u128(2)).unwrap()
            },
        ] {
            let mut checkpoint = valid.clone();
            mutate(&mut checkpoint["runtime"]["instances"][0]["expression_tape"]);
            let checkpoint: DynamicRuntimeSourceCheckpoint =
                serde_json::from_value(checkpoint).unwrap();
            assert!(
                prepare_restored_dynamic_candidate(
                    &light_engine::EngineSnapshot::default(),
                    &base,
                    checkpoint,
                )
                .is_err()
            );
            assert_eq!(serde_json::to_string(&base.snapshot()).unwrap(), before);
        }
    }
}
