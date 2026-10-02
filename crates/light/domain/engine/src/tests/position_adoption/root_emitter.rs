//! A physical root may own an unheaded emitter while every DMX head is logical.
use super::*;
use crate::{
    CapturedNativeRaw, FamilyProjectionEvidence, FamilyProjectionMaster, FamilyProjectionMetadata,
    PositionNativeWrite, PreparedStaticFamilyFrame,
};
use light_core::programming::{PositionIntent, ProgrammingOwner};

fn unheaded() -> (PatchedFixture, FixtureId, Uuid) {
    let mut fixture = mover();
    redefine(&mut fixture, |profile| {
        for head in &mut profile.modes[0].heads {
            head.master_shared = false;
        }
        profile.modes[0].emitter_heads.clear();
        for emitter in &mut profile.geometry.emitters {
            emitter.head_id = None;
        }
        // Unbound fixture-level emitters are intentionally inactive in a mode. A
        // mode-local physical graph can explicitly retain a root-owned lens.
        profile.modes[0].geometry = profile.geometry.clone();
    });
    let logical = FixtureId::new();
    assert_eq!(fixture.definition.heads.len(), 1);
    let mode = &fixture.definition.profile_snapshot.as_ref().unwrap().modes[0];
    fixture.logical_heads = vec![PatchedHead {
        profile_head_id: Some(mode.heads[0].id),
        fixture_id: logical,
        head_index: fixture.definition.heads[0].index,
    }];
    let copy = Uuid::new_v4();
    fixture.multipatch.push(MultiPatchInstance {
        id: copy,
        universe: Some(1),
        address: Some(10),
        ..Default::default()
    });
    (fixture, logical, copy)
}
fn position(frame: &mut PreparedStaticFamilyFrame, owner: FixtureId) {
    frame
        .project_family(
            owner,
            ProgrammingOwner::Position,
            AttributeValue::Position(Arc::new(PositionIntent::angles(-720., 720.))),
            FamilyProjectionMetadata {
                changed_at: None,
                evidence: FamilyProjectionEvidence::PreserveBaseline,
                master: FamilyProjectionMaster::PreserveBaseline,
            },
        )
        .unwrap();
}
fn writes(fixture: &PatchedFixture, owner: FixtureId) -> Vec<PositionNativeWrite> {
    let mode = &fixture.definition.profile_snapshot.as_ref().unwrap().modes[0];
    std::iter::once(fixture.fixture_id.0)
        .chain(fixture.multipatch.iter().map(|copy| copy.id))
        .flat_map(|instance_id| {
            mode.channels
                .iter()
                .enumerate()
                .map(move |(index, channel)| PositionNativeWrite {
                    target: owner,
                    instance_id,
                    channel_index: index as u32,
                    channel_id: channel.id,
                    function_id: Some(channel.functions[0].id),
                    split: channel.split,
                    raw: if index == 0 { 0 } else { 65535 },
                })
        })
        .collect()
}
fn assert_output(
    engine: &Engine,
    output: &crate::RenderResult,
    owner: FixtureId,
    root: FixtureId,
    copy: Uuid,
) {
    assert_eq!(
        engine.position_angles_from_physical(output.generation, &output.physical, owner),
        Some(JointAngles {
            pan_degrees: -720.,
            tilt_degrees: 720.
        })
    );
    for instance_id in [root.0, copy] {
        let instances = output
            .physical
            .instances
            .iter()
            .filter(|row| row.instance_id == instance_id)
            .collect::<Vec<_>>();
        assert_eq!(instances.len(), 1);
        assert!(instances[0].complete);
        assert_eq!(instances[0].native_raw.as_ref(), &[0, 65535]);
        assert_eq!(
            instances[0].lenses.len(),
            1,
            "ownership must not duplicate physical emitters"
        );
    }
    assert_eq!(&output.universes[&1][0..4], &[0, 0, 255, 255]);
    assert_eq!(&output.universes[&1][9..13], &[0, 0, 255, 255]);
    let held = engine
        .position_freeze_from_physical(output.generation, &output.physical, owner)
        .unwrap();
    assert_eq!(held.instances.len(), 2);
    for instance_id in [root.0, copy] {
        let held_instance = held
            .instances
            .iter()
            .find(|row| row.instance_id == instance_id)
            .unwrap();
        assert_eq!(
            held_instance
                .controls
                .iter()
                .map(|control| control.raw)
                .collect::<Vec<_>>(),
            [0, 65535]
        );
        assert!(
            held_instance
                .controls
                .iter()
                .all(|control| control.signature.len() == 64)
        );
    }
    assert!(
        engine
            .position_freeze_from_physical(output.generation, &output.physical, FixtureId(copy))
            .is_none()
    );
}

#[test]
fn unheaded_root_with_only_logical_dmx_heads_has_native_baseline_projection_and_accepted_pose() {
    let (fixture, logical, copy) = unheaded();
    let root = fixture.fixture_id;
    let (engine, programmers, session) = engine(fixture.clone());
    assert!(crate::profile_head_destinations(&engine.snapshot(), root).is_empty());
    assert_eq!(
        crate::profile_head_destinations(&engine.snapshot(), logical).len(),
        1
    );
    assert!(engine.position_has_native_controls(&engine.snapshot(), root));
    assert!(!engine.position_has_native_controls(&engine.snapshot(), logical));
    set(&programmers, session, logical, "pan", 0.25);
    set(&programmers, session, logical, "tilt", 0.75);
    let baseline = engine.render(Default::default()).unwrap();
    assert!(
        engine
            .position_angles_from_physical(baseline.generation, &baseline.physical, root)
            .is_some()
    );
    assert!(
        engine
            .position_angles_from_physical(baseline.generation, &baseline.physical, logical)
            .is_none()
    );
    assert!(
        engine
            .position_freeze_from_physical(baseline.generation, &baseline.physical, logical)
            .is_none()
    );
    for owner in [root, logical] {
        programmers.set(
            session,
            owner,
            ProgrammingOwner::Position.key(),
            AttributeValue::Position(Arc::new(PositionIntent::angles(0., 0.))),
        );
    }
    let capture = engine.prepare_output_frame(Default::default());
    let token = capture.frame_token();
    let mut frame = engine.prepare_static_family_frame(&capture, &[]);
    let mut native = CapturedNativeRaw::default();
    for instance in [root.0, copy] {
        frame
            .native_position_raw_into(&capture, &token, root, instance, &mut native)
            .unwrap();
        assert_eq!(native.token(), Some(&token));
        assert_eq!(native.destination(), Some(root));
        assert_eq!(native.instance_id(), Some(instance));
        assert_eq!(
            native.raw(),
            baseline
                .physical
                .instances
                .iter()
                .find(|row| row.instance_id == instance)
                .unwrap()
                .native_raw
                .as_ref()
        );
    }
    assert!(
        frame
            .native_position_raw_into(&capture, &token, root, Uuid::new_v4(), &mut native)
            .is_err()
    );
    assert!(native.token().is_none() && native.raw().is_empty());
    let foreign = engine.prepare_output_frame(Default::default());
    assert!(
        frame
            .native_position_raw_into(&foreign, &foreign.frame_token(), root, root.0, &mut native)
            .is_err()
    );
    position(&mut frame, root);
    let native_writes = writes(&fixture, root);
    assert!(
        frame
            .project_position_native(&capture, &token, &native_writes[..2])
            .is_err(),
        "a root-only payload cannot omit its copy"
    );
    let mut wrong_copy = native_writes.clone();
    wrong_copy[2].instance_id = Uuid::new_v4();
    assert!(
        frame
            .project_position_native(&capture, &token, &wrong_copy)
            .is_err()
    );
    assert!(
        frame
            .project_position_native(&foreign, &foreign.frame_token(), &native_writes)
            .is_err()
    );
    frame
        .project_position_native(&capture, &token, &native_writes)
        .unwrap();
    let output = engine.render_static_family_frame(&capture, frame).unwrap();
    assert_output(&engine, &output, root, root, copy);
}

#[test]
fn emitter_binding_moves_position_ownership_without_aliasing_root_or_stale_generation() {
    let (mut fixture, logical, copy) = unheaded();
    let root = fixture.fixture_id;
    let (engine, programmers, session) = engine(fixture.clone());
    let old = engine.render(Default::default()).unwrap();
    assert!(
        engine
            .position_angles_from_physical(old.generation, &old.physical, root)
            .is_some()
    );
    redefine(&mut fixture, |profile| {
        let head_id = profile.modes[0].heads[0].id;
        profile.modes[0].geometry.emitters[0].head_id = Some(head_id);
        profile.modes[0].emitter_heads = vec![EmitterHeadBinding {
            emitter_id: profile.geometry.emitters[0].id,
            head_id: profile.modes[0].heads[0].id,
        }];
    });
    let mut snapshot = engine.snapshot().as_ref().clone();
    Arc::make_mut(&mut snapshot.fixtures)[0] = fixture.clone();
    snapshot.revision += 1;
    engine.replace_snapshot(snapshot).unwrap();
    assert!(!engine.position_has_native_controls(&engine.snapshot(), root));
    assert!(engine.position_has_native_controls(&engine.snapshot(), logical));
    assert!(
        engine
            .position_angles_from_physical(old.generation, &old.physical, root)
            .is_none()
    );
    assert!(
        engine
            .position_freeze_from_physical(old.generation, &old.physical, root)
            .is_none()
    );
    for owner in [root, logical] {
        programmers.set(
            session,
            owner,
            ProgrammingOwner::Position.key(),
            AttributeValue::Position(Arc::new(PositionIntent::angles(0., 0.))),
        );
    }
    let capture = engine.prepare_output_frame(Default::default());
    let token = capture.frame_token();
    let mut frame = engine.prepare_static_family_frame(&capture, &[]);
    position(&mut frame, root);
    position(&mut frame, logical);
    assert!(
        frame
            .project_position_native(&capture, &token, &writes(&fixture, root))
            .is_err(),
        "root no longer owns the emitter's motor footprint"
    );
    frame
        .project_position_native(&capture, &token, &writes(&fixture, logical))
        .unwrap();
    let output = engine.render_static_family_frame(&capture, frame).unwrap();
    assert_output(&engine, &output, logical, root, copy);
    assert!(
        engine
            .position_angles_from_physical(output.generation, &output.physical, root)
            .is_none()
    );
    assert!(
        engine
            .position_freeze_from_physical(output.generation, &output.physical, root)
            .is_none()
    );
    assert!(
        engine
            .position_angles_from_physical(output.generation, &old.physical, logical)
            .is_none(),
        "new caller generation cannot authorize old physical layout"
    );
}
