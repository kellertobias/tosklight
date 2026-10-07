//! TL-610 regressions: the episode gate and readouts prepared from the real retained paired
//! Position evaluator (synthetic authored profile, real engine/evaluator/physical lanes).
use super::super::retained_preload_hybrid::tests::position::RealPositionPendingRig;
use super::*;

fn identity(programmer: ProgrammerId, show_id: ShowId) -> PendingEpisodeIdentity {
    PendingEpisodeIdentity {
        show_id,
        activation: Uuid::new_v4(),
        programmer,
        episode: Uuid::new_v4(),
    }
}

fn next_episode(identity: PendingEpisodeIdentity) -> PendingEpisodeIdentity {
    PendingEpisodeIdentity {
        episode: Uuid::new_v4(),
        ..identity
    }
}

/// Gate-only payload; real readouts below come from `prepare`.
fn synthetic(identity: PendingEpisodeIdentity, ticket: u64) -> PendingNativeReadout {
    PendingNativeReadout {
        identity,
        ticket: PendingAttemptTicket::new(ticket),
        lane: OutputNativeLane {
            show_id: Some(identity.show_id.0),
            revision: ticket,
            frame: None,
            instances: Vec::new(),
            points: Vec::new(),
        },
        values: PendingLaneValues::default(),
        omitted: PendingReadoutOmissions::default(),
    }
}

fn ticket(sequence: u64) -> PendingAttemptTicket {
    PendingAttemptTicket::new(sequence)
}

fn real_readout(
    rig: &RealPositionPendingRig,
    identity: PendingEpisodeIdentity,
    sequence: u64,
) -> PendingNativeReadout {
    let accepted = rig.accepted().expect("an accepted real pair");
    PendingNativeReadout::prepare(identity, ticket(sequence), accepted, &accepted.capture)
        .expect("the accepted pair and its own capture form a readout")
}

/// The rig's Pending Position is Dynamic-sourced; a static Pending intensity is what the
/// engine's Preload projection reports as a winning pending address with a native mask.
fn accepted_rig(with_copy: bool) -> RealPositionPendingRig {
    let mut rig = RealPositionPendingRig::new(with_copy);
    rig.set_pending(
        light_core::AttributeKey::intensity(),
        light_core::AttributeValue::Normalized(0.5),
    );
    assert_eq!(rig.attempt().successful_attempts, 1);
    rig
}

/// TL-610 finding, fixed in TL-548 C2: the rig's only Pending value is a Dynamic-sourced
/// Position. The engine now previews every Dynamic Pending address, and a previewed owner owns
/// exactly the controls its fitted native writes installed. Both physical rows are emitted under
/// that mask. The invariant is unchanged: the same complete rows without a mask emit nothing.
#[test]
fn real_dynamic_only_pending_position_rows_are_emitted_only_under_their_native_mask() {
    let mut rig = RealPositionPendingRig::new(true);
    assert_eq!(rig.attempt().successful_attempts, 1);
    let accepted = rig.accepted().unwrap();
    let projection = &accepted.value.rendered.projection;
    assert_eq!(
        projection.native_ownership.len(),
        1,
        "only the Pending target is owned"
    );
    let mask = projection
        .native_ownership
        .get(&rig.target())
        .expect("a Dynamic-only Pending Position projects a native ownership mask");
    let fixture = &accepted.capture.frame.snapshot().fixtures[0];
    let mode = fixture
        .definition
        .profile_snapshot
        .as_ref()
        .unwrap()
        .modes
        .iter()
        .find(|mode| Some(mode.id) == fixture.definition.mode_id)
        .unwrap();
    let owned: Vec<_> = mode
        .channels
        .iter()
        .zip(mask.iter())
        .filter(|(_, owned)| **owned)
        .map(|(channel, _)| channel.attribute.0.as_ref())
        .collect();
    assert_eq!(
        owned,
        ["pan", "tilt"],
        "exactly the fitted Position controls; Focus and intensity stay unowned"
    );
    let complete = projection
        .physical
        .instances
        .iter()
        .filter(|i| i.complete)
        .count();
    assert_eq!(complete, 2, "both projected rows are complete");
    let readout = real_readout(&rig, identity(rig.programmer(), ShowId::new()), 1);
    assert_eq!(readout.omitted(), PendingReadoutOmissions::default());
    assert_eq!(
        readout.lane().instances.len(),
        2,
        "root and its physical copy"
    );
    for instance in &projection.physical.instances {
        let emitted = readout
            .lane()
            .instances
            .iter()
            .find(|i| i.instance_id == instance.instance_id)
            .unwrap();
        assert_eq!(emitted.raw, instance.native_raw.to_vec());
        assert_eq!(emitted.owned_channels.as_deref(), Some(mask.as_ref()));
    }
    assert!(readout.lane().frame.is_some());
    let rows: Vec<_> = projected_rows(&projection.physical).collect();
    let (emitted, omitted) = emit_native_instances(rows.iter().copied(), &HashMap::new());
    assert!(
        emitted.is_empty(),
        "complete rows without a mask never become complete native overrides"
    );
    assert_eq!(omitted.unowned, 2);
}

#[test]
fn real_pair_readout_derives_rows_points_tracking_and_frame_from_the_accepted_pair() {
    let rig = accepted_rig(true);
    let show = ShowId::new();
    let identity = identity(rig.programmer(), show);
    let readout = real_readout(&rig, identity, 7);
    let accepted = rig.accepted().unwrap();
    let projection = &accepted.value.rendered.projection;
    let capture = &accepted.capture.frame;
    let after = &accepted.value.after.frame_token;
    let lane = readout.lane();

    assert_eq!(readout.identity(), identity);
    assert_eq!(readout.ticket(), ticket(7));
    assert_eq!(
        lane.show_id,
        Some(show.0),
        "actual show, not the activation nonce"
    );
    assert_ne!(Some(identity.activation), lane.show_id);
    assert_eq!(lane.revision, capture.snapshot().revision);
    let frame = lane.frame.as_ref().unwrap();
    assert_eq!(frame.sequence, 7, "sequence is the owner's attempt ticket");
    assert_eq!(frame.generation, after.generation());
    assert_eq!(frame.sampled_at, after.sampled_at().to_rfc3339());
    let tracking = frame.tracking.as_ref().unwrap();
    assert_eq!(
        tracking.accepted_sequence,
        capture.tracking().accepted_sequence
    );
    assert_eq!(
        tracking.configuration_generation,
        capture.tracking().configuration_generation
    );
    assert_eq!(
        tracking.point_generation,
        capture.tracking().point_generation
    );
    assert_eq!(
        tracking.source_generation,
        capture.tracking().source_generation
    );
    assert_eq!(
        tracking.sampled_at_millis,
        capture.tracking().sampled_at_millis
    );

    assert_eq!(lane.points, native_points(&projection.points));
    assert_eq!(
        lane.points.len(),
        projection.points.len(),
        "this authored rig has no Point poses; mapping is checked separately"
    );

    let mask = projection
        .native_ownership
        .get(&rig.target())
        .expect("the winning static pending intensity projects a native ownership mask");
    assert!(
        mask.iter().any(|v| *v) && mask.iter().any(|v| !*v),
        "a sparse mask: only channels of the winning pending address: {mask:?}"
    );
    assert_eq!(lane.instances.len(), 2, "root and its physical copy");
    assert_eq!(readout.omitted(), PendingReadoutOmissions::default());
    for instance in &projection.physical.instances {
        assert!(instance.complete);
        let emitted = lane
            .instances
            .iter()
            .find(|i| i.instance_id == instance.instance_id)
            .unwrap();
        assert_eq!(emitted.fixture_id, rig.target().0);
        assert_eq!(emitted.raw, instance.native_raw.to_vec());
        assert_eq!(
            emitted.native_identity,
            instance.native_identity.to_string()
        );
        assert_eq!(emitted.owned_channels.as_deref(), Some(mask.as_ref()));
    }
    assert!(
        lane.instances
            .iter()
            .any(|i| Some(FixtureId(i.instance_id)) == rig.copy()),
        "the multipatch copy is emitted as its own instance"
    );

    let mut gate = PendingPublicationGate::default();
    let lease = gate.begin_episode(identity);
    let published = gate.publish(&lease, readout.clone()).unwrap();
    assert_eq!(*published, readout);
}

#[test]
fn swapped_cross_attempt_and_foreign_capture_inputs_are_rejected() {
    let mut rig = accepted_rig(false);
    let identity = identity(rig.programmer(), ShowId::new());
    let first = rig.accepted().unwrap();
    let first_capture = Arc::clone(&first.capture);
    let first_before = first.value.before.frame_token.clone();
    let first_after = first.value.after.frame_token.clone();

    let parts = |accepted: &Arc<RetainedInputCapture>,
                 exact: &Arc<RetainedInputCapture>,
                 before: &CapturedFrameToken,
                 after: &CapturedFrameToken,
                 rendered: &RenderedPreloadFrame| {
        PendingNativeReadout::prepare_parts(
            identity,
            ticket(1),
            accepted,
            exact,
            before,
            after,
            rendered,
        )
    };
    let rendered = &first.value.rendered;
    assert!(
        parts(
            &first_capture,
            &first_capture,
            &first_before,
            &first_after,
            rendered
        )
        .is_ok()
    );
    assert_eq!(
        parts(
            &first_capture,
            &first_capture,
            &first_after,
            &first_before,
            rendered
        ),
        Err(PendingReadoutError::BranchLineage),
        "swapped Before/After tokens"
    );
    assert_eq!(
        parts(
            &first_capture,
            &first_capture,
            &first_after,
            &first_after,
            rendered
        ),
        Err(PendingReadoutError::BranchLineage),
        "one branch supplied twice"
    );
    assert_eq!(
        PendingNativeReadout::prepare(
            identity_with_programmer(identity, ProgrammerId::new()),
            ticket(1),
            first,
            &first_capture
        ),
        Err(PendingReadoutError::ForeignProgrammer)
    );

    rig.move_position(90., 10.);
    assert_eq!(rig.attempt().successful_attempts, 1);
    let second = rig.accepted().unwrap();
    assert!(!Arc::ptr_eq(&second.capture, &first_capture));
    assert_eq!(
        PendingNativeReadout::prepare(identity, ticket(2), second, &first_capture),
        Err(PendingReadoutError::ForeignCapture),
        "the exact retained capture must be the pair's own allocation"
    );
    let second_rendered = &second.value.rendered;
    assert_eq!(
        parts(
            &second.capture,
            &second.capture,
            &first_before,
            &second.value.after.frame_token,
            second_rendered
        ),
        Err(PendingReadoutError::TokenCapture(
            PreloadBranch::BeforeRelease
        )),
        "a Before token from an earlier capture"
    );
    assert_eq!(
        parts(
            &second.capture,
            &second.capture,
            &second.value.before.frame_token,
            &first_after,
            second_rendered
        ),
        Err(PendingReadoutError::TokenCapture(
            PreloadBranch::AfterRelease
        )),
        "an After token from an earlier capture"
    );
    assert!(PendingNativeReadout::prepare(identity, ticket(2), second, &second.capture).is_ok());
}

fn identity_with_programmer(
    identity: PendingEpisodeIdentity,
    programmer: ProgrammerId,
) -> PendingEpisodeIdentity {
    PendingEpisodeIdentity {
        programmer,
        ..identity
    }
}

#[test]
fn readout_bytes_points_and_tracking_are_unchanged_after_external_inputs_advance() {
    let mut rig = accepted_rig(true);
    let identity = identity(rig.programmer(), ShowId::new());
    let mut gate = PendingPublicationGate::default();
    let lease = gate.begin_episode(identity);
    let first = gate
        .publish(&lease, real_readout(&rig, identity, 1))
        .unwrap();
    let frozen = (*first).clone();

    rig.move_position(120., -20.);
    assert_eq!(rig.attempt().successful_attempts, 1);
    let second = real_readout(&rig, identity, 2);
    assert_ne!(
        second.lane().instances,
        frozen.lane.instances,
        "the newer accepted pair commands different native bytes"
    );
    assert_ne!(second.lane().frame, frozen.lane.frame);
    assert_eq!(
        *first, frozen,
        "the earlier readout owns its bytes, points and tracking"
    );
    assert_eq!(**gate.latest().unwrap(), frozen);

    gate.publish(&lease, second.clone()).unwrap();
    assert_eq!(**gate.latest().unwrap(), second);
    assert_eq!(*first, frozen, "a held Arc survives replacement unchanged");
}

#[test]
fn sparse_masks_survive_missing_or_wrong_sized_masks_and_incomplete_rows_are_omitted() {
    let rig = accepted_rig(true);
    let projection = &rig.accepted().unwrap().value.rendered.projection;
    let rows = projected_rows(&projection.physical).collect::<Vec<_>>();
    let masks = projection.native_ownership.clone();
    let mask = masks.get(&rig.target()).unwrap().clone();
    assert_eq!(rows.len(), 2);

    let (emitted, omitted) = emit_native_instances(rows.iter().copied(), &masks);
    assert_eq!(omitted, PendingReadoutOmissions::default());
    assert!(
        emitted
            .iter()
            .all(|i| i.owned_channels.as_deref() == Some(mask.as_ref()))
    );

    // Another fixture keeps its own sparse mask rather than inheriting a neighbour's.
    let other = FixtureId::new();
    let other_raw = [11, 22, 33];
    let mut with_other = masks.clone();
    with_other.insert(other, vec![false, true, false].into_boxed_slice());
    let other_row = NativeRow {
        fixture_id: other,
        instance_id: other.0,
        native_identity: "other",
        raw: &other_raw,
        complete: true,
    };
    let (emitted, _) = emit_native_instances(rows.iter().copied().chain([other_row]), &with_other);
    let emitted_other = emitted.iter().find(|i| i.fixture_id == other.0).unwrap();
    assert_eq!(emitted_other.owned_channels, Some(vec![false, true, false]));
    assert_eq!(emitted_other.raw, vec![11, 22, 33]);

    let mut missing = masks.clone();
    missing.remove(&rig.target());
    let (emitted, omitted) = emit_native_instances(rows.iter().copied(), &missing);
    assert!(
        emitted.is_empty(),
        "absent mask never becomes a complete override"
    );
    assert_eq!(omitted.unowned, 2);

    for wrong in [
        mask[..mask.len() - 1].to_vec(),
        [mask.to_vec(), vec![true]].concat(),
        vec![false; mask.len()],
    ] {
        let mut sized = masks.clone();
        sized.insert(rig.target(), wrong.into_boxed_slice());
        let (emitted, omitted) = emit_native_instances(rows.iter().copied(), &sized);
        assert!(emitted.is_empty());
        assert_eq!(omitted.invalid_mask, 2);
    }

    let mut incomplete = rows.clone();
    incomplete[1].complete = false;
    let (emitted, omitted) = emit_native_instances(incomplete.iter().copied(), &masks);
    assert_eq!(omitted.incomplete, 1);
    assert_eq!(emitted.len(), 1);
    assert_eq!(emitted[0].instance_id, rows[0].instance_id);
    assert!(emitted.iter().all(|i| i.owned_channels.is_some()));
}

#[test]
fn stale_lease_after_clear_go_new_episode_and_same_show_reload_is_rejected() {
    let programmer = ProgrammerId::new();
    let show = ShowId::new();
    let first = identity(programmer, show);
    let mut gate = PendingPublicationGate::default();
    let lease = gate.begin_episode(first);
    gate.publish(&lease, synthetic(first, 1)).unwrap();

    // Clear: the episode ends and its readout is hidden.
    assert!(gate.end_episode(&lease));
    assert!(gate.latest().is_none());
    assert_eq!(
        gate.publish(&lease, synthetic(first, 2)),
        Err(PendingPublicationRejection::NoEpisode)
    );

    // GO / new episode of the same activation.
    let second = next_episode(first);
    let second_lease = gate.begin_episode(second);
    assert!(
        gate.latest().is_none(),
        "a new episode exposes no old result"
    );
    assert_eq!(
        gate.publish(&lease, synthetic(first, 3)),
        Err(PendingPublicationRejection::StaleLease)
    );
    gate.publish(&second_lease, synthetic(second, 1)).unwrap();

    // Same-show reload: same show and Programmer, new activation nonce.
    let reload = PendingEpisodeIdentity {
        activation: Uuid::new_v4(),
        episode: Uuid::new_v4(),
        ..second
    };
    assert_eq!(reload.show_id, second.show_id);
    let reload_lease = gate.begin_episode(reload);
    assert!(gate.latest().is_none());
    assert_eq!(
        gate.publish(&second_lease, synthetic(second, 2)),
        Err(PendingPublicationRejection::StaleLease)
    );
    assert_eq!(
        gate.record_gap(&second_lease, ticket(9)),
        Err(PendingPublicationRejection::StaleLease)
    );

    // Even an identical identity begun again yields a fresh lease.
    let again = gate.begin_episode(reload);
    assert_eq!(
        gate.publish(&reload_lease, synthetic(reload, 1)),
        Err(PendingPublicationRejection::StaleLease)
    );
    assert!(gate.latest().is_none());
    gate.publish(&again, synthetic(reload, 1)).unwrap();
    assert_eq!(gate.current(), Some(reload));
}

#[test]
fn failed_real_attempt_keeps_last_accepted_arc_and_reset_hides_it() {
    let mut rig = accepted_rig(false);
    let identity = identity(rig.programmer(), ShowId::new());
    let mut gate = PendingPublicationGate::default();
    let lease = gate.begin_episode(identity);
    let accepted = gate
        .publish(&lease, real_readout(&rig, identity, 1))
        .unwrap();
    let accepted_capture = Arc::clone(&rig.accepted().unwrap().capture);

    rig.move_position(120., -20.);
    let failed = rig.attempt_with_swapped_finalization();
    assert_eq!(failed.successful_attempts, 0);
    assert_eq!(failed.failed_attempts.len(), 1);
    assert!(
        Arc::ptr_eq(&rig.accepted().unwrap().capture, &accepted_capture),
        "the paired evaluator kept its last success"
    );
    gate.record_gap(&lease, ticket(2)).unwrap();
    assert!(Arc::ptr_eq(gate.latest().unwrap(), &accepted));

    // A late success with a ticket at or before the gap cannot replace the retained result.
    assert_eq!(
        gate.publish(&lease, real_readout(&rig, identity, 2)),
        Err(PendingPublicationRejection::StaleTicket {
            ticket: ticket(2),
            floor: ticket(2)
        })
    );
    assert!(Arc::ptr_eq(gate.latest().unwrap(), &accepted));

    assert_eq!(rig.attempt().successful_attempts, 1);
    let retried = gate
        .publish(&lease, real_readout(&rig, identity, 3))
        .unwrap();
    assert!(!Arc::ptr_eq(&retried, &accepted));
    assert!(Arc::ptr_eq(gate.latest().unwrap(), &retried));

    assert!(gate.end_episode(&lease));
    assert!(
        gate.latest().is_none(),
        "reset hides the last accepted readout"
    );
}

#[test]
fn duplicate_and_older_tickets_are_rejected_without_replacing_latest() {
    let identity = identity(ProgrammerId::new(), ShowId::new());
    let mut gate = PendingPublicationGate::default();
    let lease = gate.begin_episode(identity);
    let five = gate.publish(&lease, synthetic(identity, 5)).unwrap();
    for stale in [5, 4, 0] {
        assert_eq!(
            gate.publish(&lease, synthetic(identity, stale)),
            Err(PendingPublicationRejection::StaleTicket {
                ticket: ticket(stale),
                floor: ticket(5)
            })
        );
        assert_eq!(
            gate.record_gap(&lease, ticket(stale)),
            Err(PendingPublicationRejection::StaleTicket {
                ticket: ticket(stale),
                floor: ticket(5)
            })
        );
        assert!(Arc::ptr_eq(gate.latest().unwrap(), &five));
    }
    let six = gate.publish(&lease, synthetic(identity, 6)).unwrap();
    assert!(Arc::ptr_eq(gate.latest().unwrap(), &six));
}

#[test]
fn foreign_identity_and_foreign_gate_lease_are_rejected() {
    let programmer = ProgrammerId::new();
    let identity = identity(programmer, ShowId::new());
    let mut gate = PendingPublicationGate::default();
    let lease = gate.begin_episode(identity);
    let accepted = gate.publish(&lease, synthetic(identity, 1)).unwrap();

    for foreign in [
        identity_with_programmer(identity, ProgrammerId::new()),
        next_episode(identity),
        PendingEpisodeIdentity {
            activation: Uuid::new_v4(),
            ..identity
        },
        PendingEpisodeIdentity {
            show_id: ShowId::new(),
            ..identity
        },
    ] {
        assert_eq!(
            gate.publish(&lease, synthetic(foreign, 2)),
            Err(PendingPublicationRejection::ForeignIdentity)
        );
    }
    // Another gate's lease for the very same identity is not this episode's authority.
    let mut other = PendingPublicationGate::default();
    let other_lease = other.begin_episode(identity);
    assert_eq!(
        gate.publish(&other_lease, synthetic(identity, 2)),
        Err(PendingPublicationRejection::StaleLease)
    );
    assert!(!gate.end_episode(&other_lease));
    assert!(Arc::ptr_eq(gate.latest().unwrap(), &accepted));
    assert_eq!(gate.current(), Some(identity));

    // A foreign identity does not consume a ticket.
    gate.publish(&lease, synthetic(identity, 2)).unwrap();
}

#[test]
fn repeated_reads_return_the_same_immutable_allocation() {
    let rig = accepted_rig(false);
    let identity = identity(rig.programmer(), ShowId::new());
    let mut gate = PendingPublicationGate::default();
    let lease = gate.begin_episode(identity);
    assert!(gate.latest().is_none());
    let published = gate
        .publish(&lease, real_readout(&rig, identity, 1))
        .unwrap();
    let first = Arc::clone(gate.latest().unwrap());
    let second = Arc::clone(gate.latest().unwrap());
    assert!(Arc::ptr_eq(&first, &published));
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(Arc::strong_count(&published), 4);
}

#[test]
fn end_episode_only_ends_the_matching_current_lease() {
    let identity = identity(ProgrammerId::new(), ShowId::new());
    let mut gate = PendingPublicationGate::default();
    let old = gate.begin_episode(identity);
    let current = gate.begin_episode(next_episode(identity));
    let latest = gate
        .publish(&current, synthetic(current.identity(), 1))
        .unwrap();
    assert!(
        !gate.end_episode(&old),
        "a superseded lease cannot clear the current episode"
    );
    assert!(Arc::ptr_eq(gate.latest().unwrap(), &latest));
    assert!(gate.end_episode(&current));
    assert!(!gate.end_episode(&current));
    assert_eq!(gate.current(), None);
}

#[test]
fn point_poses_are_copied_exactly_from_the_projection() {
    let fixture = FixtureId::new();
    let projected = [ResolvedPointPose {
        fixture_id: fixture,
        origin_metres: [9., 9., 9.],
        offset_metres: [1.5, -2.25, 3.],
        rotation_degrees: [10., -20., 30.5],
    }];
    assert_eq!(
        native_points(&projected),
        vec![OutputPointPose {
            fixture_id: fixture.0,
            offset_metres: [1.5, -2.25, 3.],
            rotation_degrees: [10., -20., 30.5],
        }]
    );
}
