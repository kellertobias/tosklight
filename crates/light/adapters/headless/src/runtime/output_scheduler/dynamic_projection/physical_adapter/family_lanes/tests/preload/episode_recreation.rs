//! TL-548 C4: the TL-556 Position recreation suite, ported to the all-family Pending episode.
//! Clear, GO and same-show reload each build a new `FamilyPendingEpisode`; nothing of the
//! previous episode may survive in any family's lanes (Position, lamp and Media Color, Focus,
//! Zoom), tracking, fit memo, descriptor caches, the ONE shared evaluator token lineage, the
//! accepted pair or the gate publication.
use super::*;
use crate::runtime::output_scheduler::dynamic_projection::pending_publication::{
    PendingAttemptTicket, PendingEpisodeIdentity, PendingPublicationGate,
    PendingPublicationRejection,
};
use crate::runtime::output_scheduler::dynamic_projection::retained_preload_hybrid::family_episode::FamilyPendingEpisode;
use crate::runtime::output_scheduler::dynamic_projection::retained_preload_hybrid::pending_episode::{
    PendingEpisodeError, PendingEpisodePublishError,
};
use light_core::ShowId;
use light_engine::CapturedFrameLane;
use uuid::Uuid;

/// The mixed Preload rig with an explicit gate and a replaceable all-family episode.
struct Desk {
    rig: PreloadRig,
    show: ShowId,
    gate: PendingPublicationGate,
    episode: Option<FamilyPendingEpisode>,
    /// A capture retained before any episode seed, never fed to a window.
    unfed: Arc<RetainedInputCapture>,
}

impl Desk {
    fn new() -> Self {
        let rig = PreloadRig::new();
        let unfed = rig.capture();
        let mut desk = Self {
            rig,
            show: ShowId::new(),
            gate: PendingPublicationGate::default(),
            episode: None,
            unfed,
        };
        let activation = desk.rig.key.activation;
        desk.episode = Some(desk.begin(desk.identity(activation)));
        desk
    }

    fn identity(&self, activation: Uuid) -> PendingEpisodeIdentity {
        PendingEpisodeIdentity {
            show_id: self.show,
            activation,
            programmer: self.rig.key.programmer,
            episode: Uuid::new_v4(),
        }
    }

    /// Both branch seeds forked from the same authoritative boundary (the `seeds()` recipe).
    fn seeds(&self, activation: Uuid) -> (PendingHistorySeed, PendingHistorySeed) {
        let rig = &self.rig;
        let mut live = rig.live.borrow_mut();
        let (cold, controls) = rig
            .publication
            .begin_retained_history(&mut live, &rig.rig.engine.snapshot(), capacity(64))
            .unwrap();
        let key = PendingEpisodeKey {
            activation,
            ..rig.key
        };
        let seed = |branch| PendingHistorySeed {
            key: PendingEpisodeKey { branch, ..key },
            runtime: live.fork_for_pending_preview(),
            origins: Default::default(),
            snapshot: rig.rig.engine.snapshot(),
            position: PendingHistoryPosition {
                inputs: rig.publication.input_capture_cursor().unwrap(),
                cold,
                controls,
            },
            live_sample: live.committed_sample_boundary(),
        };
        (seed(BRANCHES[0]), seed(BRANCHES[1]))
    }

    /// Begin another episode without ending the current bundle (a lagging owner's leftover).
    fn begin(&mut self, identity: PendingEpisodeIdentity) -> FamilyPendingEpisode {
        let (before, after) = self.seeds(identity.activation);
        FamilyPendingEpisode::begin(&mut self.gate, identity, before, after).unwrap()
    }

    /// Clear/GO (same activation) or same-show reload (new activation).
    fn recreate(&mut self, activation: Uuid) -> PendingEpisodeIdentity {
        let identity = self.identity(activation);
        let (before, after) = self.seeds(activation);
        let old = self.episode.take().unwrap();
        self.episode = Some(
            old.recreate(&mut self.gate, identity, before, after)
                .unwrap(),
        );
        identity
    }

    fn episode(&self) -> &FamilyPendingEpisode {
        self.episode.as_ref().unwrap()
    }

    fn attempt(&mut self, swap: bool) -> PendingPairWindowOutcome {
        let input = self.rig.capture();
        let episode = self.episode.as_mut().unwrap();
        let (before, after) = episode.positions();
        let live = self.rig.live.borrow();
        let before_controls = live.controls_since(before.controls).unwrap().unwrap();
        let after_controls = live.controls_since(after.controls).unwrap().unwrap();
        drop(live);
        let window = episode
            .prepare_window(
                &[input],
                &[],
                &before_controls,
                &[],
                &after_controls,
                limits(),
            )
            .unwrap();
        if swap {
            episode.consume_window_with_swapped_finalization(&self.rig.rig.engine, window)
        } else {
            episode.consume_window(&self.rig.rig.engine, window)
        }
    }

    fn accept(&mut self) {
        succeeded(self.attempt(false));
    }

    fn publish(&mut self, sequence: u64) -> Result<(), PendingEpisodePublishError> {
        let episode = self.episode.as_ref().unwrap();
        let exact = episode
            .accepted()
            .map(|pair| Arc::clone(&pair.capture))
            .unwrap_or_else(|| Arc::clone(&self.unfed));
        episode
            .publish_accepted(&mut self.gate, PendingAttemptTicket::new(sequence), &exact)
            .map(drop)
    }

    /// Move every family's Pending request; each `step` must differ from the previous one.
    fn move_all(&self, step: f32) {
        let rig = &self.rig.rig;
        let show = rig.show;
        rig.fix(
            show.mover,
            ProgrammingOwner::Position,
            angles(10., 20.),
            angles(75. + step, 15.),
        );
        rig.fix(
            show.wash,
            ProgrammingOwner::Color,
            program(&magenta()),
            program(&intent([0., 1., 0.5 + step / 200.], 0.)),
        );
        rig.fix(
            show.optics,
            ProgrammingOwner::Zoom,
            field(30.),
            field(25. - step / 10.),
        );
    }

    /// Accept two moving pairs and publish them, so every family is warm.
    fn warm(&mut self) -> Warm {
        self.accept();
        self.publish(1).unwrap();
        self.move_all(0.);
        self.accept();
        self.publish(2).unwrap();
        let show = self.rig.rig.show;
        let pair = self.episode().accepted().unwrap();
        for branch in BRANCHES {
            let lanes = self.episode().lanes().lanes(branch);
            assert!(lanes.last_accepted().iter().all(Option::is_some));
            assert!(lanes.position().adapter().counters().fits > 0);
            assert!(continuity(lanes, &show).iter().all(|row| row != "None"));
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

/// The ONE evaluator state (engine token lineage) a Preload token was rendered under.
fn lineage(token: &CapturedFrameToken) -> Arc<()> {
    match token.lane() {
        CapturedFrameLane::Preload { state, .. } => Arc::clone(state),
        CapturedFrameLane::Live => panic!("a Pending pair carries Preload tokens"),
    }
}

/// A begun episode exposes nothing in any family until its own first pair is accepted.
fn assert_fresh(desk: &Desk) {
    let episode = desk.episode();
    let show = desk.rig.rig.show;
    assert!(episode.accepted().is_none(), "no last accepted pair");
    assert!(desk.gate.latest().is_none(), "no gate publication");
    assert_eq!(desk.gate.current(), Some(episode.identity()));
    assert_eq!(episode.lease().identity(), episode.identity());
    for branch in BRANCHES {
        let lanes = episode.lanes().lanes(branch);
        assert_eq!(lanes.kind(), PhysicalLaneKind::Preload(branch));
        assert_eq!(
            lanes.last_accepted(),
            [None, None, None, None],
            "{branch:?}"
        );
        assert!(lanes.released().is_empty(), "{branch:?} released nothing");
        assert!(
            continuity(lanes, &show).iter().all(|row| row == "None"),
            "{branch:?} has no continuity in any family"
        );
        let optics = |family| lanes.optics().lane(family).descriptor_generation();
        assert_eq!(
            [
                lanes.position().descriptor_generation(),
                lanes.color().descriptor_generation(),
                optics(OpticsFamily::Focus),
                optics(OpticsFamily::Zoom),
            ],
            [None; 4],
            "{branch:?} descriptor caches"
        );
        let color = lanes.color().adapter();
        assert_eq!(lanes.position().adapter().counters(), Default::default());
        assert_eq!(color.counters(), Default::default(), "{branch:?} routes");
        assert_eq!(
            color.lamp().counters(),
            Default::default(),
            "{branch:?} lamp"
        );
        assert_eq!(
            color.media().counters(),
            Default::default(),
            "{branch:?} Media"
        );
        assert_eq!(
            lanes.optics().counters(),
            Default::default(),
            "{branch:?} optics"
        );
    }
}

/// The first accepted pair of a new episode is evaluated from scratch in every family of both
/// branches and comes from a new capture under a new evaluator token lineage.
fn assert_first_pair_is_fresh(desk: &Desk, warm: &Warm) {
    let episode = desk.episode();
    let show = desk.rig.rig.show;
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
        let lanes = episode.lanes().lanes(branch);
        assert_eq!(lanes.last_accepted(), every(&result.frame_token));
        for (target, owner) in show.owners() {
            assert!(
                row(&result.sidecars, target, owner).is_some(),
                "{branch:?} {owner:?} evaluated"
            );
        }
        let counters = lanes.position().adapter().counters();
        assert!(counters.fits > 0, "{branch:?} fitted");
        assert_eq!(counters.fit_cache_hits, 0, "{branch:?} reused no fit memo");
        let position = row(&result.sidecars, show.mover, ProgrammingOwner::Position)
            .and_then(FamilySidecar::position)
            .unwrap();
        assert!(
            position.quality.geometry_dirty,
            "{branch:?} has no accepted tracking to reuse"
        );
        assert_eq!(position.quality.reused_fits, 0);
        assert!(
            lanes
                .color()
                .adapter()
                .lamp()
                .counters()
                .descriptor_compiles
                > 0
        );
        assert!(lanes.optics().counters().descriptor_compiles > 0);
    }
}

#[test]
fn a_new_family_episode_after_an_accepted_frame_has_no_accepted_state_tracking_or_publication() {
    let mut desk = Desk::new();
    let warm = desk.warm();
    let old = desk.episode().identity();
    let identity = desk.recreate(old.activation);
    assert_ne!(identity.episode, old.episode);
    assert_eq!(identity.activation, old.activation);
    assert_fresh(&desk);
    assert_eq!(
        desk.publish(3),
        Err(PendingEpisodePublishError::NothingAccepted)
    );
    desk.accept();
    assert_first_pair_is_fresh(&desk, &warm);
    desk.publish(1)
        .expect("a new episode restarts its own ticket order");
    assert_eq!(desk.gate.latest().unwrap().identity(), identity);
}

#[test]
fn a_same_show_reload_of_every_family_starts_from_nothing() {
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
    let (before, after) = desk.seeds(old.activation);
    let reloaded = desk.identity(Uuid::new_v4());
    let refused = FamilyPendingEpisode::begin(&mut desk.gate, reloaded, before, after);
    assert_eq!(refused.err(), Some(PendingEpisodeError::SeedIdentity));
    assert_eq!(desk.gate.current(), current);
}

#[test]
fn a_lease_from_the_old_family_episode_cannot_publish_into_the_new_episode() {
    let mut desk = Desk::new();
    desk.accept();
    let old = desk.episode.take().unwrap();
    let ticket = PendingAttemptTicket::new;
    let exact = Arc::clone(&old.accepted().unwrap().capture);
    let stale = Err(PendingEpisodePublishError::Rejected(
        PendingPublicationRejection::StaleLease,
    ));

    // Even a new episode under the unchanged identity invalidates the old lease.
    let same = desk.begin(old.identity());
    assert_eq!(
        old.publish_accepted(&mut desk.gate, ticket(1), &exact)
            .map(drop),
        stale
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
            .map(drop),
        stale
    );
    assert!(desk.gate.latest().is_none());
    assert!(!old.end(&mut desk.gate), "a superseded bundle ends nothing");
    assert!(!same.end(&mut desk.gate));
    assert_eq!(desk.gate.current(), Some(go.identity()));
    assert!(go.end(&mut desk.gate));
    assert_eq!(desk.gate.current(), None);
}

#[test]
fn a_failed_first_pair_of_a_new_family_episode_exposes_nothing() {
    let mut desk = Desk::new();
    let warm = desk.warm();
    desk.recreate(desk.rig.key.activation);
    desk.move_all(40.);
    let failed = desk.attempt(true);
    assert_eq!(failed.successful_attempts, 0);
    assert_eq!(failed.failed_attempts.len(), 1);
    let show = desk.rig.rig.show;
    let episode = desk.episode();
    assert!(
        episode.accepted().is_none(),
        "not the previous episode's pair"
    );
    for branch in BRANCHES {
        let lanes = episode.lanes().lanes(branch);
        assert_eq!(lanes.last_accepted(), [None, None, None, None]);
        assert!(continuity(lanes, &show).iter().all(|row| row == "None"));
    }
    assert_eq!(
        desk.publish(1),
        Err(PendingEpisodePublishError::NothingAccepted)
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
        Err(PendingEpisodePublishError::Rejected(
            PendingPublicationRejection::StaleTicket {
                ticket: PendingAttemptTicket::new(1),
                floor: PendingAttemptTicket::new(1),
            }
        ))
    );
    desk.publish(2).unwrap();
}

#[test]
fn before_and_after_family_lanes_never_share_continuity() {
    let mut desk = Desk::new();
    let show = desk.rig.rig.show;
    let episode = desk.episode();
    let (before, after) = (
        episode.lanes().lanes(BRANCHES[0]),
        episode.lanes().lanes(BRANCHES[1]),
    );
    assert!(!std::ptr::eq(
        before.position().adapter(),
        after.position().adapter()
    ));
    assert!(!std::ptr::eq(
        before.color().adapter(),
        after.color().adapter()
    ));
    desk.accept();
    let episode = desk.episode();
    let pair = episode.accepted().unwrap();
    for (branch, result) in [
        (BRANCHES[0], &pair.value.before),
        (BRANCHES[1], &pair.value.after),
    ] {
        let lanes = episode.lanes().lanes(branch);
        assert_eq!(lanes.last_accepted(), every(&result.frame_token));
        assert_eq!(result.frame_token.lane().preload_branch(), Some(branch));
        // Each branch fitted for itself: the other branch's fit memo was never reused.
        let counters = lanes.position().adapter().counters();
        assert!(counters.fits > 0);
        assert_eq!(counters.fit_cache_hits, 0);
        assert!(continuity(lanes, &show).iter().all(|row| row != "None"));
    }

    // A rejected pair advances no family lane of either branch.
    let accepted = BRANCHES.map(|branch| episode.lanes().lanes(branch).last_accepted());
    let committed = BRANCHES.map(|branch| continuity(episode.lanes().lanes(branch), &show));
    desk.move_all(55.);
    assert_eq!(desk.attempt(true).successful_attempts, 0);
    let episode = desk.episode();
    for (index, branch) in BRANCHES.into_iter().enumerate() {
        let lanes = episode.lanes().lanes(branch);
        assert_eq!(lanes.last_accepted(), accepted[index]);
        assert_eq!(continuity(lanes, &show), committed[index]);
    }
}

#[test]
fn nothing_of_any_family_survives_clear_go_or_reload() {
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
        desk.move_all(20.);
        desk.accept();
        assert_first_pair_is_fresh(&desk, &warm);
        // The new episode's first accepted readout is its own, under its own lease.
        desk.publish(1).unwrap();
        assert_eq!(desk.gate.latest().unwrap().identity(), identity);
        // Restart the next round's ticket order inside a fresh episode.
        let next = desk.identity(identity.activation);
        let (before, after) = desk.seeds(next.activation);
        let current = desk.episode.take().unwrap();
        desk.episode = Some(
            current
                .recreate(&mut desk.gate, next, before, after)
                .unwrap(),
        );
        assert_fresh(&desk);
    }
}
