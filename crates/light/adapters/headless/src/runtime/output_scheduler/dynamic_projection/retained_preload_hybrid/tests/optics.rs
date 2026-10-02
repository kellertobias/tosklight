//! TL-558 retained Preload: independent Before/After Focus/Zoom lane pairs plugged into the
//! existing paired evaluator. Pending FixAT Focus and Zoom are separate owners in each branch.
use super::*;
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::optics::profiles::{
    multi_function_zoom, patched, wash_a,
};
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::optics::tests::{
    field, focus,
};
use crate::runtime::output_scheduler::dynamic_projection::physical_adapter::*;
use crate::runtime::preload::retained_history::paired::PendingPairWindowOutcome;
use light_fixture::{OpticsFamily, OpticsFitStatus};

type Sidecar = PhysicalHeadResult<OpticsAdapter>;
type Pair = PairedPendingHistory<PendingHybridResult<Sidecar>>;

fn install_wash(rig: &Rig) {
    install_profile(rig, &wash_a());
}

fn install_profile(rig: &Rig, profile: &light_fixture::FixtureProfile) {
    let snapshot = rig.engine.snapshot();
    rig.engine
        .replace_snapshot(light_engine::EngineSnapshot {
            fixtures: vec![patched(profile, rig.target, 1)].into(),
            dynamics: snapshot.dynamics.clone(),
            revision: snapshot.revision + 1,
            ..Default::default()
        })
        .unwrap();
    // The fixture list is a new installed snapshot for the retained Dynamics publication.
    rig.publication.installed(rig.engine.snapshot());
}

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

/// Pending FixAT for one owner over an authored static underlay of the same owner.
fn fix_at(rig: &Rig, owner: ProgrammingOwner, underlay: AttributeValue, value: AttributeValue) {
    rig.programmers
        .set(rig.session, rig.target, owner.key(), underlay);
    assert!(rig.programmers.apply_dynamic_values(
        rig.session,
        &[light_programmer::DynamicProgrammerValueMutation::Set {
            fixture_id: rig.target,
            attribute: owner.key(),
            value: DynamicSemanticValue::ProgrammingFixAt {
                mask: ProgrammingFamilyFixAt::from_family(owner, None, value).unwrap(),
                timing: Default::default(),
            },
        }],
        None
    ));
}

fn owned(branch: &PendingHybridBranch<Sidecar>, owner: ProgrammingOwner) -> Option<&Sidecar> {
    branch.sidecars.iter().find(|row| row.owner == owner)
}

#[test]
fn preload_focus_and_zoom_resolve_per_branch_and_release_only_the_released_owner() {
    let rig = Rig::new();
    install_wash(&rig);
    fix_at(&rig, ProgrammingOwner::Focus, focus(0.2), focus(0.6));
    fix_at(&rig, ProgrammingOwner::Zoom, field(30.), field(20.));
    let mut pair = pair(&rig);
    let live_lanes = OpticsLanes::live();
    let lanes = OpticsPreloadLanes::default();
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
    let mut zoom_writes = Vec::new();
    for (branch, result) in [
        (PreloadBranch::BeforeRelease, &first.before),
        (PreloadBranch::AfterRelease, &first.after),
    ] {
        let f = owned(result, ProgrammingOwner::Focus).expect("pending Focus");
        let z = owned(result, ProgrammingOwner::Zoom).expect("pending Zoom");
        for sidecar in [f, z] {
            assert_eq!(sidecar.token, result.frame_token, "{branch:?}");
            assert_eq!(sidecar.token.lane().preload_branch(), Some(branch));
            assert_eq!(sidecar.quality.status, OpticsFitStatus::Fitted);
            assert_eq!(sidecar.writes.len(), 1);
        }
        assert_eq!((f.value.clone(), z.value.clone()), (focus(0.6), field(20.)));
        assert_eq!(
            (
                f.writes[0].slot.channel_index,
                z.writes[0].slot.channel_index
            ),
            (2, 1)
        );
        assert_eq!(z.writes[0].raw, 32768);
        zoom_writes.push(z.writes.clone());
        let pair_lanes = lanes.lanes(branch);
        for family in [OpticsFamily::Focus, OpticsFamily::Zoom] {
            assert_eq!(
                pair_lanes.lane(family).last_accepted(),
                Some(result.frame_token.clone()),
                "{branch:?} {family:?}"
            );
        }
    }
    assert_eq!(zoom_writes[0], zoom_writes[1]);
    let before_focus = owned(&first.before, ProgrammingOwner::Focus)
        .unwrap()
        .writes
        .clone();

    // Releasing only Zoom: After loses exactly Zoom; Focus and the Before branch are unchanged.
    assert!(rig.programmers.apply_dynamic_values(
        rig.session,
        &[light_programmer::DynamicProgrammerValueMutation::Set {
            fixture_id: rig.target,
            attribute: ProgrammingOwner::Zoom.key(),
            value: DynamicSemanticValue::Release,
        }],
        None
    ));
    assert_eq!(
        consume(&rig, &mut pair, &mut evaluator).successful_attempts,
        1
    );
    let second = &pair.last_success().unwrap().value;
    assert!(owned(&second.after, ProgrammingOwner::Zoom).is_none());
    assert_eq!(
        owned(&second.after, ProgrammingOwner::Focus).unwrap().value,
        focus(0.6)
    );
    assert_eq!(
        owned(&second.before, ProgrammingOwner::Focus)
            .unwrap()
            .writes,
        before_focus,
        "Before keeps Focus unchanged"
    );
    // Before-branch retention of a released pending owner is Color-specific in the programmer
    // (`retain_released_fixture_color_for`); Zoom follows the ordinary pending Release.
    for branch in [PreloadBranch::BeforeRelease, PreloadBranch::AfterRelease] {
        let [released] =
            lanes
                .lanes(branch)
                .released()
                .try_into()
                .unwrap_or_else(|rows: Vec<_>| {
                    panic!("{branch:?}: exactly one released owner, got {}", rows.len())
                });
        assert_eq!(
            (released.target, released.owner),
            (rig.target, ProgrammingOwner::Zoom),
            "{branch:?}: only Zoom is released"
        );
    }
    // Pending evaluation never touched an independent Live pair.
    assert_eq!(live_lanes.counters().descriptor_compiles, 0);
    assert!(
        live_lanes
            .lane(OpticsFamily::Zoom)
            .last_accepted()
            .is_none()
    );
}

/// The authored native underlay of the pending Zoom hold, edited in place.
fn edit_command(rig: &Rig, raw: u32) {
    rig.programmers.set(
        rig.session,
        rig.target,
        ProgrammingOwner::Zoom.key(),
        AttributeValue::RawDmxExact(raw),
    );
}

/// TL-601: each branch's accepted Zoom continuity follows a fresh authored native edit, and a
/// pair that fails in the finalizer commits neither branch's proposed continuity.
#[test]
fn preload_zoom_continuity_follows_fresh_native_edits_and_failed_pairs_commit_nothing() {
    let rig = Rig::new();
    let profile = multi_function_zoom();
    let ids: Vec<_> = profile.modes[0].channels[1]
        .functions
        .iter()
        .map(|f| f.id)
        .collect();
    install_profile(&rig, &profile);
    fix_at(&rig, ProgrammingOwner::Focus, focus(0.2), focus(0.6));
    // Authored native command in the wide function under a pending 30° Field.
    fix_at(
        &rig,
        ProgrammingOwner::Zoom,
        AttributeValue::RawDmxExact(200),
        field(30.),
    );
    let mut pair = pair(&rig);
    let lanes = OpticsPreloadLanes::default();
    let mut evaluator = RetainedPreloadHybridEvaluator::new(
        &rig.engine,
        rig.key.programmer,
        &lanes,
        |branch, observation: HybridFamilyObservation<'_>| lanes.observe(branch, observation),
    );
    let branches = [PreloadBranch::BeforeRelease, PreloadBranch::AfterRelease];
    let zoom_of = |pair: &Pair| {
        let result = &pair.last_success().unwrap().value;
        [&result.before, &result.after].map(|branch| {
            let z = owned(branch, ProgrammingOwner::Zoom).expect("pending Zoom");
            assert_eq!(z.token, branch.frame_token);
            assert_eq!(z.requested.value, 30., "the request is unchanged");
            (z.writes[0].raw, z.quality.function_id, z.quality.status)
        })
    };
    let committed = |lanes: &OpticsPreloadLanes| {
        branches.map(|branch| {
            let pair_lanes = lanes.lanes(branch);
            (
                pair_lanes
                    .lane(OpticsFamily::Zoom)
                    .continuity(rig.target, ProgrammingOwner::Zoom),
                pair_lanes
                    .lane(OpticsFamily::Focus)
                    .continuity(rig.target, ProgrammingOwner::Focus),
                pair_lanes.lane(OpticsFamily::Zoom).last_accepted(),
            )
        })
    };
    // Each branch's continuity is exactly its own accepted sidecar write.
    let own_continuity = |pair: &Pair, lanes: &OpticsPreloadLanes| {
        let result = &pair.last_success().unwrap().value;
        for (branch, sidecars) in branches.into_iter().zip([&result.before, &result.after]) {
            let z = owned(sidecars, ProgrammingOwner::Zoom).unwrap();
            let continuity = lanes
                .lanes(branch)
                .lane(OpticsFamily::Zoom)
                .continuity(rig.target, ProgrammingOwner::Zoom)
                .unwrap();
            assert_eq!(continuity.control.unwrap().2, z.writes[0].raw, "{branch:?}");
            assert_eq!(
                lanes.lanes(branch).lane(OpticsFamily::Zoom).last_accepted(),
                Some(sidecars.frame_token.clone()),
                "{branch:?} commits only its own token"
            );
        }
    };

    assert_eq!(
        consume(&rig, &mut pair, &mut evaluator).successful_attempts,
        1
    );
    let wide = (160, Some(ids[2]), OpticsFitStatus::Fitted);
    assert_eq!(zoom_of(&pair), [wide; 2]);
    own_continuity(&pair, &lanes);

    // A fresh authored edit into the reversed narrow function: both branches follow it.
    edit_command(&rig, 50);
    assert_eq!(
        consume(&rig, &mut pair, &mut evaluator).successful_attempts,
        1
    );
    let narrow = (33, Some(ids[0]), OpticsFitStatus::Fitted);
    assert_eq!(
        zoom_of(&pair),
        [narrow; 2],
        "the accepted wide raw is not replayed in either branch"
    );
    own_continuity(&pair, &lanes);
    let accepted = committed(&lanes);

    // Edit back, but the finalizer rejects the pair: no branch installs its proposal.
    edit_command(&rig, 200);
    evaluator.swap_finalization_tokens = true;
    assert_eq!(
        consume(&rig, &mut pair, &mut evaluator)
            .failed_attempts
            .len(),
        1
    );
    assert_eq!(committed(&lanes), accepted, "failed pair commits nothing");
    evaluator.swap_finalization_tokens = false;
    assert_eq!(
        consume(&rig, &mut pair, &mut evaluator).successful_attempts,
        1
    );
    assert_eq!(
        zoom_of(&pair),
        [wide; 2],
        "the retry follows the edited command"
    );
    own_continuity(&pair, &lanes);
}

// TL-602: passive Zoom requirements of a successful retained pair hold each branch's own
// accepted continuity in its own lane pair (the TL-598 contract), never a Release.

/// A pending Dynamic whose Zoom lane adopts the authored native command (`raw`) through
/// Current in Field, plus a pending Focus FixAT that always produces. Returns the Zoom instance.
fn install_current_zoom(rig: &Rig, profile: &light_fixture::FixtureProfile, raw: u32) -> Uuid {
    let mut definition = pan_definition();
    definition.id = Uuid::new_v4();
    definition.pool_number = 2;
    definition.name = "Zoom Current".into();
    definition.lanes.truncate(1);
    let lane = &mut definition.lanes[0];
    lane.id = Uuid::new_v4();
    let DynamicLaneBody::Programming(body) = &mut lane.body else {
        unreachable!()
    };
    body.address = DynamicValueAddress {
        representation: DynamicFamilyRepresentation::Zoom {
            convention: light_core::OpeningConvention::Field,
        },
        component: Some(ProgrammingComponent::Zoom),
    };
    let snapshot = rig.engine.snapshot();
    let dynamics: Vec<_> = snapshot
        .dynamics
        .iter()
        .cloned()
        .chain([definition.clone()])
        .collect();
    rig.engine
        .replace_snapshot(light_engine::EngineSnapshot {
            fixtures: vec![patched(profile, rig.target, 1)].into(),
            dynamics: Arc::new(dynamics.clone()),
            revision: snapshot.revision + 1,
            ..Default::default()
        })
        .unwrap();
    rig.live.borrow_mut().install_definitions(dynamics).unwrap();
    rig.publication.installed(rig.engine.snapshot());
    fix_at(rig, ProgrammingOwner::Focus, focus(0.2), focus(0.6));
    edit_command(rig, raw);
    let link = Uuid::new_v4();
    assert!(rig.programmers.apply_dynamic_values(
        rig.session,
        &[light_programmer::DynamicProgrammerValueMutation::Set {
            fixture_id: rig.target,
            attribute: ProgrammingOwner::Zoom.key(),
            value: DynamicSemanticValue::DynamicOn {
                instance_link: link,
                lane_id: definition.lanes[0].id,
                dynamic: DynamicReference {
                    dynamic_id: Some(definition.id),
                    last_known_pool_number: definition.pool_number,
                    embedded_fallback: DynamicDefinitionSnapshot {
                        definition: Arc::new(definition),
                    },
                },
                overrides: DynamicInstanceOverrides {
                    size: 1.,
                    speed_multiplier: Rational::ONE,
                    phase_offset_degrees: 0.,
                },
                timing: Default::default(),
            },
        }],
        None
    ));
    link
}

const BRANCHES: [PreloadBranch; 2] = [PreloadBranch::BeforeRelease, PreloadBranch::AfterRelease];

fn branches(pair: &Pair) -> [&PendingHybridBranch<Sidecar>; 2] {
    let result = &pair.last_success().unwrap().value;
    [&result.before, &result.after]
}

type BranchState = (
    Option<OpticsContinuity>,
    Option<OpticsContinuity>,
    Option<CapturedFrameToken>,
    Option<CapturedFrameToken>,
);

/// Zoom continuity, Focus continuity and both lanes' accepted tokens of each branch.
fn committed(rig: &Rig, lanes: &OpticsPreloadLanes) -> [BranchState; 2] {
    BRANCHES.map(|branch| {
        let pair = lanes.lanes(branch);
        (
            pair.lane(OpticsFamily::Zoom)
                .continuity(rig.target, ProgrammingOwner::Zoom),
            pair.lane(OpticsFamily::Focus)
                .continuity(rig.target, ProgrammingOwner::Focus),
            pair.lane(OpticsFamily::Zoom).last_accepted(),
            pair.lane(OpticsFamily::Focus).last_accepted(),
        )
    })
}

/// Every branch holds exactly `accepted` (its own) for Zoom, without a sidecar or Release,
/// while its Focus commits its own new solve under the branch token.
fn assert_held(
    rig: &Rig,
    pair: &Pair,
    lanes: &OpticsPreloadLanes,
    accepted: [OpticsContinuity; 2],
) {
    for ((branch, result), accepted) in BRANCHES.into_iter().zip(branches(pair)).zip(accepted) {
        assert!(
            owned(result, ProgrammingOwner::Zoom).is_none(),
            "{branch:?}: no stale Zoom sidecar"
        );
        assert!(
            result
                .requirements
                .iter()
                .any(|r| r.owner == ProgrammingOwner::Zoom && r.target == rig.target),
            "{branch:?}: a scoped passive Zoom requirement"
        );
        let focus = owned(result, ProgrammingOwner::Focus).expect("Focus continues");
        assert_eq!(focus.token, result.frame_token);
        let pair_lanes = lanes.lanes(branch);
        assert!(
            pair_lanes.released().is_empty(),
            "{branch:?}: a passive requirement is not a Release"
        );
        let zoom = pair_lanes.lane(OpticsFamily::Zoom);
        assert_eq!(
            zoom.continuity(rig.target, ProgrammingOwner::Zoom),
            Some(accepted),
            "{branch:?}: its own accepted continuity is held"
        );
        assert_eq!(zoom.last_accepted(), Some(result.frame_token.clone()));
        assert_eq!(
            pair_lanes
                .lane(OpticsFamily::Focus)
                .continuity(rig.target, ProgrammingOwner::Focus)
                .unwrap()
                .control
                .unwrap()
                .2,
            focus.writes[0].raw
        );
    }
}

#[test]
fn preload_passive_zoom_requirements_hold_per_branch_and_failed_pairs_retry() {
    let rig = Rig::new();
    let profile = multi_function_zoom();
    let wide = profile.modes[0].channels[1].functions[2].id;
    install_current_zoom(&rig, &profile, 200);
    let mut pair = pair(&rig);
    let lanes = OpticsPreloadLanes::default();
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
    for result in branches(&pair) {
        let z = owned(result, ProgrammingOwner::Zoom).expect("adopted Zoom");
        assert_eq!((z.writes[0].raw, z.quality.function_id), (200, Some(wide)));
    }
    let accepted = BRANCHES.map(|branch| {
        lanes
            .lanes(branch)
            .lane(OpticsFamily::Zoom)
            .continuity(rig.target, ProgrammingOwner::Zoom)
            .unwrap()
    });
    let solves = branches(&pair).map(|result| result.frame_token.clone());
    assert_ne!(solves[0], solves[1], "each branch has its own token");

    // Into the macro: both branches hold their own continuity.
    edit_command(&rig, 110);
    assert_eq!(
        consume(&rig, &mut pair, &mut evaluator).successful_attempts,
        1
    );
    assert_held(&rig, &pair, &lanes, accepted);
    // A Live token addresses neither branch.
    let live_token = rig
        .engine
        .prepare_output_frame(Default::default())
        .frame_token();
    assert!(
        HybridFrameResolver::hold_frame(&lanes, &live_token, &branches(&pair)[0].requirements)
            .is_err()
    );

    // Both branches hold again, but the paired finalizer rejects the pair: nothing commits.
    let before = committed(&rig, &lanes);
    evaluator.swap_finalization_tokens = true;
    assert_eq!(
        consume(&rig, &mut pair, &mut evaluator)
            .failed_attempts
            .len(),
        1
    );
    assert_eq!(
        committed(&rig, &lanes),
        before,
        "a failed pair commits nothing"
    );
    evaluator.swap_finalization_tokens = false;
    assert_eq!(
        consume(&rig, &mut pair, &mut evaluator).successful_attempts,
        1
    );
    assert_held(&rig, &pair, &lanes, accepted);

    // Recovery to the authored command: both TL-601 witnesses hold, the held raw is reused.
    let stale = BRANCHES.map(|branch| lanes.lanes(branch).counters().stale_continuity);
    edit_command(&rig, 200);
    assert_eq!(
        consume(&rig, &mut pair, &mut evaluator).successful_attempts,
        1
    );
    for (branch, result) in BRANCHES.into_iter().zip(branches(&pair)) {
        let z = owned(result, ProgrammingOwner::Zoom).expect("recovered Zoom");
        assert_eq!((z.writes[0].raw, z.quality.function_id), (200, Some(wide)));
        assert!(lanes.lanes(branch).released().is_empty());
    }
    assert_eq!(
        BRANCHES.map(|branch| lanes.lanes(branch).counters().stale_continuity),
        stale
    );
}

#[test]
fn preload_zoom_hold_recovery_respects_the_baseline_guard_and_removal_releases() {
    let rig = Rig::new();
    let profile = multi_function_zoom();
    let narrow = profile.modes[0].channels[1].functions[0].id;
    let link = install_current_zoom(&rig, &profile, 200);
    let mut pair = pair(&rig);
    let lanes = OpticsPreloadLanes::default();
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
    let accepted = BRANCHES.map(|branch| {
        lanes
            .lanes(branch)
            .lane(OpticsFamily::Zoom)
            .continuity(rig.target, ProgrammingOwner::Zoom)
            .unwrap()
    });
    edit_command(&rig, 110);
    assert_eq!(
        consume(&rig, &mut pair, &mut evaluator).successful_attempts,
        1
    );
    assert_held(&rig, &pair, &lanes, accepted);

    // A fresh edit after the hold: each branch drops its held raw 200 and writes the edit.
    let stale = BRANCHES.map(|branch| lanes.lanes(branch).counters().stale_continuity);
    edit_command(&rig, 50);
    assert_eq!(
        consume(&rig, &mut pair, &mut evaluator).successful_attempts,
        1
    );
    for (branch, result) in BRANCHES.into_iter().zip(branches(&pair)) {
        let z = owned(result, ProgrammingOwner::Zoom).expect("edited Zoom");
        assert_eq!((z.writes[0].raw, z.quality.function_id), (50, Some(narrow)));
        let continuity = lanes
            .lanes(branch)
            .lane(OpticsFamily::Zoom)
            .continuity(rig.target, ProgrammingOwner::Zoom)
            .unwrap();
        assert_eq!(
            (continuity.control.unwrap().2, continuity.baseline),
            (50, Some(50))
        );
    }
    let after = BRANCHES.map(|branch| lanes.lanes(branch).counters().stale_continuity);
    assert!(
        after
            .iter()
            .zip(stale)
            .all(|(after, before)| *after > before),
        "each branch met the changed baseline witness of its held continuity: {stale:?} {after:?}"
    );
    let solves = branches(&pair).map(|result| result.frame_token.clone());

    // Held again, then the Zoom Dynamic is genuinely removed: each branch releases Zoom with
    // its own last produced solve and retires continuity; Focus is untouched.
    edit_command(&rig, 110);
    assert_eq!(
        consume(&rig, &mut pair, &mut evaluator).successful_attempts,
        1
    );
    let held = BRANCHES.map(|branch| {
        lanes
            .lanes(branch)
            .lane(OpticsFamily::Zoom)
            .continuity(rig.target, ProgrammingOwner::Zoom)
            .unwrap()
    });
    assert_held(&rig, &pair, &lanes, held);
    assert!(rig.programmers.apply_dynamic_values(
        rig.session,
        &[light_programmer::DynamicProgrammerValueMutation::Release {
            fixture_id: rig.target,
            attribute: ProgrammingOwner::Zoom.key(),
            instance_link: Some(link),
        }],
        None
    ));
    assert_eq!(
        consume(&rig, &mut pair, &mut evaluator).successful_attempts,
        1
    );
    for ((branch, result), solve) in BRANCHES.into_iter().zip(branches(&pair)).zip(solves) {
        assert!(owned(result, ProgrammingOwner::Zoom).is_none());
        assert!(owned(result, ProgrammingOwner::Focus).is_some());
        let pair_lanes = lanes.lanes(branch);
        let [released] = pair_lanes
            .released()
            .try_into()
            .unwrap_or_else(|rows: Vec<_>| panic!("{branch:?}: {} released", rows.len()));
        assert_eq!(
            (released.target, released.owner),
            (rig.target, ProgrammingOwner::Zoom)
        );
        assert_eq!(
            released.last_token, solve,
            "{branch:?}: the Release names its own original solve token"
        );
        assert!(
            pair_lanes
                .lane(OpticsFamily::Zoom)
                .continuity(rig.target, ProgrammingOwner::Zoom)
                .is_none()
        );
    }
}

#[test]
fn preload_passive_convention_zoom_requirements_hold_per_branch_and_recover() {
    let rig = Rig::new();
    let profile = multi_function_zoom();
    install_current_zoom(&rig, &profile, 200);
    let mut pair = pair(&rig);
    let lanes = OpticsPreloadLanes::default();
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
    let solved = branches(&pair).map(|result| {
        owned(result, ProgrammingOwner::Zoom)
            .expect("adopted Zoom")
            .writes
            .clone()
    });
    let accepted = BRANCHES.map(|branch| {
        lanes
            .lanes(branch)
            .lane(OpticsFamily::Zoom)
            .continuity(rig.target, ProgrammingOwner::Zoom)
            .unwrap()
    });
    // A Beam opening under the Field Current lane: no conversion, a passive requirement.
    rig.programmers.set(
        rig.session,
        rig.target,
        ProgrammingOwner::Zoom.key(),
        crate::runtime::output_scheduler::dynamic_projection::physical_adapter::optics::tests::zoom(
            30.,
            light_core::OpeningConvention::Beam,
        ),
    );
    assert_eq!(
        consume(&rig, &mut pair, &mut evaluator).successful_attempts,
        1
    );
    assert_held(&rig, &pair, &lanes, accepted);
    edit_command(&rig, 200);
    assert_eq!(
        consume(&rig, &mut pair, &mut evaluator).successful_attempts,
        1
    );
    for ((branch, result), solved) in BRANCHES.into_iter().zip(branches(&pair)).zip(solved) {
        let z = owned(result, ProgrammingOwner::Zoom).expect("recovered Zoom");
        assert_eq!(z.writes, solved, "{branch:?}");
        assert!(lanes.lanes(branch).released().is_empty());
    }
}
