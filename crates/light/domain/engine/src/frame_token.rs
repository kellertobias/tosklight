//! Owned identity of one captured output frame and lane.
//!
//! A [`CapturedFrameToken`] is the only identity a physical family adapter may attach to a
//! retained result. It names the show revision, the runtime generation (new for every patch,
//! mode, calibration, rebind or show installation), the exact capture, its sample time and
//! tracking sample, and the evaluating lane (Live or one Preload Release branch). It owns
//! nothing from the capture except identity handles, so a retained sidecar can outlive the
//! capture without borrowing it and without keeping frame buffers alive.
//!
//! Equality is identity equality: two captures at the same timestamp, or two Preload bundles
//! of one capture, are different tokens. A token never certifies that its frame was published.

use std::sync::Arc;

use chrono::{DateTime, Utc};

use crate::{
    PreloadBranch, PreloadFrameState, PreparedFrameGeometry, PreparedOutputFrame,
    PreparedPreloadFrame, PreparedStaticFamilyFrame,
};

/// The evaluating lane of a captured frame. Live and each Preload branch keep separate
/// adapter continuity; a token from one lane is never valid for another.
#[derive(Clone, Debug)]
pub enum CapturedFrameLane {
    Live,
    Preload {
        bundle: Arc<()>,
        state: Arc<()>,
        revision: u64,
        branch: PreloadBranch,
    },
}

impl CapturedFrameLane {
    pub fn preload_branch(&self) -> Option<PreloadBranch> {
        match self {
            Self::Live => None,
            Self::Preload { branch, .. } => Some(*branch),
        }
    }

    /// Stable domain for a memo carried by accepted lane continuity across frames.
    /// A Preload state belongs to one retained episode; its two Release branches remain
    /// independent. New bundles and successful render revisions do not create a new domain.
    /// This comparison never admits a frame or replaces exact token equality. Memo callers
    /// must still compare the captured generation and every input of the cached calculation.
    pub fn same_memo_domain(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Live, Self::Live) => true,
            (
                Self::Preload { state, branch, .. },
                Self::Preload {
                    state: other_state,
                    branch: other_branch,
                    ..
                },
            ) => Arc::ptr_eq(state, other_state) && branch == other_branch,
            _ => false,
        }
    }
}

impl PartialEq for CapturedFrameLane {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Live, Self::Live) => true,
            (
                Self::Preload {
                    bundle,
                    state,
                    revision,
                    branch,
                },
                Self::Preload {
                    bundle: other_bundle,
                    state: other_state,
                    revision: other_revision,
                    branch: other_branch,
                },
            ) => {
                Arc::ptr_eq(bundle, other_bundle)
                    && Arc::ptr_eq(state, other_state)
                    && revision == other_revision
                    && branch == other_branch
            }
            _ => false,
        }
    }
}

impl Eq for CapturedFrameLane {}

/// Owned show/generation/frame/sample/lane identity of one captured frame.
#[derive(Clone, Debug)]
pub struct CapturedFrameToken {
    frame: Arc<()>,
    show_revision: u64,
    generation: u64,
    sampled_at: DateTime<Utc>,
    tracking_sequence: u64,
    lane: CapturedFrameLane,
}

impl PartialEq for CapturedFrameToken {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.frame, &other.frame)
            && self.show_revision == other.show_revision
            && self.generation == other.generation
            && self.sampled_at == other.sampled_at
            && self.tracking_sequence == other.tracking_sequence
            && self.lane == other.lane
    }
}

impl Eq for CapturedFrameToken {}

impl CapturedFrameToken {
    pub fn show_revision(&self) -> u64 {
        self.show_revision
    }
    /// The runtime generation identity. Destination descriptors compile against this value.
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn sampled_at(&self) -> DateTime<Utc> {
        self.sampled_at
    }
    /// Accepted tracking sample sequence held by this capture.
    pub fn tracking_sequence(&self) -> u64 {
        self.tracking_sequence
    }
    pub fn lane(&self) -> &CapturedFrameLane {
        &self.lane
    }
    /// True when both tokens name the same capture, regardless of lane.
    pub fn same_capture(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.frame, &other.frame)
    }
    /// Geometry is valid for this token only when it was resolved from the same generation
    /// and sample. Geometry carries no capture pointer; the caller still pairs it with its token.
    pub fn matches_geometry(&self, geometry: &PreparedFrameGeometry) -> bool {
        geometry.generation() == self.generation && geometry.sampled_at() == self.sampled_at
    }
    /// A static family token matches when it came from this exact capture and lane.
    pub fn matches_static_frame(&self, frame: &PreparedStaticFamilyFrame) -> bool {
        if !Arc::ptr_eq(&self.frame, &frame.capture_identity) {
            return false;
        }
        match (&self.lane, &frame.preload) {
            (CapturedFrameLane::Live, None) => true,
            (
                CapturedFrameLane::Preload {
                    bundle,
                    state,
                    revision,
                    branch,
                },
                Some(identity),
            ) => identity.same_identity(bundle, state, *revision, *branch),
            _ => false,
        }
    }
}

impl PreparedOutputFrame {
    /// The Live lane token of this capture.
    pub fn frame_token(&self) -> CapturedFrameToken {
        CapturedFrameToken {
            frame: Arc::clone(&self.identity),
            show_revision: self.generation.snapshot().revision,
            generation: self.generation.identity(),
            sampled_at: self.sampled_at,
            tracking_sequence: self.tracked.accepted_sequence,
            lane: CapturedFrameLane::Live,
        }
    }
}

impl PreparedPreloadFrame<'_> {
    /// The token of one Release branch of this exact pending bundle and state revision.
    pub fn frame_token(
        &self,
        state: &PreloadFrameState,
        branch: PreloadBranch,
    ) -> CapturedFrameToken {
        let (bundle, state, revision) = self.token_identity(state);
        CapturedFrameToken {
            lane: CapturedFrameLane::Preload {
                bundle,
                state,
                revision,
                branch,
            },
            ..self.frame().frame_token()
        }
    }
}
