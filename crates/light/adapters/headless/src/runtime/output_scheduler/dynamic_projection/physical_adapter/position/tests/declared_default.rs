//! TL-552: an Angle Dynamic over a fixture nothing has given a Position starts from the
//! fixture's declared default pose (its channels' `default_raw` decoded through the compiled
//! Position model) instead of staying passive. Unknown defaults stay unknown, never 0°.
use super::numeric::requirement_debug;
use super::programs::{commanded_angles, destination_dynamic_rig, position_definition};
use super::*;
use light_dynamics::{
    DynamicDefinition, DynamicDefinitionSnapshot, DynamicFamilyRepresentation,
    DynamicInstanceOverrides, DynamicReference, DynamicValue, DynamicValueTiming, Rational,
};

const PAN: f32 = 60.;

/// Same as `programs::start_position_dynamic_sized`, without a Programmer Position base.
fn start_without_base(rig: &Rig, definition: &DynamicDefinition, size: f32) -> DynamicRuntime {
    let snapshot = rig.engine.snapshot();
    rig.engine
        .replace_snapshot(EngineSnapshot {
            dynamics: vec![definition.clone()].into(),
            revision: snapshot.revision + 1,
            ..snapshot.as_ref().clone()
        })
        .unwrap();
    assert!(rig.programmers.apply_dynamic_values(
        rig.session,
        &[DynamicProgrammerValueMutation::Set {
            fixture_id: rig.root,
            attribute: ProgrammingOwner::Position.key(),
            value: DynamicSemanticValue::DynamicOn {
                instance_link: Uuid::new_v4(),
                lane_id: definition.lanes[0].id,
                dynamic: DynamicReference {
                    dynamic_id: Some(definition.id),
                    last_known_pool_number: definition.pool_number,
                    embedded_fallback: DynamicDefinitionSnapshot {
                        definition: Arc::new(definition.clone())
                    },
                },
                overrides: DynamicInstanceOverrides {
                    size,
                    speed_multiplier: Rational::ONE,
                    phase_offset_degrees: 0.
                },
                timing: DynamicValueTiming {
                    fade_millis: None,
                    delay_millis: None
                },
            },
        }],
        None
    ));
    let mut runtime =
        DynamicRuntime::with_programming_contract_support(PROGRAMMING_CONTRACT_VERSION);
    runtime.install_definitions([definition.clone()]).unwrap();
    runtime
}

/// A constant Pan; the definition's automatic Tilt partner reads Current.
fn pan_definition() -> DynamicDefinition {
    position_definition(
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Angles,
            component: Some(ProgrammingComponent::Pan),
        },
        [DynamicValue::Scalar(PAN), DynamicValue::Scalar(PAN)],
    )
}

fn frame(rig: &Rig, runtime: &mut DynamicRuntime) -> PublishedPhysicalFrame<PositionAdapter> {
    let lane = PhysicalAdapterLane::live(PositionAdapter::default());
    let capture = rig.capture();
    prepare_live(
        rig,
        &capture,
        &capture,
        &lane,
        runtime,
        &mut DynamicSourceOrigins::default(),
        &mut HybridFrameScratch::default(),
    )
    .unwrap()
}

#[test]
fn angle_dynamic_without_static_position_starts_from_the_declared_default_pose() {
    for size in [1.0_f32, 1.25] {
        let rig = Rig::single();
        let default = rig
            .engine
            .declared_default_position(&rig.engine.snapshot(), rig.root)
            .expect("the U16 mover's default_raw maps into Angles");
        // 32768 of 65535 on a 720..−720° channel: just off centre, never an invented 0.
        assert!(default.pan_degrees.abs() < 0.05 && default.tilt_degrees.abs() < 0.05);
        assert_ne!(default.pan_degrees, 0.0);
        let definition = pan_definition();
        let mut runtime = start_without_base(&rig, &definition, size);
        // First frame already: the Dynamic must not wait for an unrelated Position edit.
        let output = frame(&rig, &mut runtime);
        assert!(
            output.requirements.is_empty(),
            "size {size}: {:?}",
            output
                .requirements
                .iter()
                .map(|r| requirement_debug(&r.reason))
                .collect::<Vec<_>>()
        );
        assert_eq!(output.results.len(), 1, "size {size}");
        let row = &output.results[0];
        let [destination] = row.achieved.destinations.as_slice() else {
            panic!("one physical destination")
        };
        let [pan, tilt] = commanded_angles(&destination.value);
        // Controller Size works around the declared default: Current + (sample − Current)·Size.
        let expected_pan = f64::from(default.pan_degrees)
            + (f64::from(PAN) - f64::from(default.pan_degrees)) * f64::from(size);
        assert!((pan - expected_pan).abs() < 0.05, "size {size}: Pan {pan}");
        assert!(
            (tilt - f64::from(default.tilt_degrees)).abs() < 0.05,
            "size {size}: the Current Tilt partner is the default Tilt, got {tilt}"
        );
        assert!(
            row.writes.iter().any(|w| w.raw != 32768),
            "the fitted Pan leaves the default word"
        );
    }
}

#[test]
fn unknown_or_absent_defaults_stay_passive_and_idle_fixtures_get_no_position_row() {
    // Negative control 1: the copy's calibration decodes the same default word to different
    // Angles, so no common declared default exists. The Dynamic stays a passive requirement.
    let (rig, _) = destination_dynamic_rig();
    assert!(
        rig.engine
            .declared_default_position(&rig.engine.snapshot(), rig.root)
            .is_none()
    );
    let mut runtime = start_without_base(&rig, &pan_definition(), 1.);
    let output = frame(&rig, &mut runtime);
    assert!(output.results.is_empty());
    assert!(
        output
            .requirements
            .iter()
            .any(|r| r.owner == ProgrammingOwner::Position),
        "unknown default remains a visible requirement, not a 0° pose"
    );
    // Negative control 2: nothing running, nothing programmed: no Position row is invented.
    let rig = Rig::single();
    let mut runtime =
        DynamicRuntime::with_programming_contract_support(PROGRAMMING_CONTRACT_VERSION);
    let output = frame(&rig, &mut runtime);
    assert!(output.results.is_empty());
    assert!(output.requirements.is_empty());
}
