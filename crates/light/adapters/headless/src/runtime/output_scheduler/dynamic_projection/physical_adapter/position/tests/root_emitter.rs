//! Real fitter/finalizer coverage for a root emitter whose DMX head belongs to a logical fixture.
use super::programs::verify_live_native;
use super::*;

pub(super) fn root_emitter_rig(bind_emitter: bool) -> (Rig, FixtureId, FixtureId) {
    let mut profile = moving_head();
    let head = profile.modes[0].heads[0].id;
    profile.modes[0].heads[0].master_shared = false;
    profile.geometry.emitters[0].head_id = None;
    if !bind_emitter {
        profile.modes[0].emitter_heads.clear();
        // Fixture-level unbound emitters are inactive; the explicit mode-local
        // physical graph retains this unheaded lens as a root-owned emitter.
        profile.modes[0].geometry = profile.geometry.clone();
    }
    profile.validate().unwrap();
    let root = FixtureId::new();
    let logical = FixtureId::new();
    let copy = FixtureId::new();
    let mut fixture = patched(&profile, root, 1);
    fixture.logical_heads = vec![PatchedHead {
        profile_head_id: Some(head),
        head_index: fixture.definition.heads[0].index,
        fixture_id: logical,
    }];
    fixture.location.z = 3000;
    fixture.multipatch = vec![MultiPatchInstance {
        id: copy.0,
        universe: Some(1),
        address: Some(10),
        location: FixtureLocation {
            x: 3000,
            y: 1000,
            z: 5000,
        },
        invert_pan: true,
        position_calibration: Some(InstalledPositionCalibration {
            pan_zero_degrees: 11.,
            tilt_zero_degrees: -7.,
            ..Default::default()
        }),
        ..Default::default()
    }];
    (Rig::new(vec![fixture], root), logical, copy)
}

#[test]
fn actual_live_unheaded_root_target_fits_root_and_copy_with_only_logical_dmx_heads() {
    let (rig, logical, copy) = root_emitter_rig(false);
    let snapshot = rig.engine.snapshot();
    assert!(profile_head_destinations(&snapshot, rig.root).is_empty());
    let descriptor = rig.adapter.compile(&snapshot, rig.root).unwrap().unwrap();
    assert_eq!(descriptor.root, rig.root);
    assert_eq!(descriptor.emitters.len(), 1);
    assert_eq!(descriptor.footprint.len(), 4);
    assert!(
        rig.adapter.compile(&snapshot, logical).unwrap().is_none(),
        "channel ownership cannot invent an emitter"
    );
    let value = target(TargetReference::Origin, [2., 6., 1.]);
    rig.programmers.set(
        rig.session,
        rig.root,
        ProgrammingOwner::Position.key(),
        value.clone(),
    );
    let lane = PhysicalAdapterLane::live(PositionAdapter::default());
    let mut runtime =
        DynamicRuntime::with_programming_contract_support(PROGRAMMING_CONTRACT_VERSION);
    let mut origins = DynamicSourceOrigins::default();
    let mut scratch = HybridFrameScratch::default();
    let capture = rig.capture();
    let foreign = rig.capture();
    assert!(
        prepare_live(
            &rig,
            &capture,
            &foreign,
            &lane,
            &mut runtime,
            &mut origins,
            &mut scratch
        )
        .is_err()
    );
    assert!(
        lane.continuity(rig.root, ProgrammingOwner::Position)
            .is_none()
    );
    let published = prepare_live(
        &rig,
        &capture,
        &capture,
        &lane,
        &mut runtime,
        &mut origins,
        &mut scratch,
    )
    .unwrap();
    assert!(published.requirements.is_empty());
    assert_eq!(published.results.len(), 1);
    let row = &published.results[0];
    assert_eq!(row.target, rig.root);
    assert_eq!(row.value, value);
    assert_eq!(row.writes.len(), 4);
    assert_eq!(row.achieved.outcomes.len(), 2);
    assert!(!row.quality.held);
    for outcome in &row.achieved.outcomes {
        assert_eq!(outcome.result.status, PositionFitStatus::Fitted);
        assert!(outcome.result.angular_error_degrees.unwrap() < 0.04);
        assert_eq!(
            outcome.result.requested,
            Some(PositionFitRequest::Target {
                world: Some(RigidTransform::DESK_TO_PROFILE.point([2., 6., 1.])),
            })
        );
    }
    assert_ne!(
        row.achieved.outcomes[0].result.achieved,
        row.achieved.outcomes[1].result.achieved
    );
    verify_live_native(&rig, &capture, &lane, &published);
    assert!(
        rig.engine
            .position_angles_from_physical(
                published.rendered.generation,
                &published.rendered.physical,
                rig.root
            )
            .is_none(),
        "different calibrated copy joints cannot become one common Angle readout"
    );
    assert!(
        rig.engine
            .position_angles_from_physical(
                published.rendered.generation,
                &published.rendered.physical,
                logical
            )
            .is_none()
    );
    let held = rig
        .engine
        .position_freeze_from_physical(
            published.rendered.generation,
            &published.rendered.physical,
            rig.root,
        )
        .unwrap();
    assert_eq!(held.instances.len(), 2);
    assert!(
        held.instances
            .iter()
            .any(|instance| instance.instance_id == copy.0)
    );
    assert!(
        held.instances
            .iter()
            .all(|instance| instance.controls.len() == 2)
    );
    assert_eq!(
        rig.engine
            .prepare_static_family_frame(&capture, &[])
            .value(rig.root, &ProgrammingOwner::Position.key()),
        Some(&value),
        "accepted fit cannot rewrite requested Target"
    );
}

#[test]
fn actual_logical_emitter_binding_has_one_owner_while_root_stays_a_native_destination() {
    let (rig, logical, copy) = root_emitter_rig(true);
    let snapshot = rig.engine.snapshot();
    assert!(profile_head_destinations(&snapshot, rig.root).is_empty());
    assert!(rig.adapter.compile(&snapshot, rig.root).unwrap().is_none());
    let descriptor = rig.adapter.compile(&snapshot, logical).unwrap().unwrap();
    assert_eq!(descriptor.root, rig.root);
    assert_eq!(descriptor.emitters.len(), 1);
    assert_eq!(descriptor.footprint.len(), 4);
    assert!(
        descriptor
            .footprint
            .iter()
            .all(|slot| [rig.root, copy].contains(&slot.destination))
    );
    let value = angles(450., 30.);
    let resolved = rig.resolve(&[(logical, value.clone())]);
    rig.verify(&resolved);
    assert_eq!(resolved.results.len(), 1);
    assert_eq!(
        resolved.results[0].requested,
        intent(&value).unwrap().clone()
    );
    assert!(
        resolved.results[0]
            .achieved
            .outcomes
            .iter()
            .all(|outcome| outcome.result.status == PositionFitStatus::Fitted)
    );
}
