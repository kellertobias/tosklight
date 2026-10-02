use super::*;
use light_core::programming::ProgrammingOwner;
use light_dynamics::DynamicSemanticValue;
use light_programmer::{PreloadProgrammerValueMutation, PreloadProgrammerValueTiming};
use std::collections::HashSet;

const X: &str = "point.position.x";
const Y: &str = "point.position.y";

struct Rig {
    engine: Engine,
    point: FixtureId,
    mount: FixtureId,
    color: AttributeValue,
}

impl Rig {
    fn new(release: bool) -> Self {
        let registry = ProgrammerRegistry::default();
        let session = SessionId::new();
        registry.start(session);
        let (mut point, point_id) = schema_v2_fixture(&[
            (X, false, false, false, false, false),
            (Y, false, false, false, false, false),
        ]);
        point.fixture_number = None;
        point.universe = None;
        point.address = None;
        point
            .freeze
            .targets
            .entry(point_id)
            .or_default()
            .values
            .insert(AttributeKey(X.into()), AttributeValue::Normalized(0.6));
        let mount_id = FixtureId::new();
        let mut mount = calibrated_visual_fixture(mount_id);
        mount.position_master = Some(point_id.0);
        mount.location.x = 2_000;
        for key in [X, Y] {
            registry.set(
                session,
                point_id,
                AttributeKey(key.into()),
                AttributeValue::Normalized(0.5),
            );
        }
        registry.set(
            session,
            mount_id,
            AttributeKey("focus".into()),
            AttributeValue::Normalized(0.2),
        );
        let color = AttributeValue::ColorXyz(light_fixture::srgb_to_xyz(1., 0., 1.));
        registry.arm_preload(session, true);
        assert!(registry.apply_preload_values(
            session,
            &[PreloadProgrammerValueMutation::SetFixture {
                fixture_id: mount_id,
                attribute: AttributeKey::color(),
                value: color.clone(),
                timing: PreloadProgrammerValueTiming::default(),
            }]
        ));
        if release {
            assert!(registry.apply_dynamic_values(
                session,
                &[light_programmer::DynamicProgrammerValueMutation::Set {
                    fixture_id: mount_id,
                    attribute: AttributeKey::color(),
                    value: DynamicSemanticValue::Release,
                }],
                None
            ));
        }
        let engine = Engine::new(registry);
        engine
            .replace_snapshot(EngineSnapshot {
                fixtures: vec![point, mount].into(),
                ..Default::default()
            })
            .unwrap();
        engine.set_color_model(light_core::ColorProgrammingModel::Intent);
        Self {
            engine,
            point: point_id,
            mount: mount_id,
            color,
        }
    }

    fn samples(&self, y: f32) -> Vec<ContributionBatch> {
        vec![ContributionBatch::new([X, Y].map(|key| {
            ContributionSample::independent(TimedValue {
                fixture_id: self.point,
                attribute: AttributeKey(key.into()),
                value: AttributeValue::Normalized(if key == X { 0.8 } else { y }),
                priority: 10_000,
                changed_at: Utc::now(),
                programmer_order: 0,
                merge_mode: MergeMode::Ltp,
                fade: false,
                fade_millis: None,
                delay_millis: None,
            })
        }))]
    }
}

fn near(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < 0.0001, "{actual} != {expected}");
}

#[test]
fn preload_tokens_reject_live_wrong_branch_bundle_capture_state_and_stale_revision() {
    let rig = Rig::new(false);
    let engine = &rig.engine;
    let frame = engine.prepare_output_frame(Default::default());
    let input = engine.prepare_preload_frame(&frame, None);
    let other_input = engine.prepare_preload_frame(&frame, None);
    let other_frame = engine.prepare_output_frame(Default::default());
    let foreign_input = engine.prepare_preload_frame(&other_frame, None);
    let mut state = PreloadFrameState::default();
    let other_state = PreloadFrameState::default();
    let live_revision = engine.capture_output_continuity().0;
    let token = |input: &PreparedPreloadFrame<'_>, state: &PreloadFrameState, branch| {
        engine.prepare_preload_static_family_frame(input, &[], state, branch)
    };
    for invalid in [
        engine.prepare_static_family_frame(&frame, &[]),
        token(&input, &state, PreloadBranch::BeforeRelease),
        token(&other_input, &state, PreloadBranch::AfterRelease),
        token(&foreign_input, &state, PreloadBranch::AfterRelease),
        token(&input, &other_state, PreloadBranch::AfterRelease),
    ] {
        assert!(matches!(
            engine.render_prepared_preload_families(&input, None, invalid, &mut state),
            Err(EngineError::StalePreparedFrame)
        ));
    }
    assert!(
        engine
            .render_static_family_frame(&frame, token(&input, &state, PreloadBranch::AfterRelease))
            .is_err()
    );
    assert!(
        engine
            .preview_static_family_frame(&frame, token(&input, &state, PreloadBranch::AfterRelease))
            .is_err()
    );
    let after = token(&input, &state, PreloadBranch::AfterRelease);
    let wrong_before = token(&input, &state, PreloadBranch::AfterRelease);
    assert!(
        engine
            .render_prepared_preload_families(&input, Some(wrong_before), after, &mut state)
            .is_err()
    );
    let stale = token(&input, &state, PreloadBranch::AfterRelease);
    let after = token(&input, &state, PreloadBranch::AfterRelease);
    engine
        .render_prepared_preload_families(&input, None, after, &mut state)
        .unwrap();
    assert!(matches!(
        engine.render_prepared_preload_families(&input, None, stale, &mut state),
        Err(EngineError::StalePreparedFrame)
    ));
    assert_eq!(engine.capture_output_continuity().0, live_revision);
}

#[test]
fn preload_family_release_reuses_each_token_geometry_and_retains_both_observers() {
    let rig = Rig::new(true);
    let engine = &rig.engine;
    let frame = engine.prepare_output_frame(Default::default());
    let input = engine.prepare_preload_frame(&frame, None);
    let mut state = PreloadFrameState::default();
    assert!(engine.preload_requires_before_release(&input, &state));
    let live_revision = engine.capture_output_continuity().0;
    let index = frame.generation.point_projection();
    let count = index.resolutions();
    let mut before = engine.prepare_preload_static_family_frame(
        &input,
        &rig.samples(0.55),
        &state,
        PreloadBranch::BeforeRelease,
    );
    let mut after = engine.prepare_preload_static_family_frame(
        &input,
        &rig.samples(0.65),
        &state,
        PreloadBranch::AfterRelease,
    );
    assert_eq!(
        before.value(rig.mount, &AttributeKey::color()),
        Some(&rig.color)
    );
    assert!(after.value(rig.mount, &AttributeKey::color()).is_none());
    assert_eq!(
        after.value(rig.point, &AttributeKey(X.into())),
        Some(&AttributeValue::Normalized(0.8)),
        "Current remains pre-Freeze"
    );
    let before_geometry = engine
        .observe_static_family_geometry(&frame, &mut before)
        .unwrap();
    let after_geometry = engine
        .observe_static_family_geometry(&frame, &mut after)
        .unwrap();
    let repeated = engine
        .observe_static_family_geometry(&frame, &mut after)
        .unwrap();
    assert!(Arc::ptr_eq(&repeated.points, &after_geometry.points));
    near(before_geometry.points()[0].offset_metres[1], 10.);
    near(after_geometry.points()[0].offset_metres[1], 30.);
    near(after_geometry.points()[0].offset_metres[0], 20.);
    after
        .project_family(
            rig.mount,
            ProgrammingOwner::Focus,
            AttributeValue::Normalized(0.75),
            FamilyProjectionMetadata {
                changed_at: None,
                evidence: FamilyProjectionEvidence::Replace {
                    origin: None,
                    family_evidence: None,
                },
                master: FamilyProjectionMaster::PreserveBaseline,
            },
        )
        .unwrap();
    assert_eq!(
        after.value(rig.mount, &AttributeKey("focus".into())),
        Some(&AttributeValue::Normalized(0.2))
    );
    let result = engine
        .render_prepared_preload_families(&input, Some(before), after, &mut state)
        .unwrap();
    assert_eq!(
        index.resolutions(),
        count + 2,
        "finalization and Release projection must not resolve Points again"
    );
    assert!(Arc::ptr_eq(&result.source.points, &after_geometry.points));
    assert!(Arc::ptr_eq(&result.source.mounts, &after_geometry.mounts));
    assert!(Arc::ptr_eq(
        &result.projection.points,
        &after_geometry.points
    ));
    let original = result.before_release.as_ref().unwrap();
    assert!(Arc::ptr_eq(&original.points, &before_geometry.points));
    assert!(Arc::ptr_eq(&original.mounts, &before_geometry.mounts));
    assert_eq!(
        original.values().value(rig.mount, &AttributeKey::color()),
        Some(&rig.color)
    );
    assert_eq!(
        result
            .source
            .values()
            .value(rig.mount, &AttributeKey("focus".into())),
        Some(&AttributeValue::Normalized(0.75))
    );
    assert_eq!(
        &*result.projection.native_ownership[&rig.mount],
        &[false, true, true, true]
    );
    // Each successful branch has its own retained transform cache, including different poses.
    for (branch, y, geometry) in [
        (PreloadBranch::BeforeRelease, 0.55, before_geometry),
        (PreloadBranch::AfterRelease, 0.65, after_geometry),
    ] {
        let mut token =
            engine.prepare_preload_static_family_frame(&input, &rig.samples(y), &state, branch);
        let again = engine
            .observe_static_family_geometry(&frame, &mut token)
            .unwrap();
        assert!(Arc::ptr_eq(&again.mounts, &geometry.mounts));
        assert_eq!(token.continuity.mounts.recomputed_mounts, 0);
    }
    assert_eq!(engine.capture_output_continuity().0, live_revision);
}

#[test]
fn preload_missing_before_and_observer_branch_mismatch_never_fall_back_to_resampling() {
    let rig = Rig::new(true);
    let engine = &rig.engine;
    let frame = engine.prepare_output_frame(Default::default());
    let input = engine.prepare_preload_frame(&frame, None);
    let other_input = engine.prepare_preload_frame(&frame, None);
    let mut state = PreloadFrameState::default();
    let count = frame.generation.point_projection().resolutions();
    let after = engine.prepare_preload_static_family_frame(
        &input,
        &[],
        &state,
        PreloadBranch::AfterRelease,
    );
    assert!(
        engine
            .render_prepared_preload_families(&input, None, after, &mut state)
            .is_err()
    );
    assert_eq!(frame.generation.point_projection().resolutions(), count);
    let before = engine.observe_prepared_preload(&input, &[], &state, true);
    let after = engine.observe_prepared_preload(&input, &[], &state, false);
    let foreign = engine.observe_prepared_preload(&other_input, &[], &state, false);
    let releases = crate::programmer_release::ProgrammerReleaseMemo::default()
        .compile(&input.sources().after, &frame.generation);
    for (before, after) in [(&after, &before), (&before, &foreign), (&after, &after)] {
        assert!(
            engine
                .profile_color_release_frames(
                    before,
                    after,
                    &releases,
                    Default::default(),
                    &HashSet::new()
                )
                .is_err()
        );
    }
    engine
        .profile_color_release_frames(
            &before,
            &after,
            &releases,
            Default::default(),
            &HashSet::new(),
        )
        .unwrap();
}

#[test]
fn preload_failed_projection_and_dropped_geometry_preserve_revision_and_mount_cache() {
    let rig = Rig::new(false);
    let engine = &rig.engine;
    let frame = engine.prepare_output_frame(Default::default());
    let input = engine.prepare_preload_frame(&frame, None);
    let mut state = PreloadFrameState::default();
    let token = engine.prepare_preload_static_family_frame(
        &input,
        &rig.samples(0.55),
        &state,
        PreloadBranch::AfterRelease,
    );
    let accepted = engine
        .render_prepared_preload_families(&input, None, token, &mut state)
        .unwrap();
    assert!(accepted.before_release.is_none());
    let live_revision = engine.capture_output_continuity().0;
    let survivor = engine.prepare_preload_static_family_frame(
        &input,
        &rig.samples(0.55),
        &state,
        PreloadBranch::AfterRelease,
    );
    let mut dropped = engine.prepare_preload_static_family_frame(
        &input,
        &rig.samples(0.9),
        &state,
        PreloadBranch::AfterRelease,
    );
    engine
        .observe_static_family_geometry(&frame, &mut dropped)
        .unwrap();
    drop(dropped);
    let mut broken = engine.prepare_output_frame(Default::default());
    broken.generation = Arc::new(RuntimeGeneration::new(
        (*broken.snapshot()).clone(),
        broken.generation.playback_arc(),
        Default::default(),
        Default::default(),
        Default::default(),
    ));
    let broken_input = engine.prepare_preload_frame(&broken, None);
    let mut token = engine.prepare_preload_static_family_frame(
        &broken_input,
        &rig.samples(0.8),
        &state,
        PreloadBranch::AfterRelease,
    );
    engine
        .observe_static_family_geometry(&broken, &mut token)
        .unwrap();
    assert!(
        engine
            .render_prepared_preload_families(&broken_input, None, token, &mut state)
            .is_err()
    );
    // A token captured before both failures still commits, and the accepted cache was not replaced.
    let recovered = engine
        .render_prepared_preload_families(&input, None, survivor, &mut state)
        .unwrap();
    assert!(Arc::ptr_eq(
        &accepted.source.mounts,
        &recovered.source.mounts
    ));
    assert_eq!(engine.capture_output_continuity().0, live_revision);
}

#[test]
fn preload_scalar_current_keeps_release_freeze_and_sources_without_geometry_work() {
    let rig = Rig::new(true);
    let engine = &rig.engine;
    let frame = engine.prepare_output_frame(Default::default());
    let input = engine.prepare_preload_frame(&frame, None);
    let mut state = PreloadFrameState::default();
    let index = frame.generation.point_projection();
    let initial_continuity = engine.capture_output_continuity().0;
    // Neither Current observation may consume the state revision used by these final tokens.
    let before_token = engine.prepare_preload_static_family_frame(
        &input,
        &[],
        &state,
        PreloadBranch::BeforeRelease,
    );
    let after_token = engine.prepare_preload_static_family_frame(
        &input,
        &[],
        &state,
        PreloadBranch::AfterRelease,
    );
    for branch in [PreloadBranch::BeforeRelease, PreloadBranch::AfterRelease] {
        let samples = rig.samples(0.55);
        let count = index.resolutions();
        let values = engine.observe_prepared_preload_values(&input, &samples, &state, branch);
        assert_eq!(
            index.resolutions(),
            count,
            "pending scalar Current must not solve geometry"
        );
        assert!(!values.materialised_by_name());
        assert_eq!(engine.capture_output_continuity().0, initial_continuity);
        assert_eq!(
            values.value(rig.point, &AttributeKey(X.into())),
            Some(&AttributeValue::Normalized(0.6))
        );
        assert_eq!(
            values.value(rig.point, &AttributeKey(Y.into())),
            Some(&AttributeValue::Normalized(0.55))
        );
        assert_eq!(
            values.value(rig.mount, &AttributeKey::color()),
            (branch == PreloadBranch::BeforeRelease).then_some(&rig.color),
            "the Release branch retains its own captured Color visibility",
        );
        let observed = engine.observe_prepared_preload(
            &input,
            &samples,
            &state,
            branch == PreloadBranch::BeforeRelease,
        );
        assert_eq!(index.resolutions(), count + 1);
        for (target, key) in [
            (rig.point, AttributeKey(X.into())),
            (rig.point, AttributeKey(Y.into())),
            (rig.mount, AttributeKey::color()),
            (rig.mount, AttributeKey("focus".into())),
        ] {
            assert_eq!(
                values.value(target, &key),
                observed.values().value(target, &key)
            );
            assert_eq!(
                values.changed_at(target, &key),
                observed.values().changed_at(target, &key)
            );
            let origin = |values: &crate::FrameValues| {
                values.contribution_origin(target, &key).map(|origin| {
                    (
                        origin.source().clone(),
                        origin.stamp().changed_at,
                        origin.stamp().programmer_order,
                        origin.transition_ordinal(),
                    )
                })
            };
            assert_eq!(origin(&values), origin(observed.values()));
            let left = values.contribution_family_evidence(target, &key);
            let right = observed.values().contribution_family_evidence(target, &key);
            assert_eq!(left.is_some(), right.is_some());
            if let (Some(left), Some(right)) = (left, right) {
                assert_eq!(left.entries().len(), right.entries().len());
                for (left, right) in left.entries().iter().zip(right.entries()) {
                    assert_eq!(left.source(), right.source());
                    assert_eq!(left.stamp().changed_at, right.stamp().changed_at);
                    assert_eq!(
                        left.stamp().programmer_order,
                        right.stamp().programmer_order
                    );
                    assert_eq!(left.transition_ordinal(), right.transition_ordinal());
                    assert_eq!(left.authored_cue_id(), right.authored_cue_id());
                    assert_eq!(left.footprint(), right.footprint());
                    assert_eq!(left.role(), right.role());
                    assert_eq!(left.effective_fields(), right.effective_fields());
                }
            }
        }
        assert!(!values.materialised_by_name());
    }
    engine
        .render_prepared_preload_families(&input, Some(before_token), after_token, &mut state)
        .unwrap();
    assert_eq!(engine.capture_output_continuity().0, initial_continuity);
}
