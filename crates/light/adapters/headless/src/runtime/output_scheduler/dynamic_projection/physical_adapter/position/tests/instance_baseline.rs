//! Position holds use the actual premaster commands of each installation. These regressions
//! compare the gated adapter with the existing engine renderer, before physical-write injection.
use super::*;

fn inverted_instances() -> (Rig, FixtureId) {
    let profile = moving_head();
    let root = FixtureId::new();
    let copy = FixtureId::new();
    let mut fixture = patched(&profile, root, 1);
    fixture.invert_pan = true;
    fixture.invert_tilt = false;
    fixture.multipatch = vec![MultiPatchInstance {
        id: copy.0,
        universe: Some(1),
        address: Some(10),
        invert_pan: false,
        invert_tilt: true,
        ..Default::default()
    }];
    (Rig::new(vec![fixture], root), copy)
}

fn missing_target() -> AttributeValue {
    target(
        TargetReference::Point {
            point_id: Uuid::new_v4(),
        },
        [0.; 3],
    )
}

/// Read both U16 coarse/fine commands from the real renderer's independently patched instances.
/// No adapter capture API or inversion helper supplies these expected values.
fn rendered_instances(rig: &Rig, copy: FixtureId) -> Vec<(FixtureId, Vec<u32>)> {
    let rendered = rig
        .engine
        .render(light_engine::RenderOptions::default())
        .unwrap();
    let bytes = &rendered.universes[&1];
    [(rig.root, 0), (copy, 9)]
        .into_iter()
        .map(|(id, start)| {
            (
                id,
                vec![
                    u32::from(u16::from_be_bytes([bytes[start], bytes[start + 1]])),
                    u32::from(u16::from_be_bytes([bytes[start + 2], bytes[start + 3]])),
                ],
            )
        })
        .collect()
}

fn written_instance(
    result: &PhysicalResolution<PositionAdapter>,
    destination: FixtureId,
) -> Vec<u32> {
    let mut values = vec![0; 2];
    let mut seen = vec![false; 2];
    for write in result
        .writes
        .iter()
        .filter(|w| w.slot.destination == destination)
    {
        let index = write.slot.channel_index as usize;
        assert!(!seen[index], "one native write per instance and axis");
        seen[index] = true;
        values[index] = write.raw;
        assert!(
            write.parked,
            "unresolvable Target holds the existing commands"
        );
    }
    assert!(seen.into_iter().all(|value| value));
    values
}

fn assert_missing(result: &PhysicalResolution<PositionAdapter>) {
    assert!(result.quality.held);
    assert_eq!(result.achieved.outcomes.len(), 2);
    for outcome in &result.achieved.outcomes {
        assert_eq!(outcome.result.status, PositionFitStatus::MissingTarget);
    }
}

#[test]
fn missing_target_holds_actual_normalized_native_baseline_per_inverted_instance() {
    let (rig, copy) = inverted_instances();
    rig.set(rig.root, "pan", 0.25);
    rig.set(rig.root, "tilt", 0.75);
    let expected = rendered_instances(&rig, copy);
    assert_ne!(
        expected[0].1, expected[1].1,
        "opposite installations have different native commands"
    );
    let resolved = rig.resolve_with(&[(rig.root, missing_target())], None, &[]);
    rig.verify(&resolved);
    let result = &resolved.results[0];
    assert_missing(result);
    for (destination, native) in &expected {
        assert_eq!(
            &written_instance(result, *destination),
            native,
            "held coarse/fine commands must equal that instance's actual rendered DMX"
        );
        let continuity = result
            .continuity
            .instances
            .iter()
            .find(|c| c.destination == *destination)
            .unwrap();
        for &(index, _, baseline, written) in &continuity.controls {
            assert_eq!(baseline, native[index as usize]);
            assert_eq!(written, baseline);
        }
    }
}

#[test]
fn normalized_native_edit_replaces_each_inverted_instances_accepted_position_hold() {
    let (rig, copy) = inverted_instances();
    rig.set(rig.root, "pan", 0.25);
    rig.set(rig.root, "tilt", 0.75);
    let accepted = accept_angles(&rig, angles(450., 30.));
    rig.set(rig.root, "pan", 0.1);
    let expected = rendered_instances(&rig, copy);
    let resolved = rig.resolve_with(&[(rig.root, missing_target())], Some(&accepted), &[]);
    rig.verify(&resolved);
    let result = &resolved.results[0];
    assert_missing(result);
    for (destination, native) in &expected {
        let old = accepted
            .instances
            .iter()
            .find(|c| c.destination == *destination)
            .unwrap();
        let old_pan = old
            .controls
            .iter()
            .find(|(index, _, _, _)| *index == 0)
            .unwrap()
            .3;
        assert_ne!(
            native[0], old_pan,
            "fresh native Pan is distinguishable from fitted continuity"
        );
        assert_eq!(
            &written_instance(result, *destination),
            native,
            "the whole accepted pair is invalidated against its own fresh native baseline"
        );
        let current = result
            .continuity
            .instances
            .iter()
            .find(|c| c.destination == *destination)
            .unwrap();
        assert_ne!(current.joints, old.joints);
    }
}

#[test]
fn copy_only_installation_change_invalidates_its_normalized_hold_without_resetting_root() {
    let (rig, copy) = inverted_instances();
    rig.set(rig.root, "pan", 0.25);
    rig.set(rig.root, "tilt", 0.75);
    let accepted = accept_angles(&rig, angles(450., 30.));
    let snapshot = rig.engine.snapshot();
    let mut fixtures = snapshot.fixtures.as_ref().clone();
    fixtures[0].multipatch[0].invert_pan = true;
    rig.engine
        .replace_snapshot(EngineSnapshot {
            fixtures: fixtures.into(),
            revision: snapshot.revision + 1,
            ..snapshot.as_ref().clone()
        })
        .unwrap();
    let expected = rendered_instances(&rig, copy);
    let resolved = rig.resolve_with(&[(rig.root, missing_target())], Some(&accepted), &[]);
    rig.verify(&resolved);
    let result = &resolved.results[0];
    assert_missing(result);
    let root_before = accepted
        .instances
        .iter()
        .find(|c| c.destination == rig.root)
        .unwrap();
    let copy_before = accepted
        .instances
        .iter()
        .find(|c| c.destination == copy)
        .unwrap();
    let root_after = result
        .continuity
        .instances
        .iter()
        .find(|c| c.destination == rig.root)
        .unwrap();
    let copy_after = result
        .continuity
        .instances
        .iter()
        .find(|c| c.destination == copy)
        .unwrap();
    assert_eq!(root_after.compatibility, root_before.compatibility);
    assert_eq!(
        root_after.joints, root_before.joints,
        "unchanged root keeps its accepted mechanical branch"
    );
    let mut old_root = vec![0; 2];
    for &(index, _, _, written) in &root_before.controls {
        old_root[index as usize] = written;
    }
    assert_eq!(written_instance(result, rig.root), old_root);
    assert_ne!(copy_after.compatibility, copy_before.compatibility);
    assert_eq!(
        written_instance(result, copy),
        expected.iter().find(|(id, _)| *id == copy).unwrap().1
    );
    assert_ne!(copy_after.joints, copy_before.joints);
}

#[test]
fn raw_native_position_hold_bypasses_independent_patch_inversions_at_full_u16_precision() {
    let (rig, copy) = inverted_instances();
    for (attribute, raw) in [("pan", 0x1234), ("tilt", 0xabcd)] {
        rig.programmers.set(
            rig.session,
            rig.root,
            AttributeKey(attribute.into()),
            AttributeValue::RawDmxExact(raw),
        );
    }
    let expected = rendered_instances(&rig, copy);
    for (_, values) in &expected {
        assert_eq!(
            values,
            &[0x1234, 0xabcd],
            "explicit Raw values bypass installation inversion"
        );
    }
    let resolved = rig.resolve(&[(rig.root, missing_target())]);
    rig.verify(&resolved);
    let result = &resolved.results[0];
    assert_missing(result);
    for (destination, native) in &expected {
        assert_eq!(
            &written_instance(result, *destination),
            native,
            "hold preserves actual Raw coarse/fine bytes without another inversion"
        );
    }
}
