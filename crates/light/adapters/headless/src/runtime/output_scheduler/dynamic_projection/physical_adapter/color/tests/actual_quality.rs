//! Actual quality must follow the written shared control, while fitting match figures keep
//! describing the unshared proposal. Cross-target arbitration remains a TL-548 adoption gate.
use super::super::profiles::*;
use super::super::tests::{intent, program};
use super::super::*;
use light_core::{ManualClock, SessionId};
use light_dynamics::DynamicRuntime;
use light_engine::{Engine, RenderOptions};
use light_fixture::{FixtureHead, OpticalTransmission, PatchedHead};
use light_programmer::ProgrammerRegistry;

#[test]
fn a_shared_wheel_unknown_on_the_losing_head_reports_actual_incomplete_quality() {
    let mut profile = wheel_only();
    let mode = &mut profile.modes[0];
    mode.heads[0].master_shared = true;
    let second = Uuid::new_v4();
    mode.heads.push(FixtureHead {
        id: second,
        name: "Cell 2".into(),
        master_shared: false,
    });
    let paths = &mut mode.color_physical.as_mut().unwrap().paths;
    let mut path = paths[0].clone();
    path.id = Uuid::new_v4();
    path.head_id = second;
    path.filters[0].id = Uuid::new_v4();
    let OpticalTransmission::Spectral { samples } = &mut path.filters[0].transmission else {
        unreachable!()
    };
    // Open is known for both heads. Blue is known only for the first head; the missing
    // spectral sample makes the actual Blue value incomplete on the second head.
    samples.retain(|sample| sample.raw_to < 16);
    paths.push(path);
    profile.validate().unwrap();

    let root = FixtureId::new();
    let unpatched = patched(&profile, root, 1);
    let mut validated = unpatched.clone();
    validated.logical_heads.push(PatchedHead {
        profile_head_id: Some(second),
        head_index: 1,
        fixture_id: FixtureId::new(),
    });
    let clock = Arc::new(ManualClock::new(
        chrono::DateTime::from_timestamp_millis(1_000_000).unwrap(),
    ));
    let programmers = ProgrammerRegistry::with_clock(clock);
    programmers.start(SessionId::new());
    let engine = Engine::new(programmers);
    engine
        .replace_snapshot(light_engine::EngineSnapshot {
            fixtures: vec![validated].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    let adapter = ColorAdapter::default();
    // Exercise same-descriptor sharing. A validated patch assigns Cell 2 a logical target;
    // compiling the unpatched form gives one descriptor both heads, with identical channel
    // space. Production cross-target shared-slot arbitration is separately gated by TL-548.
    let descriptor = adapter
        .compile(
            &light_engine::EngineSnapshot {
                fixtures: vec![unpatched].into(),
                revision: 1,
                ..Default::default()
            },
            root,
        )
        .unwrap()
        .unwrap();
    assert_eq!(descriptor.heads.len(), 2);
    let capture = engine.prepare_output_frame(RenderOptions::default());
    let token = capture.frame_token();
    let mut scalar = engine.prepare_static_family_frame(&capture, &[]);
    let geometry = engine
        .observe_static_family_geometry(&capture, &mut scalar)
        .unwrap();
    let models = DynamicRuntime::default().captured_native_color_models();
    let request = intent([0., 0., 1.], 0.);
    let value = program(&request);
    let resolved = adapter
        .resolve(PhysicalRequest {
            frame: HybridFrameContext {
                capture: &capture,
                geometry: &geometry,
                native_models: models.as_ref(),
                token: &token,
                scalar: &scalar,
            },
            target: root,
            owner: ProgrammingOwner::Color,
            descriptor: &descriptor,
            value: &value,
            previous: None,
        })
        .unwrap();
    validate_complete_writes(&descriptor.footprint, &resolved.writes).unwrap();
    assert_eq!(resolved.requested, request);
    assert_eq!(resolved.writes.len(), 1);
    assert!(
        (32..=47).contains(&resolved.writes[0].raw),
        "first head selects Blue"
    );
    assert!(resolved.quality.heads[0].achieved.visible.is_some());

    let head = &descriptor.heads[1];
    let scratch = head.scratch.lock();
    let proposal = &scratch.output;
    assert!(
        proposal.visible.achieved.is_some(),
        "second head proposed known Open"
    );
    assert_ne!(proposal.total_quality, PhysicalDataQuality::Unknown);
    assert!(proposal.writes.iter().any(|write| write.raw < 16));
    let actual = &scratch.forward[head.head];
    assert!(!actual.visible_complete);
    let outcome = &resolved.quality.heads[1];
    assert!(outcome.quality.shared_conflict);
    assert!(
        outcome
            .quality
            .limitations
            .contains(ColorFitLimitations::SHARED_CONTROL)
    );
    assert_eq!(outcome.achieved.visible, None);
    assert_eq!(outcome.achieved.known_xyz, actual.known_xyz);
    assert_eq!(
        outcome.quality.visible,
        VisibleFitStatus::PredictionIncomplete
    );
    assert_eq!(outcome.quality.data_quality, actual.data_quality);
    assert_eq!(outcome.quality.total_quality, PhysicalDataQuality::Unknown);
    assert_eq!(
        outcome.quality.nominal,
        matches!(
            actual.data_quality,
            PhysicalDataQuality::Unknown | PhysicalDataQuality::Estimated
        )
    );
    assert!(
        outcome.quality.uv_appearance_known,
        "no active UV is known zero"
    );
    assert_eq!(outcome.quality.color_match, proposal.visible.color_match);
    assert_eq!(outcome.quality.delta_uv, proposal.visible.delta_uv);
    assert_eq!(
        outcome.quality.luminance_ratio,
        proposal.visible.luminance_ratio
    );
    assert_eq!(
        outcome.quality.luminance_limited,
        proposal.visible.luminance_limited
    );
}
