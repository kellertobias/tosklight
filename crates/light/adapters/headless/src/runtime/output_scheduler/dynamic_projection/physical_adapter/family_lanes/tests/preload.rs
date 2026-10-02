//! TL-548 C1 retained Preload: ONE `FamilyPreloadLanes` set through the real paired evaluator,
//! paired history and Preload finalizer, over the same mixed show as the Live cases.
use super::*;
use crate::runtime::dynamic_snapshot_publication::{
    DynamicSnapshotPublication, RetainedFrameCapture, RetainedInputCapture,
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
use std::cell::{Cell, RefCell};
use std::num::NonZeroUsize;
use std::time::{Duration, Instant};

#[path = "preload/episode_recreation.rs"]
mod episode_recreation;

type Pair = PairedPendingHistory<PendingHybridResult<FamilySidecar>>;
const BRANCHES: [PreloadBranch; 2] = [PreloadBranch::BeforeRelease, PreloadBranch::AfterRelease];

fn capacity(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap()
}

fn limits() -> PendingHistoryLimits {
    PendingHistoryLimits {
        attempts: capacity(16),
        cold_changes: capacity(16),
        controls: capacity(64),
    }
}

/// The retained rig of the existing paired evaluator tests, over the mixed show. Every family
/// is programmed as a pending Preload FixAT.
struct PreloadRig {
    rig: Rig,
    key: PendingEpisodeKey,
    publication: DynamicSnapshotPublication,
    live: RefCell<DynamicRuntime>,
    started: Instant,
    selected: Cell<u64>,
}

impl PreloadRig {
    fn new() -> Self {
        let rig = Rig::new();
        let programmer = rig.programmers.get(rig.session).unwrap().id;
        let publication = DynamicSnapshotPublication::new(rig.engine.snapshot());
        let mut live =
            DynamicRuntime::with_programming_contract_support(PROGRAMMING_CONTRACT_VERSION);
        publication
            .begin_retained_history(&mut live, &rig.engine.snapshot(), capacity(64))
            .unwrap();
        rig.programmers.arm_preload(rig.session, true);
        rig.clock.advance_millis(10);
        rig.program_all();
        Self {
            rig,
            key: PendingEpisodeKey {
                activation: uuid::Uuid::new_v4(),
                programmer,
                branch: PreloadBranch::BeforeRelease,
            },
            publication,
            live: RefCell::new(live),
            started: Instant::now(),
            selected: Cell::new(0),
        }
    }

    fn pair(&self) -> Pair {
        let mut live = self.live.borrow_mut();
        let (cold, controls) = self
            .publication
            .begin_retained_history(&mut live, &self.rig.engine.snapshot(), capacity(64))
            .unwrap();
        let seed = |branch| PendingHistorySeed {
            key: PendingEpisodeKey { branch, ..self.key },
            runtime: live.fork_for_pending_preview(),
            origins: Default::default(),
            snapshot: self.rig.engine.snapshot(),
            position: PendingHistoryPosition {
                inputs: self.publication.input_capture_cursor().unwrap(),
                cold,
                controls,
            },
            live_sample: live.committed_sample_boundary(),
        };
        PairedPendingHistory::new(seed(BRANCHES[0]), seed(BRANCHES[1])).unwrap()
    }

    fn capture(&self) -> Arc<RetainedInputCapture> {
        self.rig.clock.advance_millis(40);
        let cursor = self.publication.input_capture_cursor().unwrap();
        let selected = self.selected.get();
        self.selected.set(selected + 1);
        let frame = RetainedFrameCapture::select(
            self.rig.engine.prepare_output_frame(Default::default()),
            &self.publication,
            self.started + Duration::from_millis(selected * 40),
        );
        self.publication.retain_accepted_input(
            &self.live.borrow(),
            frame.retained().unwrap(),
            &[],
            &transports(),
            37,
            None,
        );
        self.publication
            .input_captures_since(cursor)
            .unwrap()
            .remove(0)
    }

    fn consume(
        &self,
        pair: &mut Pair,
        evaluator: &mut impl PendingPairEvaluator<PendingHybridResult<FamilySidecar>>,
    ) -> PendingPairWindowOutcome {
        let input = self.capture();
        let (before, after) = pair.positions();
        let live = self.live.borrow();
        let before_controls = live.controls_since(before.controls).unwrap().unwrap();
        let after_controls = live.controls_since(after.controls).unwrap().unwrap();
        let window = pair
            .prepare_window(
                &[input],
                &[],
                &before_controls,
                &[],
                &after_controls,
                limits(),
            )
            .unwrap();
        drop(live);
        pair.consume_window(window, evaluator)
    }
}

fn succeeded(outcome: PendingPairWindowOutcome) {
    assert_eq!(
        outcome.successful_attempts, 1,
        "{:?}",
        outcome.failed_attempts
    );
}

#[test]
fn preload_branches_produce_every_family_in_isolation_with_after_only_release_and_pair_rollback() {
    let rig = PreloadRig::new();
    let show = rig.rig.show;
    let mut pair = rig.pair();
    let lanes = FamilyPreloadLanes::default();
    let mut evaluator = RetainedPreloadHybridEvaluator::new_with_observer(
        &rig.rig.engine,
        rig.key.programmer,
        &lanes,
        FamilyPreloadObserver::new(&lanes),
    );
    succeeded(rig.consume(&mut pair, &mut evaluator));
    let first = &pair.last_success().unwrap().value;
    assert_ne!(first.before.frame_token, first.after.frame_token);
    assert!(
        first
            .before
            .frame_token
            .same_capture(&first.after.frame_token)
    );
    for (branch, value) in [(BRANCHES[0], &first.before), (BRANCHES[1], &first.after)] {
        let branch_lanes = lanes.lanes(branch);
        for (target, owner) in show.owners() {
            let row = row(&value.sidecars, target, owner)
                .unwrap_or_else(|| panic!("{branch:?}: {owner:?} sidecar"));
            assert_eq!(row.token(), &value.frame_token, "{branch:?}: its own frame");
            assert_eq!(row.token().lane().preload_branch(), Some(branch));
            assert!(!row.writes().is_empty(), "{branch:?} {owner:?}");
        }
        assert_eq!(branch_lanes.last_accepted(), every(&value.frame_token));
        assert!(
            !lanes.accept_frame(&value.frame_token),
            "accepted exactly once"
        );
        let committed = continuity(branch_lanes, &show);
        assert!(committed.iter().all(|row| row != "None"), "{branch:?}");
    }
    for (target, owner) in show.owners() {
        assert_eq!(
            row(&first.before.sidecars, target, owner).unwrap().writes(),
            row(&first.after.sidecars, target, owner).unwrap().writes(),
            "{owner:?}: identical inputs fit identically in both branches",
        );
    }
    // The After Position writes reach the retained physical projection.
    let position = row(
        &first.after.sidecars,
        show.mover,
        ProgrammingOwner::Position,
    )
    .and_then(FamilySidecar::position)
    .unwrap();
    let instance = first
        .rendered
        .projection
        .physical
        .instances
        .iter()
        .find(|instance| instance.instance_id == show.mover.0)
        .expect("the mover's physical instance");
    for write in &position.writes {
        assert_eq!(
            instance.native_raw[write.slot.channel_index as usize],
            write.raw
        );
    }
    let first_tokens = [
        first.before.frame_token.clone(),
        first.after.frame_token.clone(),
    ];
    // A Live token addresses neither branch.
    let live = rig.rig.engine.prepare_output_frame(Default::default());
    assert!(lanes.begin_frame(&live.frame_token()).is_err());

    // A pending Release of the Media layer's Color changes After only; a pending Zoom Release
    // removes Zoom in both branches. Each branch releases with its own last token.
    rig.rig.release(show.layer, ProgrammingOwner::Color);
    rig.rig.release(show.optics, ProgrammingOwner::Zoom);
    // TL-560: a static typed Zoom is a Zoom adapter owner too (as Color since TL-554); un-stage
    // the pending static underlay as well so Zoom is genuinely removed in both branches.
    assert!(rig.rig.programmers.apply_preload_values(
        rig.rig.session,
        &[
            light_programmer::PreloadProgrammerValueMutation::ReleaseFixture {
                fixture_id: show.optics,
                attribute: ProgrammingOwner::Zoom.key(),
            }
        ],
    ));
    succeeded(rig.consume(&mut pair, &mut evaluator));
    let second = &pair.last_success().unwrap().value;
    let (before, after) = (lanes.lanes(BRANCHES[0]), lanes.lanes(BRANCHES[1]));
    let (layer, zoom) = (
        (show.layer, ProgrammingOwner::Color),
        (show.optics, ProgrammingOwner::Zoom),
    );
    assert!(row(&second.before.sidecars, layer.0, layer.1).is_some());
    assert!(row(&second.after.sidecars, layer.0, layer.1).is_none());
    for value in [&second.before, &second.after] {
        assert!(row(&value.sidecars, zoom.0, zoom.1).is_none());
        assert!(row(&value.sidecars, show.optics, ProgrammingOwner::Focus).is_some());
    }
    assert_eq!(released(before), [zoom], "Before keeps the Media layer");
    assert_eq!(released(after), [layer, zoom], "After releases per family");
    for (lanes, first) in [before, after].into_iter().zip(&first_tokens) {
        let tokens: Vec<_> = lanes
            .released()
            .into_iter()
            .map(|row| row.last_token)
            .collect();
        assert!(tokens.iter().all(|token| token == first), "own lineage");
    }
    let (before_rows, after_rows) = (continuity(before, &show), continuity(after, &show));
    assert_eq!(
        (before_rows[2] != "None", before_rows[4] == "None"),
        (true, true)
    );
    assert_eq!(
        (after_rows[2] == "None", after_rows[4] == "None"),
        (true, true)
    );

    rejected_pair_advances_no_lane_and_the_retry_commits(&rig, &mut pair, &mut evaluator, &lanes);
}

type Evaluator<'a> =
    RetainedPreloadHybridEvaluator<'a, &'a FamilyPreloadLanes, FamilyPreloadObserver<'a>>;

/// A rejected pair (paired engine finalizer token mismatch) advances no lane of either branch;
/// the retry commits the new Position and Color requests in both.
fn rejected_pair_advances_no_lane_and_the_retry_commits(
    rig: &PreloadRig,
    pair: &mut Pair,
    evaluator: &mut Evaluator<'_>,
    lanes: &FamilyPreloadLanes,
) {
    let show = rig.rig.show;
    let accepted = BRANCHES.map(|branch| lanes.lanes(branch).last_accepted());
    let committed = BRANCHES.map(|branch| continuity(lanes.lanes(branch), &show));
    let position = ProgrammingOwner::Position;
    rig.rig
        .fix(show.mover, position, angles(10., 20.), angles(-200., 60.));
    rig.rig.fix(
        show.wash,
        ProgrammingOwner::Color,
        program(&magenta()),
        cyan(),
    );
    evaluator.swap_finalization_tokens = true;
    let outcome = rig.consume(pair, evaluator);
    assert_eq!(outcome.failed_attempts.len(), 1);
    for (index, branch) in BRANCHES.into_iter().enumerate() {
        let branch_lanes = lanes.lanes(branch);
        assert_eq!(branch_lanes.last_accepted(), accepted[index], "{branch:?}");
        assert_eq!(
            continuity(branch_lanes, &show),
            committed[index],
            "{branch:?}: rolled back"
        );
    }
    evaluator.swap_finalization_tokens = false;
    succeeded(rig.consume(pair, evaluator));
    let third = &pair.last_success().unwrap().value;
    for (index, (branch, value)) in [(BRANCHES[0], &third.before), (BRANCHES[1], &third.after)]
        .into_iter()
        .enumerate()
    {
        let branch_lanes = lanes.lanes(branch);
        assert_eq!(branch_lanes.last_accepted(), every(&value.frame_token));
        let advanced = continuity(branch_lanes, &show);
        for owner in [0, 1] {
            assert_ne!(
                advanced[owner], committed[index][owner],
                "{branch:?} {owner}"
            );
        }
    }
}
