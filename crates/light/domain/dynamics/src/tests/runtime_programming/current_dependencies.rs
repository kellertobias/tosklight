use super::*;
use light_core::programming::{
    ProgrammingFieldScope, ProgrammingFieldTransfer, ProgrammingTraceField,
};
use std::cell::RefCell;

struct EvidenceSources {
    value: Cell<f32>,
    dependency: RefCell<DynamicSourceDependency>,
    reads: Cell<usize>,
}
impl DynamicValueSourceResolver for EvidenceSources {
    fn current(&self, _: FixtureId, address: &DynamicValueAddress) -> Option<DynamicValue> {
        assert_eq!(address.component, Some(ProgrammingComponent::Focus));
        self.reads.set(self.reads.get() + 1);
        Some(DynamicValue::Scalar(self.value.get()))
    }
    fn current_dependency(&self, _: FixtureId, _: &DynamicValueAddress) -> DynamicSourceDependency {
        self.dependency.borrow().clone()
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

fn dependencies(expression: &DynamicSampleExpression) -> Vec<DynamicSourceDependency> {
    let tape = RetainedExpressionTape::from_roots(&[Arc::new(expression.clone())]).unwrap();
    tape.nodes
        .iter()
        .filter_map(|node| match node {
            RetainedExpressionNode::Programming {
                dependency_occurrence,
                ..
            } => dependency_occurrence.clone(),
            _ => None,
        })
        .collect()
}

fn sample_evidence(
    runtime: &mut DynamicRuntime,
    instance: Uuid,
    at: u64,
    sources: &EvidenceSources,
) -> DynamicSampleExpression {
    runtime
        .sample_programming(instance, at, 1000, 10, &Sources { current: 0.99 }, sources)
        .unwrap()
        .remove(0)
        .expression
}

#[test]
fn used_current_dependency_survives_pause_tree_and_tape_reload_and_interrupted_resume() {
    for anonymous_unknown in [false, true] {
        for tree_snapshot in [false, true] {
            let original = if anonymous_unknown {
                DynamicSourceDependency::unknown(None)
            } else {
                DynamicSourceDependency::identity(Some(
                    DynamicSourceOccurrenceId::new(Uuid::from_u128(901)).unwrap(),
                ))
            };
            let sources = EvidenceSources {
                value: Cell::new(0.2),
                dependency: RefCell::new(original.clone()),
                reads: Cell::new(0),
            };
            let address = DynamicValueAddress {
                representation: DynamicFamilyRepresentation::Focus,
                component: Some(ProgrammingComponent::Focus),
            };
            let lane = ramp(
                address,
                DynamicValueSource::Current,
                DynamicValueSource::Current,
            );
            let definition = definition(lane);
            let target = FixtureId::new();
            let control = controller(1, 1, false);
            let mut runtime = DynamicRuntime::default();
            runtime.install_definitions([definition.clone()]).unwrap();
            let mut request = start_request(definition.id, control.clone(), target, 0, false);
            request.activation_policy_override = Some(ActivationPolicy::JoinSyncNow);
            request.activation_duration_millis = 1000;
            let instance = runtime.start(request).unwrap();
            assert_eq!(
                dependencies(&sample_evidence(&mut runtime, instance, 200, &sources)),
                vec![original.clone()]
            );
            runtime
                .set_controller_paused(instance, control.id, true, 200)
                .unwrap();
            let held = sample_evidence(&mut runtime, instance, 200, &sources);
            assert_eq!(dependencies(&held), vec![original.clone()]);
            let mut snapshot = runtime.snapshot();
            if tree_snapshot {
                for instance in &mut snapshot.instances {
                    let tape = instance.expression_tape.take().unwrap();
                    for row in instance
                        .last_sample_values
                        .iter_mut()
                        .chain(&mut instance.synchronized_hold_values)
                    {
                        let DynamicHeldPayload::TapeRoot { tape_root } = row.payload else {
                            panic!()
                        };
                        row.payload = DynamicHeldPayload::Expression {
                            expression: DynamicSampleExpression::Retained {
                                tape: tape.clone(),
                                root: tape_root,
                            }
                            .shallow()
                            .unwrap(),
                        };
                    }
                }
            }
            let snapshot = serde_json::from_value(serde_json::to_value(snapshot).unwrap()).unwrap();
            let mut restored = DynamicRuntime::default();
            restored.restore_snapshot(snapshot).unwrap();
            sources.value.set(0.8);
            let next = if anonymous_unknown {
                DynamicSourceDependency::unknown(None)
            } else {
                DynamicSourceDependency::identity(Some(
                    DynamicSourceOccurrenceId::new(Uuid::from_u128(902)).unwrap(),
                ))
            };
            *sources.dependency.borrow_mut() = next.clone();
            let before_reads = sources.reads.get();
            let still_held = sample_evidence(&mut restored, instance, 400, &sources);
            assert_eq!(dependencies(&still_held), vec![original.clone()]);
            assert_eq!(
                sources.reads.get(),
                before_reads,
                "held Current must not resolve another frame"
            );
            restored
                .set_controller_paused_with_resume(
                    instance,
                    control.id,
                    false,
                    500,
                    Some(ActivationPolicy::JoinSyncNow),
                )
                .unwrap();
            let resumed = sample_evidence(&mut restored, instance, 1000, &sources);
            assert_eq!(
                dependencies(&resumed),
                vec![original.clone(), next.clone()],
                "even anonymous unknown dependencies prevent numeric-only history collapse"
            );
            restored
                .set_controller_paused(instance, control.id, true, 1000)
                .unwrap();
            let interrupted = sample_evidence(&mut restored, instance, 1000, &sources);
            assert_eq!(dependencies(&interrupted), vec![original, next]);
            let roundtrip =
                serde_json::from_value(serde_json::to_value(restored.snapshot()).unwrap()).unwrap();
            restored.restore_snapshot(roundtrip).unwrap();
            assert_eq!(
                dependencies(&sample_evidence(&mut restored, instance, 1200, &sources)),
                dependencies(&interrupted)
            );
        }
    }
}

#[test]
fn invalid_dependency_owner_in_retained_checkpoint_rejects_atomically() {
    let sources = EvidenceSources {
        value: Cell::new(0.2),
        dependency: RefCell::new(DynamicSourceDependency::identity(None)),
        reads: Cell::new(0),
    };
    let definition = definition(ramp(
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Focus,
            component: Some(ProgrammingComponent::Focus),
        },
        DynamicValueSource::Current,
        DynamicValueSource::Current,
    ));
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition.clone()]).unwrap();
    let instance = runtime
        .start(start_request(
            definition.id,
            controller(1, 1, false),
            FixtureId::new(),
            0,
            false,
        ))
        .unwrap();
    sample_evidence(&mut runtime, instance, 200, &sources);
    let valid = runtime.snapshot();
    let mut invalid = valid.clone();
    let tape = Arc::make_mut(invalid.instances[0].expression_tape.as_mut().unwrap());
    for node in &mut tape.nodes {
        if let RetainedExpressionNode::Programming {
            dependency_occurrence,
            ..
        } = node
        {
            *dependency_occurrence = Some(DynamicSourceDependency::mapped(
                None,
                ProgrammingFieldTransfer {
                    identity: ProgrammingFieldScope::empty(),
                    remap: vec![(ProgrammingTraceField::Pan, ProgrammingTraceField::Focus)].into(),
                },
            ));
        }
    }
    assert!(runtime.restore_snapshot(invalid).is_err());
    assert_eq!(runtime.snapshot(), valid);
}
