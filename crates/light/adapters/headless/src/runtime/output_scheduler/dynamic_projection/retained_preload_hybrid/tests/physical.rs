//! TL-590 seam tests for retained Preload: independent Before/After physical adapter lanes
//! plugged into the existing paired evaluator and Preload finalizer.
use super::*;
use crate::runtime::dynamic_source_origins::{
    DynamicFixedSource, DynamicProgrammerSourceLane, DynamicSourceBinding,
};
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::test_adapter::*;
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::*;
use crate::runtime::preload::retained_history::paired::PendingPairWindowOutcome;

type Sidecar = PhysicalHeadResult<SeamAdapter>;
type Pair = PairedPendingHistory<PendingHybridResult<Sidecar>>;

fn pair(rig: &Rig) -> Pair {
    let mut live = rig.live.borrow_mut();
    let (cold, controls) = rig
        .publication
        .begin_retained_history(&mut live, &rig.engine.snapshot(), capacity(64))
        .unwrap();
    let seed = |branch| PendingHistorySeed {
        key: PendingEpisodeKey { branch, ..rig.key },
        runtime: live.fork_for_pending_preview(),
        origins: Default::default(),
        snapshot: rig.engine.snapshot(),
        position: PendingHistoryPosition {
            inputs: rig.publication.input_capture_cursor().unwrap(),
            cold,
            controls,
        },
        live_sample: live.committed_sample_boundary(),
    };
    PairedPendingHistory::new(
        seed(PreloadBranch::BeforeRelease),
        seed(PreloadBranch::AfterRelease),
    )
    .unwrap()
}

fn consume(
    rig: &Rig,
    pair: &mut Pair,
    evaluator: &mut impl PendingPairEvaluator<PendingHybridResult<Sidecar>>,
) -> PendingPairWindowOutcome {
    let input = rig.capture();
    let (before, after) = pair.positions();
    let live = rig.live.borrow();
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

fn lanes(owner: ProgrammingOwner) -> PhysicalPreloadLanes<SeamAdapter> {
    PhysicalPreloadLanes::new(SeamAdapter::new(&[owner]), SeamAdapter::new(&[owner]))
}

fn owned(branch: &PendingHybridBranch<Sidecar>, owner: ProgrammingOwner) -> Option<&Sidecar> {
    branch.sidecars.iter().find(|row| row.owner == owner)
}

#[test]
fn preload_lanes_are_independent_and_a_failed_pair_advances_neither_branch() {
    let rig = Rig::new();
    let mut pair = pair(&rig);
    let live_lane = PhysicalAdapterLane::live(SeamAdapter::new(&[ProgrammingOwner::Position]));
    let lanes = lanes(ProgrammingOwner::Position);
    let mut evaluator = RetainedPreloadHybridEvaluator::new(
        &rig.engine,
        rig.key.programmer,
        &lanes,
        |branch, observation: HybridFamilyObservation<'_>| lanes.observe(branch, observation),
    );
    assert_eq!(
        consume(&rig, &mut pair, &mut evaluator).successful_attempts,
        1
    );
    let first = &pair.last_success().unwrap().value;
    assert_ne!(first.before.frame_token, first.after.frame_token);
    assert!(
        first
            .before
            .frame_token
            .same_capture(&first.after.frame_token)
    );
    for (branch, result) in [
        (PreloadBranch::BeforeRelease, &first.before),
        (PreloadBranch::AfterRelease, &first.after),
    ] {
        let sidecar = owned(result, ProgrammingOwner::Position).unwrap();
        assert_eq!(sidecar.token, result.frame_token, "{branch:?}");
        assert_eq!(sidecar.token.lane().preload_branch(), Some(branch));
        assert_eq!(sidecar.value, position(60., 40.));
        assert_eq!(sidecar.achieved.primary, 60.);
        assert!(!sidecar.provenance.sources.entries().unwrap().is_empty());
        let lane = lanes.lane(branch);
        assert_eq!(lane.last_accepted(), Some(result.frame_token.clone()));
        assert_eq!(
            lane.adapter().compiles.get(),
            1,
            "each lane compiles its own"
        );
        assert_eq!(
            lane.continuity(rig.target, ProgrammingOwner::Position),
            Some(SeamContinuity {
                primary: 60.,
                frames: 1
            })
        );
    }
    let accepted = [PreloadBranch::BeforeRelease, PreloadBranch::AfterRelease]
        .map(|branch| lanes.lane(branch).last_accepted());

    // After fails; Before prepared successfully in the same pair but must not advance.
    rig.position(90., 10.);
    lanes
        .lane(PreloadBranch::AfterRelease)
        .adapter()
        .failure
        .set(Some(SeamFailure::Resolve));
    assert_eq!(
        consume(&rig, &mut pair, &mut evaluator)
            .failed_attempts
            .len(),
        1
    );
    for (index, branch) in [PreloadBranch::BeforeRelease, PreloadBranch::AfterRelease]
        .into_iter()
        .enumerate()
    {
        let lane = lanes.lane(branch);
        assert_eq!(lane.last_accepted(), accepted[index], "{branch:?}");
        assert_eq!(
            lane.continuity(rig.target, ProgrammingOwner::Position)
                .unwrap()
                .frames,
            1
        );
    }

    lanes
        .lane(PreloadBranch::AfterRelease)
        .adapter()
        .failure
        .set(None);
    assert_eq!(
        consume(&rig, &mut pair, &mut evaluator).successful_attempts,
        1
    );
    let result = &pair.last_success().unwrap().value;
    for branch in [&result.before, &result.after] {
        let sidecar = owned(branch, ProgrammingOwner::Position).unwrap();
        assert_eq!(sidecar.value, position(90., 10.));
        assert_eq!(sidecar.quality.previous.unwrap().frames, 1);
    }
    // Pending evaluation never touched an independent Live lane.
    assert_eq!(live_lane.last_accepted(), None);
    assert_eq!(live_lane.adapter().compiles.get(), 0);
}

#[test]
fn preload_color_release_is_source_aware_and_retained_evidence_is_owned() {
    let rig = Rig::new();
    let color = rig.fixed_color();
    let mut pair = pair(&rig);
    let lanes = lanes(ProgrammingOwner::Color);
    let mut evaluator = RetainedPreloadHybridEvaluator::new(
        &rig.engine,
        rig.key.programmer,
        &lanes,
        |branch, observation: HybridFamilyObservation<'_>| lanes.observe(branch, observation),
    );
    assert_eq!(
        consume(&rig, &mut pair, &mut evaluator).successful_attempts,
        1
    );
    let first = &pair.last_success().unwrap().value;
    let after_color = owned(&first.after, ProgrammingOwner::Color).expect("pending FixAT Color");
    assert_eq!(after_color.value, color);
    assert_eq!(after_color.requested.primary, 0.8);
    assert_eq!(after_color.writes[0].raw, 80);
    // Position is not owned by this adapter: a passive requirement, not a failure.
    assert!(
        first
            .after
            .requirements
            .iter()
            .any(|row| row.owner == ProgrammingOwner::Position)
    );
    let fixed_source = |sidecar: &PhysicalProvenance| {
        sidecar
            .sources
            .entries()
            .unwrap()
            .iter()
            .find(|entry| {
                matches!(
                    entry.record().binding,
                    DynamicSourceBinding::Fixed {
                        source: DynamicFixedSource::Programmer {
                            lane: DynamicProgrammerSourceLane::Preload,
                            ..
                        },
                        owner: ProgrammingOwner::Color,
                        ..
                    }
                )
            })
            .map(|entry| Arc::clone(entry.record()))
    };
    let original = fixed_source(&after_color.provenance).expect("Fixed Preload source");
    let retained = after_color.provenance.clone();
    let first_after_token = first.after.frame_token.clone();

    // A successful passive hold must retain the last produced source token/provenance.
    let before_frames = lanes
        .lane(PreloadBranch::BeforeRelease)
        .continuity(rig.target, ProgrammingOwner::Color)
        .unwrap()
        .frames;
    lanes
        .lane(PreloadBranch::AfterRelease)
        .adapter()
        .failure
        .set(Some(SeamFailure::PassiveResolve));
    assert_eq!(
        consume(&rig, &mut pair, &mut evaluator).successful_attempts,
        1
    );
    let held = &pair.last_success().unwrap().value;
    assert!(owned(&held.after, ProgrammingOwner::Color).is_none());
    assert!(!held.after.requirements.is_empty());
    assert!(
        lanes
            .lane(PreloadBranch::AfterRelease)
            .released()
            .is_empty()
    );
    assert_eq!(
        lanes
            .lane(PreloadBranch::AfterRelease)
            .continuity(rig.target, ProgrammingOwner::Color)
            .unwrap()
            .frames,
        1
    );
    lanes
        .lane(PreloadBranch::AfterRelease)
        .adapter()
        .failure
        .set(None);
    rig.release_color();
    assert_eq!(
        consume(&rig, &mut pair, &mut evaluator).successful_attempts,
        1
    );
    let second = &pair.last_success().unwrap().value;
    assert!(owned(&second.after, ProgrammingOwner::Color).is_none());
    let before_color = owned(&second.before, ProgrammingOwner::Color).expect("Before keeps it");
    // Same source binding, but Before's own catalogue occurrence: branches never share records.
    let before_source = fixed_source(&before_color.provenance).unwrap();
    assert_eq!(before_source.binding, original.binding);
    assert_ne!(before_source.occurrence_id, original.occurrence_id);
    // The After branch reports the Release with the source that owned the output.
    let after_lane = lanes.lane(PreloadBranch::AfterRelease);
    let [released] = after_lane
        .released()
        .try_into()
        .unwrap_or_else(|rows: Vec<_>| panic!("one released Color owner, got {}", rows.len()));
    assert_eq!(
        (released.target, released.owner),
        (rig.target, ProgrammingOwner::Color)
    );
    assert_eq!(released.last_token, first_after_token);
    assert_eq!(
        fixed_source(&released.last_provenance),
        Some(Arc::clone(&original))
    );
    assert_eq!(
        after_lane.continuity(rig.target, ProgrammingOwner::Color),
        None
    );
    // Before still owns Color; its lane releases nothing and keeps continuity.
    let before_lane = lanes.lane(PreloadBranch::BeforeRelease);
    assert!(before_lane.released().is_empty());
    assert_eq!(
        before_lane
            .continuity(rig.target, ProgrammingOwner::Color)
            .unwrap()
            .frames,
        before_frames + 2
    );
    // Evidence retained from the first frame is owned: the After catalogue has since unbound
    // the Fixed source, yet the old sidecar still names the identical record.
    assert!(second.after.origins.binding(&original.binding).is_none());
    assert_eq!(fixed_source(&retained), Some(original));
}

#[test]
fn preload_passive_holds_keep_independent_continuity_and_failed_pair_retries() {
    let rig = Rig::new();
    let mut pair = pair(&rig);
    let lanes = lanes(ProgrammingOwner::Position);
    let mut evaluator = RetainedPreloadHybridEvaluator::new(
        &rig.engine,
        rig.key.programmer,
        &lanes,
        |branch, observation: HybridFamilyObservation<'_>| lanes.observe(branch, observation),
    );
    assert_eq!(
        consume(&rig, &mut pair, &mut evaluator).successful_attempts,
        1
    );
    let branches = [PreloadBranch::BeforeRelease, PreloadBranch::AfterRelease];
    let initial = branches.map(|branch| {
        lanes
            .lane(branch)
            .continuity(rig.target, ProgrammingOwner::Position)
            .unwrap()
    });
    rig.position(90., 10.);
    // Before remains unresolved while After still produces its own result.
    lanes
        .lane(branches[0])
        .adapter()
        .failure
        .set(Some(SeamFailure::PassiveResolve));
    assert_eq!(
        consume(&rig, &mut pair, &mut evaluator).successful_attempts,
        1
    );
    let held = &pair.last_success().unwrap().value;
    assert!(owned(&held.before, ProgrammingOwner::Position).is_none());
    assert!(!held.before.requirements.is_empty());
    assert!(lanes.lane(branches[0]).released().is_empty());
    assert_eq!(
        lanes
            .lane(branches[0])
            .continuity(rig.target, ProgrammingOwner::Position),
        Some(initial[0])
    );
    assert_eq!(
        owned(&held.after, ProgrammingOwner::Position)
            .unwrap()
            .quality
            .previous,
        Some(initial[1])
    );
    let accepted = branches.map(|branch| lanes.lane(branch).last_accepted());
    let continuity = branches.map(|branch| {
        lanes
            .lane(branch)
            .continuity(rig.target, ProgrammingOwner::Position)
    });
    // Both preparations can hold; the paired engine token rejection must accept neither.
    lanes
        .lane(branches[1])
        .adapter()
        .failure
        .set(Some(SeamFailure::PassiveResolve));
    evaluator.swap_finalization_tokens = true;
    assert_eq!(
        consume(&rig, &mut pair, &mut evaluator)
            .failed_attempts
            .len(),
        1
    );
    assert_eq!(
        branches.map(|branch| lanes.lane(branch).last_accepted()),
        accepted
    );
    assert_eq!(
        branches.map(|branch| lanes
            .lane(branch)
            .continuity(rig.target, ProgrammingOwner::Position)),
        continuity
    );
    evaluator.swap_finalization_tokens = false;
    assert_eq!(
        consume(&rig, &mut pair, &mut evaluator).successful_attempts,
        1
    );
    let held = &pair.last_success().unwrap().value;
    assert!(owned(&held.before, ProgrammingOwner::Position).is_none());
    assert!(owned(&held.after, ProgrammingOwner::Position).is_none());
    assert_eq!(
        branches.map(|branch| lanes
            .lane(branch)
            .continuity(rig.target, ProgrammingOwner::Position)),
        continuity
    );
    for branch in branches {
        assert!(lanes.lane(branch).released().is_empty());
        lanes.lane(branch).adapter().failure.set(None);
    }
    assert_eq!(
        consume(&rig, &mut pair, &mut evaluator).successful_attempts,
        1
    );
    let result = &pair.last_success().unwrap().value;
    assert_eq!(
        owned(&result.before, ProgrammingOwner::Position)
            .unwrap()
            .quality
            .previous,
        continuity[0]
    );
    assert_eq!(
        owned(&result.after, ProgrammingOwner::Position)
            .unwrap()
            .quality
            .previous,
        continuity[1]
    );
}
