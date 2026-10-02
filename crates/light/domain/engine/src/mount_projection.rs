//! Saved physical placements carried by final, coherent Point poses. This is a forward mount
//! transform only: optical installation calibration and inverse aiming remain separate.

use std::sync::Arc;

use light_core::{FixtureId, spatial::RigidTransform};
use light_fixture::{FixtureLocation, FixtureVector, PatchedFixture};
use uuid::Uuid;

use crate::{Pooled, ResolvedPointPose, Reusable, ValuePool};

/// One root or multipatch body's placement in desk world axes, in metres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedFixtureMount {
    pub fixture_id: FixtureId,
    pub instance_id: Uuid,
    /// The effective, existing Point reference. Missing/deleted/non-Point references fall back
    /// to saved stage placement, and Points themselves cannot take a reference.
    pub position_master: Option<FixtureId>,
    /// Invalid recovered rotations/poses have no transform; they never become a fabricated
    /// identity or cause otherwise successful output to fail. Installation zero corrections,
    /// axis inversions and bracket settings already belong to profile-local physical output.
    pub world_from_fixture: Option<RigidTransform>,
}

/// Immutable mount results retained with the same Point/value frame as output. The dense copy
/// uses pooled storage when a Point moves; unchanged frames share the entire retained result.
#[derive(Debug, Default)]
pub struct FixtureMountFrame {
    layout: Option<Arc<()>>,
    point_poses: Vec<Option<ResolvedPointPose>>,
    mounts: Vec<ResolvedFixtureMount>,
}

impl FixtureMountFrame {
    pub fn mounts(&self) -> &[ResolvedFixtureMount] {
        &self.mounts
    }

    pub fn mount(&self, instance_id: Uuid) -> Option<&ResolvedFixtureMount> {
        self.mounts
            .binary_search_by_key(&instance_id, |mount| mount.instance_id)
            .ok()
            .map(|index| &self.mounts[index])
    }
}

impl Reusable for FixtureMountFrame {
    fn reset(&mut self) {
        self.layout = None;
        self.point_poses.clear();
        self.mounts.clear();
    }
}

/// Cheap to fork at capture: old readers keep their frame and candidates borrow fresh reusable
/// storage only when relevant poses change. The pool is shared storage, never shared history.
#[derive(Clone, Default)]
pub(crate) struct MountTransformWorkspace {
    frame: Option<Arc<Pooled<FixtureMountFrame>>>,
    #[cfg(test)]
    pub(crate) recomputed_points: usize,
    #[cfg(test)]
    pub(crate) recomputed_mounts: usize,
}

struct PointDependents {
    fixture_id: FixtureId,
    mounts: Vec<usize>,
}

pub(crate) struct MountProjectionIndex {
    identity: Arc<()>,
    base: Vec<ResolvedFixtureMount>,
    points: Vec<PointDependents>,
    pool: Arc<ValuePool<FixtureMountFrame>>,
}

impl MountProjectionIndex {
    pub(crate) fn compile(
        fixtures: &[PatchedFixture],
        points: &crate::point_projection::PointProjectionIndex,
    ) -> Self {
        let mut base = Vec::new();
        for fixture in fixtures {
            let reference = points
                .origin_for(fixture.fixture_id)
                .is_none()
                .then(|| {
                    fixture
                        .position_master
                        .map(FixtureId)
                        .filter(|id| points.origin_for(*id).is_some())
                })
                .flatten();
            let mut append = |instance_id, location, rotation| {
                base.push(ResolvedFixtureMount {
                    fixture_id: fixture.fixture_id,
                    instance_id,
                    position_master: reference,
                    world_from_fixture: saved_placement(location, rotation),
                });
            };
            append(fixture.fixture_id.0, fixture.location, fixture.rotation);
            for copy in &fixture.multipatch {
                append(copy.id, copy.location, copy.rotation);
            }
        }
        base.sort_by_key(|mount| mount.instance_id);
        let mut by_point = std::collections::BTreeMap::<Uuid, Vec<usize>>::new();
        for (index, mount) in base.iter().enumerate() {
            if let Some(point) = mount.position_master {
                by_point.entry(point.0).or_default().push(index);
            }
        }
        Self {
            identity: Arc::new(()),
            base,
            points: by_point
                .into_iter()
                .map(|(fixture_id, mounts)| PointDependents {
                    fixture_id: FixtureId(fixture_id),
                    mounts,
                })
                .collect(),
            pool: Arc::new(ValuePool::default()),
        }
    }

    pub(crate) fn resolve(
        &self,
        points: &[ResolvedPointPose],
        workspace: &mut MountTransformWorkspace,
    ) -> Arc<Pooled<FixtureMountFrame>> {
        #[cfg(test)]
        {
            workspace.recomputed_points = 0;
            workspace.recomputed_mounts = 0;
        }
        let previous = workspace.frame.as_ref().filter(|frame| {
            frame
                .layout
                .as_ref()
                .is_some_and(|layout| Arc::ptr_eq(layout, &self.identity))
        });
        // PointProjectionIndex orders this small Point-only slice by fixture identity.
        let pose = |id: FixtureId| {
            points
                .binary_search_by_key(&id.0, |pose| pose.fixture_id.0)
                .ok()
                .map(|index| points[index])
        };
        if let Some(previous) = previous {
            if self
                .points
                .iter()
                .zip(&previous.point_poses)
                .all(|(entry, old)| pose(entry.fixture_id) == *old)
            {
                return Arc::clone(previous);
            }
        }
        let mut next = self.pool.take();
        next.layout = Some(Arc::clone(&self.identity));
        next.mounts
            .extend_from_slice(previous.map_or(self.base.as_slice(), |frame| &frame.mounts));
        next.point_poses.reserve(self.points.len());
        for (index, entry) in self.points.iter().enumerate() {
            let current = pose(entry.fixture_id);
            next.point_poses.push(current);
            if previous.is_some_and(|frame| frame.point_poses[index] == current) {
                continue;
            }
            #[cfg(test)]
            {
                workspace.recomputed_points += 1;
                workspace.recomputed_mounts += entry.mounts.len();
            }
            let carry = current.map_or(Some(RigidTransform::IDENTITY), point_carry);
            for &mount in &entry.mounts {
                next.mounts[mount].world_from_fixture = carry
                    .zip(self.base[mount].world_from_fixture)
                    .map(|(carry, placement)| carry.compose(placement));
            }
        }
        let next = Arc::new(next);
        workspace.frame = Some(Arc::clone(&next));
        next
    }
}

fn saved_placement(location: FixtureLocation, rotation: FixtureVector) -> Option<RigidTransform> {
    Some(
        RigidTransform::translation(
            [location.x, location.y, location.z].map(|v| f64::from(v) / 1_000.0),
        )?
        .compose(RigidTransform::euler_xyz(
            [rotation.x, rotation.y, rotation.z].map(f64::from),
        )?),
    )
}

fn point_carry(point: ResolvedPointPose) -> Option<RigidTransform> {
    Some(
        RigidTransform::translation(point.offset_metres.map(f64::from))?.compose(
            RigidTransform::about_pivot(
                RigidTransform::euler_xyz(point.rotation_degrees.map(f64::from))?,
                point.origin_metres.map(f64::from),
            )?,
        ),
    )
}
