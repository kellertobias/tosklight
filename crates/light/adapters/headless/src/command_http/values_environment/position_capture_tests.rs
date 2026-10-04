//! First real Angle edits adopt accepted commanded joints, never a fresh hypothetical Target fit.
//! Synthetic U16 profiles prove capture/ownership semantics rather than lamp calibration accuracy.
use super::*;
use crate::runtime::visualization_frame::RenderedSemanticFrame;
use light_application::{ProgrammingValueIntent, ProgrammingValueOperation};
use light_core::programming::{
    ComponentEdit, FamilyEditContext, JointAngles, PositionIntent, ProgrammingComponent,
    ProgrammingOwner, ScalarEdit, TargetReference, edit_family,
};
use light_core::{MergeMode, SessionId, TimedValue};
use light_engine::{ContributionBatch, ContributionSample, EngineSnapshot};
use light_fixture::*;
use light_wire::v2::visualization::VisualizationScope;
use std::sync::Arc;
use uuid::Uuid;

fn channel(head: Uuid, name: &str, slot: u16) -> FixtureChannel {
    let attribute = AttributeKey(name.into());
    FixtureChannel {
        id: Uuid::new_v4(),
        head_id: head,
        split: 1,
        fixture_attribute: attribute.clone(),
        attribute: attribute.clone(),
        canonical_transform: CanonicalTransform::Identity,
        resolution: ChannelResolution::U16,
        secondary_slots: vec![slot + 1],
        default_raw: 32768,
        highlight_raw: 65535,
        physical_min: None,
        physical_max: None,
        unit: None,
        invert: false,
        snap: false,
        reacts_to_virtual_intensity: false,
        virtual_intensity_inverted: false,
        reacts_to_sequence_master: true,
        reacts_to_group_master: true,
        reacts_to_grand_master: false,
        behavior: ChannelBehavior::Controlled,
        functions: vec![ChannelFunction::continuous(name, attribute, 65535)],
    }
}

fn patched(profile: &FixtureProfile, id: FixtureId, address: u16) -> PatchedFixture {
    PatchedFixture {
        model_scale: None,
        scenery_options: Default::default(),
        scenery_size_metres: None,
        fixture_id: id,
        fixture_number: Some(1),
        virtual_fixture_number: None,
        name: profile.name.clone(),
        definition: profile.resolved_definition(profile.modes[0].id).unwrap(),
        universe: Some(1),
        address: Some(address),
        split_patches: vec![],
        layer_id: "default".into(),
        note: None,
        position_master: None,
        direct_control: None,
        internal_bindings: Default::default(),
        location: Default::default(),
        rotation: Default::default(),
        logical_heads: vec![],
        multipatch: vec![],
        group_masters_enabled: true,
        grand_master_enabled: true,
        invert_pan: false,
        invert_tilt: false,
        position_calibration: None,
        color_calibration: None,
        bracket_angle: 0.0,
        shaper_angle: None,
        installed_appearance: Default::default(),
        move_in_black_enabled: true,
        move_in_black_delay_millis: 0,
        highlight_overrides: Default::default(),
        freeze: Default::default(),
    }
}

fn mover() -> PatchedFixture {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Test".into();
    profile.name = "TL-556 accepted command capture".into();
    let head = profile.modes[0].heads[0].id;
    profile.geometry = GeometryGraph::template(GeometryTemplate::MovingHead, &[head]);
    profile.geometry.physical_contract = Some(GeometryPhysicalContract {
        version: 1,
        provenance: OpticalProvenance::default(),
        bracket: GeometryBracket::Fixed,
    });
    let mut bindings = Vec::new();
    for (index, (attribute, role)) in [
        ("pan", PositionAxisRole::Pan),
        ("tilt", PositionAxisRole::Tilt),
    ]
    .into_iter()
    .enumerate()
    {
        let mut channel = channel(head, attribute, 1 + 2 * index as u16);
        channel.id = Uuid::new_v4();
        channel.head_id = head;
        channel.attribute = AttributeKey(attribute.into());
        channel.fixture_attribute = channel.attribute.clone();
        channel.resolution = ChannelResolution::U16;
        channel.secondary_slots = vec![2 + 2 * index as u16];
        channel.default_raw = 32768;
        channel.highlight_raw = 65535;
        channel.physical_min = None;
        channel.physical_max = None;
        channel.reacts_to_grand_master = false;
        channel.functions = vec![ChannelFunction::continuous(
            attribute,
            channel.attribute.clone(),
            65535,
        )];
        channel.functions[0].behavior = ChannelFunctionBehavior::Continuous {
            physical_min: -720.,
            physical_max: 720.,
            unit: Some("deg".into()),
        };
        channel.functions[0].angular_motion = Some(AngularMotion {
            kind: AngularMotionKind::AbsolutePosition,
            max_speed_degrees_per_second: None,
            acceleration_degrees_per_second_squared: None,
            deceleration_degrees_per_second_squared: None,
        });
        bindings.push(MotionFunctionBinding {
            node_id: profile.geometry.nodes[index + 1].id,
            channel_id: channel.id,
            function_id: channel.functions[0].id,
            role,
        });
        profile.modes[0].channels.push(channel);
    }
    profile.modes[0].splits[0].footprint = 4;
    profile.modes[0].position_physical = Some(PositionPhysicalModel {
        kinematics: Default::default(),
        version: 1,
        revision: 1,
        bindings,
    });
    let emitter = GeometryEmitter {
        id: Uuid::new_v4(),
        name: "Lens".into(),
        node_id: profile.geometry.nodes[2].id,
        head_id: None,
        origin: Vector3::default(),
        orientation_degrees: Vector3::default(),
        beam_angle_degrees: 10.,
        field_angle_degrees: 20.,
        feather: 0.,
        focus: 1.,
        directional: true,
        layout: EmitterLayout::Point,
    };
    profile.modes[0].emitter_heads = vec![EmitterHeadBinding {
        emitter_id: emitter.id,
        head_id: head,
    }];
    profile.geometry.emitters = vec![emitter];
    profile.validate().unwrap();
    patched(&profile, FixtureId::new(), 1)
}

fn install(state: &AppState, fixture: PatchedFixture) {
    state
        .output
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
}

fn requested(offset: [f32; 3]) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Origin,
        offset,
    )))
}

fn sample(owner: FixtureId, attribute: AttributeKey, value: AttributeValue) -> ContributionSample {
    ContributionSample::independent(TimedValue {
        fixture_id: owner,
        attribute,
        value,
        priority: 100,
        changed_at: chrono::Utc::now(),
        programmer_order: 0,
        merge_mode: MergeMode::Ltp,
        fade: false,
        fade_millis: None,
        delay_millis: None,
    })
}

fn render(
    state: &AppState,
    owner: FixtureId,
    pan: f32,
    tilt: f32,
    target: Option<AttributeValue>,
) -> RenderedSemanticFrame {
    let mut samples = vec![
        sample(
            owner,
            AttributeKey("pan".into()),
            AttributeValue::Normalized(pan),
        ),
        sample(
            owner,
            AttributeKey("tilt".into()),
            AttributeValue::Normalized(tilt),
        ),
    ];
    if let Some(value) = target {
        samples.push(sample(owner, ProgrammingOwner::Position.key(), value));
    }
    RenderedSemanticFrame::untraced(
        state
            .output
            .engine()
            .render_with_contribution_batches(
                Default::default(),
                &[ContributionBatch::new(samples)],
            )
            .unwrap(),
        Default::default(),
    )
}

fn publish(state: &AppState, frame: &RenderedSemanticFrame, show_id: Option<Uuid>) {
    state
        .output
        .render_frames_and_publish(frame, VisualizationScope { show_id });
}

fn intent(owner: FixtureId) -> ProgrammingValueIntent {
    ProgrammingValueIntent {
        fixture_ids: vec![owner],
        group_id: None,
        attribute: ProgrammingOwner::Position.key(),
        operation: ProgrammingValueOperation::ComponentEdits(vec![ComponentEdit::Scalar {
            component: ProgrammingComponent::Pan,
            operation: ScalarEdit::Relative(5.),
        }]),
        undo_group: Some("accepted-position-turn".into()),
        timing: Default::default(),
        displayed_source: None,
        color_adoption: Default::default(),
    }
}

fn capture(state: &AppState, owner: FixtureId, preload: bool) -> ProgrammingValuesEnvironment {
    let mut environment = values_environment(state);
    // Reused environments must not retain a solved pair from another output episode.
    environment
        .family_contexts
        .entry(owner)
        .or_default()
        .solved_angles = Some(JointAngles {
        pan_degrees: -999.,
        tilt_degrees: -999.,
    });
    prepare_family_edit_context(
        state,
        SessionId::new(),
        preload,
        &intent(owner),
        &mut environment,
    );
    environment
}

#[tokio::test]
async fn first_angle_turn_uses_accepted_calibrated_unwrapped_commands_and_that_frames_target_seed()
{
    let (state, directory) = crate::runtime::tests::test_state();
    let mut fixture = mover();
    let owner = fixture.fixture_id;
    let calibration = InstalledPositionCalibration {
        pan_zero_degrees: 15.,
        tilt_zero_degrees: -10.,
        ..Default::default()
    };
    fixture.position_calibration = Some(calibration.clone());
    fixture.multipatch.push(MultiPatchInstance {
        id: Uuid::new_v4(),
        universe: Some(1),
        address: Some(10),
        invert_pan: true,
        position_calibration: Some(calibration),
        ..Default::default()
    });
    install(&state, fixture);
    // The target deliberately differs from the native command pose. Capture must read the
    // accepted output's actual motor commands, without re-solving that target on first touch.
    let target = requested([120., -80., 40.]);
    let accepted = render(&state, owner, 1., 0., Some(target.clone()));
    assert_eq!(
        accepted.rendered.physical.instances[0].native_raw.as_ref(),
        &[65535, 0]
    );
    assert_eq!(
        accepted.rendered.physical.instances[1].native_raw.as_ref(),
        &[0, 0]
    );
    publish(&state, &accepted, None);

    let mut environment = values_environment(&state);
    environment.current_values.insert(
        (owner, ProgrammingOwner::Position.key()),
        requested([1., 2., 3.]),
    );
    prepare_family_edit_context(
        &state,
        SessionId::new(),
        false,
        &intent(owner),
        &mut environment,
    );
    let pose = JointAngles {
        pan_degrees: 735.,
        tilt_degrees: -730.,
    };
    assert_eq!(
        environment.family_contexts[&owner].solved_angles,
        Some(pose)
    );
    assert_eq!(
        environment.current_values[&(owner, ProgrammingOwner::Position.key())],
        target
    );
    let ProgrammingValueOperation::ComponentEdits(edits) = intent(owner).operation else {
        unreachable!()
    };
    let edited = edit_family(
        &target,
        &edits,
        &FamilyEditContext {
            solved_angles: Some(pose),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        edited,
        AttributeValue::Position(Arc::new(PositionIntent::angles(740., -730.))),
        "the first turn replaces Target with the whole achieved Angle pair and preserves Tilt"
    );

    let newer = render(&state, owner, 0., 1., Some(requested([9., 8., 7.])));
    assert_eq!(newer.rendered.generation, accepted.rendered.generation);
    let retained = capture(&state, owner, false);
    assert_eq!(
        retained.family_contexts[&owner].solved_angles,
        Some(pose),
        "a rendered but unpublished frame cannot change the first-edit baseline"
    );
    assert_eq!(
        retained.current_values[&(owner, ProgrammingOwner::Position.key())],
        target
    );
    let _ = std::fs::remove_dir_all(directory);
}

/// The declared default pose of the test mover, as the engine decodes it.
fn declared(state: &AppState, owner: FixtureId) -> JointAngles {
    let engine = state.output.engine();
    engine
        .declared_default_position(&engine.snapshot(), owner)
        .expect("the U16 mover's default_raw maps into Angles")
}

#[tokio::test]
async fn no_accepted_frame_clears_old_adoption_context_without_fitting_target() {
    let (state, directory) = crate::runtime::tests::test_state();
    let fixture = mover();
    let owner = fixture.fixture_id;
    install(&state, fixture);
    let unaccepted = render(&state, owner, 1., 0., Some(requested([1., 2., 3.])));
    assert!(unaccepted.rendered.physical.instances[0].complete);
    assert!(state.output.latest_visualization_frame().is_none());
    // TL-552: nothing programs this idle fixture, so before the first frame it adopts its
    // declared default pose: never the stale -999 context, never the unaccepted render's Target
    // fit or its 720° commands.
    let default = declared(&state, owner);
    assert!(default.pan_degrees.abs() < 0.05 && default.tilt_degrees.abs() < 0.05);
    assert_eq!(
        capture(&state, owner, false).family_contexts[&owner].solved_angles,
        Some(default)
    );
    let _ = std::fs::remove_dir_all(directory);
}

#[tokio::test]
async fn first_angle_turn_before_any_published_frame_is_not_a_silent_no_change() {
    let (state, directory) = crate::runtime::tests::test_state();
    let fixture = mover();
    let owner = fixture.fixture_id;
    install(&state, fixture);
    assert!(state.output.latest_visualization_frame().is_none());
    let default = declared(&state, owner);
    let environment = capture(&state, owner, false);
    let seed = AttributeValue::Position(Arc::new(PositionIntent::angles(
        default.pan_degrees,
        default.tilt_degrees,
    )));
    assert_eq!(
        environment
            .current_values
            .get(&(owner, ProgrammingOwner::Position.key())),
        Some(&seed)
    );
    // The relative turn therefore lands on the default pose rather than changing nothing.
    let turned = edit_family(
        &seed,
        &[ComponentEdit::Scalar {
            component: ProgrammingComponent::Pan,
            operation: ScalarEdit::Relative(5.),
        }],
        &FamilyEditContext::default(),
    )
    .unwrap();
    assert_eq!(
        turned,
        AttributeValue::Position(Arc::new(PositionIntent::angles(
            default.pan_degrees + 5.,
            default.tilt_degrees,
        )))
    );
    // Pending never borrows this Live seed, and a named displayed source still holds exactly.
    assert_eq!(
        capture(&state, owner, true).family_contexts[&owner].solved_angles,
        None
    );
    let mut environment = values_environment(&state);
    let mut named = intent(owner);
    named.displayed_source = Some(light_application::ProgrammingDisplayedSource {
        lane: light_application::ProgrammingDisplayedLane::Normal,
        lease: 1,
    });
    prepare_family_edit_context(&state, SessionId::new(), false, &named, &mut environment);
    assert_eq!(environment.family_contexts[&owner].solved_angles, None);
    let _ = std::fs::remove_dir_all(directory);
}

#[tokio::test]
async fn fixture_sheet_reads_the_accepted_commanded_pose_of_the_current_show_only() {
    let (state, directory) = crate::runtime::tests::test_state();
    let fixture = mover();
    let owner = fixture.fixture_id;
    install(&state, fixture);
    let owners = std::collections::HashSet::from([owner.0, Uuid::new_v4()]);
    let rows =
        || crate::runtime::position_readout::fixture_sheet_commanded_positions(&state, &owners);
    assert!(rows().is_empty(), "no accepted frame, no commanded pose");
    let frame = render(&state, owner, 1., 0., None);
    publish(&state, &frame, Some(Uuid::new_v4()));
    assert!(rows().is_empty(), "another show's frame is never shown");
    publish(&state, &frame, None);
    // Normalized 1/0 on the ±720° U16 mover: the same 720°/−720° the encoders read; the
    // unknown owner is left out rather than shown as 0°.
    assert_eq!(
        rows(),
        vec![serde_json::json!({
            "fixture_id": owner.0,
            "pan_degrees": 720.0,
            "tilt_degrees": -720.0,
        })]
    );
    let _ = std::fs::remove_dir_all(directory);
}

#[tokio::test]
async fn stale_generation_and_wrong_show_publications_remain_passive() {
    let (state, directory) = crate::runtime::tests::test_state();
    let fixture = mover();
    let owner = fixture.fixture_id;
    install(&state, fixture);
    let accepted = render(&state, owner, 1., 0., Some(requested([1., 2., 3.])));
    publish(&state, &accepted, None);
    assert!(
        capture(&state, owner, false).family_contexts[&owner]
            .solved_angles
            .is_some()
    );
    let mut snapshot = state.output.snapshot().as_ref().clone();
    snapshot.revision += 1;
    state.output.replace_snapshot(snapshot).unwrap();
    // TL-552: until the new generation publishes, the idle fixture adopts its declared default
    // pose, never the retained frame's 720° command pose.
    assert_eq!(
        capture(&state, owner, false).family_contexts[&owner].solved_angles,
        Some(declared(&state, owner)),
        "same show with a new runtime generation cannot reinterpret the retained pose"
    );
    let current = render(&state, owner, 1., 0., Some(requested([1., 2., 3.])));
    publish(&state, &current, Some(Uuid::new_v4()));
    assert_eq!(
        capture(&state, owner, false).family_contexts[&owner].solved_angles,
        None,
        "an accepted physical frame belonging to another show cannot seed this desk"
    );
    publish(&state, &current, None);
    assert!(
        capture(&state, owner, false).family_contexts[&owner]
            .solved_angles
            .is_some()
    );
    let _ = std::fs::remove_dir_all(directory);
}

#[tokio::test]
async fn divergent_copies_do_not_silently_adopt_the_root_or_fold_turns() {
    let (state, directory) = crate::runtime::tests::test_state();
    let mut fixture = mover();
    let owner = fixture.fixture_id;
    fixture.multipatch.push(MultiPatchInstance {
        id: Uuid::new_v4(),
        universe: Some(1),
        address: Some(10),
        position_calibration: Some(InstalledPositionCalibration {
            pan_zero_degrees: 360.,
            ..Default::default()
        }),
        ..Default::default()
    });
    install(&state, fixture);
    let frame = render(&state, owner, 1., 0., Some(requested([1., 2., 3.])));
    assert!(
        frame
            .rendered
            .physical
            .instances
            .iter()
            .all(|instance| instance.complete)
    );
    let pans = frame
        .rendered
        .physical
        .instances
        .iter()
        .map(|instance| {
            instance
                .axes()
                .iter()
                .find(|axis| axis.role == Some(PositionAxisRole::Pan))
                .unwrap()
                .absolute_degrees()
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(pans, [720., 1080.]);
    publish(&state, &frame, None);
    assert_eq!(
        capture(&state, owner, false).family_contexts[&owner].solved_angles,
        None,
        "one persisted Angle pair cannot represent divergent physical copies"
    );
    let _ = std::fs::remove_dir_all(directory);
}

#[tokio::test]
async fn pending_capture_cannot_borrow_a_valid_live_publication() {
    let (state, directory) = crate::runtime::tests::test_state();
    let fixture = mover();
    let owner = fixture.fixture_id;
    install(&state, fixture);
    let frame = render(&state, owner, 1., 0., Some(requested([1., 2., 3.])));
    publish(&state, &frame, None);
    assert!(
        capture(&state, owner, false).family_contexts[&owner]
            .solved_angles
            .is_some()
    );
    assert_eq!(
        capture(&state, owner, true).family_contexts[&owner].solved_angles,
        None,
        "Pending has its own retained output episode; Live is never its fallback"
    );
    let _ = std::fs::remove_dir_all(directory);
}

#[tokio::test]
async fn native_only_accepted_pair_can_seed_angle_edit_and_navigation_does_not_capture() {
    let (state, directory) = crate::runtime::tests::test_state();
    let fixture = mover();
    let owner = fixture.fixture_id;
    install(&state, fixture);
    let frame = render(&state, owner, 1., 0., None);
    publish(&state, &frame, None);
    let mut adopted = values_environment(&state);
    adopted.current_values.insert(
        (owner, ProgrammingOwner::Position.key()),
        requested([9., 8., 7.]),
    );
    prepare_family_edit_context(
        &state,
        SessionId::new(),
        false,
        &intent(owner),
        &mut adopted,
    );
    assert_eq!(
        adopted.family_contexts[&owner].solved_angles,
        Some(JointAngles {
            pan_degrees: 720.,
            tilt_degrees: -720.
        })
    );
    assert_eq!(
        adopted.current_values[&(owner, ProgrammingOwner::Position.key())],
        AttributeValue::Position(Arc::new(PositionIntent::angles(720., -720.)))
    );
    let mut environment = values_environment(&state);
    let mut navigation = intent(owner);
    navigation.operation = ProgrammingValueOperation::ComponentEdits(vec![]);
    prepare_family_edit_context(
        &state,
        SessionId::new(),
        false,
        &navigation,
        &mut environment,
    );
    assert!(!environment.family_contexts.contains_key(&owner));
    assert!(
        !environment
            .current_values
            .contains_key(&(owner, ProgrammingOwner::Position.key())),
        "opening or paging Position controls must not activate a typed Angle owner"
    );
    let _ = std::fs::remove_dir_all(directory);
}

#[tokio::test]
async fn native_custom_motor_aliases_expose_position_without_claiming_an_accepted_pose() {
    let (state, directory) = crate::runtime::tests::test_state();
    let mut fixture = mover();
    let owner = fixture.fixture_id;
    let mut profile = fixture
        .definition
        .profile_snapshot
        .as_deref()
        .unwrap()
        .clone();
    let mode_id = fixture.definition.mode_id.unwrap();
    for (channel, alias) in profile.modes[0]
        .channels
        .iter_mut()
        .zip(["motor.base.rotation", "motor.head.rotation"])
    {
        let key = AttributeKey(alias.into());
        channel.attribute = key.clone();
        channel.fixture_attribute = key.clone();
        for function in &mut channel.functions {
            function.attribute = key.clone();
        }
    }
    fixture.definition = profile.resolved_definition(mode_id).unwrap();
    install(&state, fixture);
    let environment = values_environment(&state);
    let supported = &environment.supported_attributes[&owner];
    assert!(
        supported.contains(&ProgrammingOwner::Position.key()),
        "semantic Position addressability derives from cold geometry motor ownership"
    );
    assert!(supported.contains(&AttributeKey("motor.base.rotation".into())));
    assert!(supported.contains(&AttributeKey("motor.head.rotation".into())));
    assert!(!supported.contains(&AttributeKey("pan".into())));
    assert!(!supported.contains(&AttributeKey("tilt".into())));
    assert!(state.output.latest_visualization_frame().is_none());
    let captured = capture(&state, owner, false);
    assert!(
        captured.family_contexts[&owner].position_adoption_attempted,
        "an actual Angle turn must record its attempted capture even when no output exists"
    );
    // TL-552: before any accepted frame the seed is the declared default pose decoded through
    // the compiled model from the profile's default words, never ownership alone or a guess.
    assert_eq!(
        captured.family_contexts[&owner].solved_angles,
        Some(declared(&state, owner)),
        "motor ownership alone is not evidence of an accepted commanded pair"
    );

    let old_snapshot = state.output.snapshot();
    assert!(
        state
            .output
            .engine()
            .position_has_native_controls(&old_snapshot, owner)
    );
    let mut replacement = old_snapshot.as_ref().clone();
    replacement.revision += 1;
    state.output.replace_snapshot(replacement).unwrap();
    assert!(
        !state
            .output
            .engine()
            .position_has_native_controls(&old_snapshot, owner),
        "capability reads cannot join an old snapshot to newer cold motor ownership"
    );
    assert!(
        state
            .output
            .engine()
            .position_has_native_controls(&state.output.snapshot(), owner)
    );
    assert!(
        values_environment(&state).supported_attributes[&owner]
            .contains(&ProgrammingOwner::Position.key())
    );
    let _ = std::fs::remove_dir_all(directory);
}

#[path = "displayed_source_tests.rs"]
mod displayed_source_tests;
