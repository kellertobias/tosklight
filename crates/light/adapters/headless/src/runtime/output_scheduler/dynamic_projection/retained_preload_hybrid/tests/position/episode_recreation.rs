//! TL-556: Position Pending episode recreation invariants. Clear, GO and same-show reload each
//! build a new `PositionPendingEpisode`; nothing of the previous episode may survive in its
//! lanes, tracking, fit memo, evaluator token lineage, accepted pair or gate publication.
use super::*;
use crate::runtime::dynamic_snapshot_publication::RetainedInputCapture;
use crate::runtime::output_scheduler::dynamic_projection::pending_publication::{
    PendingAttemptTicket, PendingPublicationRejection,
};
use crate::runtime::output_scheduler::dynamic_projection::retained_preload_hybrid::position_episode::{
    PositionPendingEpisodeError, PositionPendingPublishError,
};
use light_engine::CapturedFrameLane;

/// The real Position rig with an explicit gate and a replaceable episode.
struct Desk {
    rig: Rig,
    copy: FixtureId,
    show: ShowId,
    gate: PendingPublicationGate,
    episode: Option<PositionPendingEpisode>,
    /// A capture retained before any episode seed, never fed to a window.
    unfed: Arc<RetainedInputCapture>,
}

impl Desk {
    fn new() -> Self {
        let rig = Rig::new();
        let copy = install_mover(&rig, true).unwrap();
        let show = ShowId::new();
        let unfed = rig.capture();
        let mut desk = Self {
            rig,
            copy,
            show,
            gate: PendingPublicationGate::default(),
            episode: None,
            unfed,
        };
        let activation = desk.rig.key.activation;
        desk.episode = Some(desk.begin(desk.identity(activation)));
        desk
    }

    fn identity(&self, activation: Uuid) -> PendingEpisodeIdentity {
        episode_identity(&self.rig, self.show, activation)
    }

    /// Begin another episode on the shared gate without ending the current bundle, as a
    /// lagging owner would leave it behind.
    fn begin(&mut self, identity: PendingEpisodeIdentity) -> PositionPendingEpisode {
        let (before, after) = seeds(&self.rig, identity.activation);
        PositionPendingEpisode::begin(&mut self.gate, identity, before, after).unwrap()
    }

    /// Clear/GO (same activation) or same-show reload (new activation).
    fn recreate(&mut self, activation: Uuid) -> PendingEpisodeIdentity {
        let identity = self.identity(activation);
        let (before, after) = seeds(&self.rig, activation);
        let old = self.episode.take().unwrap();
        self.episode = Some(
            old.recreate(&mut self.gate, identity, before, after)
                .unwrap(),
        );
        identity
    }

    fn episode(&self) -> &PositionPendingEpisode {
        self.episode.as_ref().unwrap()
    }

    fn attempt(&mut self, swap: bool) -> PendingPairWindowOutcome {
        let episode = self.episode.as_mut().unwrap();
        let window = window(&self.rig, episode.positions(), |inputs, before, after| {
            episode.prepare_window(inputs, &[], before, &[], after, limits())
        });
        if swap {
            episode.consume_window_with_swapped_finalization(&self.rig.engine, window)
        } else {
            episode.consume_window(&self.rig.engine, window)
        }
    }

    fn accept(&mut self) {
        assert_eq!(self.attempt(false).successful_attempts, 1);
    }

    fn publish(&mut self, sequence: u64) -> Result<(), PositionPendingPublishError> {
        let episode = self.episode.as_ref().unwrap();
        let exact = episode
            .accepted()
            .map(|pair| Arc::clone(&pair.capture))
            .unwrap_or_else(|| Arc::clone(&self.unfed));
        episode
            .publish_accepted(&mut self.gate, PendingAttemptTicket::new(sequence), &exact)
            .map(drop)
    }

    /// Accept two moving pairs and publish them, so continuity, tracking and fit memo are warm.
    fn warm(&mut self) -> Warm {
        self.accept();
        self.publish(1).unwrap();
        self.rig.position(75., 15.);
        self.accept();
        self.publish(2).unwrap();
        let pair = self.episode().accepted().unwrap();
        for branch in BRANCHES {
            let lane = self.episode().lanes().lane(branch);
            assert!(lane.last_accepted().is_some());
            assert!(lane.adapter().counters().fits > 0);
            assert!(
                lane.continuity(self.rig.target, ProgrammingOwner::Position)
                    .is_some()
            );
        }
        assert!(self.gate.latest().is_some());
        Warm {
            capture: Arc::clone(&pair.capture),
            lineage: lineage(&pair.value.after.frame_token),
        }
    }
}

/// What a warm episode exposed, to prove the next episode cannot see it.
struct Warm {
    capture: Arc<RetainedInputCapture>,
    lineage: Arc<()>,
}

/// The evaluator state (engine token lineage) a Preload token was rendered under.
fn lineage(token: &CapturedFrameToken) -> Arc<()> {
    match token.lane() {
        CapturedFrameLane::Preload { state, .. } => Arc::clone(state),
        CapturedFrameLane::Live => panic!("a Pending pair carries Preload tokens"),
    }
}

/// A begun episode exposes nothing until its own first pair is accepted.
fn assert_fresh(desk: &Desk) {
    let episode = desk.episode();
    assert!(episode.accepted().is_none(), "no last accepted pair");
    assert!(desk.gate.latest().is_none(), "no gate publication");
    assert_eq!(desk.gate.current(), Some(episode.identity()));
    assert_eq!(episode.lease().identity(), episode.identity());
    for branch in BRANCHES {
        let lane = episode.lanes().lane(branch);
        assert!(
            lane.last_accepted().is_none(),
            "{branch:?} has no last accepted frame"
        );
        assert!(lane.released().is_empty());
        assert_eq!(
            lane.descriptor_generation(),
            None,
            "{branch:?} descriptor cache"
        );
        assert_eq!(
            lane.adapter().counters(),
            Default::default(),
            "{branch:?} has no compiles, fits or fit-cache hits"
        );
        for target in [desk.rig.target, desk.copy] {
            assert_eq!(
                lane.continuity(target, ProgrammingOwner::Position),
                None,
                "{branch:?} has no continuity"
            );
        }
    }
}

/// The first accepted pair of a new episode is fitted from scratch in both branches and comes
/// from a new capture under a new evaluator token lineage.
fn assert_first_pair_is_fresh(desk: &Desk, warm: &Warm) {
    let episode = desk.episode();
    let pair = episode.accepted().unwrap();
    assert!(!Arc::ptr_eq(&pair.capture, &warm.capture));
    for (branch, result) in [
        (BRANCHES[0], &pair.value.before),
        (BRANCHES[1], &pair.value.after),
    ] {
        assert!(
            !Arc::ptr_eq(&lineage(&result.frame_token), &warm.lineage),
            "{branch:?} token belongs to a fresh evaluator state"
        );
        let counters = episode.lanes().lane(branch).adapter().counters();
        assert!(counters.fits > 0, "{branch:?} fitted");
        assert_eq!(counters.fit_cache_hits, 0, "{branch:?} reused no fit memo");
        let sidecar = owned(result);
        assert!(
            sidecar.quality.geometry_dirty,
            "{branch:?} has no accepted tracking to reuse"
        );
        assert_eq!(sidecar.quality.reused_fits, 0);
        assert_eq!(
            episode.lanes().lane(branch).last_accepted(),
            Some(result.frame_token.clone())
        );
    }
}

#[test]
fn a_new_episode_after_an_accepted_pose_has_no_accepted_state_tracking_or_publication() {
    let mut desk = Desk::new();
    let warm = desk.warm();
    let old = desk.episode().identity();
    let identity = desk.recreate(old.activation);
    assert_ne!(identity.episode, old.episode);
    assert_eq!(identity.activation, old.activation);
    assert_fresh(&desk);
    assert_eq!(
        desk.publish(3),
        Err(PositionPendingPublishError::NothingAccepted)
    );
    desk.accept();
    assert_first_pair_is_fresh(&desk, &warm);
    desk.publish(1)
        .expect("a new episode restarts its own ticket order");
    assert_eq!(desk.gate.latest().unwrap().identity(), identity);
}

#[test]
fn a_same_show_reload_with_a_new_activation_starts_from_nothing() {
    let mut desk = Desk::new();
    let warm = desk.warm();
    let old = desk.episode().identity();
    let identity = desk.recreate(Uuid::new_v4());
    assert_ne!(identity.activation, old.activation);
    assert_eq!(identity.show_id, old.show_id);
    assert_fresh(&desk);
    desk.accept();
    assert_first_pair_is_fresh(&desk, &warm);

    // Seeds of another activation cannot begin this identity; the gate keeps its episode.
    let current = desk.gate.current();
    let (before, after) = seeds(&desk.rig, old.activation);
    let reloaded = desk.identity(Uuid::new_v4());
    let refused = PositionPendingEpisode::begin(&mut desk.gate, reloaded, before, after);
    assert_eq!(
        refused.err(),
        Some(PositionPendingEpisodeError::SeedIdentity)
    );
    assert_eq!(desk.gate.current(), current);
}

#[test]
fn a_lease_from_the_old_episode_cannot_publish_into_the_new_episode() {
    let mut desk = Desk::new();
    desk.accept();
    let old = desk.episode.take().unwrap();
    let ticket = PendingAttemptTicket::new;
    let exact = Arc::clone(&old.accepted().unwrap().capture);

    // Even a new episode under the unchanged identity invalidates the old lease.
    let same = desk.begin(old.identity());
    assert_eq!(same.identity(), old.identity());
    assert_eq!(
        old.publish_accepted(&mut desk.gate, ticket(1), &exact)
            .err(),
        Some(PositionPendingPublishError::Rejected(
            PendingPublicationRejection::StaleLease
        ))
    );
    assert_eq!(
        old.record_gap(&mut desk.gate, ticket(1)),
        Err(PendingPublicationRejection::StaleLease)
    );
    assert!(desk.gate.latest().is_none(), "nothing old became visible");

    // A GO episode: the old bundle still cannot publish, nor end its successor.
    let go = desk.begin(desk.identity(old.identity().activation));
    assert_eq!(
        old.publish_accepted(&mut desk.gate, ticket(2), &exact)
            .err(),
        Some(PositionPendingPublishError::Rejected(
            PendingPublicationRejection::StaleLease
        ))
    );
    assert!(desk.gate.latest().is_none());
    assert!(!old.end(&mut desk.gate), "a superseded bundle ends nothing");
    assert!(!same.end(&mut desk.gate));
    assert_eq!(desk.gate.current(), Some(go.identity()));
    assert!(go.end(&mut desk.gate));
    assert_eq!(desk.gate.current(), None);
}

#[test]
fn a_failed_first_pair_attempt_of_a_new_episode_exposes_nothing() {
    let mut desk = Desk::new();
    let warm = desk.warm();
    desk.recreate(desk.rig.key.activation);
    desk.rig.position(110., -5.);
    let failed = desk.attempt(true);
    assert_eq!(failed.successful_attempts, 0);
    assert_eq!(failed.failed_attempts.len(), 1);
    let episode = desk.episode();
    assert!(
        episode.accepted().is_none(),
        "not the previous episode's pair"
    );
    for branch in BRANCHES {
        let lane = episode.lanes().lane(branch);
        assert!(lane.last_accepted().is_none());
        assert_eq!(
            lane.continuity(desk.rig.target, ProgrammingOwner::Position),
            None
        );
    }
    assert_eq!(
        desk.publish(1),
        Err(PositionPendingPublishError::NothingAccepted)
    );
    assert!(desk.gate.latest().is_none());
    desk.episode
        .as_ref()
        .unwrap()
        .record_gap(&mut desk.gate, PendingAttemptTicket::new(1))
        .unwrap();
    desk.accept();
    assert_first_pair_is_fresh(&desk, &warm);
    assert_eq!(
        desk.publish(1),
        Err(PositionPendingPublishError::Rejected(
            PendingPublicationRejection::StaleTicket {
                ticket: PendingAttemptTicket::new(1),
                floor: PendingAttemptTicket::new(1),
            }
        ))
    );
    desk.publish(2).unwrap();
}

#[test]
fn before_and_after_lanes_never_share_continuity() {
    let mut desk = Desk::new();
    let episode = desk.episode();
    let (before, after) = (
        episode.lanes().lane(BRANCHES[0]),
        episode.lanes().lane(BRANCHES[1]),
    );
    assert!(!std::ptr::eq(before.adapter(), after.adapter()));
    assert_eq!(before.kind(), PhysicalLaneKind::Preload(BRANCHES[0]));
    assert_eq!(after.kind(), PhysicalLaneKind::Preload(BRANCHES[1]));
    desk.accept();
    let episode = desk.episode();
    let pair = episode.accepted().unwrap();
    for (branch, result) in [
        (BRANCHES[0], &pair.value.before),
        (BRANCHES[1], &pair.value.after),
    ] {
        let lane = episode.lanes().lane(branch);
        let token = lane.last_accepted().unwrap();
        assert_eq!(token, result.frame_token);
        assert_eq!(token.lane().preload_branch(), Some(branch));
        // Each branch fitted for itself: the other branch's fit memo was never reused.
        let counters = lane.adapter().counters();
        assert!(counters.fits > 0);
        assert_eq!(counters.fit_cache_hits, 0);
        assert!(
            lane.continuity(desk.rig.target, ProgrammingOwner::Position)
                .is_some()
        );
    }
    assert_ne!(
        episode.lanes().lane(BRANCHES[0]).last_accepted(),
        episode.lanes().lane(BRANCHES[1]).last_accepted()
    );

    // A rejected pair advances neither branch: no branch commits from the other's token.
    let accepted = BRANCHES.map(|branch| episode.lanes().lane(branch).last_accepted());
    let continuity = BRANCHES.map(|branch| {
        episode
            .lanes()
            .lane(branch)
            .continuity(desk.rig.target, ProgrammingOwner::Position)
    });
    desk.rig.position(130., 25.);
    assert_eq!(desk.attempt(true).successful_attempts, 0);
    let episode = desk.episode();
    for (index, branch) in BRANCHES.into_iter().enumerate() {
        let lane = episode.lanes().lane(branch);
        assert_eq!(lane.last_accepted(), accepted[index]);
        assert_eq!(
            lane.continuity(desk.rig.target, ProgrammingOwner::Position),
            continuity[index]
        );
    }
}

#[test]
fn nothing_survives_clear_go_or_reload() {
    let mut desk = Desk::new();
    for (transition, activation) in [
        ("clear", None),
        ("GO", None),
        ("reload", Some(Uuid::new_v4())),
    ] {
        let warm = desk.warm();
        let old = desk.episode().identity();
        let identity = desk.recreate(activation.unwrap_or(old.activation));
        assert_ne!(identity, old, "{transition} begins a new identity");
        assert_fresh(&desk);
        desk.rig.position(20., 30.);
        desk.accept();
        assert_first_pair_is_fresh(&desk, &warm);
        // The new episode's first accepted readout is its own, under its own lease.
        desk.publish(1).unwrap();
        assert_eq!(desk.gate.latest().unwrap().identity(), identity);
        // Restart the next round's ticket order inside a fresh episode.
        let next = desk.identity(identity.activation);
        let (before, after) = seeds(&desk.rig, next.activation);
        let current = desk.episode.take().unwrap();
        desk.episode = Some(
            current
                .recreate(&mut desk.gate, next, before, after)
                .unwrap(),
        );
        assert_fresh(&desk);
    }
}
