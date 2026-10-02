//! TL-560 matrix row "Unpatched fixture" (behaviour, not only the stored `dormant()` row).
//!
//! AGENTS operator semantics: an unpatched fixture remains part of the show. It is programmed,
//! stored in a live Group and in Cues, and displayed (physical forward prediction for Stage);
//! only DMX output is suppressed until it is patched again.
//!
//! An RGB wash `u` and a shipped AURO SPOT Z300 mover `m` are unpatched (no universe/address).
//! A live Group holding a patched RGB wash `a` and `u` is programmed magenta; `u` itself gets a
//! per-fixture warm white in a second Cue; `m` gets Angles. Both Cues are recorded through the
//! real Cue writer and the SQLite show is reopened. On a fresh contract-1 desk the played
//! semantic owners equal the stored intent, the physical forward frame shows `u` exactly like
//! its patched twin and `m` at the commanded angles, but no byte of either reaches a universe.
//! Re-patching both resumes encoded output without any Cue rewrite.
use super::*;
use light_core::programming::PositionIntent;

const GROUP_CUE: f64 = 1.0;
const FIXTURE_CUE: f64 = 2.0;

fn angles() -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::angles(-35.5, 72.25)))
}

fn unpatch(mut fixture: PatchedFixture) -> PatchedFixture {
    fixture.universe = None;
    fixture.address = None;
    fixture
}

fn instance<'a>(frame: &'a DeskFrame, id: FixtureId) -> &'a light_engine::PhysicalInstanceOutput {
    frame
        .rendered
        .physical
        .instances
        .iter()
        .find(|instance| instance.instance_id == id.0)
        .unwrap_or_else(|| panic!("physical instance {id:?}"))
}

fn axes(frame: &DeskFrame, id: FixtureId) -> [f64; 2] {
    use light_fixture::PositionAxisRole;
    [PositionAxisRole::Pan, PositionAxisRole::Tilt].map(|role| {
        instance(frame, id)
            .axes
            .iter()
            .find(|axis| axis.role == Some(role))
            .and_then(|axis| axis.absolute_degrees())
            .expect("commanded axis")
    })
}

fn assert_angles(frame: &DeskFrame, id: FixtureId) {
    for (achieved, requested) in axes(frame, id).iter().zip([-35.5, 72.25]) {
        assert!(
            (achieved - requested).abs() < 0.05,
            "{requested} -> {achieved}"
        );
    }
}

#[tokio::test]
async fn unpatched_members_keep_semantic_programming_and_visibility_and_only_lose_dmx_until_repatched()
 {
    let show = Show::new();
    let (a, u, m) = (FixtureId::new(), FixtureId::new(), FixtureId::new());
    let auro = super::position_replacement::shipped("cameo--auro-spot-z300");
    let patched_a = fixture(&rgb(), a, 1, 1);
    let unpatched_u = unpatch(fixture(&rgb(), u, 2, 40));
    let unpatched_m = unpatch(fixture(&auro, m, 3, 100));
    for fixture in [&patched_a, &unpatched_u, &unpatched_m] {
        show.patch(fixture);
    }
    show.group(&[a, u]);
    let color = ProgrammingOwner::Color.key();
    let position = ProgrammingOwner::Position.key();
    show.programmers.set_group(
        show.session,
        GROUP.into(),
        color.clone(),
        program(&magenta()),
    );
    show.programmers
        .set(show.session, m, position.clone(), angles());
    show.record(GROUP_CUE);
    show.programmers
        .set(show.session, u, color.clone(), program(&warm_white()));
    show.record(FIXTURE_CUE);

    // Stored: the Group value, the unpatched fixture's own value and the unpatched mover.
    let (list_id, recorded) = show.cue_list();
    assert_eq!(
        stored_group_value(&recorded, 0, "color"),
        program(&magenta())
    );
    assert_eq!(stored_fixture_value(&recorded, 0, m, "position"), angles());
    assert_eq!(
        stored_fixture_value(&recorded, 1, u, "color"),
        program(&warm_white()),
        "an unpatched fixture is recorded like any other"
    );
    let snapshot = show.compile();
    for id in [u, m] {
        let fixture = snapshot
            .fixtures
            .iter()
            .find(|f| f.fixture_id == id)
            .unwrap();
        assert_eq!(
            (fixture.universe, fixture.address),
            (None, None),
            "still unpatched"
        );
    }

    // Reopened desk: semantic programming plays and is visible, no DMX leaves the desk.
    let desk = Desk::open(snapshot);
    desk.go(GROUP_CUE);
    assert_eq!(
        desk.played(u, ProgrammingOwner::Color),
        Some(program(&magenta()))
    );
    assert_eq!(desk.played(m, ProgrammingOwner::Position), Some(angles()));
    let frame = desk.frame();
    frame.assert_encoded(&patched_a);
    assert_eq!(
        frame.rendered.universes.keys().copied().collect::<Vec<_>>(),
        [1],
        "only the patched fixture's universe is output"
    );
    let bytes = frame.universe(1).unwrap();
    let footprint = usize::from(rgb().modes[0].splits[0].footprint);
    assert!(
        bytes[footprint..].iter().all(|byte| *byte == 0),
        "no unpatched byte lands at its former or any other address"
    );
    assert_eq!(
        instance(&frame, u).colors[0].known_xyz,
        instance(&frame, a).colors[0].known_xyz,
        "the unpatched member is predicted for Stage exactly like its patched twin"
    );
    assert_eq!(
        instance(&frame, u).native_raw,
        instance(&frame, a).native_raw,
        "the same fitted native command, only not transmitted"
    );
    assert_angles(&frame, m);

    desk.go(FIXTURE_CUE);
    assert_eq!(
        desk.played(u, ProgrammingOwner::Color),
        Some(program(&warm_white()))
    );
    let frame = desk.frame();
    assert_ne!(
        instance(&frame, u).colors[0].known_xyz,
        instance(&frame, a).colors[0].known_xyz,
        "the unpatched fixture's own Cue value is applied"
    );

    // Re-patch both: output resumes from the same stored Cues.
    let repatched_u = fixture(&rgb(), u, 2, 40);
    let mut repatched_m = fixture(&auro, m, 3, 1);
    repatched_m.universe = Some(2);
    show.patch(&repatched_u);
    show.patch(&repatched_m);
    assert_eq!(show.cue_list(), (list_id, recorded), "no Cue rewrite");
    let desk = Desk::open(show.compile());
    desk.go(GROUP_CUE);
    let frame = desk.frame();
    let native_a = frame.assert_encoded(&patched_a);
    let native_u = frame.assert_encoded(&repatched_u);
    assert_eq!(
        native_u, native_a,
        "the re-patched member outputs the Group colour"
    );
    assert!(
        !frame.family(ProgrammingOwner::Color, u).is_empty(),
        "Color is a family write"
    );
    frame.assert_encoded(&repatched_m);
    assert_angles(&frame, m);
    assert!(
        frame.universe(2).unwrap().iter().any(|byte| *byte != 0),
        "the mover's universe carries its Position"
    );
}
