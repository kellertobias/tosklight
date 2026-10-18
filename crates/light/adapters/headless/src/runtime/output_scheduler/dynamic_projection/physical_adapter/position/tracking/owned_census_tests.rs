//! Private draft oracles: original borrowed preparation and canonicalization stay frozen.
use super::*;
use light_core::{AttributeKey, AttributeValue, SessionId};
use light_dynamics::DynamicRuntime;
use light_engine::{Engine, EngineSnapshot};
use light_fixture::{ChannelFunction, FixtureProfile, PatchedFixture};
use light_programmer::ProgrammerRegistry;
use serde_json::json;

fn fixture(id: FixtureId, point: bool) -> PatchedFixture {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Test".into();
    profile.name = "Tracking dependency test".into();
    if point {
        let mode = &mut profile.modes[0];
        let head = mode.heads[0].id;
        mode.splits[0].footprint = 1;
        mode.channels.push(serde_json::from_value(json!({
                "id":Uuid::new_v4(), "head_id":head, "split":1,
                "fixture_attribute":"point.position.x", "attribute":"point.position.x",
                "resolution":"u8", "secondary_slots":[], "default_raw":128,
                "highlight_raw":255,
                "functions":[ChannelFunction::continuous("X", AttributeKey("point.position.x".into()),255)]
            })).unwrap());
    }
    let definition = profile.resolved_definition(profile.modes[0].id).unwrap();
    serde_json::from_value(json!({"fixture_id":id,"definition":definition})).unwrap()
}

fn engine(fixtures: Vec<PatchedFixture>) -> (Engine, ProgrammerRegistry, SessionId) {
    let programmers = ProgrammerRegistry::default();
    let session = SessionId::new();
    programmers.start(session);
    for fixture in &fixtures {
        if fixture
            .definition
            .heads
            .iter()
            .flat_map(|head| &head.parameters)
            .any(|parameter| parameter.attribute.0.as_ref() == "point.position.x")
        {
            programmers.set(
                session,
                fixture.fixture_id,
                AttributeKey("point.position.x".into()),
                AttributeValue::Normalized(0.5),
            );
        }
    }
    let engine = Engine::new(programmers.clone());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: fixtures.into(),
            ..Default::default()
        })
        .unwrap();
    (engine, programmers, session)
}

fn owner(target: FixtureId, root: FixtureId, points: Vec<FixtureId>) -> TrackingOwner {
    TrackingOwner {
        target,
        root,
        destinations: vec![root],
        points,
        mount_references: vec![(root, None)],
        incomplete: false,
    }
}

fn old_canonical_owners(owners: &[TrackingOwner]) -> Result<Vec<TrackingOwner>, TransitionError> {
    let mut canonical = owners.to_vec();
    for owner in &mut canonical {
        if owner.target.0.is_nil()
            || owner.root.0.is_nil()
            || owner.destinations.is_empty()
            || owner.destinations.iter().any(|id| id.0.is_nil())
            || owner.points.iter().any(|id| id.0.is_nil())
            || owner.mount_references.iter().any(|(id, reference)| {
                id.0.is_nil() || reference.is_some_and(|point| point.0.is_nil())
            })
        {
            return Err(invalid(
                "Position tracking owners require non-nil owners, destinations and references",
            ));
        }
        owner.destinations.sort_by_key(|id| id.0);
        owner.destinations.dedup();
        owner.points.sort_by_key(|id| id.0);
        owner.points.dedup();
        owner.mount_references.sort_by_key(|(id, _)| id.0);
        if owner.mount_references.len() != owner.destinations.len()
            || owner
                .mount_references
                .iter()
                .zip(&owner.destinations)
                .any(|((mount, _), destination)| mount != destination)
        {
            return Err(invalid(
                "Position tracking requires one mount reference per destination",
            ));
        }
    }
    canonical.sort_by_key(|owner| owner.target.0);
    if canonical
        .windows(2)
        .any(|pair| pair[0].target == pair[1].target)
    {
        return Err(invalid(
            "Position tracking census contains a duplicate logical owner",
        ));
    }
    Ok(canonical)
}

fn old_prepare(
    state: &mut TrackingState,
    frame: HybridFrameContext<'_>,
    owners: &[TrackingOwner],
) -> Result<(), TransitionError> {
    if !state
        .staged
        .as_ref()
        .is_some_and(|staged| staged.token == *frame.token)
        || !frame.token.same_capture(&frame.capture.frame_token())
        || frame.token.generation() != frame.capture.generation()
        || !frame.token.matches_geometry(frame.geometry)
        || !frame.token.matches_static_frame(frame.scalar)
    {
        return Err(invalid(
            "Position tracking dependencies use a foreign capture, geometry or staged token",
        ));
    }
    let canonical = old_canonical_owners(owners)?;
    let registry = state
        .accepted
        .as_ref()
        .filter(|accepted| accepted.registry.owners.as_ref() == canonical.as_slice())
        .map(|accepted| Arc::clone(&accepted.registry))
        .unwrap_or_else(|| Arc::new(build_registry(canonical)));
    let registry_changed = state
        .accepted
        .as_ref()
        .is_none_or(|accepted| !Arc::ptr_eq(&registry, &accepted.registry));
    state.changed_points.clear();
    if let Some(accepted) = &state.accepted {
        frame
            .geometry
            .changed_points_into(&accepted.geometry, &mut state.changed_points);
    } else {
        state
            .changed_points
            .extend(frame.geometry.points().iter().map(|point| point.fixture_id));
    }
    let mut dirty = Vec::new();
    let cold_change = state
        .accepted
        .as_ref()
        .is_none_or(|accepted| accepted.token.generation() != frame.token.generation());
    for owner in registry.owners() {
        if cold_change
            || (owner.incomplete && !state.changed_points.is_empty())
            || (registry_changed
                && state
                    .accepted
                    .as_ref()
                    .and_then(|accepted| accepted.registry.owner(owner.target))
                    != Some(owner))
        {
            mark_owner(&mut dirty, owner);
        }
    }
    // Retire stale cohort cache entries for removals/rebindings too. A changed peer makes
    // its complete root/instance dirty, so no other logical peer may reuse an old fit.
    if let Some(accepted) = state.accepted.as_ref().filter(|_| registry_changed) {
        for old in accepted.registry.owners() {
            if registry.owner(old.target) != Some(old) {
                mark_owner(&mut dirty, old);
            }
        }
    }
    for point in &state.changed_points {
        for edge in registry.dependents(*point) {
            dirty.push(TrackingInstance {
                root: edge.root,
                destination: edge.destination,
            });
        }
    }
    if let Some(accepted) = &state.accepted {
        for owner in registry.owners() {
            for &destination in &owner.destinations {
                if frame.geometry.mounts().mount(destination.0)
                    != accepted.geometry.mounts().mount(destination.0)
                {
                    dirty.push(TrackingInstance {
                        root: owner.root,
                        destination,
                    });
                }
            }
        }
    }
    dirty.sort_by_key(|row| (row.root.0, row.destination.0));
    dirty.dedup();
    let snapshot = Arc::new(TrackingSnapshot {
        token: frame.token.clone(),
        geometry: frame.geometry.clone(),
        registry,
        changed_points: state.changed_points.clone().into(),
        dirty: dirty.into(),
    });
    state
        .staged
        .as_mut()
        .expect("validated staged token")
        .snapshot = Some(snapshot);
    Ok(())
}

fn with_frame<T>(engine: &Engine, run: impl FnOnce(HybridFrameContext<'_>) -> T) -> T {
    let capture = engine.prepare_output_frame(Default::default());
    let token = capture.frame_token();
    let mut scalar = engine.prepare_static_family_frame(&capture, &[]);
    let geometry = engine
        .observe_static_family_geometry(&capture, &mut scalar)
        .unwrap();
    let models = DynamicRuntime::default().captured_native_color_models();
    run(HybridFrameContext {
        capture: &capture,
        geometry: &geometry,
        native_models: models.as_ref(),
        token: &token,
        scalar: &scalar,
    })
}

fn equal_snapshots(a: &TrackingSnapshot, b: &TrackingSnapshot) {
    assert_eq!(a.token(), b.token());
    assert_eq!(a.registry().owners(), b.registry().owners());
    assert_eq!(a.registry().instances(), b.registry().instances());
    assert_eq!(a.changed_points(), b.changed_points());
    assert_eq!(a.dirty_instances(), b.dirty_instances());
    for owner in a.registry().owners() {
        for point in owner.points.iter().copied().chain(
            owner
                .mount_references
                .iter()
                .filter_map(|(_, point)| *point),
        ) {
            assert_eq!(
                a.registry().dependents(point),
                b.registry().dependents(point)
            );
        }
    }
}

fn compare_attempt(
    engine: &Engine,
    states: &mut [TrackingState; 2],
    owners: Vec<TrackingOwner>,
    accept: bool,
) -> Result<[Arc<TrackingSnapshot>; 2], TransitionError> {
    with_frame(engine, |frame| {
        for state in states.iter_mut() {
            state.begin(frame.token).unwrap();
        }
        let actual = states[0].prepare_owned(frame, owners.clone());
        let expected = old_prepare(&mut states[1], frame, &owners);
        assert_eq!(actual, expected);
        actual?;
        let snapshots = states
            .each_ref()
            .map(|state| state.staged_snapshot().unwrap());
        equal_snapshots(&snapshots[0], &snapshots[1]);
        for state in states.iter_mut() {
            state.verify(frame.token).unwrap();
            if accept {
                assert!(state.accept(frame.token));
            }
        }
        Ok(snapshots)
    })
}

#[test]
fn owned_census_moves_all_nonempty_nested_buffers_into_the_real_registry() {
    let root = FixtureId::new();
    let copy = FixtureId::new();
    let aim = FixtureId::new();
    let missing = FixtureId::new();
    let mut mounted = fixture(root, false);
    mounted
        .multipatch
        .push(serde_json::from_value(json!({"id": copy.0})).unwrap());
    let (engine, _, _) = engine(vec![mounted, fixture(aim, true)]);
    let mut row = owner(root, root, vec![missing, aim]);
    row.destinations.push(copy);
    row.mount_references = vec![(copy, Some(missing)), (root, Some(aim))];
    assert!(
        !row.destinations.is_empty() && !row.mount_references.is_empty() && !row.points.is_empty()
    );
    let pointers = (
        row.destinations.as_ptr(),
        row.mount_references.as_ptr(),
        row.points.as_ptr(),
    );
    with_frame(&engine, |frame| {
        let mut state = TrackingState::default();
        state.begin(frame.token).unwrap();
        state.prepare_owned(frame, vec![row]).unwrap();
        let snapshot = state.staged_snapshot().unwrap();
        assert_eq!(snapshot.registry().owners().len(), 1);
        let kept = &snapshot.registry().owners()[0];
        // Do not print addresses or assume the outer Vec survives Vec->Arc conversion.
        assert!(kept.destinations.as_ptr() == pointers.0);
        assert!(kept.mount_references.as_ptr() == pointers.1);
        assert!(kept.points.as_ptr() == pointers.2);
        assert_eq!(kept.destinations.len(), 2);
        assert_eq!(kept.points.len(), 2);
        assert_eq!(snapshot.registry().instances().len(), 2);
        state.verify(frame.token).unwrap();
        assert!(state.accept(frame.token));
        assert!(Arc::ptr_eq(&snapshot, &state.snapshot().unwrap()));
    });
}

#[test]
fn owned_canonicalization_matches_frozen_borrowed_validation_and_order() {
    let root = FixtureId::new();
    let second = FixtureId::new();
    let point = FixtureId::new();
    let mut first = owner(root, root, vec![point, point]);
    first.destinations.extend([second, root]);
    first.mount_references = vec![(second, Some(point)), (root, None)];
    let other = owner(FixtureId::new(), second, vec![]);
    let mut cases = vec![
        vec![],
        vec![first.clone(), other.clone()],
        vec![other, first.clone()],
        vec![first.clone(), first.clone()],
    ];
    let mut invalid = first.clone();
    invalid.target = FixtureId(Uuid::nil());
    cases.push(vec![invalid]);
    let mut invalid = first.clone();
    invalid.mount_references.pop();
    cases.push(vec![invalid]);
    let mut invalid = first;
    invalid.points.push(FixtureId(Uuid::nil()));
    cases.push(vec![invalid]);
    let mut successes = 0;
    let mut failures = 0;
    for rows in cases {
        let expected = old_canonical_owners(&rows);
        successes += usize::from(expected.is_ok());
        failures += usize::from(expected.is_err());
        assert_eq!(canonical_owners(&rows), expected);
        assert_eq!(canonical_owned_owners(rows), expected);
    }
    assert_eq!(successes, 3);
    assert_eq!(failures, 4);
}

#[test]
fn owned_census_matches_original_registry_reuse_missing_points_and_failed_retry() {
    let root = FixtureId::new();
    let copy = FixtureId::new();
    let aim = FixtureId::new();
    let missing = FixtureId::new();
    let mut mounted = fixture(root, false);
    mounted
        .multipatch
        .push(serde_json::from_value(json!({"id": copy.0})).unwrap());
    let (engine, programmers, session) = engine(vec![mounted, fixture(aim, true)]);
    let mut first_owner = owner(root, root, vec![missing, aim]);
    first_owner.destinations.push(copy);
    first_owner.mount_references = vec![(copy, Some(missing)), (root, None)];
    let rows = vec![first_owner];
    let mut states = [TrackingState::default(), TrackingState::default()];
    let first = compare_attempt(&engine, &mut states, rows.clone(), true).unwrap();
    assert_eq!(first[0].registry().owners().len(), 1);
    assert_eq!(first[0].registry().instances().len(), 2);
    assert_eq!(first[0].registry().dependents(missing).len(), 2);
    let unchanged = compare_attempt(&engine, &mut states, rows.clone(), true).unwrap();
    for index in 0..2 {
        assert!(Arc::ptr_eq(
            first[index].registry(),
            unchanged[index].registry()
        ));
        assert!(unchanged[index].dirty_instances().is_empty());
    }
    programmers.set(
        session,
        aim,
        AttributeKey("point.position.x".into()),
        AttributeValue::Normalized(0.6),
    );
    let failed_candidate = compare_attempt(&engine, &mut states, rows.clone(), false).unwrap();
    assert!(failed_candidate[0].changed_points().contains(&aim));
    assert!(failed_candidate[0].dirty_instance(root, copy));
    for state in &mut states {
        state.abandon();
    }
    let retry = compare_attempt(&engine, &mut states, rows.clone(), true).unwrap();
    assert_eq!(
        retry[0].changed_points(),
        failed_candidate[0].changed_points()
    );
    let stable = states.each_ref().map(|state| state.snapshot().unwrap());
    let mut invalid = rows.clone();
    invalid[0].mount_references.clear();
    assert!(compare_attempt(&engine, &mut states, invalid, true).is_err());
    for index in 0..2 {
        assert!(Arc::ptr_eq(
            &stable[index],
            &states[index].snapshot().unwrap()
        ));
        states[index].abandon();
    }
    compare_attempt(&engine, &mut states, rows, true).unwrap();
    let removed = compare_attempt(&engine, &mut states, vec![], true).unwrap();
    assert!(removed[0].registry().owners().is_empty());
    assert!(removed[0].dirty_instance(root, copy));
}

#[test]
fn owned_and_borrowed_token_errors_precede_invalid_census_and_preserve_acceptance() {
    let root = FixtureId::new();
    let (engine, _, _) = engine(vec![fixture(root, false)]);
    let invalid_rows = vec![owner(FixtureId(Uuid::nil()), root, vec![])];
    with_frame(&engine, |frame| {
        let mut owned = TrackingState::default();
        let mut borrowed = TrackingState::default();
        let mut reference = TrackingState::default();
        // No staged token: token failure must precede invalid-owner validation in every API.
        let expected = old_prepare(&mut reference, frame, &invalid_rows).unwrap_err();
        assert_eq!(
            owned
                .prepare_owned(frame, invalid_rows.clone())
                .unwrap_err(),
            expected
        );
        assert_eq!(
            borrowed.prepare(frame, &invalid_rows).unwrap_err(),
            expected
        );
        for state in [&mut owned, &mut borrowed, &mut reference] {
            state.begin(frame.token).unwrap();
        }
        let census_error = old_prepare(&mut reference, frame, &invalid_rows).unwrap_err();
        assert_ne!(expected, census_error);
        assert_eq!(
            owned
                .prepare_owned(frame, invalid_rows.clone())
                .unwrap_err(),
            census_error
        );
        assert_eq!(
            borrowed.prepare(frame, &invalid_rows).unwrap_err(),
            census_error
        );
        assert!(owned.snapshot().is_none() && owned.staged_snapshot().is_none());
        assert!(!owned.accept(frame.token));
        let valid = vec![owner(root, root, vec![])];
        owned.prepare_owned(frame, valid.clone()).unwrap();
        old_prepare(&mut reference, frame, &valid).unwrap();
        equal_snapshots(
            &owned.staged_snapshot().unwrap(),
            &reference.staged_snapshot().unwrap(),
        );
        owned.verify(frame.token).unwrap();
        assert!(owned.accept(frame.token));
    });
}
