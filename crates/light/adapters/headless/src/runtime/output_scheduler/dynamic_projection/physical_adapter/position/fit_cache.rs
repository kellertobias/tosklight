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
    /// Copies only on successful creation. A held, unknown, protected or incomplete solve can
    /// never become reusable. Dirty geometry vetoes a hit but permits storing a newly fitted
    /// result. Keeping this memo is not itself a continuity acceptance operation.
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
                    result.status != PositionFitStatus::Fitted
                        || result.requested != *requested
                        || result
                            .achieved
                            .is_none_or(|pair| pair.iter().any(|value| !value.is_finite()))
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
                current_raw: input.fit.current_raw.to_vec(),
                available: input.fit.available.to_vec(),
                requests: input.fit.requests.to_vec(),
                previous: input.fit.previous.to_vec(),
                mount: input.fit.mount,
                native_baseline: input.native_baseline.to_vec(),
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
            && key.current_raw == input.fit.current_raw
            && key.available == input.fit.available
            && key.requests == input.fit.requests
            && key.previous == input.fit.previous
            && key.mount == input.fit.mount
            && key.native_baseline == input.native_baseline
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
fn cacheable(input: &PositionFitMemoInput<'_>) -> bool {
    if input.missing_mount
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
                current_raw: &[100, 200],
                available: &[true, true],
                requests: &[Some(PositionFitRequest::Angles {
                    pan: 10.,
                    tilt: 20.,
                })],
                previous: &[Some(9.), Some(19.)],
                mount: RigidTransform::IDENTITY,
            },
            native_baseline: &[50, 60],
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
            &[101, 201],
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
        assert_eq!(saved.proposed_raw(), &[101, 201]);
        assert_eq!(saved.achieved_axes(), &[Some(10.), Some(20.)]);
        assert_eq!(saved.output()[0].requested, original.fit.requests[0]);
        let mut changed = original;
        changed.fit.previous = &[Some(10.), Some(20.)];
        assert!(
            !saved.matches(&changed),
            "a new accepted seed is another numerical iteration"
        );
        changed = original;
        changed.fit.current_raw = &[101, 201];
        assert!(!saved.matches(&changed));
        changed = original;
        changed.native_baseline = &[51, 60];
        assert!(!saved.matches(&changed));
        changed = original;
        changed.fit.available = &[true, false];
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
                    &[101, 201],
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
                &[101, 201],
                &[Some(10.), Some(20.)]
            )
            .is_none()
        );
        let mut held = result(original.fit.requests[0]);
        held.status = PositionFitStatus::MissingTarget;
        assert!(
            PositionFitMemo::new(&original, &[held], &[101, 201], &[Some(10.), Some(20.)])
                .is_none()
        );
        let duplicates = [(fixture(3), &[0][..]), (fixture(4), &[0][..])];
        let mut incomplete = original;
        incomplete.owners = &duplicates;
        assert!(
            PositionFitMemo::new(
                &incomplete,
                &[result(original.fit.requests[0])],
                &[101, 201],
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
                &[101, 201],
                &[Some(10.), Some(20.)]
            )
            .is_none()
        );
    }
}
