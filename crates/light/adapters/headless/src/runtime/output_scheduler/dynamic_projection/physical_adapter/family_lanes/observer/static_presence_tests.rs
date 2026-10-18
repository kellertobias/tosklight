use super::super::super::color::tests::{magenta, program};
use super::super::super::optics::tests::{field, focus};
use super::super::super::position::tests::angles;
use super::super::tests::Rig;
use super::*;

// Frozen old unguarded call order; each old scan still performs its own exact value query.
fn old_targets(
    observer: &mut FamilyFrameObserver<'_>,
    frame: HybridFrameContext<'_>,
    baseline: &light_engine::PreparedStaticFamilyFrame,
) -> Result<Vec<(FixtureId, ProgrammingOwner)>, TransitionError> {
    let mut targets = observer.position.static_program_targets(frame, baseline)?;
    // TL-596: a set for membership; scanning `targets` per head was quadratic at full-rig
    // size. `targets` keeps its order.
    let mut seen = targets.iter().copied().collect::<FxHashSet<_>>();
    let pool = observer.position.pool.as_deref();
    // TL-554: a static Color program (Semantic or Direct) is resolved by the routed Color
    // adapter like any other Color owner, so the published frame carries its complete
    // native output, the premaster values a first native edit adopts, and the per-head
    // Direct replay status. Targets without a compiled Color destination keep their
    // ordinary baseline output.
    observer.lanes.color.static_targets(
        frame,
        baseline,
        ProgrammingOwner::Color,
        |value| matches!(value, AttributeValue::ColorProgram(_)),
        &mut targets,
        &mut seen,
        pool,
    )?;
    // TL-560: a static typed Zoom (Programmer, played Cue, committed Preload) is owned by
    // the Zoom adapter like a static Color program; the scalar path cannot render a typed
    // opening, so without this the Zoom control kept its default whenever no Dynamic sampled
    // the owner. Targets without a compiled Zoom destination keep their ordinary baseline
    // output.
    observer
        .lanes
        .optics
        .lane(light_fixture::OpticsFamily::Zoom)
        .static_targets(
            frame,
            baseline,
            ProgrammingOwner::Zoom,
            |value| matches!(value, AttributeValue::Zoom(_)),
            &mut targets,
            &mut seen,
            pool,
        )?;
    // TL-544 G10: static Focus is a typed family like Zoom. Its complete payload stays the
    // normalized 0-1 setting at `focus`; the Focus adapter fits it through the profile's
    // focus function, publishes its requested/achieved sidecar and keeps it independent of
    // Zoom. Targets without a compiled Focus destination keep their ordinary baseline output.
    observer
        .lanes
        .optics
        .lane(light_fixture::OpticsFamily::Focus)
        .static_targets(
            frame,
            baseline,
            ProgrammingOwner::Focus,
            |value| matches!(value, AttributeValue::Normalized(_)),
            &mut targets,
            &mut seen,
            pool,
        )?;
    Ok(targets)
}

fn compare(
    rig: &Rig,
    pool: Option<Arc<light_engine::parallel::OutputPool>>,
    foreign: bool,
) -> (u64, u64) {
    let lanes = FamilyLanes::live();
    let capture = rig.engine.prepare_output_frame(Default::default());
    let other = rig.engine.prepare_output_frame(Default::default());
    let token = if foreign {
        other.frame_token()
    } else {
        capture.frame_token()
    };
    lanes.begin_frame(&token).unwrap();
    let mut baseline = rig.engine.prepare_static_family_frame(&capture, &[]);
    let geometry = rig
        .engine
        .observe_static_family_geometry(&capture, &mut baseline)
        .unwrap();
    let models = light_dynamics::DynamicRuntime::default().captured_native_color_models();
    let frame = HybridFrameContext {
        capture: &capture,
        geometry: &geometry,
        native_models: models.as_ref(),
        token: &token,
        scalar: &baseline,
    };
    let mut current = FamilyFrameObserver::new(&lanes).with_pool(pool.clone());
    let mut old = FamilyFrameObserver::new(&lanes).with_pool(pool);
    let (actual, now_jobs) = light_engine::parallel::diagnostics::covered(true, || {
        current.static_program_targets(frame, &baseline)
    });
    let (expected, old_jobs) = light_engine::parallel::diagnostics::covered(true, || {
        old_targets(&mut old, frame, &baseline)
    });
    assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
    if foreign {
        assert!(actual.is_err());
    }
    (now_jobs.unwrap().jobs, old_jobs.unwrap().jobs)
}

#[test]
fn static_targets_match_unguarded_scans_for_mixed_heads_edits_and_foreign_tokens() {
    let rig = Rig::new();
    for workers in [1, 2, 4] {
        compare(
            &rig,
            light_engine::parallel::OutputPool::new(workers).map(Arc::new),
            false,
        );
    }
    for (target, owner, value) in [
        (rig.show.mover, ProgrammingOwner::Position, angles(10., 20.)),
        (rig.show.wash, ProgrammingOwner::Color, program(&magenta())),
        (rig.show.layer, ProgrammingOwner::Color, program(&magenta())),
        (rig.show.optics, ProgrammingOwner::Zoom, field(30.)),
        (rig.show.optics, ProgrammingOwner::Focus, focus(0.4)),
    ] {
        rig.clock.advance_millis(25);
        rig.programmers.set(rig.session, target, owner.key(), value);
        compare(&rig, None, false);
        compare(
            &rig,
            light_engine::parallel::OutputPool::new(2).map(Arc::new),
            false,
        );
    }
    compare(&rig, None, true);
    // Wrong typed owner values do not create a new adapter error or target.
    rig.programmers.set(
        rig.session,
        rig.show.optics,
        ProgrammingOwner::Zoom.key(),
        focus(0.2),
    );
    compare(&rig, None, false);
    let mut snapshot = (*rig.engine.snapshot()).clone();
    let optics = Arc::make_mut(&mut snapshot.fixtures)
        .iter_mut()
        .find(|fixture| fixture.fixture_id == rig.show.optics)
        .unwrap();
    let frozen = optics.freeze.targets.entry(rig.show.optics).or_default();
    frozen.families = vec![light_fixture::FreezeFamily::Beam];
    frozen
        .values
        .insert(ProgrammingOwner::Focus.key(), focus(0.9));
    snapshot.revision += 1;
    rig.engine.replace_snapshot(snapshot).unwrap();
    compare(&rig, None, false);
    compare(
        &rig,
        light_engine::parallel::OutputPool::new(4).map(Arc::new),
        false,
    );
}

#[test]
fn absent_family_scans_skip_actual_pool_jobs_and_unpatched_targets_still_match() {
    let rig = Rig::new();
    let capture = rig.engine.prepare_output_frame(Default::default());
    let mut snapshot = (*capture.snapshot()).clone();
    let template = snapshot.fixtures[0].clone();
    snapshot.fixtures = (0..128)
        .map(|number| {
            let mut fixture = template.clone();
            fixture.fixture_id = FixtureId::new();
            fixture.fixture_number = Some(number + 1);
            fixture.universe = None;
            fixture.address = None;
            fixture
        })
        .collect::<Vec<_>>()
        .into();
    snapshot.revision += 1;
    rig.engine.replace_snapshot(snapshot).unwrap();
    let (current, old) = compare(
        &rig,
        light_engine::parallel::OutputPool::new(2).map(Arc::new),
        false,
    );
    // Engine is a normal dependency here: its existing coverage is enabled only explicitly.
    if std::env::var("LIGHT_OUTPUT_PHASE_DIAGNOSTICS").as_deref() == Ok("all") {
        assert!(
            old > current,
            "old scans must launch more actual covered jobs"
        );
        assert_eq!(old - current, 6); // Three absent scans, two fixture chunks each.
    } else {
        assert_eq!((current, old), (0, 0));
    }
}
