//! Immutable memo of one successful complete physical-copy fit. The caller must additionally
//! prove every cohort owner carries this same accepted Arc; numerical equality is not acceptance.
use light_core::{FixtureId, spatial::RigidTransform};
use light_engine::CapturedFrameLane;
use light_fixture::{PositionFitInput, PositionFitRequest, PositionFitResult, PositionFitStatus};

/// Borrowed exact solver inputs. Owners and each owner's emitter indices are strictly sorted.
/// A complete cohort covers each requested emitter exactly once. Frame/sample IDs intentionally
/// do not participate: this memo may cross frames only through accepted lane continuity.
#[derive(Clone, Copy)]
pub(super) struct PositionFitMemoInput<'a> {
    pub root: FixtureId,
    pub destination: FixtureId,
    pub generation: u64,
    pub lane: &'a CapturedFrameLane,
    pub compatibility: &'a [u8; 32],
    pub owners: &'a [(FixtureId, &'a [usize])],
    pub fit: PositionFitInput<'a>,
    pub native_baseline: &'a [u32],
    /// TL-553: every channel the fit reads (the axis drivers), sorted and unique. Native values
    /// and availability outside them, such as Intensity, never change a fit result.
    pub inputs: &'a [usize],
    /// The fit's whole-vector validation accepts `fit.current_raw`.
    pub raw_accepted: bool,
    pub missing_mount: bool,
    pub protected: bool,
    pub geometry_dirty: bool,
}
#[derive(Debug, PartialEq)]
struct PositionFitKey {
    root: FixtureId,
    destination: FixtureId,
    generation: u64,
    lane: CapturedFrameLane,
    compatibility: [u8; 32],
    owners: Vec<(FixtureId, Vec<usize>)>,
    inputs: Vec<usize>,
    /// Native values, availability and native baseline at `inputs` only.
    current_raw: Vec<u32>,
    available: Vec<bool>,
    requests: Vec<Option<PositionFitRequest>>,
    previous: Vec<Option<f64>>,
    mount: RigidTransform,
    native_baseline: Vec<u32>,
}
#[derive(Debug, PartialEq)]
pub(super) struct PositionFitMemo {
    key: PositionFitKey,
    output: Vec<PositionFitResult>,
    proposed_raw: Vec<u32>,
    achieved_axes: Vec<Option<f64>>,
}
impl PositionFitMemo {
    /// Copies only on successful creation. An unknown, protected or incomplete solve can never
    /// become reusable; of the held results only a bounded search that found no solution can
    /// (TL-553): it is a pure function of the key, so the same inputs repeat the same search
    /// and the same reported status. Dirty geometry vetoes a hit but permits storing a newly
    /// fitted result. Keeping this memo is not itself a continuity acceptance operation.
    pub fn new(
        input: &PositionFitMemoInput<'_>,
        output: &[PositionFitResult],
        proposed_raw: &[u32],
        achieved_axes: &[Option<f64>],
    ) -> Option<Self> {
        if !cacheable(input)
            || output.len() != input.fit.requests.len()
            || proposed_raw.len() != input.fit.current_raw.len()
            || achieved_axes.len() != input.fit.previous.len()
            || output
                .iter()
                .zip(input.fit.requests)
                .any(|(result, requested)| {
                    let achieved_finite =
                        |pair: [f64; 2]| pair.iter().all(|value| value.is_finite());
                    result.requested != *requested
                        || match result.status {
                            PositionFitStatus::Fitted => {
                                !result.achieved.is_some_and(achieved_finite)
                            }
                            PositionFitStatus::UnreachableTarget => {
                                !result.achieved.is_none_or(achieved_finite)
                            }
                            _ => true,
                        }
                })
            || achieved_axes
                .iter()
                .flatten()
                .any(|value| !value.is_finite())
        {
            return None;
        }
        Some(Self {
            key: PositionFitKey {
                root: input.root,
                destination: input.destination,
                generation: input.generation,
                lane: input.lane.clone(),
                compatibility: *input.compatibility,
                owners: input
                    .owners
                    .iter()
                    .map(|(owner, emitters)| (*owner, emitters.to_vec()))
                    .collect(),
                inputs: input.inputs.to_vec(),
                current_raw: at(input.fit.current_raw, input.inputs).collect(),
                available: at(input.fit.available, input.inputs).collect(),
                requests: input.fit.requests.to_vec(),
                previous: input.fit.previous.to_vec(),
                mount: input.fit.mount,
                native_baseline: at(input.native_baseline, input.inputs).collect(),
            },
            output: output.to_vec(),
            proposed_raw: proposed_raw.to_vec(),
            achieved_axes: achieved_axes.to_vec(),
        })
    }

    /// Allocation-free comparison of every real solver input, including the previous-joint
    /// seed. A repeated request alone is insufficient; a second iteration may improve its fit.
    pub fn matches(&self, input: &PositionFitMemoInput<'_>) -> bool {
        let key = &self.key;
        !input.missing_mount
            && !input.protected
            && !input.geometry_dirty
            && key.root == input.root
            && key.destination == input.destination
            && key.generation == input.generation
            && key.lane.same_memo_domain(input.lane)
            && key.compatibility == *input.compatibility
            && key.owners.len() == input.owners.len()
            && key
                .owners
                .iter()
                .zip(input.owners)
                .all(|((owner, emitters), (other, indices))| {
                    owner == other && emitters.as_slice() == *indices
                })
            && input.raw_accepted
            && key.inputs == input.inputs
            && input.fit.current_raw.len() == input.fit.available.len()
            && input.fit.current_raw.len() == input.native_baseline.len()
            && at(input.fit.current_raw, input.inputs).eq(key.current_raw.iter().copied())
            && at(input.fit.available, input.inputs).eq(key.available.iter().copied())
            && key.requests == input.fit.requests
            && key.previous == input.fit.previous
            && key.mount == input.fit.mount
            && at(input.native_baseline, input.inputs).eq(key.native_baseline.iter().copied())
    }
    pub fn output(&self) -> &[PositionFitResult] {
        &self.output
    }
    pub fn proposed_raw(&self) -> &[u32] {
        &self.proposed_raw
    }
    pub fn achieved_axes(&self) -> &[Option<f64>] {
        &self.achieved_axes
    }
}
/// Values of `values` at the (validated, in-range) input channels.
fn at<'v, T: Copy>(values: &'v [T], inputs: &'v [usize]) -> impl Iterator<Item = T> + 'v {
    inputs.iter().map(move |&index| values[index])
}

fn cacheable(input: &PositionFitMemoInput<'_>) -> bool {
    if input.missing_mount
        || !input.raw_accepted
        || input.inputs.windows(2).any(|pair| pair[0] >= pair[1])
        || input
            .inputs
            .last()
            .is_some_and(|&last| last >= input.fit.current_raw.len())
        || input.protected
        || input.root.0.is_nil()
        || input.destination.0.is_nil()
        || input.owners.is_empty()
        || input.fit.requests.is_empty()
        || input.fit.current_raw.len() != input.fit.available.len()
        || input.fit.current_raw.len() != input.native_baseline.len()
        || input
            .fit
            .previous
            .iter()
            .flatten()
            .any(|value| !value.is_finite())
        || input.fit.requests.iter().any(|request| match request {
            Some(PositionFitRequest::Angles { pan, tilt }) => !pan.is_finite() || !tilt.is_finite(),
            Some(PositionFitRequest::Target { world: Some(world) }) => {
                world.iter().any(|value| !value.is_finite())
            }
            _ => true,
        })
        || input
            .owners
            .windows(2)
            .any(|pair| pair[0].0.0 >= pair[1].0.0)
    {
        return false;
    }
    let mut count = 0;
    for (owner_index, (owner, emitters)) in input.owners.iter().enumerate() {
        if owner.0.is_nil()
            || emitters.is_empty()
            || emitters.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return false;
        }
        for &emitter in *emitters {
            if emitter >= input.fit.requests.len()
                || input.owners[..owner_index]
                    .iter()
                    .any(|(_, earlier)| earlier.binary_search(&emitter).is_ok())
            {
                return false;
            }
            count += 1;
        }
    }
    count == input.fit.requests.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use light_engine::PreloadBranch;
    use std::sync::Arc;
    use uuid::Uuid;

    fn fixture(value: u128) -> FixtureId {
        FixtureId(Uuid::from_u128(value))
    }
    fn input<'a>(
        lane: &'a CapturedFrameLane,
        owners: &'a [(FixtureId, &'a [usize])],
    ) -> PositionFitMemoInput<'a> {
        PositionFitMemoInput {
            root: fixture(1),
            destination: fixture(2),
            generation: 10,
            lane,
            compatibility: &[7; 32],
            owners,
            fit: PositionFitInput {
                current_raw: &[100, 200, 255],
                available: &[true, true, true],
                requests: &[Some(PositionFitRequest::Angles {
                    pan: 10.,
                    tilt: 20.,
                })],
                previous: &[Some(9.), Some(19.)],
                mount: RigidTransform::IDENTITY,
            },
            native_baseline: &[50, 60, 70],
            inputs: &[0, 1],
            raw_accepted: true,
            missing_mount: false,
            protected: false,
            geometry_dirty: false,
        }
    }
    fn result(requested: Option<PositionFitRequest>) -> PositionFitResult {
        PositionFitResult {
            emitter_id: Uuid::from_u128(30),
            head_id: None,
            requested,
            status: PositionFitStatus::Fitted,
            achieved: Some([10., 20.]),
            pose: Some(RigidTransform::IDENTITY),
            angular_error_degrees: Some(0.),
            clipped: false,
            search_limited: false,
            quality: light_fixture::PhysicalDataQuality::Measured,
            flags: Default::default(),
            writes: [None, None],
        }
    }
    fn memo(input: &PositionFitMemoInput<'_>) -> PositionFitMemo {
        PositionFitMemo::new(
            input,
            &[result(input.fit.requests[0])],
            &[101, 201, 255],
            &[Some(10.), Some(20.)],
        )
        .unwrap()
    }
    #[test]
    fn exact_fit_input_comparison_rejects_previous_raw_native_pose_and_identity_changes() {
        let owners = [(fixture(3), &[0][..])];
        let lane = CapturedFrameLane::Live;
        let original = input(&lane, &owners);
        let saved = memo(&original);
        assert!(saved.matches(&original));
        assert_eq!(saved.proposed_raw(), &[101, 201, 255]);
        assert_eq!(saved.achieved_axes(), &[Some(10.), Some(20.)]);
        assert_eq!(saved.output()[0].requested, original.fit.requests[0]);
        let mut changed = original;
        changed.fit.previous = &[Some(10.), Some(20.)];
        assert!(
            !saved.matches(&changed),
            "a new accepted seed is another numerical iteration"
        );
        changed = original;
        changed.fit.current_raw = &[101, 201, 255];
        assert!(!saved.matches(&changed));
        changed = original;
        changed.native_baseline = &[51, 60, 70];
        assert!(!saved.matches(&changed));
        changed = original;
        changed.fit.available = &[true, false, true];
        assert!(!saved.matches(&changed));
        // TL-553: a channel the fit never reads (here an Intensity at index 2) is not a key.
        changed = original;
        changed.fit.current_raw = &[100, 200, 0];
        changed.native_baseline = &[50, 60, 0];
        changed.fit.available = &[true, true, false];
        assert!(saved.matches(&changed));
        // The whole-vector validation must still accept the values, and the inputs must agree.
        changed.raw_accepted = false;
        assert!(!saved.matches(&changed));
        changed = original;
        changed.inputs = &[0];
        assert!(!saved.matches(&changed));
        changed = original;
        changed.fit.requests = &[Some(PositionFitRequest::Target {
            world: Some([1., 2., 3.]),
        })];
        assert!(!saved.matches(&changed));
        changed = original;
        changed.fit.mount = RigidTransform::translation([0., 0., 1.]).unwrap();
        assert!(!saved.matches(&changed));
        changed = original;
        changed.fit.mount = RigidTransform::axis_angle([0., 1., 0.], 1.).unwrap();
        assert!(!saved.matches(&changed));
        changed = original;
        changed.root = fixture(4);
        assert!(!saved.matches(&changed));
        changed = original;
        changed.destination = fixture(4);
        assert!(!saved.matches(&changed));
        changed = original;
        changed.generation += 1;
        assert!(!saved.matches(&changed));
        changed = original;
        changed.compatibility = &[8; 32];
        assert!(!saved.matches(&changed));
        let other_owners = [(fixture(4), &[0][..])];
        changed = original;
        changed.owners = &other_owners;
        assert!(!saved.matches(&changed));
    }
    #[test]
    fn exact_preload_branch_and_episode_identity_cannot_borrow_another_memo() {
        let owners = [(fixture(3), &[0][..])];
        let lane = CapturedFrameLane::Preload {
            bundle: Arc::new(()),
            state: Arc::new(()),
            revision: 2,
            branch: PreloadBranch::BeforeRelease,
        };
        let original = input(&lane, &owners);
        let saved = memo(&original);
        let cloned = lane.clone();
        assert!(saved.matches(&input(&cloned, &owners)));
        let CapturedFrameLane::Preload {
            bundle,
            state,
            revision,
            ..
        } = &lane
        else {
            unreachable!()
        };
        let next_frame = CapturedFrameLane::Preload {
            bundle: Arc::new(()),
            state: state.clone(),
            revision: *revision + 1,
            branch: PreloadBranch::BeforeRelease,
        };
        assert_ne!(
            lane, next_frame,
            "another bundle/revision is not the same frame lane"
        );
        assert!(
            saved.matches(&input(&next_frame, &owners)),
            "unchanged solver inputs in the accepted episode may reuse across frames"
        );
        let other_branch = CapturedFrameLane::Preload {
            bundle: bundle.clone(),
            state: state.clone(),
            revision: *revision,
            branch: PreloadBranch::AfterRelease,
        };
        assert!(!saved.matches(&input(&other_branch, &owners)));
        let other_episode = CapturedFrameLane::Preload {
            bundle: bundle.clone(),
            state: Arc::new(()),
            revision: *revision,
            branch: PreloadBranch::BeforeRelease,
        };
        assert!(!saved.matches(&input(&other_episode, &owners)));
        assert!(!saved.matches(&input(&CapturedFrameLane::Live, &owners)));
    }
    #[test]
    fn unknown_protected_dirty_incomplete_and_nonfitted_results_never_become_cache_entries() {
        let owners = [(fixture(3), &[0][..])];
        let lane = CapturedFrameLane::Live;
        let original = input(&lane, &owners);
        let saved = memo(&original);
        for flag in 0..2 {
            let mut changed = original;
            match flag {
                0 => changed.missing_mount = true,
                _ => changed.protected = true,
            }
            assert!(!saved.matches(&changed));
            assert!(
                PositionFitMemo::new(
                    &changed,
                    &[result(changed.fit.requests[0])],
                    &[101, 201, 255],
                    &[Some(10.), Some(20.)]
                )
                .is_none()
            );
        }
        let mut dirty = original;
        dirty.geometry_dirty = true;
        assert!(!saved.matches(&dirty));
        let refreshed = memo(&dirty);
        assert!(
            refreshed.matches(&original),
            "dirty-frame fitting may establish the next clean frame's exact key"
        );
        let mut unknown = original;
        unknown.fit.requests = &[Some(PositionFitRequest::Target { world: None })];
        assert!(
            PositionFitMemo::new(
                &unknown,
                &[result(unknown.fit.requests[0])],
                &[101, 201, 255],
                &[Some(10.), Some(20.)]
            )
            .is_none()
        );
        // TL-553: a bounded search without a solution repeats exactly for the same key.
        let mut unreachable = result(original.fit.requests[0]);
        unreachable.status = PositionFitStatus::UnreachableTarget;
        let saved_unreachable = PositionFitMemo::new(
            &original,
            &[unreachable.clone()],
            &[100, 200, 255],
            &[Some(9.), Some(19.)],
        )
        .expect("an unreachable bounded search is reusable");
        assert!(saved_unreachable.matches(&original));
        assert_eq!(
            saved_unreachable.output()[0].status,
            PositionFitStatus::UnreachableTarget
        );
        let mut changed = original;
        changed.fit.previous = &[Some(10.), Some(20.)];
        assert!(!saved_unreachable.matches(&changed));
        unreachable.achieved = Some([f64::NAN, 0.]);
        assert!(
            PositionFitMemo::new(&original, &[unreachable], &[100, 200, 255], &[None, None])
                .is_none()
        );
        for status in [
            PositionFitStatus::MissingTarget,
            PositionFitStatus::CoincidentTarget,
            PositionFitStatus::OwnershipConflict,
            PositionFitStatus::UnavailableInput,
        ] {
            let mut held = result(original.fit.requests[0]);
            held.status = status;
            assert!(
                PositionFitMemo::new(
                    &original,
                    &[held],
                    &[101, 201, 255],
                    &[Some(10.), Some(20.)]
                )
                .is_none(),
                "{status:?}"
            );
        }
        let mut held = result(original.fit.requests[0]);
        held.status = PositionFitStatus::MissingTarget;
        assert!(
            PositionFitMemo::new(
                &original,
                &[held],
                &[101, 201, 255],
                &[Some(10.), Some(20.)]
            )
            .is_none()
        );
        let duplicates = [(fixture(3), &[0][..]), (fixture(4), &[0][..])];
        let mut incomplete = original;
        incomplete.owners = &duplicates;
        assert!(
            PositionFitMemo::new(
                &incomplete,
                &[result(original.fit.requests[0])],
                &[101, 201, 255],
                &[Some(10.), Some(20.)]
            )
            .is_none()
        );
        let missing = [(fixture(3), &[][..])];
        incomplete.owners = &missing;
        assert!(
            PositionFitMemo::new(
                &incomplete,
                &[result(original.fit.requests[0])],
                &[101, 201, 255],
                &[Some(10.), Some(20.)]
            )
            .is_none()
        );
    }
}
