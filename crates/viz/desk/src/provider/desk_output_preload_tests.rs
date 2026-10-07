//! Pending (Preload) frame admission by its published identity, on the TL-605 harness: the real
//! provider inbox, the real scene and the real decode, read back through `poll`.
//!
//! Every snapshot below carries a newer Live stamp than the last, so only the Preload lane's own
//! Pending identity (episode + ticket) decides what happens.
use super::*;
use light_wire::v2::output_control::{OutputPreloadState, OutputPreloadStatus};

/// A snapshot whose Live frame is `live` and whose Preload lane is the accepted Pending frame
/// `ticket` of `episode`, putting its Point at `z`.
fn pending(desk: &Desk, live: u64, episode: Uuid, ticket: u64, z: f32) -> OutputDmxSnapshot {
    let mut output = desk.output(Some(stamp(live, 4)), A);
    let mut lane = output.native.clone().unwrap();
    lane.frame = Some(stamp(ticket, 7));
    lane.instances[0].raw = vec![255, 0, 0, 0, 0, 0, 255];
    lane.instances[0].owned_channels = Some(vec![true; 7]);
    lane.points[0].offset_metres[2] = z;
    output.preload = Some(lane);
    output.preload_status = Some(OutputPreloadStatus {
        state: OutputPreloadState::Published,
        episode: Some(episode),
    });
    output
}

/// A newer Live frame with a passive Pending state: nothing published, no Preload lane.
fn passive(desk: &Desk, live: u64, state: OutputPreloadState) -> OutputDmxSnapshot {
    let mut output = desk.output(Some(stamp(live, 4)), A);
    output.preload_status = Some(OutputPreloadStatus {
        state,
        episode: None,
    });
    output
}

fn following() -> Desk {
    let mut desk = Desk::new(false);
    desk.provider.follow_preload(true);
    desk
}

fn preload(desk: &Desk) -> &super::super::preload::PreloadFrameAdmission {
    &desk.admission().preload
}

/// Within one episode an older ticket is a replay: it never replaces the newer Pending picture,
/// even inside a newer Live snapshot.
#[test]
fn a_replayed_pending_ticket_never_replaces_the_newer_preload_picture() {
    let mut desk = following();
    let episode = Uuid::new_v4();
    assert_eq!(desk.deliver([pending(&desk, 10, episode, 5, -2.5)]), 1);
    assert_eq!(desk.point_height(), -2.5);
    assert_eq!(desk.deliver([pending(&desk, 11, episode, 6, -3.5)]), 1);
    let newer = desk.picture();
    assert_eq!(desk.point_height(), -3.5);

    assert_eq!(
        desk.deliver([pending(&desk, 12, episode, 5, -2.5)]),
        0,
        "a replayed Pending ticket is not presented"
    );
    assert_eq!(desk.picture(), newer, "nothing was mutated");
    assert_eq!(preload(&desk).stale, 1);
    assert_eq!(desk.admission().stale, 0, "Live itself was newer");

    // The order is the ticket, not the Pending frame's generation.
    let mut regenerated = pending(&desk, 13, episode, 4, -2.5);
    regenerated.preload.as_mut().unwrap().frame = Some(stamp(4, 900));
    assert_eq!(desk.deliver([regenerated]), 0);
    assert_eq!(preload(&desk).stale, 2);
    assert_eq!(desk.deliver([pending(&desk, 14, episode, 7, -1.0)]), 1);
    assert_eq!(desk.point_height(), -1.0);
}

/// The very Pending frame already applied, with the same Live frame, is neither decoded nor
/// presented again; a new ticket with a new picture is.
#[test]
fn a_duplicate_pending_frame_is_neither_decoded_nor_presented_again() {
    let mut desk = following();
    let episode = Uuid::new_v4();
    assert_eq!(desk.deliver([pending(&desk, 3, episode, 1, -2.5)]), 1);
    let picture = desk.picture();
    for _ in 0..4 {
        assert_eq!(desk.deliver([pending(&desk, 3, episode, 1, -2.5)]), 0);
    }
    assert_eq!(desk.picture(), picture);
    assert_eq!(desk.admission().decoded, 1, "no decode for a duplicate");
    assert_eq!(desk.admission().held, 4);
    assert_eq!(preload(&desk).stale + preload(&desk).incoherent, 0);

    // The same Live frame with the episode's next accepted Pending frame is a new picture.
    assert_eq!(desk.deliver([pending(&desk, 3, episode, 2, -3.0)]), 1);
    assert_eq!(desk.point_height(), -3.0);
    assert_eq!(desk.admission().decoded, 2);
}

/// Pending stamps that contradict each other or the applied frame are not one picture; nothing
/// is decoded, and a later coherent frame is still accepted.
#[test]
fn incoherent_pending_stamps_are_rejected_before_mutation() {
    let mut desk = following();
    let episode = Uuid::new_v4();
    assert_eq!(desk.deliver([pending(&desk, 20, episode, 3, -2.5)]), 1);
    let held = desk.picture();

    // One ticket naming two different frames.
    let mut retimed = pending(&desk, 21, episode, 3, -2.5);
    retimed
        .preload
        .as_mut()
        .unwrap()
        .frame
        .as_mut()
        .unwrap()
        .sampled_at = "later".into();
    // A published state without its episode.
    let mut anonymous = pending(&desk, 22, episode, 4, -3.0);
    anonymous.preload_status.as_mut().unwrap().episode = None;
    // A published state whose lane carries no Pending stamp.
    let mut unstamped = pending(&desk, 23, episode, 4, -3.0);
    unstamped.preload.as_mut().unwrap().frame = None;
    // A passive state that still carries a lane.
    let mut contradicting = pending(&desk, 24, episode, 4, -3.0);
    contradicting.preload_status.as_mut().unwrap().state = OutputPreloadState::NotYetAvailable;
    for (case, output) in [
        ("retimed", retimed),
        ("anonymous", anonymous),
        ("unstamped", unstamped),
        ("contradicting", contradicting),
    ] {
        assert_eq!(desk.deliver([output]), 0, "{case}");
        assert_eq!(desk.picture(), held, "{case}");
    }
    assert_eq!(preload(&desk).incoherent, 4);
    assert_eq!(desk.admission().decoded, 1);

    assert_eq!(desk.deliver([pending(&desk, 25, episode, 4, -3.0)]), 1);
    assert_eq!(desk.point_height(), -3.0);
}

/// A new episode restarts its tickets and resets the order; the retired episode's late
/// snapshots are replays even with a higher ticket. A passive state moves nothing. A reconnect
/// forgets every Pending proof, like Live.
#[test]
fn a_new_episode_resets_the_order_and_a_reconnect_forgets_it() {
    let mut desk = following();
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    assert_eq!(desk.deliver([pending(&desk, 30, first, 9, -2.5)]), 1);
    assert_eq!(
        desk.deliver([pending(&desk, 31, second, 1, -3.5)]),
        1,
        "a new episode's first ticket is accepted"
    );
    assert_eq!(preload(&desk).resets, 1);
    assert_eq!(desk.point_height(), -3.5);
    let current = desk.picture();
    assert_eq!(
        desk.deliver([pending(&desk, 32, first, 10, -2.5)]),
        0,
        "the retired episode is a replay, whatever its ticket"
    );
    assert_eq!(desk.picture(), current);
    assert_eq!(preload(&desk).stale, 1);

    // Nothing published: the snapshot is admitted (Live only, no Preload lane to present), and
    // the Pending order is unchanged, so the episode's older ticket is still a replay after it.
    assert_eq!(
        desk.deliver([passive(&desk, 33, OutputPreloadState::NotYetAvailable)]),
        1
    );
    assert_eq!(
        desk.point_height(),
        -0.5,
        "no Preload lane, no Pending Point"
    );
    assert_eq!(desk.deliver([pending(&desk, 34, second, 2, -1.5)]), 1);
    assert_eq!(desk.deliver([pending(&desk, 35, second, 1, -3.5)]), 0);
    assert_eq!(desk.point_height(), -1.5);

    // A reconnect: the restarted server's episodes and tickets mean nothing to the old ones.
    desk.connect();
    assert_eq!(desk.deliver([pending(&desk, 1, first, 1, -2.0)]), 1);
    assert_eq!(desk.point_height(), -2.0);
    assert_eq!(
        desk.deliver([pending(&desk, 2, first, 1, -2.0)]),
        0,
        "duplicate"
    );
}

/// Without following Preload, the lane is not presented and its stamps are not consulted.
#[test]
fn an_unfollowed_preload_lane_is_neither_ordered_nor_rejected() {
    let mut desk = Desk::new(false);
    let episode = Uuid::new_v4();
    assert_eq!(desk.deliver([pending(&desk, 40, episode, 5, -2.5)]), 1);
    assert_eq!(desk.point_height(), -0.5, "the Live Point");
    // An older ticket in a newer Live snapshot is simply decoded: the lane is not shown.
    assert_eq!(desk.deliver([pending(&desk, 41, episode, 4, -2.5)]), 0);
    assert_eq!(desk.admission().decoded, 2);
    assert_eq!(preload(&desk).stale + preload(&desk).incoherent, 0);
}
