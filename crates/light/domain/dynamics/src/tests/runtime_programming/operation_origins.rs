//! TL-613: genuine producer-operation identity through sampling, retained tapes and checkpoints.
//! Every expression here comes from the real sampler; no origin is constructed by the test.
use super::*;
use crate::{
    DynamicControllerSizeRole as Role, DynamicHeldPayload, DynamicOperationCorrespondence,
    DynamicOperationHandle, DynamicOperationMismatch, DynamicOperationSite as Site,
    FamilyCompositionSample, RETAINED_OPERATION_TAPE_VERSION, RetainedExpressionTape,
    bundle_position_component_forest, normalize_legacy_programmer_controller_ids,
    programmer_dynamic_controller_id,
};
use light_core::{ProgrammerId, programming::ProgrammingOwner};
use serde_json::Value;

const HEADS: [FixtureId; 2] = [
    FixtureId(Uuid::from_u128(61_301)),
    FixtureId(Uuid::from_u128(61_302)),
];
const KEYFRAME: Site = Site::KeyframeTransition { segment_index: 0 };
const SIZE: Site = Site::ControllerSize {
    role: Role::FamilyScale,
};
const SHARED_LIVE: DynamicOperationCorrespondence =
    DynamicOperationCorrespondence::Shared { historical: false };
const SHARED_HISTORICAL: DynamicOperationCorrespondence =
    DynamicOperationCorrespondence::Shared { historical: true };
const INDEPENDENT: DynamicOperationCorrespondence =
    DynamicOperationCorrespondence::Uncorrelated(DynamicOperationMismatch::DifferentEmission);

fn target_value(reference: TargetReference) -> DynamicValue {
    DynamicValue::Family(AttributeValue::Position(Arc::new(PositionIntent::target(
        reference,
        [1.0, 2.0, 3.0],
    ))))
}

/// Origin -> Point keyframes: every sampled frame needs a Required reference transition.
fn required_definition() -> DynamicDefinition {
    let lane = DynamicLane {
        id: Uuid::from_u128(61_310),
        body: DynamicLaneBody::Programming(ProgrammingLaneBody {
            address: DynamicValueAddress {
                representation: DynamicFamilyRepresentation::Target { reference: None },
                component: None,
            },
            configuration: ProgrammingLaneConfiguration::Keyframes(KeyframeConfiguration {
                points: vec![
                    DynamicKeyframe {
                        position: 0.0,
                        source: value(target_value(TargetReference::Origin)),
                        interpolation: ScalarInterpolation::Linear,
                    },
                    DynamicKeyframe {
                        position: 0.5,
                        source: value(target_value(TargetReference::Point {
                            point_id: Uuid::from_u128(9),
                        })),
                        interpolation: ScalarInterpolation::Linear,
                    },
                ],
                size: 1.0,
            }),
        }),
        ..lane()
    };
    let mut definition = definition(lane);
    definition.id = Uuid::from_u128(61_311);
    // Both heads stay in segment 0 at different per-target progress.
    definition.phase.span_degrees = 36.0;
    definition
}

struct Rig {
    runtime: DynamicRuntime,
    instance: Uuid,
    controller: DynamicController,
    sources: TypedSources,
}

fn rig(definition: &DynamicDefinition, size: f32, resume_millis: u64) -> Rig {
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition.clone()]).unwrap();
    let mut controller = controller(1, 1, false);
    controller.size = size;
    let mut request = start_request(definition.id, controller.clone(), HEADS[0], 0, false);
    request.target_scope.ordered_targets = HEADS.to_vec();
    request.activation_policy_override = Some(ActivationPolicy::JoinSyncNow);
    request.activation_duration_millis = resume_millis;
    let instance = runtime.start(request).unwrap();
    Rig {
        runtime,
        instance,
        controller,
        sources: TypedSources {
            current: Some(target_value(TargetReference::Origin)),
            preset: None,
            calls: Cell::new(0),
        },
    }
}

impl Rig {
    fn sample(&mut self, at: u64) -> Vec<DynamicRuntimeSample> {
        let mut samples = sampled(&mut self.runtime, self.instance, at, &self.sources);
        samples.sort_by_key(|sample| sample.target.0);
        samples
    }
    fn pause(&mut self, at: u64) {
        self.runtime
            .set_controller_paused(self.instance, self.controller.id, true, at)
            .unwrap();
    }
}

fn handles(expression: &DynamicSampleExpression) -> Vec<DynamicOperationHandle> {
    let provenance = expression.operation_provenance().unwrap();
    assert!(
        provenance.is_complete(),
        "every Required/Size operation keeps its genuine origin"
    );
    provenance.handles().to_vec()
}

fn at(handles: &[DynamicOperationHandle], site: Site) -> &DynamicOperationHandle {
    handles
        .iter()
        .find(|handle| handle.site() == site)
        .unwrap_or_else(|| panic!("missing {site:?} in {handles:?}"))
}

/// Every keyed row of both held maps, bound to the instance's one shared tape.
fn rows(snapshot: &DynamicRuntimeSnapshot) -> Vec<(FixtureId, DynamicSampleExpression)> {
    let instance = &snapshot.instances[0];
    let tape = instance.expression_tape.as_ref().expect("shared tape");
    instance
        .last_sample_values
        .iter()
        .chain(&instance.synchronized_hold_values)
        .map(|row| {
            let DynamicHeldPayload::TapeRoot { tape_root } = row.payload else {
                panic!("checkpoint rows retain tape roots")
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

fn assert_all(
    rows: &[(FixtureId, DynamicSampleExpression)],
    expected: DynamicOperationCorrespondence,
) {
    for site in [KEYFRAME, SIZE] {
        let first = handles(&rows[0].1);
        for (_, row) in rows {
            assert_eq!(
                at(&first, site).correspondence(at(&handles(row), site)),
                expected
            );
        }
    }
}

fn progress(expression: &DynamicSampleExpression) -> f32 {
    let DynamicSampleExpression::Scale { value, .. } = expression.unannotated() else {
        panic!("controller Size wraps the Required transition")
    };
    let DynamicSampleExpression::Transition { progress, .. } = value.unannotated() else {
        panic!("Required keyframe transition")
    };
    *progress
}

fn paused_rig() -> (Rig, Vec<DynamicRuntimeSample>) {
    let mut rig = rig(&required_definition(), 2.0, 0);
    let fresh = rig.sample(250);
    rig.pause(250);
    rig.sample(450);
    (rig, fresh)
}

#[test]
fn two_heads_share_one_genuine_witness_through_clone_tape_both_maps_and_json_restore() {
    let definition = required_definition();
    let mut rig = rig(&definition, 2.0, 0);
    let fresh = rig.sample(250);
    assert_eq!(fresh.len(), 2);
    let heads = fresh
        .iter()
        .map(|sample| handles(&sample.expression))
        .collect::<Vec<_>>();
    for (handles, target) in heads.iter().zip(HEADS) {
        assert_eq!(handles.len(), 2, "one keyframe and one Size site per head");
        for handle in handles {
            assert_eq!(handle.target(), target);
            assert_eq!(handle.lane_id(), definition.lanes[0].id);
            assert!(!handle.is_historical());
            let emission = handle.emission();
            assert_eq!(emission.instance_id(), rig.instance);
            assert_eq!(emission.targets(), HEADS, "ordered pinned membership");
            assert_eq!(emission.controller(), &rig.controller);
            assert_eq!(emission.definition(), &definition);
        }
    }
    // Per-target progress is a sampling parameter, never identity.
    assert_ne!(
        progress(&fresh[0].expression),
        progress(&fresh[1].expression)
    );
    for site in [KEYFRAME, SIZE] {
        assert_eq!(
            at(&heads[0], site).correspondence(at(&heads[1], site)),
            SHARED_LIVE
        );
    }
    assert_eq!(
        at(&heads[0], KEYFRAME).correspondence(at(&heads[1], SIZE)),
        DynamicOperationCorrespondence::Uncorrelated(DynamicOperationMismatch::DifferentSite)
    );

    // Cloning and joint tape import keep the one original object.
    let cloned = fresh[1].expression.clone();
    assert_eq!(
        at(&handles(&cloned), SIZE).correspondence(at(&heads[0], SIZE)),
        SHARED_LIVE
    );
    let tape = Arc::new(
        RetainedExpressionTape::from_roots(&[
            Arc::new(fresh[0].expression.clone()),
            Arc::new(fresh[1].expression.clone()),
        ])
        .unwrap(),
    );
    assert_eq!(tape.version, RETAINED_OPERATION_TAPE_VERSION);
    assert_eq!(tape.operation_emission_count(), 1);
    let imported = DynamicSampleExpression::Retained {
        tape: Arc::clone(&tape),
        root: tape.roots[1],
    };
    assert_eq!(imported, fresh[1].expression);
    assert_eq!(
        at(&handles(&imported), KEYFRAME).correspondence(at(&heads[0], KEYFRAME)),
        SHARED_LIVE
    );

    // Indefinite pause: both keyed maps hold the same original witness without resampling it.
    rig.pause(250);
    for at_millis in [450, 5_000, 60_000] {
        let held = rig.sample(at_millis);
        for (held, fresh) in held.iter().zip(&fresh) {
            assert_eq!(held.expression, fresh.expression);
            assert_eq!(
                at(&handles(&held.expression), SIZE).correspondence(at(&heads[0], SIZE)),
                SHARED_LIVE,
                "a held frame never stamps retained operations with a fresh emission"
            );
        }
    }
    let snapshot = rig.runtime.snapshot();
    let instance = &snapshot.instances[0];
    assert_eq!(instance.last_sample_values.len(), 2);
    assert_eq!(instance.synchronized_hold_values.len(), 2);
    assert_eq!(
        instance
            .expression_tape
            .as_ref()
            .unwrap()
            .operation_emission_count(),
        1,
        "two heads and both held maps reference one object-table entry"
    );
    let live_rows = rows(&snapshot);
    assert_eq!(live_rows.len(), 4);
    assert_all(&live_rows, SHARED_LIVE);
    assert_eq!(
        at(&handles(&live_rows[0].1), SIZE).correspondence(at(&heads[1], SIZE)),
        SHARED_LIVE
    );

    let json = serde_json::to_string(&snapshot).unwrap();
    assert_eq!(
        json.matches("\"cycle_duration_millis\"").count(),
        1,
        "each witness object is serialized once"
    );
    let mut restored = DynamicRuntime::default();
    restored
        .restore_snapshot(serde_json::from_str(&json).unwrap())
        .unwrap();
    assert!(
        restored.committed_sample_boundary().is_none(),
        "a restored witness carries no frame capture or sample boundary"
    );
    let restored_rows = rows(&restored.snapshot());
    assert_all(&restored_rows, SHARED_HISTORICAL);
    for (target, row) in &restored_rows {
        let handles = handles(row);
        assert!(handles.iter().all(|handle| handle.is_historical()));
        assert!(handles.iter().all(|handle| handle.target() == *target));
        assert_eq!(handles[0].emission().definition(), &definition);
        assert_eq!(
            at(&handles, SIZE).correspondence(at(&heads[0], SIZE)),
            INDEPENDENT,
            "a decoded object is never the live object"
        );
    }
    // The first paused frame after restore keeps the pre-checkpoint ordinary result and only
    // historical operations.
    let resumed = sampled(&mut restored, rig.instance, 90_000, &rig.sources);
    assert_eq!(resumed.len(), 2);
    for sample in &resumed {
        let original = fresh.iter().find(|f| f.target == sample.target).unwrap();
        assert_eq!(sample.expression, original.expression);
        assert!(
            handles(&sample.expression)
                .iter()
                .all(DynamicOperationHandle::is_historical)
        );
        assert_eq!(
            at(&handles(&sample.expression), KEYFRAME)
                .correspondence(at(&handles(&restored_rows[0].1), KEYFRAME)),
            SHARED_HISTORICAL
        );
    }
}

#[test]
fn equal_independent_emissions_frames_entries_and_decodes_never_correlate() {
    let definition = required_definition();
    let mut first = rig(&definition, 2.0, 0);
    let mut second = rig(&definition, 2.0, 0);
    let a = first.sample(250);
    let b = second.sample(250);
    assert_eq!(
        a[0].expression, b[0].expression,
        "equal values, rank and progress"
    );
    assert_eq!(a[0].controller_id, b[0].controller_id);
    for site in [KEYFRAME, SIZE] {
        assert_eq!(
            at(&handles(&a[0].expression), site)
                .correspondence(at(&handles(&b[0].expression), site)),
            INDEPENDENT
        );
    }
    // A later genuine frame of the same controller is a different emission.
    let later = first.sample(260);
    assert_eq!(
        at(&handles(&a[0].expression), SIZE)
            .correspondence(at(&handles(&later[1].expression), SIZE)),
        INDEPENDENT
    );

    // Separately decoded snapshots remain independent; one decode stays shared.
    first.pause(260);
    first.sample(300);
    let json = serde_json::to_value(first.runtime.snapshot()).unwrap();
    let decode = |json: &Value| {
        let mut runtime = DynamicRuntime::default();
        runtime
            .restore_snapshot(serde_json::from_value(json.clone()).unwrap())
            .unwrap();
        rows(&runtime.snapshot())
    };
    let (one, two) = (decode(&json), decode(&json));
    assert_all(&one, SHARED_HISTORICAL);
    assert_eq!(
        at(&handles(&one[0].1), SIZE).correspondence(at(&handles(&two[0].1), SIZE)),
        INDEPENDENT
    );

    // Two equal table entries are two objects: the heads no longer correspond.
    let mut split = json.clone();
    let tape = &mut split["instances"][0]["expression_tape"];
    let entry = tape["emissions"][0].clone();
    tape["emissions"].as_array_mut().unwrap().push(entry);
    let head = serde_json::to_value(HEADS[1]).unwrap();
    for reference in tape["operations"].as_array_mut().unwrap() {
        if reference["target"] == head {
            reference["emission"] = 1.into();
        }
    }
    let split = decode(&split);
    let (first_head, second_head) = (
        split
            .iter()
            .find(|(target, _)| *target == HEADS[0])
            .unwrap(),
        split
            .iter()
            .find(|(target, _)| *target == HEADS[1])
            .unwrap(),
    );
    assert_eq!(
        handles(&first_head.1)[0].emission(),
        handles(&second_head.1)[0].emission()
    );
    assert_eq!(
        at(&handles(&first_head.1), SIZE).correspondence(at(&handles(&second_head.1), SIZE)),
        INDEPENDENT
    );
}

#[test]
fn malformed_operation_references_reject_atomically_before_installing_state() {
    let (mut rig, _) = paused_rig();
    let valid = serde_json::to_value(rig.runtime.snapshot()).unwrap();
    let before = serde_json::to_string(&rig.runtime.snapshot()).unwrap();
    let programming_node = valid["instances"][0]["expression_tape"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .position(|node| node["kind"] == "programming")
        .unwrap();
    let mutations: Vec<(&str, Box<dyn Fn(&mut Value)>)> = vec![
        (
            "absent emission",
            Box::new(|tape| tape["operations"][0]["emission"] = 7.into()),
        ),
        (
            "site on another node kind",
            Box::new(move |tape| tape["operations"][0]["node"] = programming_node.into()),
        ),
        (
            "non-member target",
            Box::new(|tape| {
                tape["operations"][0]["target"] =
                    serde_json::to_value(FixtureId(Uuid::from_u128(1))).unwrap()
            }),
        ),
        (
            "absent keyframe segment",
            Box::new(|tape| {
                for reference in tape["operations"].as_array_mut().unwrap() {
                    if reference["site"]["kind"] == "keyframe_transition" {
                        reference["site"]["segment_index"] = 5.into();
                    }
                }
            }),
        ),
        (
            "absent lane",
            Box::new(|tape| {
                tape["operations"][0]["lane_id"] =
                    serde_json::to_value(Uuid::from_u128(404)).unwrap()
            }),
        ),
        (
            "unreferenced duplicate entry",
            Box::new(|tape| {
                let entry = tape["emissions"][0].clone();
                tape["emissions"].as_array_mut().unwrap().push(entry);
            }),
        ),
        (
            "foreign instance",
            Box::new(|tape| {
                tape["emissions"][0]["instance_id"] =
                    serde_json::to_value(Uuid::from_u128(405)).unwrap()
            }),
        ),
        (
            "foreign controller",
            Box::new(|tape| {
                tape["emissions"][0]["controller"]["id"] =
                    serde_json::to_value(Uuid::from_u128(406)).unwrap()
            }),
        ),
        (
            "member target of another row",
            Box::new(|tape| {
                for reference in tape["operations"].as_array_mut().unwrap() {
                    let swapped = if reference["target"] == serde_json::to_value(HEADS[0]).unwrap()
                    {
                        HEADS[1]
                    } else {
                        HEADS[0]
                    };
                    reference["target"] = serde_json::to_value(swapped).unwrap();
                }
            }),
        ),
        (
            "table in a version 3 tape",
            Box::new(|tape| tape["version"] = 3.into()),
        ),
        (
            "duplicated reference",
            Box::new(|tape| {
                let reference = tape["operations"][0].clone();
                tape["operations"]
                    .as_array_mut()
                    .unwrap()
                    .insert(0, reference);
            }),
        ),
    ];
    for (name, mutate) in mutations {
        let mut snapshot = valid.clone();
        mutate(&mut snapshot["instances"][0]["expression_tape"]);
        let result = serde_json::from_value::<DynamicRuntimeSnapshot>(snapshot)
            .map_err(|error| error.to_string())
            .and_then(|snapshot| {
                rig.runtime
                    .restore_snapshot(snapshot)
                    .map_err(|error| error.to_string())
            });
        assert!(result.is_err(), "{name} must be rejected");
        assert_eq!(
            serde_json::to_string(&rig.runtime.snapshot()).unwrap(),
            before,
            "{name} must not install any restored state"
        );
    }
    rig.runtime
        .restore_snapshot(serde_json::from_value(valid).unwrap())
        .unwrap();
}

#[test]
fn hot_edited_held_history_keeps_the_original_pinned_definition_evidence() {
    let original = required_definition();
    let (mut rig, fresh) = paused_rig();
    let mut edited = original.clone();
    edited.revision += 1;
    let DynamicLaneBody::Programming(body) = &mut edited.lanes[0].body else {
        unreachable!()
    };
    let ProgrammingLaneConfiguration::Keyframes(keyframes) = &mut body.configuration else {
        unreachable!()
    };
    keyframes.points[1].position = 0.75;
    keyframes.points[1].source = value(target_value(TargetReference::Point {
        point_id: Uuid::from_u128(10),
    }));
    rig.runtime.install_definitions([edited.clone()]).unwrap();
    let held = rig.sample(700);
    assert_eq!(held[0].expression, fresh[0].expression);
    let snapshot = rig.runtime.snapshot();
    assert_eq!(snapshot.instances[0].definition, edited);
    let restored = {
        let mut runtime = DynamicRuntime::default();
        runtime
            .restore_snapshot(
                serde_json::from_str(&serde_json::to_string(&snapshot).unwrap()).unwrap(),
            )
            .unwrap();
        runtime.snapshot()
    };
    for rows in [rows(&snapshot), rows(&restored)] {
        for (_, row) in rows {
            let handles = handles(&row);
            assert_eq!(
                handles[0].emission().definition(),
                &original,
                "the old segment remains valid only against its pinned evidence"
            );
        }
    }
}

#[test]
fn programmer_identity_rekey_rewrites_the_shared_witness_once_for_both_maps_and_heads() {
    let (rig, _) = paused_rig();
    let programmer = ProgrammerId(Uuid::from_u128(101));
    let normalized_id = programmer_dynamic_controller_id(programmer, rig.controller.id);
    // In-memory capture shares the live object: normalization copies it on write.
    for mut snapshot in [
        rig.runtime.snapshot(),
        serde_json::from_str(&serde_json::to_string(&rig.runtime.snapshot()).unwrap()).unwrap(),
    ] {
        assert_eq!(
            normalize_legacy_programmer_controller_ids(
                &mut snapshot,
                &[(programmer, rig.controller.id)]
            )
            .unwrap(),
            1
        );
        let tape = snapshot.instances[0].expression_tape.as_ref().unwrap();
        assert_eq!(tape.operation_emission_count(), 1);
        let mut restored = DynamicRuntime::default();
        restored.restore_snapshot(snapshot).unwrap();
        let rows = rows(&restored.snapshot());
        assert_eq!(rows.len(), 4);
        assert_all(&rows, SHARED_HISTORICAL);
        for (target, row) in &rows {
            for handle in handles(row) {
                let controller = handle.emission().controller();
                assert_eq!(controller.id, normalized_id);
                assert_eq!(
                    controller.source,
                    DynamicControllerSource::Programmer {
                        programmer_id: programmer.0,
                        instance_link: Some(rig.controller.id),
                    }
                );
                assert_eq!(handle.emission().targets(), HEADS);
                assert_eq!(handle.target(), *target);
            }
        }
    }
    // The live runtime's own object is untouched by cold normalization.
    for (_, row) in rows(&rig.runtime.snapshot()) {
        assert_eq!(
            handles(&row)[0].emission().controller().id,
            rig.controller.id
        );
    }
}

#[test]
fn synchronized_resume_and_released_member_compaction_preserve_original_identity() {
    let definition = required_definition();
    let mut rig = rig(&definition, 2.0, 1_000);
    let fresh = rig.sample(250);
    rig.pause(250);
    rig.sample(300);
    rig.runtime
        .set_controller_paused_with_resume(
            rig.instance,
            rig.controller.id,
            false,
            400,
            Some(ActivationPolicy::JoinSyncNow),
        )
        .unwrap();
    let resumed = rig.sample(650);
    let mut held = Vec::new();
    for sample in &resumed {
        let DynamicSampleExpression::Transition {
            from: Some(from),
            to: Some(to),
            reason: DynamicTransitionReason::Resume { .. },
            ..
        } = sample.expression.unannotated()
        else {
            panic!(
                "synchronized resume keeps both branches: {:?}",
                sample.expression
            )
        };
        let (old, new) = (handles(from), handles(to));
        assert_eq!(
            at(&old, SIZE).correspondence(at(&handles(&fresh[0].expression), SIZE)),
            SHARED_LIVE,
            "the held branch keeps its original emission"
        );
        assert_eq!(
            at(&old, SIZE).correspondence(at(&new, SIZE)),
            INDEPENDENT,
            "a historical emission never corresponds to the resumed frame"
        );
        held.push(old);
    }
    assert_eq!(
        at(&held[0], KEYFRAME).correspondence(at(&held[1], KEYFRAME)),
        SHARED_LIVE
    );

    // Releasing head 1 compacts its root away; head 2 keeps the same object and membership.
    let mut tape = RetainedExpressionTape::from_roots(&[
        Arc::new(fresh[0].expression.clone()),
        Arc::new(fresh[1].expression.clone()),
    ])
    .unwrap();
    tape.roots = vec![tape.roots[1]];
    assert!(tape.compact_reachable().unwrap() > 0);
    assert_eq!(tape.operation_emission_count(), 1);
    let survivor = DynamicSampleExpression::Retained {
        root: tape.roots[0],
        tape: Arc::new(tape),
    };
    let survivor = handles(&survivor);
    assert_eq!(survivor[0].emission().targets(), HEADS);
    assert!(survivor.iter().all(|handle| handle.target() == HEADS[1]));
    assert_eq!(
        at(&survivor, SIZE).correspondence(at(&handles(&fresh[0].expression), SIZE)),
        SHARED_LIVE
    );
    // Releasing every operation leaves an ordinary version 3 tape with no table.
    let mut released = RetainedExpressionTape::from_roots(&[
        Arc::new(fresh[0].expression.clone()),
        Arc::new(DynamicSampleExpression::Programming {
            address: Arc::new(pan_address()),
            value: DynamicValue::Scalar(1.0),
            occurrence: None,
            dependency_occurrence: None,
        }),
    ])
    .unwrap();
    released.roots = vec![released.roots[1]];
    released.compact_reachable().unwrap();
    assert_eq!(released.operation_emission_count(), 0);
    assert_eq!(released.version, crate::RETAINED_EXPRESSION_TAPE_VERSION);
    released.validate().unwrap();
}

#[test]
fn legacy_tapes_without_metadata_stay_explicitly_uncorrelated_and_evaluate_unchanged() {
    let (rig, fresh) = paused_rig();
    let mut json = serde_json::to_value(rig.runtime.snapshot()).unwrap();
    let tape = json["instances"][0]["expression_tape"]
        .as_object_mut()
        .unwrap();
    tape.remove("emissions");
    tape.remove("operations");
    tape.insert("version".into(), 3.into());
    let mut restored = DynamicRuntime::default();
    restored
        .restore_snapshot(serde_json::from_value(json).unwrap())
        .unwrap();
    let samples = sampled(&mut restored, rig.instance, 2_000, &rig.sources);
    for sample in &samples {
        let original = fresh.iter().find(|f| f.target == sample.target).unwrap();
        assert_eq!(sample.expression, original.expression);
        let provenance = sample.expression.operation_provenance().unwrap();
        assert!(provenance.handles().is_empty());
        assert_eq!(
            provenance.unattributed_operations(),
            2,
            "the Required transition and the Size node remain explicitly unattributed"
        );
    }
}

struct NumericSources;
impl DynamicValueSourceResolver for NumericSources {
    fn try_position_current_family(
        &self,
        _: FixtureId,
        _: &DynamicValueAddress,
    ) -> Result<Option<AttributeValue>, light_core::programming::TransitionError> {
        Ok(Some(AttributeValue::Position(Arc::new(
            PositionIntent::angles(10.0, 20.0),
        ))))
    }
    fn current(&self, _: FixtureId, address: &DynamicValueAddress) -> Option<DynamicValue> {
        Some(match address.component {
            Some(ProgrammingComponent::Pan) => DynamicValue::Scalar(10.0),
            Some(ProgrammingComponent::Tilt) => DynamicValue::Scalar(20.0),
            _ => DynamicValue::Family(AttributeValue::Position(Arc::new(PositionIntent::angles(
                10.0, 20.0,
            )))),
        })
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

fn sample_numeric(
    runtime: &mut DynamicRuntime,
    instance: Uuid,
    at: u64,
) -> Vec<DynamicRuntimeSample> {
    let mut samples = runtime
        .sample_programming(
            instance,
            at,
            1000,
            10,
            &Sources { current: 0.99 },
            &NumericSources,
        )
        .unwrap();
    samples.sort_by_key(|sample| (sample.target.0, sample.lane_id));
    samples
}

#[test]
fn optimized_numeric_angle_size_and_keyframe_sites_survive_forest_and_checkpoint() {
    let pan = DynamicLane {
        id: Uuid::from_u128(61_320),
        body: DynamicLaneBody::Programming(ProgrammingLaneBody {
            address: pan_address(),
            configuration: ProgrammingLaneConfiguration::Keyframes(KeyframeConfiguration {
                points: vec![
                    DynamicKeyframe {
                        position: 0.0,
                        source: DynamicValueSource::Current,
                        interpolation: ScalarInterpolation::Linear,
                    },
                    DynamicKeyframe {
                        position: 0.5,
                        source: value(DynamicValue::Scalar(90.0)),
                        interpolation: ScalarInterpolation::Linear,
                    },
                ],
                size: 1.0,
            }),
        }),
        ..lane()
    };
    let mut definition = definition(pan);
    definition.id = Uuid::from_u128(61_321);
    definition.phase.span_degrees = 36.0;
    let mut rig = rig(&definition, 0.5, 0);
    let numeric = |samples: &[DynamicRuntimeSample]| {
        samples
            .iter()
            .filter(|sample| sample.lane_id == Uuid::from_u128(61_320))
            .cloned()
            .collect::<Vec<_>>()
    };
    let all = sample_numeric(&mut rig.runtime, rig.instance, 250);
    let fresh = numeric(&all);
    assert_eq!(fresh.len(), 2);
    let numeric_sites = [
        Site::KeyframeTransition { segment_index: 0 },
        Site::ControllerSize {
            role: Role::AngleNumericScaleFrom,
        },
    ];
    let heads = fresh
        .iter()
        .map(|sample| {
            let DynamicSampleExpression::AngleNumeric { program } = &sample.expression else {
                panic!("the optimized numeric path stays enabled and unwrapped")
            };
            let handles = handles(&sample.expression);
            assert_eq!(handles.len(), 2);
            for handle in &handles {
                let node = handle.program_node().unwrap() as usize;
                assert!(matches!(
                    (handle.site(), &program.nodes[node]),
                    (
                        Site::KeyframeTransition { .. },
                        AngleNumericNode::Transition { .. }
                    ) | (
                        Site::ControllerSize { .. },
                        AngleNumericNode::ScaleFrom { .. }
                    )
                ));
            }
            handles
        })
        .collect::<Vec<_>>();
    for site in numeric_sites {
        assert_eq!(
            at(&heads[0], site).correspondence(at(&heads[1], site)),
            SHARED_LIVE
        );
    }

    // Genuine forest rebundling keeps the retained source lane's original origins.
    let first_head = all
        .iter()
        .filter(|sample| sample.target == HEADS[0])
        .cloned()
        .collect::<Vec<_>>();
    let bundle = bundle_position_component_forest(&first_head, &NumericSources).unwrap();
    let Some(FamilyCompositionSample::CoupledExpression { expression, .. }) = bundle.position
    else {
        panic!("complete Angle pair")
    };
    let lanes = expression.position_operation_provenance().unwrap().unwrap();
    let (_, provenance) = lanes
        .iter()
        .find(|(lane, _)| *lane == Uuid::from_u128(61_320))
        .unwrap();
    for site in numeric_sites {
        assert_eq!(
            at(provenance.handles(), site).correspondence(at(&heads[1], site)),
            SHARED_LIVE
        );
    }

    // The numeric checkpoint carrier keeps the same authority without resampling it.
    rig.pause(250);
    let held = numeric(&sample_numeric(&mut rig.runtime, rig.instance, 800));
    assert_eq!(held[0].expression, fresh[0].expression);
    let snapshot = rig.runtime.snapshot();
    assert_eq!(
        snapshot.instances[0]
            .expression_tape
            .as_ref()
            .unwrap()
            .operation_emission_count(),
        1
    );
    let mut restored = DynamicRuntime::default();
    restored
        .restore_snapshot(serde_json::from_str(&serde_json::to_string(&snapshot).unwrap()).unwrap())
        .unwrap();
    let after = numeric(&sample_numeric(&mut restored, rig.instance, 9_000));
    assert_eq!(after.len(), 2);
    let restored_heads = after
        .iter()
        .map(|sample| {
            let DynamicSampleExpression::AngleNumeric { .. } = sample.expression.shallow().unwrap()
            else {
                panic!("restored history stays a numeric program")
            };
            handles(&sample.expression)
        })
        .collect::<Vec<_>>();
    for site in numeric_sites {
        assert_eq!(
            at(&restored_heads[0], site).correspondence(at(&restored_heads[1], site)),
            SHARED_HISTORICAL
        );
        assert_eq!(
            at(&restored_heads[0], site).correspondence(at(&heads[0], site)),
            INDEPENDENT
        );
    }
}

#[test]
fn owner_splitting_carries_the_original_operation_without_inventing_one() {
    let (_, fresh) = paused_rig();
    let original = handles(&fresh[0].expression);
    let focus = AttributeValue::Normalized(0.3);
    let mut mixed = fresh[0].clone();
    mixed.expression = DynamicSampleExpression::Transition {
        from: Some(Arc::new(fresh[0].expression.clone())),
        to: Some(Arc::new(DynamicSampleExpression::Programming {
            address: Arc::new(
                DynamicValueAddress::whole_family(ProgrammingOwner::Focus, &focus).unwrap(),
            ),
            value: DynamicValue::Family(focus),
            occurrence: None,
            dependency_occurrence: None,
        })),
        progress: 0.5,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(61_330),
        },
    };
    let sources = TypedSources {
        current: Some(target_value(TargetReference::Origin)),
        preset: None,
        calls: Cell::new(0),
    };
    let mut scratch = DynamicFamilyPreparationScratch::default();
    let prepared =
        prepare_dynamic_family_samples(std::slice::from_ref(&mixed), &sources, None, &mut scratch)
            .unwrap();
    let mut owners = Vec::new();
    for group in prepared.families {
        let mut attributed = Vec::new();
        for sample in &group.samples {
            match sample {
                FamilyCompositionSample::WholeExpression { expression, .. } => {
                    attributed.extend(handles(expression.expression()))
                }
                FamilyCompositionSample::CoupledExpression { expression, .. } => {
                    if let Some(lanes) = expression.position_operation_provenance() {
                        for (_, provenance) in lanes.unwrap() {
                            assert!(provenance.is_complete());
                            attributed.extend(provenance.handles().iter().cloned());
                        }
                    } else if let Some(expression) = expression.expression() {
                        attributed.extend(handles(expression));
                    }
                }
                FamilyCompositionSample::Known(_) => {}
            }
        }
        owners.push((group.owner, attributed));
    }
    let position = &owners
        .iter()
        .find(|(owner, _)| *owner == ProgrammingOwner::Position)
        .unwrap()
        .1;
    assert!(
        !position.is_empty(),
        "the Position part keeps its operations"
    );
    for handle in position {
        assert_eq!(
            handle.correspondence(at(&original, handle.site())),
            SHARED_LIVE,
            "owner splitting carries the original witness"
        );
    }
    let focus = &owners
        .iter()
        .find(|(owner, _)| *owner == ProgrammingOwner::Focus)
        .unwrap()
        .1;
    assert!(
        focus.is_empty(),
        "synthetic owner parts receive no invented event"
    );
}
