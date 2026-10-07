//! Final Point geometry from a resolved lane, without physical projection or a second solve.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use light_core::FixtureId;

use crate::{
    Engine, EngineError, FixtureMountFrame, Pooled, PreparedOutputFrame, PreparedStaticFamilyFrame,
    ResolvedPointPose,
};

/// Retained geometry from one prepared static token. Different tokens belonging to the same
/// capture can contain different scalar/Point batches and therefore different geometry.
#[derive(Clone, Debug)]
pub struct PreparedFrameGeometry {
    generation: u64,
    sampled_at: DateTime<Utc>,
    pub(crate) points: Arc<Pooled<Vec<ResolvedPointPose>>>,
    pub(crate) mounts: Arc<Pooled<FixtureMountFrame>>,
}

impl PreparedFrameGeometry {
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn sampled_at(&self) -> DateTime<Utc> {
        self.sampled_at
    }
    pub fn points(&self) -> &[ResolvedPointPose] {
        &self.points
    }
    /// Lookup one Point in the immutable, UUID-sorted projection of this captured lane.
    /// This does not resolve geometry again or establish output acceptance.
    pub fn point(&self, id: FixtureId) -> Option<&ResolvedPointPose> {
        self.points
            .binary_search_by_key(&id.0, |point| point.fixture_id.0)
            .ok()
            .map(|index| &self.points[index])
    }

    /// Replace `out` with sorted, unique changed, added and removed Point IDs, reusing its
    /// allocation. Compare actual Point poses, independently of mount-frame allocation identity.
    /// Different generations conservatively invalidate their complete union, even if poses
    /// compare equal. Callers handle their first frame and retain the relevant prior lane;
    /// this comparison neither validates lane continuity nor accepts either frame as output.
    pub fn changed_points_into(&self, previous: &Self, out: &mut Vec<FixtureId>) {
        out.clear();
        let invalidate_all = self.generation != previous.generation;
        let (current, previous) = (self.points(), previous.points());
        let (mut current_index, mut previous_index) = (0, 0);
        while current_index < current.len() && previous_index < previous.len() {
            let now = &current[current_index];
            let before = &previous[previous_index];
            match now.fixture_id.0.cmp(&before.fixture_id.0) {
                std::cmp::Ordering::Less => {
                    out.push(now.fixture_id);
                    current_index += 1;
                }
                std::cmp::Ordering::Greater => {
                    out.push(before.fixture_id);
                    previous_index += 1;
                }
                std::cmp::Ordering::Equal => {
                    if invalidate_all || now != before {
                        out.push(now.fixture_id);
                    }
                    current_index += 1;
                    previous_index += 1;
                }
            }
        }
        out.extend(
            current[current_index..]
                .iter()
                .map(|point| point.fixture_id),
        );
        out.extend(
            previous[previous_index..]
                .iter()
                .map(|point| point.fixture_id),
        );
    }

    pub fn mounts(&self) -> &FixtureMountFrame {
        &self.mounts
    }
}

impl Engine {
    /// Observe final Point freezes and mount geometry from already-resolved scalar/Point inputs.
    /// Static Current stays pre-Freeze; no source is resampled and no physical output is solved.
    /// Geometry is cached on this token and updates only its candidate mount continuity. Later
    /// typed-family writes cannot address point.* axes. Final rendering reuses these same Arcs,
    /// and dropping/previewing the token never commits its candidate to Live.
    pub fn observe_static_family_geometry(
        &self,
        capture: &PreparedOutputFrame,
        frame: &mut PreparedStaticFamilyFrame,
    ) -> Result<PreparedFrameGeometry, EngineError> {
        if !Arc::ptr_eq(&frame.capture_identity, &capture.identity) {
            return Err(EngineError::StalePreparedFrame);
        }
        if let Some(geometry) = &frame.geometry {
            return Ok(geometry.clone());
        }
        let points = Arc::new(
            capture.generation.point_projection().resolve_pre_freeze(
                frame
                    .resolved
                    .frame
                    .as_ref()
                    .expect("prepared static resolution retains its dense frame"),
            ),
        );
        let mounts = capture
            .generation
            .mount_projection()
            .resolve(&points, &mut frame.continuity.mounts);
        let geometry = PreparedFrameGeometry {
            generation: capture.generation(),
            sampled_at: capture.sampled_at(),
            points,
            mounts,
        };
        frame.geometry = Some(geometry.clone());
        Ok(geometry)
    }
}
