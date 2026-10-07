//! TL-594: the first Angle edit adopts EXACTLY the leased source the surface displayed. A gone
//! lease holds quietly with a re-read reason; latest is never substituted, and Preload binds
//! only its own Pending lineage. Uses the accepted-command mover rig of the parent module.
use super::*;
use crate::runtime::output_readouts::{DisplayedSource, read_readouts};
use crate::runtime::position_readout::{
    CapturedPositionReadouts, PendingPositionReadoutSource, capture_position_readouts,
};
use light_application::{
    ProgrammingDisplayedLane, ProgrammingDisplayedSource, ProgrammingValuesHold,
};
use light_core::ProgrammerId;
use light_wire::v2::output_readouts::{OutputReadoutSnapshot, OutputReadoutUnavailable};
use light_wire::v2::visualization::VisualizationLane;
use std::time::{Duration, Instant};

const HELD: Option<ProgrammingValuesHold> = Some(ProgrammingValuesHold::DisplayedSourceUnavailable);

fn displayed(lane: ProgrammingDisplayedLane, lease: u64) -> ProgrammingDisplayedSource {
    ProgrammingDisplayedSource { lane, lease }
}

fn live(lease: u64) -> ProgrammingDisplayedSource {
    displayed(ProgrammingDisplayedLane::Normal, lease)
}

/// Prepare one first Angle edit of `session` that names `source` (or nothing).
fn adopt(
    state: &AppState,
    session: SessionId,
    owner: FixtureId,
    preload: bool,
    source: Option<ProgrammingDisplayedSource>,
) -> ProgrammingValuesEnvironment {
    let mut environment = values_environment(state);
    let mut intent = intent(owner);
    intent.displayed_source = source;
    prepare_family_edit_context(state, session, preload, &intent, &mut environment);
    environment
}

fn solved(environment: &ProgrammingValuesEnvironment, owner: FixtureId) -> Option<JointAngles> {
    environment
        .family_contexts
        .get(&owner)
        .and_then(|context| context.solved_angles)
}

fn common(snapshot: &OutputReadoutSnapshot) -> JointAngles {
    let common = snapshot.owners[0].position.common.unwrap();
    JointAngles {
        pan_degrees: common.pan_degrees,
        tilt_degrees: common.tilt_degrees,
    }
}

fn mover_desk() -> (AppState, std::path::PathBuf, FixtureId, SessionId) {
    let (state, directory) = crate::runtime::tests::test_state();
    let fixture = mover();
    let owner = fixture.fixture_id;
    install(&state, fixture);
    let session = SessionId::new();
    state.programming.start(session);
    (state, directory, owner, session)
}

#[tokio::test]
async fn the_first_edit_adopts_the_displayed_frame_even_after_latest_advanced() {
    let (state, directory, owner, session) = mover_desk();
    let shown = render(&state, owner, 1., 0., Some(requested([1., 2., 3.])));
    publish(&state, &shown, None);
    let readout = read_readouts(&state, session, VisualizationLane::Normal, &[owner]);
    let lease = readout.lease.expect("an accepted frame is leased");
    assert_eq!(
        readout.frame.as_ref().unwrap().sequence,
        state.output.latest_visualization_frame().unwrap().sequence
    );

    let newer = render(&state, owner, 0., 1., Some(requested([9., 8., 7.])));
    publish(&state, &newer, None);
    let latest = read_readouts(&state, session, VisualizationLane::Normal, &[owner]);
    assert_ne!(common(&latest), common(&readout), "latest has advanced");

    let exact = adopt(&state, session, owner, false, Some(live(lease)));
    assert_eq!(exact.displayed_source_hold, None);
    assert_eq!(
        solved(&exact, owner),
        Some(common(&readout)),
        "the adopted pose is the displayed readout, not latest"
    );
    assert_eq!(
        exact.current_values[&(owner, ProgrammingOwner::Position.key())],
        requested([1., 2., 3.]),
        "the requested seed comes from the same displayed frame"
    );
    // Without a displayed source (OSC, HTTP integrators) adoption keeps reading latest.
    let legacy = adopt(&state, session, owner, false, None);
    assert_eq!(solved(&legacy, owner), Some(common(&latest)));
    let _ = std::fs::remove_dir_all(directory);
}

#[tokio::test]
async fn an_expired_unknown_or_foreign_lease_holds_quietly_without_reading_latest() {
    let (state, directory, owner, session) = mover_desk();
    let frame = render(&state, owner, 1., 0., Some(requested([1., 2., 3.])));
    publish(&state, &frame, None);
    let leases = state.sessions.displayed_sources().leases();
    let source = DisplayedSource::Live(state.output.latest_visualization_frame().unwrap());
    let expired = leases.issue(
        session,
        VisualizationLane::Normal,
        source,
        Instant::now() - Duration::from_secs(3),
    );
    let foreign = read_readouts(
        &state,
        SessionId::new(),
        VisualizationLane::Normal,
        &[owner],
    )
    .lease
    .unwrap();
    for lease in [expired, foreign, 9_999_999] {
        let held = adopt(&state, session, owner, false, Some(live(lease)));
        assert_eq!(held.displayed_source_hold, HELD, "lease {lease}");
        assert_eq!(solved(&held, owner), None);
        assert!(
            !held
                .current_values
                .contains_key(&(owner, ProgrammingOwner::Position.key())),
            "a held edit seeds nothing from latest"
        );
    }
    let _ = std::fs::remove_dir_all(directory);
}

#[tokio::test]
async fn a_generation_change_or_session_close_invalidates_the_displayed_source() {
    let (state, directory, owner, session) = mover_desk();
    let frame = render(&state, owner, 1., 0., Some(requested([1., 2., 3.])));
    publish(&state, &frame, None);
    let lease = read_readouts(&state, session, VisualizationLane::Normal, &[owner])
        .lease
        .unwrap();
    let mut snapshot = state.output.snapshot().as_ref().clone();
    snapshot.revision += 1;
    state.output.replace_snapshot(snapshot).unwrap();
    assert_eq!(
        adopt(&state, session, owner, false, Some(live(lease))).displayed_source_hold,
        HELD
    );
    let current = render(&state, owner, 1., 0., Some(requested([1., 2., 3.])));
    publish(&state, &current, None);
    let lease = read_readouts(&state, session, VisualizationLane::Normal, &[owner])
        .lease
        .unwrap();
    assert!(
        solved(
            &adopt(&state, session, owner, false, Some(live(lease))),
            owner
        )
        .is_some()
    );
    state.sessions.displayed_sources().close_session(session);
    assert_eq!(
        adopt(&state, session, owner, false, Some(live(lease))).displayed_source_hold,
        HELD,
        "session teardown releases its leases"
    );
    let _ = std::fs::remove_dir_all(directory);
}

/// A test Pending source whose accepted capture the test replaces, served to one Programmer.
struct SwappablePending {
    programmer: ProgrammerId,
    captured: std::sync::Mutex<Option<CapturedPositionReadouts>>,
}

impl PendingPositionReadoutSource for SwappablePending {
    fn capture(
        &self,
        programmer: ProgrammerId,
        owners: &[FixtureId],
    ) -> Option<CapturedPositionReadouts> {
        let mut captured = self.captured.lock().unwrap().clone()?;
        (programmer == self.programmer).then_some(())?;
        captured
            .owners
            .retain(|owner| owners.contains(&owner.owner));
        Some(captured)
    }
}

fn pending_capture(state: &AppState, owner: FixtureId, pan: f32) -> CapturedPositionReadouts {
    let frame = render(state, owner, pan, 0.5, Some(requested([pan, 0., 0.])));
    publish(state, &frame, None);
    let published = state.output.latest_visualization_frame().unwrap();
    capture_position_readouts(state.output.engine(), &published, &[owner]).unwrap()
}

#[tokio::test]
async fn preload_binds_its_own_pending_lineage_and_never_uses_live() {
    let (state, directory, owner, session) = mover_desk();
    let none = read_readouts(&state, session, VisualizationLane::Preload, &[owner]);
    assert_eq!(
        none.unavailable,
        Some(OutputReadoutUnavailable::NoAcceptedPreload)
    );
    assert_eq!(
        none.lease, None,
        "no Pending publication: no lease, no Live stand-in"
    );

    let live_lease = read_readouts(&state, session, VisualizationLane::Normal, &[owner]).lease;
    let source = Arc::new(SwappablePending {
        programmer: state.programming.get(session).unwrap().id,
        captured: std::sync::Mutex::new(Some(pending_capture(&state, owner, 0.75))),
    });
    state
        .output
        .pending_position_readouts()
        .install(source.clone());
    let shown = read_readouts(&state, session, VisualizationLane::Preload, &[owner]);
    let pending_lease = shown.lease.expect("an accepted Pending capture is leased");
    *source.captured.lock().unwrap() = Some(pending_capture(&state, owner, 0.25));
    let newer = read_readouts(&state, session, VisualizationLane::Preload, &[owner]);
    assert_ne!(common(&newer), common(&shown));

    let preload = displayed(ProgrammingDisplayedLane::Preload, pending_lease);
    let exact = adopt(&state, session, owner, true, Some(preload));
    assert_eq!(solved(&exact, owner), Some(common(&shown)));
    assert_eq!(
        adopt(&state, session, owner, false, Some(preload)).displayed_source_hold,
        HELD,
        "a Normal edit cannot adopt a Pending lease"
    );
    if let Some(live_lease) = live_lease {
        for lane in [
            ProgrammingDisplayedLane::Normal,
            ProgrammingDisplayedLane::Preload,
        ] {
            assert_eq!(
                adopt(
                    &state,
                    session,
                    owner,
                    true,
                    Some(displayed(lane, live_lease))
                )
                .displayed_source_hold,
                HELD,
                "Preload never adopts a Live lease"
            );
        }
    }
    state.output.pending_position_readouts().clear();
    let _ = std::fs::remove_dir_all(directory);
}

#[tokio::test]
async fn divergent_copies_are_distinct_rows_and_leave_the_common_angle_unavailable() {
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
    let session = SessionId::new();
    let frame = render(&state, owner, 1., 0., Some(requested([1., 2., 3.])));
    publish(&state, &frame, None);
    let readout = read_readouts(&state, session, VisualizationLane::Normal, &[owner, owner]);
    assert_eq!(
        readout.owners.len(),
        2,
        "caller order and duplicates are kept"
    );
    let position = &readout.owners[0].position;
    assert!(position.available);
    assert_eq!(position.commands.len(), 2, "one row per physical copy");
    assert_ne!(
        position.commands[0].pan_degrees,
        position.commands[1].pan_degrees
    );
    assert_eq!(
        position.common, None,
        "divergent copies have no common Angle"
    );
    let held = adopt(&state, session, owner, false, readout.lease.map(live));
    assert_eq!(
        held.displayed_source_hold, None,
        "the source itself is valid"
    );
    assert_eq!(solved(&held, owner), None, "but no common pose is adopted");
    let _ = std::fs::remove_dir_all(directory);
}

#[tokio::test]
async fn duplicate_readers_of_one_frame_share_one_capture() {
    let (state, directory, owner, session) = mover_desk();
    let frame = render(&state, owner, 1., 0., None);
    publish(&state, &frame, None);
    let sources = state.sessions.displayed_sources();
    let before = sources.live_captures_computed();
    let first = read_readouts(&state, session, VisualizationLane::Normal, &[owner]);
    let duplicate = read_readouts(
        &state,
        SessionId::new(),
        VisualizationLane::Normal,
        &[owner],
    );
    assert_eq!(sources.live_captures_computed(), before + 1);
    assert_eq!(first.owners, duplicate.owners);
    assert_ne!(
        first.lease, duplicate.lease,
        "each delivery is leased separately"
    );
    let _ = std::fs::remove_dir_all(directory);
}

#[path = "displayed_source_route_tests.rs"]
mod route;
