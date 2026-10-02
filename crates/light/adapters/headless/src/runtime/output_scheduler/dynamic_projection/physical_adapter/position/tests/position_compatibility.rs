//! Position compatibility is conservative and physical, not a whole-profile revision key.
//! Synthetic captured-native fixtures establish commanded branch behavior, not lamp feedback.
use super::*;

fn rig_with_color() -> (Rig, FixtureProfile) {
    let mut profile = moving_head();
    let head = profile.modes[0].heads[0].id;
    profile.modes[0]
        .channels
        .push(channel(head, "color.red", 5));
    profile.modes[0].splits[0].footprint = 6;
    profile.validate().unwrap();
    let root = FixtureId::new();
    (Rig::new(vec![patched(&profile, root, 1)], root), profile)
}

fn install_profile(rig: &Rig, profile: &FixtureProfile) {
    profile.validate().unwrap();
    let snapshot = rig.engine.snapshot();
    let mut fixtures = snapshot.fixtures.as_ref().clone();
    // Rebuild the profile-derived definition while retaining this physical installation.
    fixtures[0].definition = patched(profile, rig.root, 1).definition;
    rig.engine
        .replace_snapshot(EngineSnapshot {
            fixtures: fixtures.into(),
            revision: snapshot.revision + 1,
            ..snapshot.as_ref().clone()
        })
        .unwrap();
}

fn target_on_accepted_ray(rig: &Rig, accepted: &PositionContinuity) -> AttributeValue {
    let resolved = rig.resolve_with(&[(rig.root, angles(450., 30.))], Some(accepted), &[]);
    rig.verify(&resolved);
    let world = resolved.results[0].achieved.outcomes[0]
        .result
        .pose
        .unwrap()
        .point([0., -10., 0.]);
    let desk = RigidTransform::DESK_TO_PROFILE.inverse().point(world);
    target(TargetReference::Origin, desk.map(|value| value as f32))
}

fn assert_same_target_branch(rig: &Rig, value: &AttributeValue, accepted: &PositionContinuity) {
    let resolved = rig.resolve_with(&[(rig.root, value.clone())], Some(accepted), &[]);
    rig.verify(&resolved);
    let row = &resolved.results[0];
    assert_eq!(row.requested, intent(value).unwrap().clone());
    let outcome = &row.achieved.outcomes[0].result;
    assert_eq!(outcome.status, PositionFitStatus::Fitted);
    let [pan, tilt] = outcome.achieved.unwrap();
    assert!(
        (pan - 450.).abs() < 0.05,
        "450-degree branch must survive, not wrap to 90: {pan}"
    );
    assert!((tilt - 30.).abs() < 0.05, "{tilt}");
    assert_eq!(
        row.continuity.instances[0].compatibility,
        accepted.instances[0].compatibility
    );
}

#[test]
fn cosmetic_and_color_only_profile_edits_preserve_the_same_target_unwrapped_branch() {
    let (rig, mut profile) = rig_with_color();
    let accepted = accept_angles(&rig, angles(450., 30.));
    let value = target_on_accepted_ray(&rig, &accepted);
    let original = value.clone();
    assert_same_target_branch(&rig, &value, &accepted);

    profile.name = "Renamed moving light".into();
    profile.manufacturer = "Renamed manufacturer".into();
    profile.revision += 1;
    install_profile(&rig, &profile);
    assert_same_target_branch(&rig, &value, &accepted);

    let color = profile.modes[0].channels.last_mut().unwrap();
    color.functions[0].behavior = ChannelFunctionBehavior::Continuous {
        physical_min: 0.,
        physical_max: 2.,
        unit: None,
    };
    profile.revision += 1;
    install_profile(&rig, &profile);
    assert_same_target_branch(&rig, &value, &accepted);
    assert_eq!(
        value, original,
        "profile edits cannot rewrite recorded Target intent"
    );
}

fn assert_fresh_native_hold(rig: &Rig, accepted: &PositionContinuity, identity_changed: bool) {
    let missing = target(
        TargetReference::Point {
            point_id: Uuid::new_v4(),
        },
        [0.; 3],
    );
    let resolved = rig.resolve_with(&[(rig.root, missing)], Some(accepted), &[]);
    rig.verify(&resolved);
    let row = &resolved.results[0];
    assert!(row.quality.held);
    assert_eq!(
        row.achieved.outcomes[0].result.status,
        PositionFitStatus::MissingTarget
    );
    assert_eq!(
        row.continuity.instances[0].compatibility != accepted.instances[0].compatibility,
        identity_changed
    );
    assert_ne!(
        row.continuity.instances[0].joints,
        accepted.instances[0].joints
    );
    assert!(!row.writes.is_empty());
    for write in &row.writes {
        assert!(write.parked);
        assert_eq!(
            write.raw, resolved.native[write.slot.channel_index as usize],
            "invalidated history must hold fresh native input, not replay accepted commands"
        );
    }
}

#[test]
fn physical_mapping_change_rejects_unwrapped_history_with_the_same_control_ids() {
    let (rig, mut profile) = rig_with_color();
    let accepted = accept_angles(&rig, angles(450., 30.));
    profile.modes[0].channels[0].functions[0].behavior = ChannelFunctionBehavior::Continuous {
        physical_min: 360.,
        physical_max: -360.,
        unit: Some("deg".into()),
    };
    install_profile(&rig, &profile);
    assert_fresh_native_hold(&rig, &accepted, true);
}

#[test]
fn installed_correction_change_rejects_unwrapped_history() {
    let (rig, _) = rig_with_color();
    let accepted = accept_angles(&rig, angles(450., 30.));
    let snapshot = rig.engine.snapshot();
    let mut fixtures = snapshot.fixtures.as_ref().clone();
    fixtures[0].position_calibration = Some(InstalledPositionCalibration {
        pan_zero_degrees: 42.,
        ..Default::default()
    });
    rig.engine
        .replace_snapshot(EngineSnapshot {
            fixtures: fixtures.into(),
            revision: snapshot.revision + 1,
            ..snapshot.as_ref().clone()
        })
        .unwrap();
    assert_fresh_native_hold(&rig, &accepted, true);
}

#[test]
fn fresh_native_change_still_rejects_history_after_unrelated_profile_edit() {
    let (rig, mut profile) = rig_with_color();
    let accepted = accept_angles(&rig, angles(450., 30.));
    profile.name = "Cosmetic edit".into();
    install_profile(&rig, &profile);
    rig.set(rig.root, "pan", 0.1);
    assert_fresh_native_hold(&rig, &accepted, false);
}
