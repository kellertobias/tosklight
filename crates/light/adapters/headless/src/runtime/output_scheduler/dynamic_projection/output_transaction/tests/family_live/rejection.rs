//! A frame rejected while it is PREPARED, before the Live finalizer runs: two heads of one
//! fixture drive one master-shared Zoom control with different openings, which the engine's
//! all-family native installation refuses. Only the C3 error path can abandon the staged lanes.
use super::*;
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::optics::profiles::two_head_shared_zoom;

#[test]
fn a_frame_rejected_during_preparation_abandons_every_lane_and_rolls_back() {
    let definition = definition();
    let (root, head) = (FixtureId::new(), FixtureId::new());
    let mut fixture = media_fixture(&two_head_shared_zoom(), root, &[head]);
    fixture.fixture_number = Some(1);
    let bench = Bench::with_fixtures(
        vec![fixture],
        &definition,
        PROGRAMMING_CONTRACT_VERSION,
        true,
    );
    let (programmers, clock, session) = (&bench.programmers, &bench.clock, bench.session);
    let zoom = ProgrammingOwner::Zoom;
    // Agreeing openings are one consistent write on the shared control.
    fix(
        programmers,
        clock,
        session,
        root,
        zoom,
        field(30.),
        field(20.),
    );
    fix(
        programmers,
        clock,
        session,
        head,
        zoom,
        field(30.),
        field(20.),
    );
    assert!(bench.frame().hybrid);
    let accepted = bench.family.take_published().unwrap().token;
    // Disagreeing openings on the one shared control.
    fix(
        programmers,
        clock,
        session,
        head,
        zoom,
        field(30.),
        field(40.),
    );
    let runtime = bench.dynamics.lock().snapshot();
    let origins = bench.origins.load_full();
    let frame = bench.capture();
    let error = bench
        .render(&frame)
        .err()
        .expect("a disagreeing shared control rejects the frame");
    assert!(
        error.to_string().contains("shared"),
        "rejected by the shared-control rule: {error}"
    );
    assert!(bench.family.take_published().is_none());
    assert_eq!(
        bench.dynamics.lock().snapshot(),
        runtime,
        "Dynamics rolled back"
    );
    assert!(Arc::ptr_eq(&origins, &bench.origins.load_full()));
    assert!(
        bench
            .cache
            .changed(frame.dynamic_programmer_values(), &frame.snapshot())
    );
    assert_eq!(
        bench.last_accepted(),
        [0; 4].map(|_| Some(accepted.clone()))
    );
    let failed = frame.frame_token();
    bench.family.with_lanes(|lanes| {
        assert!(!lanes.position().accept_frame(&failed));
        assert!(!lanes.color().accept_frame(&failed));
        for family in [
            light_fixture::OpticsFamily::Focus,
            light_fixture::OpticsFamily::Zoom,
        ] {
            assert!(
                !lanes.optics().lane(family).accept_frame(&failed),
                "{family:?} lane still stages the rejected frame"
            );
        }
    });
}
