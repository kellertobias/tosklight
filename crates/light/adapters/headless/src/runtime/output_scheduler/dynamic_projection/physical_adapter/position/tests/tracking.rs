//! Actual captured observer/finalizer coverage. Synthetic profiles verify commanded native
//! output and cache invalidation; these tests do not establish production cutover or feedback.
use super::*;

struct Desk {
    rig: Rig,
    copy: FixtureId,
    other: FixtureId,
    aim: FixtureId,
    mount: FixtureId,
    requested: AttributeValue,
    other_requested: AttributeValue,
    lane: PhysicalAdapterLane<PositionAdapter>,
    runtime: DynamicRuntime,
    origins: DynamicSourceOrigins,
    scratch: HybridFrameScratch,
}
impl Desk {
    fn new() -> Self {
        let profile = moving_head();
        let root = FixtureId::new();
        let copy = FixtureId::new();
        let other = FixtureId::new();
        let aim = FixtureId::new();
        let mount = FixtureId::new();
        let mut fixture = patched(&profile, root, 1);
        fixture.location.z = 3000;
        fixture.position_master = Some(mount.0);
        fixture.multipatch = vec![MultiPatchInstance {
            id: copy.0,
            universe: Some(1),
            address: Some(10),
            location: FixtureLocation {
                x: 3000,
                y: 500,
                z: 3500,
            },
            ..Default::default()
        }];
        let mut other_fixture = patched(&profile, other, 20);
        other_fixture.fixture_number = Some(2);
        let rig = Rig::new(
            vec![
                fixture,
                other_fixture,
                point(
                    aim,
                    FixtureLocation {
                        x: 2000,
                        y: 6000,
                        z: 1000,
                    },
                ),
                point(mount, FixtureLocation::default()),
            ],
            root,
        );
        let requested = target(TargetReference::Point { point_id: aim.0 }, [1., 0., 0.]);
        let other_requested = angles(45., 20.);
        rig.programmers.set(
            rig.session,
            root,
            ProgrammingOwner::Position.key(),
            requested.clone(),
        );
        rig.programmers.set(
            rig.session,
            other,
            ProgrammingOwner::Position.key(),
            other_requested.clone(),
        );
        Self {
            rig,
            copy,
            other,
            aim,
            mount,
            requested,
            other_requested,
            lane: PhysicalAdapterLane::live(PositionAdapter::default()),
            runtime: DynamicRuntime::with_programming_contract_support(
                PROGRAMMING_CONTRACT_VERSION,
            ),
            origins: Default::default(),
            scratch: Default::default(),
        }
    }
    fn run(
        &mut self,
        capture: &PreparedOutputFrame,
        finalize: &PreparedOutputFrame,
    ) -> Result<PublishedPhysicalFrame<PositionAdapter>, DynamicRuntimeError> {
        prepare_live(
            &self.rig,
            capture,
            finalize,
            &self.lane,
            &mut self.runtime,
            &mut self.origins,
            &mut self.scratch,
        )
    }
    fn tick(&mut self) -> PublishedPhysicalFrame<PositionAdapter> {
        let capture = self.rig.capture();
        let output = self.run(&capture, &capture).unwrap();
        self.verify(&output);
        assert!(
            self.runtime.snapshot().instances.is_empty(),
            "static geometry updates must not invent a Dynamic instance"
        );
        output
    }
    fn accepted(&self) -> Arc<super::super::tracking::TrackingSnapshot> {
        self.lane.adapter().tracking.borrow().snapshot().unwrap()
    }
    fn row<'a>(
        &self,
        output: &'a PublishedPhysicalFrame<PositionAdapter>,
        owner: FixtureId,
    ) -> &'a PhysicalHeadResult<PositionAdapter> {
        output
            .results
            .iter()
            .find(|row| row.target == owner)
            .unwrap()
    }
    fn native(
        &self,
        output: &PublishedPhysicalFrame<PositionAdapter>,
        destination: FixtureId,
    ) -> Vec<u32> {
        output
            .rendered
            .physical
            .instances
            .iter()
            .find(|instance| instance.instance_id == destination.0)
            .unwrap()
            .native_raw
            .to_vec()
    }
    fn verify(&self, output: &PublishedPhysicalFrame<PositionAdapter>) {
        assert!(output.requirements.is_empty());
        assert_eq!(output.results.len(), 2);
        let accepted = self.accepted();
        assert_eq!(accepted.token(), &output.token);
        for (owner, requested) in [
            (self.rig.root, &self.requested),
            (self.other, &self.other_requested),
        ] {
            let row = self.row(output, owner);
            assert_eq!(
                row.value, *requested,
                "resolved physical values cannot replace stored intent"
            );
            let PositionRequest::Intent(original) = &row.requested else {
                panic!("static original Intent");
            };
            assert_eq!(original, intent(requested).unwrap());
            let continuity = self
                .lane
                .continuity(owner, ProgrammingOwner::Position)
                .unwrap();
            for outcome in &row.achieved.outcomes {
                let instance = output
                    .rendered
                    .physical
                    .instances
                    .iter()
                    .find(|instance| instance.instance_id == outcome.destination.0)
                    .unwrap();
                assert!(instance.complete);
                for write in row
                    .writes
                    .iter()
                    .filter(|write| write.slot.destination == outcome.destination)
                {
                    assert_eq!(
                        instance.native_raw[write.slot.channel_index as usize], write.raw,
                        "finalizer must encode the proposed complete native pair"
                    );
                }
                let commands = continuity
                    .instances
                    .iter()
                    .find(|instance| instance.destination == outcome.destination)
                    .unwrap();
                for &(index, _, _, raw) in &commands.controls {
                    assert_eq!(instance.native_raw[index as usize], raw);
                }
                if outcome.result.status == PositionFitStatus::Fitted {
                    assert!(!row.quality.held);
                    assert!(!outcome.missing_mount);
                    if matches!(
                        outcome.result.requested,
                        Some(PositionFitRequest::Target { .. })
                    ) {
                        assert!(outcome.result.angular_error_degrees.unwrap() < 0.05);
                    }
                    let achieved = outcome.result.achieved.unwrap();
                    for (role, degrees) in [
                        (PositionAxisRole::Pan, achieved[0]),
                        (PositionAxisRole::Tilt, achieved[1]),
                    ] {
                        assert_eq!(
                            instance
                                .axes
                                .iter()
                                .find(|axis| axis.role == Some(role))
                                .unwrap()
                                .absolute_degrees(),
                            Some(degrees),
                            "Angle and Target outcomes must decode their actual calibrated native commands"
                        );
                    }
                    let local = instance
                        .lenses
                        .iter()
                        .find(|lens| lens.emitter_id == outcome.result.emitter_id)
                        .unwrap()
                        .local
                        .unwrap();
                    let mount = output
                        .geometry
                        .mounts()
                        .mount(outcome.destination.0)
                        .unwrap()
                        .world_from_fixture
                        .unwrap()
                        .desk_pose_to_profile();
                    assert_eq!(
                        outcome.result.pose,
                        Some(mount.compose(local)),
                        "cached and fresh fits agree with final native forward output and this capture's mount"
                    );
                }
            }
        }
    }
    fn warm(&mut self) -> PublishedPhysicalFrame<PositionAdapter> {
        let mut previous = self.tick();
        for _ in 0..6 {
            let before = self.lane.adapter().counters();
            let next = self.tick();
            let after = self.lane.adapter().counters();
            if after.fits == before.fits && after.fit_cache_hits - before.fit_cache_hits == 3 {
                for destination in [self.rig.root, self.copy, self.other] {
                    assert_eq!(
                        self.native(&previous, destination),
                        self.native(&next, destination)
                    );
                }
                assert_eq!(self.row(&next, self.rig.root).quality.reused_fits, 2);
                assert_eq!(self.row(&next, self.other).quality.reused_fits, 1);
                assert!(!self.row(&next, self.rig.root).quality.geometry_dirty);
                assert!(!self.row(&next, self.other).quality.geometry_dirty);
                assert!(self.accepted().dirty_instances().is_empty());
                return next;
            }
            previous = next;
        }
        panic!("three unchanged physical instances must reach fit reuse within six warm ticks");
    }
    fn assert_dependent_change(
        &mut self,
        before_output: &PublishedPhysicalFrame<PositionAdapter>,
        point: FixtureId,
    ) -> PublishedPhysicalFrame<PositionAdapter> {
        let before = self.lane.adapter().counters();
        let output = self.tick();
        let after = self.lane.adapter().counters();
        assert_eq!(
            after.fits - before.fits,
            2,
            "only the dependent root and physical copy need a new solve"
        );
        assert_eq!(
            after.fit_cache_hits - before.fit_cache_hits,
            1,
            "unrelated Angle owner remains reusable"
        );
        assert_eq!(
            self.native(before_output, self.other),
            self.native(&output, self.other)
        );
        assert_eq!(self.row(&output, self.rig.root).quality.reused_fits, 0);
        assert_eq!(self.row(&output, self.other).quality.reused_fits, 1);
        assert!(self.row(&output, self.rig.root).quality.geometry_dirty);
        assert!(!self.row(&output, self.other).quality.geometry_dirty);
        let accepted = self.accepted();
        assert_eq!(accepted.changed_points(), &[point]);
        assert!(accepted.dirty_instance(self.rig.root, self.rig.root));
        assert!(accepted.dirty_instance(self.rig.root, self.copy));
        assert!(!accepted.dirty_instance(self.other, self.other));
        assert_eq!(accepted.dirty_instances().len(), 2);
        assert_eq!(accepted.registry().dependents(point).len(), 2);
        for destination in [self.rig.root, self.copy] {
            assert_ne!(
                self.native(before_output, destination),
                self.native(&output, destination),
                "moving geometry changes that installation's aim"
            );
        }
        output
    }
}

#[test]
fn accepted_target_tracking_reuses_unchanged_fits_and_invalidates_only_aim_and_mount_dependents() {
    let mut desk = Desk::new();
    let unchanged = desk.warm();
    let before = desk.lane.adapter().counters();
    let plateau = desk.tick();
    let after = desk.lane.adapter().counters();
    assert_eq!(after.fits, before.fits);
    assert_eq!(after.fit_cache_hits - before.fit_cache_hits, 3);
    assert_eq!(
        desk.native(&unchanged, desk.rig.root),
        desk.native(&plateau, desk.rig.root)
    );

    desk.rig.set(desk.aim, "point.position.x", 0.51);
    desk.assert_dependent_change(&plateau, desk.aim);
    let unchanged = desk.warm();
    desk.rig.set(desk.aim, "point.rotation.z", 0.6);
    desk.assert_dependent_change(&unchanged, desk.aim);
    let unchanged = desk.warm();
    desk.rig.set(desk.mount, "point.position.z", 0.505);
    desk.assert_dependent_change(&unchanged, desk.mount);
    let unchanged = desk.warm();
    desk.rig.set(desk.mount, "point.rotation.z", 0.55);
    desk.assert_dependent_change(&unchanged, desk.mount);
}

#[test]
fn rejected_finalizer_preserves_accepted_tracking_and_continuity_then_retry_uses_new_geometry() {
    let mut desk = Desk::new();
    let unchanged = desk.warm();
    let accepted = desk.accepted();
    let continuity = desk
        .lane
        .continuity(desk.rig.root, ProgrammingOwner::Position)
        .unwrap();
    let other_continuity = desk
        .lane
        .continuity(desk.other, ProgrammingOwner::Position)
        .unwrap();
    desk.rig.set(desk.aim, "point.position.y", 0.51);
    let capture = desk.rig.capture();
    let foreign = desk.rig.capture();
    assert!(desk.run(&capture, &foreign).is_err());
    assert!(
        Arc::ptr_eq(&accepted, &desk.accepted()),
        "failed candidate cannot publish its dependency geometry"
    );
    assert_eq!(
        desk.lane
            .continuity(desk.rig.root, ProgrammingOwner::Position)
            .unwrap(),
        continuity
    );
    assert_eq!(
        desk.lane
            .continuity(desk.other, ProgrammingOwner::Position)
            .unwrap(),
        other_continuity
    );
    let retry = desk.tick();
    assert!(!Arc::ptr_eq(&accepted, &desk.accepted()));
    assert!(desk.accepted().changed_points().contains(&desk.aim));
    assert!(desk.row(&retry, desk.rig.root).quality.geometry_dirty);
    assert_ne!(
        desk.native(&unchanged, desk.rig.root),
        desk.native(&retry, desk.rig.root)
    );
    assert_eq!(
        desk.native(&unchanged, desk.other),
        desk.native(&retry, desk.other)
    );
}

#[test]
fn missing_then_recreated_aim_keeps_original_reference_and_inverse_subscription() {
    let mut desk = Desk::new();
    desk.warm();
    let snapshot = desk.rig.engine.snapshot();
    let mut recreated = snapshot
        .fixtures
        .iter()
        .find(|fixture| fixture.fixture_id == desk.aim)
        .unwrap()
        .clone();
    let fixtures = snapshot
        .fixtures
        .iter()
        .filter(|fixture| fixture.fixture_id != desk.aim)
        .cloned()
        .collect::<Vec<_>>();
    desk.rig
        .engine
        .replace_snapshot(EngineSnapshot {
            fixtures: fixtures.into(),
            revision: snapshot.revision + 1,
            ..snapshot.as_ref().clone()
        })
        .unwrap();
    let missing = desk.tick();
    let row = desk.row(&missing, desk.rig.root);
    assert!(row.quality.held);
    assert_eq!(row.achieved.outcomes.len(), 2);
    assert!(
        row.achieved
            .outcomes
            .iter()
            .all(|outcome| outcome.result.status == PositionFitStatus::MissingTarget)
    );
    assert!(row.writes.iter().all(|write| write.parked));
    assert_eq!(
        desk.accepted().registry().dependents(desk.aim).len(),
        2,
        "absence cannot erase subscriptions needed for recreation"
    );
    assert!(desk.accepted().changed_points().contains(&desk.aim));

    recreated.location.x = 5000;
    let snapshot = desk.rig.engine.snapshot();
    let mut fixtures = snapshot.fixtures.as_ref().clone();
    fixtures.push(recreated);
    desk.rig
        .engine
        .replace_snapshot(EngineSnapshot {
            fixtures: fixtures.into(),
            revision: snapshot.revision + 1,
            ..snapshot.as_ref().clone()
        })
        .unwrap();
    let restored = desk.tick();
    assert!(desk.accepted().changed_points().contains(&desk.aim));
    assert_eq!(desk.accepted().registry().dependents(desk.aim).len(), 2);
    let row = desk.row(&restored, desk.rig.root);
    assert!(!row.quality.held);
    assert!(
        row.achieved
            .outcomes
            .iter()
            .all(|outcome| outcome.result.status == PositionFitStatus::Fitted)
    );
    assert!(
        row.quality.geometry_dirty,
        "new generation conservatively discards fit reuse"
    );
}

use crate::runtime::dynamic_snapshot_publication::{
    DynamicSnapshotPublication, RetainedFrameCapture,
};
use crate::runtime::output_scheduler::dynamic_projection::retained_preload_hybrid::{
    PendingHybridResult, RetainedPreloadHybridEvaluator,
};
use crate::runtime::preload::retained_history::paired::{
    PairedPendingHistory, PendingPairEvaluator, PendingPairWindowOutcome,
};
use crate::runtime::preload::retained_history::{
    PendingEpisodeKey, PendingHistoryLimits, PendingHistoryPosition, PendingHistorySeed,
};

type TrackingPair = PairedPendingHistory<PendingHybridResult<PhysicalHeadResult<PositionAdapter>>>;
fn consume_tracking_pair(
    pair: &mut TrackingPair,
    input: Arc<crate::runtime::dynamic_snapshot_publication::RetainedInputCapture>,
    live: &DynamicRuntime,
    evaluator: &mut impl PendingPairEvaluator<PendingHybridResult<PhysicalHeadResult<PositionAdapter>>>,
) -> PendingPairWindowOutcome {
    let (before, after) = pair.positions();
    let before_controls = live.controls_since(before.controls).unwrap().unwrap();
    let after_controls = live.controls_since(after.controls).unwrap().unwrap();
    let capacity = |n| std::num::NonZeroUsize::new(n).unwrap();
    let prepared = pair
        .prepare_window(
            &[input],
            &[],
            &before_controls,
            &[],
            &after_controls,
            PendingHistoryLimits {
                attempts: capacity(8),
                cold_changes: capacity(8),
                controls: capacity(64),
            },
        )
        .unwrap();
    pair.consume_window(prepared, evaluator)
}

#[test]
fn actual_paired_finalizer_publishes_tracking_atomically_and_failed_pair_keeps_both_histories() {
    let mut desk = Desk::new();
    desk.warm();
    let live_tracking = desk.accepted();
    let live_continuity = desk
        .lane
        .continuity(desk.rig.root, ProgrammingOwner::Position)
        .unwrap();
    assert!(desk.rig.programmers.arm_preload(desk.rig.session, true));
    let snapshot = desk.rig.engine.snapshot();
    let publication = DynamicSnapshotPublication::new(snapshot.clone());
    let mut live = DynamicRuntime::with_programming_contract_support(PROGRAMMING_CONTRACT_VERSION);
    let (cold, controls) = publication
        .begin_retained_history(
            &mut live,
            &snapshot,
            std::num::NonZeroUsize::new(64).unwrap(),
        )
        .unwrap();
    let programmer = desk
        .rig
        .engine
        .prepare_output_frame(Default::default())
        .programmer()
        .identity
        .unwrap();
    let activation = Uuid::new_v4();
    let branches = [PreloadBranch::BeforeRelease, PreloadBranch::AfterRelease];
    let seed = |branch| PendingHistorySeed {
        key: PendingEpisodeKey {
            activation,
            programmer,
            branch,
        },
        runtime: live.fork_for_pending_preview(),
        origins: Default::default(),
        snapshot: snapshot.clone(),
        position: PendingHistoryPosition {
            inputs: publication.input_capture_cursor().unwrap(),
            cold,
            controls,
        },
        live_sample: live.committed_sample_boundary(),
    };
    let mut pair: TrackingPair =
        PairedPendingHistory::new(seed(branches[0]), seed(branches[1])).unwrap();
    let started = std::time::Instant::now();
    let mut selected = 0u64;
    let mut retained = || {
        let cursor = publication.input_capture_cursor().unwrap();
        let captured = RetainedFrameCapture::select(
            desk.rig.capture(),
            &publication,
            started + std::time::Duration::from_millis(selected * 40),
        );
        selected += 1;
        publication.retain_accepted_input(
            &live,
            captured.retained().unwrap(),
            &[],
            &[DynamicSpeedTransport {
                effective_bpm: 120.,
                phase_origin_millis: 0,
                phase_reference_millis: 0,
                beat_phase: 0.,
                phase_advancing: true,
            }; 5],
            40,
            None,
        );
        publication.input_captures_since(cursor).unwrap().remove(0)
    };
    let lanes = PhysicalPreloadLanes::new(PositionAdapter::default(), PositionAdapter::default());
    let mut evaluator = RetainedPreloadHybridEvaluator::new_with_observer(
        &desk.rig.engine,
        programmer,
        &lanes,
        PositionPreloadObserver::new(&lanes),
    );
    let first = consume_tracking_pair(&mut pair, retained(), &live, &mut evaluator);
    assert_eq!(first.successful_attempts, 1);
    assert!(first.failed_attempts.is_empty() && first.stopped.is_none());
    let before = lanes
        .lane(branches[0])
        .adapter()
        .tracking
        .borrow()
        .snapshot()
        .unwrap();
    let after = lanes
        .lane(branches[1])
        .adapter()
        .tracking
        .borrow()
        .snapshot()
        .unwrap();
    assert!(!Arc::ptr_eq(&before, &after));
    assert_eq!(before.token().lane().preload_branch(), Some(branches[0]));
    assert_eq!(after.token().lane().preload_branch(), Some(branches[1]));
    for accepted in [&before, &after] {
        assert_eq!(accepted.registry().dependents(desk.aim).len(), 2);
        assert!(
            !Arc::ptr_eq(accepted, &live_tracking),
            "a Pending branch never borrows Live tracking"
        );
    }
    let accepted_continuities = branches.map(|branch| {
        lanes
            .lane(branch)
            .continuity(desk.rig.root, ProgrammingOwner::Position)
            .unwrap()
    });
    let last_success = pair.last_success().unwrap().value.after.frame_token.clone();
    let native = pair
        .last_success()
        .unwrap()
        .value
        .rendered
        .projection
        .physical
        .instances
        .iter()
        .map(|instance| (instance.instance_id, instance.native_raw.to_vec()))
        .collect::<Vec<_>>();

    desk.rig.set(desk.aim, "point.position.x", 0.51);
    evaluator.swap_finalization_tokens = true;
    let failed = consume_tracking_pair(&mut pair, retained(), &live, &mut evaluator);
    assert_eq!(failed.successful_attempts, 0);
    assert_eq!(failed.failed_attempts.len(), 1);
    assert!(failed.stopped.is_none());
    assert!(Arc::ptr_eq(
        &before,
        &lanes
            .lane(branches[0])
            .adapter()
            .tracking
            .borrow()
            .snapshot()
            .unwrap()
    ));
    assert!(Arc::ptr_eq(
        &after,
        &lanes
            .lane(branches[1])
            .adapter()
            .tracking
            .borrow()
            .snapshot()
            .unwrap()
    ));
    for (index, branch) in branches.into_iter().enumerate() {
        assert_eq!(
            lanes
                .lane(branch)
                .continuity(desk.rig.root, ProgrammingOwner::Position)
                .unwrap(),
            accepted_continuities[index]
        );
    }
    assert_eq!(
        pair.last_success().unwrap().value.after.frame_token,
        last_success
    );
    assert_eq!(
        pair.last_success()
            .unwrap()
            .value
            .rendered
            .projection
            .physical
            .instances
            .iter()
            .map(|instance| (instance.instance_id, instance.native_raw.to_vec()))
            .collect::<Vec<_>>(),
        native,
        "the failed pair cannot publish either candidate's native output"
    );

    evaluator.swap_finalization_tokens = false;
    let retry = consume_tracking_pair(&mut pair, retained(), &live, &mut evaluator);
    assert_eq!(retry.successful_attempts, 1);
    assert!(retry.failed_attempts.is_empty() && retry.stopped.is_none());
    for (index, branch) in branches.into_iter().enumerate() {
        let accepted = lanes
            .lane(branch)
            .adapter()
            .tracking
            .borrow()
            .snapshot()
            .unwrap();
        assert!(accepted.changed_points().contains(&desk.aim));
        assert!(accepted.dirty_instance(desk.rig.root, desk.rig.root));
        assert!(accepted.dirty_instance(desk.rig.root, desk.copy));
        let old = if index == 0 { &before } else { &after };
        assert!(!Arc::ptr_eq(old, &accepted));
        assert_ne!(
            lanes
                .lane(branch)
                .continuity(desk.rig.root, ProgrammingOwner::Position)
                .unwrap(),
            accepted_continuities[index]
        );
    }
    assert_ne!(
        pair.last_success()
            .unwrap()
            .value
            .rendered
            .projection
            .physical
            .instances
            .iter()
            .map(|instance| (instance.instance_id, instance.native_raw.to_vec()))
            .collect::<Vec<_>>(),
        native
    );
    assert!(Arc::ptr_eq(&desk.accepted(), &live_tracking));
    assert_eq!(
        desk.lane
            .continuity(desk.rig.root, ProgrammingOwner::Position)
            .unwrap(),
        live_continuity
    );
}
