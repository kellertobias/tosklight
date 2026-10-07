//! Actual controller Size then partial activation: the envelope's incoming replay needs Size.
//! A complete fixed peer differs from static Current, and each physical copy has its own oracle.
use super::super::super::cut_coordinator::operation::{inspect_attempt, with_environment_limit};
use super::*;
use light_dynamics::{
    DynamicControllerSizeRole, DynamicOperationSite, RetainedExpressionNode, RetainedExpressionTape,
};

struct Oracle {
    baseline: HashMap<FixtureId, [f64; 2]>,
    peer: HashMap<FixtureId, [f64; 2]>,
    endpoint: [f64; 2],
}
fn oracle(desk: &EnvelopeDesk) -> Oracle {
    let resolved = desk.shared.rig.resolve(&[
        (desk.shared.heads[0], desk.shared.targets[0].clone()),
        (desk.shared.heads[1], desk.fixed.clone()),
    ]);
    let mut baseline = HashMap::new();
    let mut peer = HashMap::new();
    assert_eq!(
        resolved.results.len(),
        2,
        "one ordered resolution for each requested owner"
    );
    let snapshot = desk.shared.rig.engine.snapshot();
    let profile = snapshot.fixtures[0]
        .definition
        .profile_snapshot
        .as_ref()
        .unwrap();
    for (index, row) in resolved.results.iter().enumerate() {
        let expected = if index == 0 {
            &desk.shared.targets[0]
        } else {
            &desk.fixed
        };
        let AttributeValue::Position(expected) = expected else {
            panic!("Position oracle request")
        };
        assert!(
            row.requested == *expected.as_ref(),
            "ordered result preserves its original requested intent"
        );
        let pairs = if index == 0 { &mut baseline } else { &mut peer };
        assert_eq!(
            row.achieved.outcomes.len(),
            2,
            "one physical outcome per root and copy"
        );
        for outcome in &row.achieved.outcomes {
            assert_eq!(
                outcome.result.emitter_id, profile.geometry.emitters[index].id,
                "each result retains its actual logical emitter"
            );
            assert!(
                outcome.destination == desk.shared.rig.root || outcome.destination == desk.copy
            );
            assert_eq!(outcome.result.status, PositionFitStatus::Fitted);
            assert!(!outcome.input_requirement && !outcome.missing_mount);
            assert!(
                pairs
                    .insert(outcome.destination, outcome.result.achieved.unwrap())
                    .is_none(),
                "no duplicate physical destination"
            );
        }
    }
    assert_eq!(baseline.len(), 2);
    assert_eq!(peer.len(), 2);
    assert!((baseline[&desk.copy][1] - baseline[&desk.shared.rig.root][1]).abs() > 1.);
    assert!((peer[&desk.copy][1] - peer[&desk.shared.rig.root][1]).abs() > 1.);
    assert!(
        (peer[&desk.shared.rig.root][1] - f64::from(desk.shared.angles[1][1]) - 8.).abs() < 0.05
    );
    Oracle {
        baseline,
        peer,
        endpoint: [
            f64::from(desk.shared.angles[0][0]),
            f64::from(desk.shared.angles[0][1]) + 10.,
        ],
    }
}
fn verify(
    desk: &EnvelopeDesk,
    oracle: &Oracle,
    capture: &PreparedOutputFrame,
    output: &PublishedPhysicalFrame<PositionAdapter>,
) {
    assert!(
        output.requirements.is_empty(),
        "real Size and the following original activation must both complete"
    );
    assert_eq!(output.results.len(), 2);
    let sample = output
        .sampled
        .samples
        .iter()
        .find(|sample| sample.target == desk.shared.heads[0])
        .unwrap();
    let runtime = desk.runtime.snapshot();
    assert_eq!(runtime.instances.len(), 1);
    let instance = &runtime.instances[0];
    assert_eq!(instance.targets, vec![desk.shared.heads[0]]);
    assert_eq!(instance.controllers.len(), 1);
    assert_eq!(
        (sample.instance_id, sample.controller_id),
        (instance.id, instance.controllers[0].id)
    );
    assert_eq!(instance.controllers[0].size, 0.5);
    assert_eq!(
        desk.definition.revision, 1,
        "no fabricated or hot-edited graph is necessary"
    );
    assert!(instance.synchronized_resume_transition.is_none());
    assert!(sample.activation_mix > 0. && sample.activation_mix < 1.);
    let mix = f64::from(sample.activation_mix);
    let provenance = sample.expression.operation_provenance().unwrap();
    assert!(provenance.is_complete());
    let handles = provenance
        .handles()
        .iter()
        .filter(|handle| {
            matches!(
                handle.site(),
                DynamicOperationSite::ControllerSize {
                    role: DynamicControllerSizeRole::FamilyScale
                }
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(handles.len(), 1);
    let handle = handles[0];
    assert_eq!(handle.target(), desk.shared.heads[0]);
    assert_eq!(handle.lane_id(), sample.lane_id);
    assert_eq!(handle.emission().instance_id(), instance.id);
    assert_eq!(
        handle.emission().controller().id,
        instance.controllers[0].id
    );
    assert_eq!(handle.emission().controller().size, 0.5);
    assert_eq!(handle.emission().definition().revision, 1);
    assert_eq!(handle.emission().targets(), instance.targets);
    let tape = RetainedExpressionTape::from_roots(&[Arc::new(sample.expression.clone())]).unwrap();
    let factors = tape
        .nodes
        .iter()
        .filter_map(|node| match node {
            RetainedExpressionNode::Scale { factor, .. } => Some(*factor),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(factors, vec![0.5]);
    let mut claims = HashMap::new();
    for (index, head) in desk.shared.heads.into_iter().enumerate() {
        let row = output
            .results
            .iter()
            .find(|row| row.target == head)
            .unwrap();
        assert!(!row.quality.held);
        let original = program(row);
        assert_eq!(original.base, desk.shared.targets[index]);
        assert_eq!(original.samples.len(), 1);
        if index == 0 {
            assert!(
                matches!(
                    &original.samples[0],
                    FamilyCompositionSample::WholeExpression { .. }
                        | FamilyCompositionSample::CoupledExpression { .. }
                ),
                "genuine retained Size, not a fabricated Known sample"
            );
        } else {
            let FamilyCompositionSample::Known(fixed) = &original.samples[0] else {
                panic!("independently constant fixed peer")
            };
            assert_eq!(fixed.activation_mix, 1.);
            assert_eq!(
                fixed.materialized_value(),
                Some(&DynamicValue::Family(desk.fixed.clone()))
            );
        }
        assert_eq!(row.achieved.destinations.len(), 2);
        for destination in [desk.shared.rig.root, desk.copy] {
            let computed = &row
                .achieved
                .destinations
                .iter()
                .find(|value| value.destination == destination)
                .unwrap()
                .value;
            let outcome = row
                .achieved
                .outcomes
                .iter()
                .find(|outcome| outcome.destination == destination)
                .unwrap();
            assert_eq!(outcome.result.status, PositionFitStatus::Fitted);
            let pair = if index == 0 {
                commanded_angles(computed)
            } else {
                assert_eq!(
                    computed, &desk.fixed,
                    "constant peer retains its actual Target intent"
                );
                assert_eq!(&row.value, &desk.fixed);
                outcome
                    .result
                    .achieved
                    .expect("fixed Target has a complete fitted command pair")
            };
            let expected: [f64; 2] = if index == 0 {
                std::array::from_fn(|axis| {
                    let base = oracle.baseline[&destination][axis];
                    let sized = base + 0.5 * (oracle.endpoint[axis] - base);
                    base + mix * (sized - base)
                })
            } else {
                oracle.peer[&destination]
            };
            for axis in 0..2 {
                assert!(
                    (pair[axis] - expected[axis]).abs() < 0.07,
                    "per-copy Size then activation exactly once, with actual fixed peer: {pair:?} != {expected:?}"
                );
            }
            let physical = output
                .rendered
                .physical
                .instances
                .iter()
                .find(|instance| instance.instance_id == destination.0)
                .unwrap();
            assert!(physical.complete);
            assert!((physical.axes()[0].absolute_degrees().unwrap() - pair[0]).abs() < 0.03);
            assert!(
                (physical.axes()[index + 1].absolute_degrees().unwrap() - pair[1]).abs() < 0.03
            );
            for write in row
                .writes
                .iter()
                .filter(|write| write.slot.destination == destination)
            {
                assert!(!write.parked);
                if let Some(previous) =
                    claims.insert((destination, write.slot.channel_index), write.raw)
                {
                    assert_eq!(previous, write.raw, "both owners agree on shared Pan");
                }
            }
        }
    }
    assert_eq!(claims.len(), 6);
    for (destination, start) in [(desk.shared.rig.root, 0), (desk.copy, 19)] {
        let physical = output
            .rendered
            .physical
            .instances
            .iter()
            .find(|instance| instance.instance_id == destination.0)
            .unwrap();
        for channel in 0..3u32 {
            let slot = start + channel as usize * 2;
            let bytes = &output.rendered.universes[&1];
            let wire = u32::from(u16::from_be_bytes([bytes[slot], bytes[slot + 1]]));
            assert_eq!(wire, claims[&(destination, channel)]);
            assert_eq!(physical.native_raw[channel as usize], wire);
        }
    }
    assert_eq!(output.token, capture.frame_token());
}
#[test]
fn actual_size_prefix_then_activation_replays_nested_graph_with_fixed_peer_per_copy() {
    let mut desk = EnvelopeDesk::new_configured(false, 0.5, false, true);
    let oracle = oracle(&desk);
    let ((capture, output), evidence) = inspect_attempt(|| desk.tick());
    assert_eq!(
        evidence.completed, 1,
        "heterogeneous consumer, not a synchronous shortcut: {evidence:?}"
    );
    assert!(
        evidence.endpoint_cohorts >= 4,
        "actual graph plus activation endpoint environments: {evidence:?}"
    );
    verify(&desk, &oracle, &capture, &output);
}
#[test]
fn graph_activation_budget_refusal_leaves_accepted_continuity_and_recovers_same_workspace() {
    let mut desk = EnvelopeDesk::new_configured(false, 0.5, false, true);
    let oracle = oracle(&desk);
    let (_, accepted) = desk.tick();
    assert!(accepted.requirements.is_empty());
    let continuity = desk
        .shared
        .heads
        .map(|head| desk.lane.continuity(head, ProgrammingOwner::Position));
    assert!(continuity.iter().all(Option::is_some));
    let (capture, held) = with_environment_limit(1, || desk.tick());
    assert!(!held.requirements.is_empty());
    assert!(
        held.results
            .iter()
            .all(|row| row.target != desk.shared.heads[0]),
        "unresolved changing owner cannot publish a partial result"
    );
    for row in &held.results {
        assert_eq!(
            row.target, desk.shared.heads[1],
            "only the independently constant peer can retain a passive row"
        );
        assert!(
            row.quality.held,
            "shared mechanical protection must reach the constant peer"
        );
        assert_eq!(
            row.value, desk.fixed,
            "passive peer preserves its authored Target"
        );
        assert!(!row.writes.is_empty());
        assert!(
            row.writes.iter().all(|write| write.parked),
            "a passive row must emit no fragment of the shared mechanical controls"
        );
        assert_eq!(row.achieved.outcomes.len(), 2);
        assert!(
            row.achieved
                .outcomes
                .iter()
                .all(|outcome| outcome.input_requirement)
        );
    }
    for (index, head) in desk.shared.heads.into_iter().enumerate() {
        assert_eq!(
            desk.lane.continuity(head, ProgrammingOwner::Position),
            continuity[index],
            "bounded refusal must not accept speculative continuity for owner {index}"
        );
    }
    let baseline = desk
        .shared
        .rig
        .engine
        .preview_static_family_frame(
            &capture,
            desk.shared
                .rig
                .engine
                .prepare_static_family_frame(&capture, &[]),
        )
        .unwrap();
    assert_eq!(held.rendered.universes, baseline.universes);
    for expected in &baseline.physical.instances {
        let actual = held
            .rendered
            .physical
            .instances
            .iter()
            .find(|instance| instance.instance_id == expected.instance_id)
            .unwrap();
        assert_eq!(actual.native_raw, expected.native_raw);
        assert_eq!(actual.complete, expected.complete);
    }
    let ((capture, output), evidence) = inspect_attempt(|| desk.tick());
    assert_eq!(evidence.completed, 1);
    verify(&desk, &oracle, &capture, &output);
}

#[test]
fn first_protected_frame_cannot_create_owner_continuity_and_next_frame_recovers() {
    let mut desk = EnvelopeDesk::new_configured(false, 0.5, false, true);
    let oracle = oracle(&desk);
    // Keep the real runtime/controller and captured authorship; only this output lane has
    // never accepted either physical owner. A parked peer is not a first accepted pose.
    desk.lane = PhysicalAdapterLane::live(PositionAdapter::default());
    for head in desk.shared.heads {
        assert!(
            desk.lane
                .continuity(head, ProgrammingOwner::Position)
                .is_none()
        );
    }
    let (capture, held) = with_environment_limit(1, || desk.tick());
    assert!(!held.requirements.is_empty());
    assert_eq!(
        held.results.len(),
        1,
        "exercise the actual protected known peer row"
    );
    assert!(
        held.results
            .iter()
            .all(|row| row.target != desk.shared.heads[0])
    );
    for row in &held.results {
        assert_eq!(row.target, desk.shared.heads[1]);
        assert!(row.quality.held);
        assert!(row.writes.iter().all(|write| write.parked));
        assert_eq!(row.value, desk.fixed);
    }
    for head in desk.shared.heads {
        assert!(
            desk.lane
                .continuity(head, ProgrammingOwner::Position)
                .is_none(),
            "first protected output must not invent accepted owner continuity"
        );
    }
    assert!(desk.lane.released().is_empty());
    let baseline = desk
        .shared
        .rig
        .engine
        .preview_static_family_frame(
            &capture,
            desk.shared
                .rig
                .engine
                .prepare_static_family_frame(&capture, &[]),
        )
        .unwrap();
    assert_eq!(held.rendered.universes, baseline.universes);
    for expected in &baseline.physical.instances {
        let actual = held
            .rendered
            .physical
            .instances
            .iter()
            .find(|instance| instance.instance_id == expected.instance_id)
            .unwrap();
        assert_eq!(actual.native_raw, expected.native_raw);
        assert_eq!(actual.complete, expected.complete);
    }
    let ((capture, recovered), evidence) = inspect_attempt(|| desk.tick());
    assert_eq!(evidence.completed, 1);
    verify(&desk, &oracle, &capture, &recovered);
    for head in desk.shared.heads {
        assert!(
            desk.lane
                .continuity(head, ProgrammingOwner::Position)
                .is_some()
        );
    }
}

fn assert_original_provenance(actual: &PhysicalProvenance, expected: &PhysicalProvenance) {
    assert_eq!(actual.fields, expected.fields);
    assert_eq!(actual.controls, expected.controls);
    assert_eq!(actual.sources.unknown(), expected.sources.unknown());
    match (actual.sources.entries(), expected.sources.entries()) {
        (Some(actual), Some(expected)) => {
            assert_eq!(actual.len(), expected.len());
            for (actual, expected) in actual.iter().zip(expected) {
                assert!(
                    Arc::ptr_eq(actual.record(), expected.record()),
                    "retain exact immutable authored source records"
                );
                assert_eq!(actual.footprint(), expected.footprint());
                assert_eq!(actual.fields(), expected.fields());
                assert_eq!(actual.relationship(), expected.relationship());
                assert_eq!(actual.role(), expected.role());
                match (actual.static_source(), expected.static_source()) {
                    (Some(actual), Some(expected)) => assert!(std::ptr::eq(actual, expected)),
                    (None, None) => {}
                    _ => panic!("release changed its original static source part"),
                }
            }
        }
        (None, None) => {}
        _ => panic!("release changed whether original source evidence is available"),
    }
}

#[test]
fn lane_release_after_physical_hold_reports_original_accepted_token_and_provenance() {
    let mut desk = EnvelopeDesk::new_configured(false, 0.5, false, true);
    let oracle = oracle(&desk);
    let (capture, accepted) = desk.tick();
    verify(&desk, &oracle, &capture, &accepted);
    let accepted_token = accepted.token.clone();
    let provenance = desk.shared.heads.map(|head| {
        accepted
            .results
            .iter()
            .find(|row| row.target == head)
            .unwrap()
            .provenance
            .clone()
    });
    let continuity = desk
        .shared
        .heads
        .map(|head| desk.lane.continuity(head, ProgrammingOwner::Position));
    let (_, held) = with_environment_limit(1, || desk.tick());
    assert!(!held.requirements.is_empty());
    assert_eq!(
        held.results.len(),
        1,
        "the protected peer row must not replace its accepted evidence"
    );
    assert_ne!(held.token, accepted_token);
    for row in &held.results {
        assert!(row.quality.held);
        assert!(row.writes.iter().all(|write| write.parked));
    }
    assert!(
        desk.lane.released().is_empty(),
        "physical protection is not a source Release"
    );
    for (index, head) in desk.shared.heads.into_iter().enumerate() {
        assert_eq!(
            desk.lane.continuity(head, ProgrammingOwner::Position),
            continuity[index]
        );
    }

    // Lower-level lane lifecycle check only: no renderer/finalizer output is claimed for
    // this deliberately empty accepted lane frame. Its token is a genuine fresh capture.
    let release_capture = desk.shared.rig.capture();
    let release_token = release_capture.frame_token();
    desk.lane.begin(&release_token).unwrap();
    let mut scalar = desk
        .shared
        .rig
        .engine
        .prepare_static_family_frame(&release_capture, &[]);
    let geometry = desk
        .shared
        .rig
        .engine
        .observe_static_family_geometry(&release_capture, &mut scalar)
        .unwrap();
    let models = desk.runtime.captured_native_color_models();
    let frame = HybridFrameContext {
        capture: &release_capture,
        token: &release_token,
        scalar: &scalar,
        geometry: &geometry,
        native_models: models.as_ref(),
    };
    let mut observer = PositionFrameObserver::new(&desk.lane);
    observer.begin_frame(&release_token).unwrap();
    observer.prepare_programs(frame, &[]).unwrap();
    desk.lane.verify(&release_token).unwrap();
    assert!(desk.lane.accept(&release_token));
    let released = desk.lane.released();
    assert_eq!(released.len(), 2);
    for (index, head) in desk.shared.heads.into_iter().enumerate() {
        let release = released
            .iter()
            .find(|release| release.target == head)
            .unwrap();
        assert_eq!(release.owner, ProgrammingOwner::Position);
        assert_eq!(
            release.last_token, accepted_token,
            "held publication cannot replace last accepted owner token"
        );
        assert_ne!(release.last_token, held.token);
        assert_original_provenance(&release.last_provenance, &provenance[index]);
        assert!(
            desk.lane
                .continuity(head, ProgrammingOwner::Position)
                .is_none()
        );
    }
}
