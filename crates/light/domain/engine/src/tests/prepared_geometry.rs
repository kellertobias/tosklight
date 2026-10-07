use super::*;
use light_core::programming::ProgrammingOwner;

const X: &str = "point.position.x";
const Y: &str = "point.position.y";

fn fixtures(logical: bool) -> (PatchedFixture, PatchedFixture, FixtureId) {
    let (mut point, root) = schema_v2_fixture(&[(X, false, false), (Y, false, false)]);
    point.fixture_number = None;
    point.universe = None;
    point.address = None;
    let owner = if logical {
        let mut profile = point
            .definition
            .profile_snapshot
            .as_deref()
            .unwrap()
            .clone();
        let mode = &mut profile.modes[0];
        mode.heads[0].master_shared = false;
        let head_id = mode.heads[0].id;
        let mode_id = mode.id;
        let child = FixtureId::new();
        point.definition = profile.resolved_definition(mode_id).unwrap();
        point.logical_heads = vec![PatchedHead {
            profile_head_id: Some(head_id),
            head_index: 0,
            fixture_id: child,
        }];
        child
    } else {
        root
    };
    let (mut mount, _) = schema_v2_fixture(&[("focus", false, false)]);
    mount.fixture_number = None;
    mount.universe = None;
    mount.address = None;
    mount.position_master = Some(root.0);
    mount.location.x = 2_000;
    (point, mount, owner)
}

fn freeze(fixture: &mut PatchedFixture, owner: FixtureId, attribute: &str, value: AttributeValue) {
    fixture
        .freeze
        .targets
        .entry(owner)
        .or_default()
        .values
        .insert(AttributeKey(attribute.into()), value);
}

fn engine(point: PatchedFixture, mount: PatchedFixture, owner: FixtureId) -> Engine {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    registry.start(session);
    for name in [X, Y] {
        registry.set(
            session,
            owner,
            AttributeKey(name.into()),
            AttributeValue::Normalized(0.5),
        );
    }
    registry.set(
        session,
        mount.fixture_id,
        AttributeKey("focus".into()),
        AttributeValue::Normalized(0.2),
    );
    let engine = Engine::new(registry);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![point, mount].into(),
            ..Default::default()
        })
        .unwrap();
    engine
}

fn sample(owner: FixtureId, attribute: &str, value: f32) -> ContributionBatch {
    ContributionBatch::new([ContributionSample::independent(TimedValue {
        fixture_id: owner,
        attribute: AttributeKey(attribute.into()),
        value: AttributeValue::Normalized(value),
        priority: 10_000,
        changed_at: Utc::now(),
        programmer_order: 0,
        merge_mode: MergeMode::Ltp,
        fade: false,
        fade_millis: None,
        delay_millis: None,
    })])
}

fn near(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 0.0001, "{actual} != {expected}");
}

#[test]
fn scalar_current_observation_keeps_freeze_and_evidence_without_geometry_work() {
    let (mut point, mount, owner) = fixtures(false);
    let mounted = mount.fixture_id;
    freeze(&mut point, owner, X, AttributeValue::Normalized(0.7));
    let engine = engine(point, mount, owner);
    let capture = engine.prepare_output_frame(Default::default());
    let index = capture.generation.point_projection();
    let before = index.resolutions();
    let continuity = engine.capture_output_continuity().0;
    let values = engine.observe_prepared_values(&capture, &[sample(owner, Y, 0.55)]);
    assert_eq!(
        index.resolutions(),
        before,
        "scalar Current never solves geometry"
    );
    assert_eq!(engine.capture_output_continuity().0, continuity);
    assert!(!values.materialised_by_name());
    assert_eq!(
        values.value(owner, &AttributeKey(X.into())),
        Some(&AttributeValue::Normalized(0.7))
    );
    let observed = engine.observe_prepared_frame(&capture, &[sample(owner, Y, 0.55)]);
    assert_eq!(index.resolutions(), before + 1);
    for (target, key) in [
        (owner, AttributeKey(X.into())),
        (owner, AttributeKey(Y.into())),
        (mounted, AttributeKey("focus".into())),
    ] {
        assert_eq!(
            values.value(target, &key),
            observed.values().value(target, &key)
        );
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
                assert_eq!(left.footprint(), right.footprint());
                assert_eq!(left.role(), right.role());
                assert_eq!(left.effective_fields(), right.effective_fields());
                assert_eq!(left.transition_ordinal(), right.transition_ordinal());
                assert_eq!(left.authored_cue_id(), right.authored_cue_id());
            }
        }
    }
    assert_eq!(engine.capture_output_continuity().0, continuity);
}

#[test]
fn frozen_geometry_is_cached_once_and_reused_by_final_render_after_typed_writes() {
    let (mut point, mut mount, owner) = fixtures(false);
    let id = mount.fixture_id;
    freeze(&mut point, owner, X, AttributeValue::Normalized(0.6));
    // The later fixture's stored value wins even without full/family metadata.
    freeze(&mut mount, owner, X, AttributeValue::Normalized(0.7));
    let engine = engine(point, mount, owner);
    let capture = engine.prepare_output_frame(Default::default());
    let mut token = engine
        .prepare_static_family_frame(&capture, &[sample(owner, X, 0.55), sample(owner, Y, 0.55)]);
    let index = capture.generation.point_projection();
    let before = index.resolutions();
    let geometry = engine
        .observe_static_family_geometry(&capture, &mut token)
        .unwrap();
    assert_eq!(index.resolutions(), before + 1);
    assert_eq!(geometry.generation(), capture.generation());
    assert_eq!(geometry.sampled_at(), capture.sampled_at());
    near(f64::from(geometry.points()[0].offset_metres[0]), 40.0);
    near(f64::from(geometry.points()[0].offset_metres[1]), 10.0);
    near(
        geometry
            .mounts()
            .mount(id.0)
            .unwrap()
            .world_from_fixture
            .unwrap()
            .point([0.0; 3])[0],
        42.0,
    );
    // Freeze holds parameters before DMX (2026-10-05): the baseline already holds the frozen
    // value, so family adapters render it.
    assert_eq!(
        token.value(owner, &AttributeKey(X.into())),
        Some(&AttributeValue::Normalized(0.7))
    );
    token
        .project_family(
            id,
            ProgrammingOwner::Focus,
            AttributeValue::Normalized(0.8),
            FamilyProjectionMetadata {
                changed_at: None,
                evidence: FamilyProjectionEvidence::PreserveBaseline,
            },
        )
        .unwrap();
    let again = engine
        .observe_static_family_geometry(&capture, &mut token)
        .unwrap();
    assert!(Arc::ptr_eq(&geometry.points, &again.points));
    assert!(Arc::ptr_eq(&geometry.mounts, &again.mounts));
    let output = engine.render_static_family_frame(&capture, token).unwrap();
    assert_eq!(
        index.resolutions(),
        before + 1,
        "final rendering must not resolve Point geometry again"
    );
    assert!(Arc::ptr_eq(&output.points, &geometry.points));
    assert!(Arc::ptr_eq(&output.mounts, &geometry.mounts));
    // A second mount resolve with these unchanged poses would reset both counters to zero.
    // The candidate from the observation is committed directly after physical success.
    {
        let continuity = engine.output_continuity.lock();
        assert_eq!(continuity.mounts.recomputed_points, 1);
        assert_eq!(continuity.mounts.recomputed_mounts, 1);
    }
    assert!(!output.resolved_values.materialised_by_name());
    assert_eq!(
        output
            .resolved_values
            .value(id, &AttributeKey("focus".into())),
        Some(&AttributeValue::Normalized(0.8))
    );
    assert_eq!(
        output.resolved_values.value(owner, &AttributeKey(X.into())),
        Some(&AttributeValue::Normalized(0.7))
    );
}

#[test]
fn same_capture_tokens_keep_distinct_scalar_geometry_and_dropping_them_does_not_commit() {
    let (point, mount, owner) = fixtures(false);
    let engine = engine(point, mount, owner);
    let live = engine.render(Default::default()).unwrap();
    let capture = engine.prepare_output_frame(Default::default());
    let revision = engine.capture_output_continuity().0;
    let mut left = engine.prepare_static_family_frame(&capture, &[sample(owner, X, 0.55)]);
    let mut right = engine.prepare_static_family_frame(&capture, &[sample(owner, X, 0.6)]);
    let a = engine
        .observe_static_family_geometry(&capture, &mut left)
        .unwrap();
    let b = engine
        .observe_static_family_geometry(&capture, &mut right)
        .unwrap();
    near(f64::from(a.points()[0].offset_metres[0]), 10.0);
    near(f64::from(b.points()[0].offset_metres[0]), 20.0);
    assert!(!Arc::ptr_eq(&a.points, &b.points));
    assert!(!Arc::ptr_eq(&a.mounts, &b.mounts));
    drop((left, right));
    assert_eq!(engine.capture_output_continuity().0, revision);
    let unchanged = engine.render(Default::default()).unwrap();
    assert!(Arc::ptr_eq(&live.mounts, &unchanged.mounts));
    near(f64::from(a.points()[0].offset_metres[0]), 10.0);
}

#[test]
fn wrong_capture_rejects_before_geometry_or_candidate_continuity_changes() {
    let (point, mount, owner) = fixtures(false);
    let engine = engine(point, mount, owner);
    let capture = engine.prepare_output_frame(Default::default());
    let other = engine.prepare_output_frame(Default::default());
    let mut token = engine.prepare_static_family_frame(&capture, &[]);
    let index = capture.generation.point_projection();
    let count = index.resolutions();
    assert!(matches!(
        engine.observe_static_family_geometry(&other, &mut token),
        Err(EngineError::StalePreparedFrame)
    ));
    assert!(token.geometry.is_none());
    assert_eq!(index.resolutions(), count);
    assert_eq!(token.continuity.mounts.recomputed_mounts, 0);
    let geometry = engine
        .observe_static_family_geometry(&capture, &mut token)
        .unwrap();
    assert!(matches!(
        engine.observe_static_family_geometry(&other, &mut token),
        Err(EngineError::StalePreparedFrame)
    ));
    assert_eq!(index.resolutions(), count + 1);
    assert!(Arc::ptr_eq(
        &geometry.points,
        &token.geometry.as_ref().unwrap().points
    ));
}

#[test]
fn cached_geometry_preview_and_failed_render_never_advance_live() {
    let (point, mount, owner) = fixtures(false);
    let engine = engine(point, mount, owner);
    let live = engine.render(Default::default()).unwrap();
    let capture = engine.prepare_output_frame(Default::default());
    let revision = engine.capture_output_continuity().0;
    let mut token = engine.prepare_static_family_frame(&capture, &[sample(owner, X, 0.55)]);
    let geometry = engine
        .observe_static_family_geometry(&capture, &mut token)
        .unwrap();
    let count = capture.generation.point_projection().resolutions();
    let preview = engine.preview_static_family_frame(&capture, token).unwrap();
    assert!(Arc::ptr_eq(&geometry.points, &preview.points));
    assert!(Arc::ptr_eq(&geometry.mounts, &preview.mounts));
    assert_eq!(capture.generation.point_projection().resolutions(), count);
    assert_eq!(engine.capture_output_continuity().0, revision);
    let mut broken = engine.prepare_output_frame(Default::default());
    broken.generation = Arc::new(RuntimeGeneration::new(
        (*broken.snapshot()).clone(),
        broken.generation.playback_arc(),
        Default::default(),
        Default::default(),
        Default::default(),
    ));
    let mut token = engine.prepare_static_family_frame(&broken, &[sample(owner, X, 0.6)]);
    let failed_geometry = engine
        .observe_static_family_geometry(&broken, &mut token)
        .unwrap();
    assert!(engine.render_static_family_frame(&broken, token).is_err());
    assert_eq!(engine.capture_output_continuity().0, revision);
    let unchanged = engine.render(Default::default()).unwrap();
    assert!(Arc::ptr_eq(&live.mounts, &unchanged.mounts));
    near(
        f64::from(failed_geometry.points()[0].offset_metres[0]),
        20.0,
    );
}

#[test]
fn compiled_freeze_reader_matches_existing_override_for_v1_v2_heads_and_root_fallback() {
    // The v1 row validates semantic geometry parity; it does not add v1 physical rendering.
    for (schema, logical) in [(1, false), (2, false), (2, true)] {
        for non_normalized in [false, true] {
            let (mut point, mut mount, owner) = fixtures(logical);
            let root = point.fixture_id;
            if schema == 1 {
                point.definition.schema_version = 1;
                point.definition.profile_snapshot = None;
                point.definition.mode_id = None;
            }
            freeze(&mut point, root, X, AttributeValue::Normalized(0.6));
            freeze(&mut mount, root, X, AttributeValue::Normalized(0.65));
            if non_normalized {
                freeze(&mut mount, owner, X, AttributeValue::RawDmx(128));
            }
            let fixtures = vec![point, mount];
            let slots = Arc::new(SlotTable::compile(next_generation(), &fixtures));
            let index = crate::point_projection::PointProjectionIndex::compile(&fixtures, &slots);
            let mut resolved = EngineContributionResolver::unpooled(&slots).finish();
            // No logical owner exists in this underlay: root fallback is required. A present
            // invalid frozen owner must nevertheless suppress that fallback, producing zero.
            resolved.override_value(
                root,
                &AttributeKey(X.into()),
                AttributeValue::Normalized(0.55),
                None,
            );
            let borrowed = index.resolve_pre_freeze(resolved.frame.as_ref().unwrap());
            assert_eq!(
                resolved
                    .frame
                    .as_ref()
                    .unwrap()
                    .winner(root, &AttributeKey(X.into()))
                    .unwrap()
                    .value,
                AttributeValue::Normalized(0.55)
            );
            crate::render::apply_fixture_freezes(&fixtures, &mut resolved);
            let ordinary = index.resolve(&resolved.named_values());
            assert_eq!(
                borrowed.as_slice(),
                ordinary.as_slice(),
                "schema={schema}, logical={logical}, non_normalized={non_normalized}"
            );
            near(
                f64::from(borrowed[0].offset_metres[0]),
                if non_normalized { 0.0 } else { 30.0 },
            );
        }
    }
}

#[test]
fn logical_owner_freeze_and_replacement_generation_match_ordinary_render() {
    let (mut point, mount, owner) = fixtures(true);
    let root = point.fixture_id;
    freeze(&mut point, root, X, AttributeValue::Normalized(0.7));
    freeze(&mut point, owner, X, AttributeValue::Normalized(0.6));
    let engine = engine(point.clone(), mount.clone(), owner);
    let old = engine.prepare_output_frame(Default::default());
    let mut token = engine.prepare_static_family_frame(&old, &[sample(owner, X, 0.55)]);
    let geometry = engine
        .observe_static_family_geometry(&old, &mut token)
        .unwrap();
    near(f64::from(geometry.points()[0].offset_metres[0]), 20.0);
    let observed = engine.observe_prepared_frame(&old, &[sample(owner, X, 0.55)]);
    assert_eq!(geometry.points(), observed.points());
    assert_eq!(geometry.mounts().mounts(), observed.mounts().mounts());
    freeze(&mut point, owner, X, AttributeValue::Normalized(0.65));
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![point, mount].into(),
            revision: 2,
            ..Default::default()
        })
        .unwrap();
    let old_again = engine
        .observe_static_family_geometry(&old, &mut token)
        .unwrap();
    assert!(Arc::ptr_eq(&geometry.points, &old_again.points));
    let next = engine.prepare_output_frame(Default::default());
    let mut token = engine.prepare_static_family_frame(&next, &[]);
    let updated = engine
        .observe_static_family_geometry(&next, &mut token)
        .unwrap();
    near(f64::from(updated.points()[0].offset_metres[0]), 30.0);
    let final_output = engine.render_static_family_frame(&next, token).unwrap();
    assert!(Arc::ptr_eq(&updated.points, &final_output.points));
    near(f64::from(geometry.points()[0].offset_metres[0]), 20.0);
}

#[path = "prepared_geometry/point_changes.rs"]
mod point_changes;
