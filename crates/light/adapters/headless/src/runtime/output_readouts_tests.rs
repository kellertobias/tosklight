//! TL-594 lease fix: a second reader of the source the desk was shown never invalidates the
//! desk's lease. Every delivery of one accepted source to a session lane shares one lease.
use super::super::displayed_source_leases::LEASE_RING;
use super::*;
use crate::runtime::{AppState, Session, visualization_readouts::lane_stamp};
use light_engine::PositionCommandedOwnerReadout;
use std::sync::Arc;

const NORMAL: VisualizationLane = VisualizationLane::Normal;
const PRELOAD: VisualizationLane = VisualizationLane::Preload;

fn session(state: &AppState) -> Session {
    let session = Session {
        id: SessionId::new(),
        token: "displayed-source-readers".into(),
        connected: true,
        desk: state.installation.desk().unwrap(),
        capability: Default::default(),
    };
    state.programming.start(session.id);
    session
}

fn publish_empty_frame(state: &AppState) -> Arc<PublishedVisualizationFrame> {
    let rendered = state
        .output
        .engine()
        .render_with_contribution_batches(Default::default(), &[])
        .unwrap();
    state.output.render_frames_and_publish(
        &crate::runtime::visualization_frame::RenderedSemanticFrame::untraced(
            rendered,
            Default::default(),
        ),
        VisualizationScope { show_id: None },
    );
    state.output.latest_visualization_frame().unwrap()
}

fn adopts(state: &AppState, session: SessionId, lease: u64, owners: &[FixtureId]) -> bool {
    displayed_position_readouts(
        state,
        session,
        false,
        ProgrammingDisplayedSource {
            lane: ProgrammingDisplayedLane::Normal,
            lease,
        },
        None,
        owners,
    )
    .is_some()
}

#[test]
fn a_second_reader_of_the_displayed_frame_never_invalidates_the_desks_lease() {
    let (state, directory) = crate::runtime::tests::test_state();
    let desk = session(&state);
    let (encoder, modal) = (FixtureId::new(), FixtureId::new());
    let shown = publish_empty_frame(&state);

    let desk_lease = read_readouts(&state, desk.id, NORMAL, &[encoder])
        .lease
        .expect("the desk's readout is leased");
    // The modal, a probe and the desk's own WebSocket lane deliver the very same frame to the
    // same session, more often than the ring holds deliveries.
    let mut others = Vec::new();
    for _ in 0..LEASE_RING {
        others.push(
            read_readouts(&state, desk.id, NORMAL, &[modal])
                .lease
                .unwrap(),
        );
        others.push(lane_stamp(&state, &desk, NORMAL, &shown).lease.unwrap());
    }
    let sources = state.sessions.displayed_sources();
    let Some(DisplayedSource::Live(resolved)) =
        sources
            .leases()
            .resolve(desk.id, NORMAL, desk_lease, Instant::now())
    else {
        panic!("the desk's lease still names the displayed frame");
    };
    assert!(Arc::ptr_eq(&resolved, &shown));
    assert!(
        adopts(&state, desk.id, desk_lease, &[encoder, modal]),
        "the desk's next edit adopts the frame it was shown instead of holding"
    );
    for lease in &others {
        assert_eq!(
            *lease, desk_lease,
            "every delivery of one accepted frame to one session lane shares its lease"
        );
    }
    assert_eq!(
        sources.leases().retained(desk.id, NORMAL),
        1,
        "one shown frame occupies one ring slot"
    );

    // A newer frame read by another consumer does not retire the older, still-bounded lease.
    let newer = publish_empty_frame(&state);
    let newer_lease = read_readouts(&state, desk.id, NORMAL, &[modal])
        .lease
        .unwrap();
    assert_ne!(
        newer_lease, desk_lease,
        "a different frame is a different lease"
    );
    assert_eq!(
        lane_stamp(&state, &desk, NORMAL, &newer).lease,
        Some(newer_lease)
    );
    assert!(adopts(&state, desk.id, desk_lease, &[encoder]));
    assert!(adopts(&state, desk.id, newer_lease, &[encoder]));

    // Foreign sessions and the other lane still hold quietly; Preload never uses Live.
    let foreign = session(&state);
    assert!(!adopts(&state, foreign.id, desk_lease, &[encoder]));
    assert_eq!(
        sources
            .leases()
            .resolve(desk.id, PRELOAD, desk_lease, Instant::now())
            .map(|_| ()),
        None
    );
    assert!(
        displayed_position_readouts(
            &state,
            desk.id,
            true,
            ProgrammingDisplayedSource {
                lane: ProgrammingDisplayedLane::Normal,
                lease: desk_lease,
            },
            None,
            &[encoder],
        )
        .is_none()
    );
    let _ = std::fs::remove_dir_all(directory);
}

fn pending(
    identity: &OutputFrameIdentity,
    snapshot: &Arc<EngineSnapshot>,
    owners: &[FixtureId],
) -> DisplayedSource {
    DisplayedSource::Pending {
        readouts: Arc::new(CapturedPositionReadouts {
            identity: identity.clone(),
            scope: VisualizationScope { show_id: None },
            show_revision: 1,
            owners: owners
                .iter()
                .map(
                    |owner| super::super::position_readout::CapturedPositionOwner {
                        owner: *owner,
                        requested: None,
                        zoom: None,
                        color: None,
                        readout: PositionCommandedOwnerReadout {
                            owner: *owner,
                            commands: None,
                        },
                    },
                )
                .collect(),
        }),
        snapshot: Arc::clone(snapshot),
    }
}

fn delivered_owners(
    source: Option<DisplayedSource>,
    members: &[FixtureId],
) -> Option<Vec<FixtureId>> {
    let Some(DisplayedSource::Pending { readouts, .. }) = source else {
        return None;
    };
    select_members(&readouts, members)
        .map(|captured| captured.owners.iter().map(|owner| owner.owner).collect())
}

#[test]
fn readers_of_one_pending_frame_share_one_lease_covering_every_delivered_owner() {
    let (state, directory) = crate::runtime::tests::test_state();
    let desk = SessionId::new();
    let leases = DisplayedSourceLeases::<DisplayedSource>::default();
    let snapshot = state.output.engine().snapshot();
    let frame = OutputFrameIdentity {
        generation: 1,
        sequence: 7,
        sampled_at: "2026-10-02T00:00:00Z".into(),
        tracking: None,
    };
    let (encoder, modal) = (FixtureId::new(), FixtureId::new());
    let now = Instant::now();
    let desk_lease = leases.issue(desk, PRELOAD, pending(&frame, &snapshot, &[encoder]), now);
    let modal_lease = leases.issue(desk, PRELOAD, pending(&frame, &snapshot, &[modal]), now);
    assert_eq!(
        modal_lease, desk_lease,
        "one Pending frame, one session lease"
    );
    assert_eq!(
        delivered_owners(
            leases.resolve(desk, PRELOAD, desk_lease, now),
            &[encoder, modal]
        ),
        Some(vec![encoder, modal]),
        "the lease covers every owner delivered from that frame"
    );
    let next = OutputFrameIdentity {
        sequence: 8,
        ..frame.clone()
    };
    let next_lease = leases.issue(desk, PRELOAD, pending(&next, &snapshot, &[modal]), now);
    assert_ne!(
        next_lease, desk_lease,
        "a newer Pending frame is its own lease"
    );
    assert_eq!(
        delivered_owners(leases.resolve(desk, PRELOAD, desk_lease, now), &[encoder]),
        Some(vec![encoder]),
        "the older frame stays resolvable within the ring"
    );
    assert_eq!(
        delivered_owners(leases.resolve(desk, PRELOAD, next_lease, now), &[encoder]),
        None,
        "a member never delivered from a frame still holds"
    );
    let _ = std::fs::remove_dir_all(directory);
}
