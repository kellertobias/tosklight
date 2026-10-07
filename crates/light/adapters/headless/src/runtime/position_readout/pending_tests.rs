//! TL-556: Pending Position readouts from the real retained paired Position evaluator
//! (synthetic authored profile, real engine/evaluator/physical lanes).
use super::*;
use crate::runtime::output_scheduler::{PendingNativeReadout, RealPositionPendingRig};
use light_core::programming::{PositionIntent, ProgrammingOwner, TargetReference};

fn ticket(sequence: u64) -> PendingAttemptTicket {
    PendingAttemptTicket::new(sequence)
}

fn target(offset: [f32; 3]) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Origin,
        offset,
    )))
}

/// A rig whose last accepted pair carries a whole Target FixAT for its target.
fn accepted_target_rig(with_copy: bool) -> RealPositionPendingRig {
    let mut rig = RealPositionPendingRig::new(with_copy);
    rig.fix_target([2., 6., 1.]);
    assert_eq!(rig.attempt().successful_attempts, 1);
    rig
}

fn capture(
    rig: &RealPositionPendingRig,
    sequence: u64,
) -> Result<CapturedPositionReadouts, PendingPositionReadoutError> {
    let accepted = rig.accepted().expect("an accepted real pair");
    capture_pending_position_readouts(
        rig.engine(),
        rig.identity(),
        ticket(sequence),
        accepted,
        &accepted.capture,
        &[rig.target()],
    )
}

/// One branch's composed Position request, if that branch produced a Position sidecar.
macro_rules! branch_position {
    ($branch:expr) => {
        $branch
            .sidecars
            .iter()
            .find(|row| row.owner == ProgrammingOwner::Position)
            .map(|row| &row.value)
    };
}

#[test]
fn pending_position_identity_is_the_native_readout_identity() {
    let mut rig = accepted_target_rig(false);
    let identity = rig.identity();
    let accepted = rig.accepted().unwrap();
    let native =
        PendingNativeReadout::prepare(identity, ticket(7), accepted, &accepted.capture).unwrap();
    let captured = capture(&rig, 7).unwrap();
    let lane = native.lane();
    assert_eq!(Some(&captured.identity), lane.frame.as_ref());
    assert_eq!(captured.identity.sequence, 7, "the owner's attempt ticket");
    assert_eq!(
        captured.identity.generation,
        accepted.value.after.frame_token.generation()
    );
    assert_eq!(captured.show_revision, lane.revision);
    assert_eq!(captured.scope.show_id, lane.show_id);
    assert_eq!(captured.scope.show_id, Some(identity.show_id.0));

    // The same shared validation refuses what the native readout refuses.
    let foreign = PendingEpisodeIdentity {
        programmer: ProgrammerId::new(),
        ..identity
    };
    assert_eq!(
        capture_pending_position_readouts(
            rig.engine(),
            foreign,
            ticket(7),
            accepted,
            &accepted.capture,
            &[rig.target()],
        )
        .unwrap_err(),
        PendingPositionReadoutError::Pair(PendingReadoutError::ForeignProgrammer)
    );
    let first_capture = Arc::clone(&accepted.capture);
    rig.move_position(90., 10.);
    assert_eq!(rig.attempt().successful_attempts, 1);
    let second = rig.accepted().unwrap();
    assert_eq!(
        capture_pending_position_readouts(
            rig.engine(),
            identity,
            ticket(8),
            second,
            &first_capture,
            &[rig.target()],
        )
        .unwrap_err(),
        PendingPositionReadoutError::Pair(PendingReadoutError::ForeignCapture),
        "the exact retained capture must be the pair's own allocation"
    );
}

#[test]
fn requested_value_comes_from_the_after_branch_never_before() {
    let mut rig = accepted_target_rig(false);
    let captured = capture(&rig, 1).unwrap();
    let after = branch_position!(&rig.accepted().unwrap().value.after).cloned();
    assert_eq!(after, Some(target([2., 6., 1.])));
    assert_eq!(captured.owners[0].requested, after);

    // A pending Position Release: Before keeps the pending Target family, After releases it.
    rig.release_position();
    assert_eq!(rig.attempt().successful_attempts, 1);
    let accepted = rig.accepted().unwrap();
    assert!(
        branch_position!(&accepted.value.before).is_some(),
        "Before keeps the pending Position family"
    );
    assert_eq!(branch_position!(&accepted.value.after), None);
    let released = capture(&rig, 2).unwrap();
    let owner = &released.owners[0];
    assert_eq!(
        owner.requested, None,
        "After released the typed request; Before's request is never borrowed"
    );
    let after_commands = rig
        .engine()
        .position_commanded_readouts_from_physical(
            accepted.value.after.frame_token.generation(),
            &accepted.value.rendered.projection.physical,
            &[rig.target()],
        )
        .unwrap();
    assert_eq!(owner.readout, after_commands[0]);
    assert!(owner.readout.commands.is_some());
}

#[test]
fn divergent_copies_are_unavailable_as_a_common_pose() {
    let rig = accepted_target_rig(true);
    let captured = capture(&rig, 1).unwrap();
    let owner = &captured.owners[0];
    let commands = owner.readout.commands.as_ref().unwrap();
    assert_eq!(
        commands.len(),
        2,
        "root and its physical copy stay distinct"
    );
    assert_ne!(
        (commands[0].pan_degrees, commands[0].tilt_degrees),
        (commands[1].pan_degrees, commands[1].tilt_degrees)
    );
    assert_eq!(owner.common_angles(), None, "no guessed common pair");
    assert_eq!(owner.requested, Some(target([2., 6., 1.])));
}

#[test]
fn a_swapped_or_failed_pair_keeps_the_previous_accepted_readout() {
    // The rig's moving Angle Dynamic: a newer static Position moves the accepted pose.
    let mut rig = RealPositionPendingRig::new(false);
    assert_eq!(rig.attempt().successful_attempts, 1);
    let before = capture(&rig, 1).unwrap();
    let accepted_capture = Arc::clone(&rig.accepted().unwrap().capture);

    rig.move_position(120., -20.);
    let failed = rig.attempt_with_swapped_finalization();
    assert_eq!(failed.successful_attempts, 0);
    assert!(Arc::ptr_eq(
        &rig.accepted().unwrap().capture,
        &accepted_capture
    ));
    let retained = capture(&rig, 1).unwrap();
    assert_eq!(retained.identity, before.identity);
    assert_eq!(retained.owners[0].requested, before.owners[0].requested);
    assert_eq!(retained.owners[0].readout, before.owners[0].readout);

    assert_eq!(rig.attempt().successful_attempts, 1);
    let newer = capture(&rig, 2).unwrap();
    assert_ne!(newer.identity, before.identity);
    assert_ne!(newer.owners[0].readout, before.owners[0].readout);
}

#[test]
fn a_stale_generation_is_passive() {
    let rig = accepted_target_rig(false);
    assert!(capture(&rig, 1).is_ok());
    rig.bump_generation();
    assert_eq!(
        capture(&rig, 1).unwrap_err(),
        PendingPositionReadoutError::StaleGeneration,
        "a newer runtime generation cannot reinterpret the accepted pair's commands"
    );
}
