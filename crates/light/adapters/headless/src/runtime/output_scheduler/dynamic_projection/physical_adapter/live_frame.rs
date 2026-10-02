//! The one Live finalizer of every physical lane set (TL-548 C1). `finalize_live_physical_frame`
//! (one family lane), `finalize_live_optics_frame` (Focus + Zoom) and
//! `finalize_live_family_frame` (Position + Color + Focus/Zoom) only choose the lane set and
//! sidecar type; the token checks, the abandon-on-error rule and the commit order live here.
use super::programming_projection::hybrid::{
    HybridFamilyRequirement, HybridFrameResolver, PreparedHybridFrame,
};
use super::*;

/// A successfully finalized Live frame. Only this value may be published.
pub(in crate::runtime) struct PublishedLiveFrame<T> {
    pub rendered: RenderResult,
    pub token: CapturedFrameToken,
    pub geometry: PreparedFrameGeometry,
    pub sampled: CapturedDynamicSample,
    pub results: Vec<T>,
    pub requirements: Vec<HybridFamilyRequirement>,
    /// Owners these lanes published last frame and no longer own, with their last provenance.
    pub released: Vec<ReleasedPhysicalOwner>,
}

/// A sidecar bound to exactly one captured frame.
pub(in crate::runtime) trait LiveFrameSidecar {
    fn frame_token(&self) -> &CapturedFrameToken;
}

impl<A: PhysicalFamilyAdapter> LiveFrameSidecar for PhysicalHeadResult<A> {
    fn frame_token(&self) -> &CapturedFrameToken {
        &self.token
    }
}

/// A Live lane set that stages one frame: `verify_frame`/`accept_frame` come from the resolver,
/// `abandon` discards every staged attempt and `released` reports the last accepted frame.
pub(in crate::runtime) trait LiveFrameLanes: HybridFrameResolver {
    fn abandon_frame(&self);
    fn released_owners(&self) -> Vec<ReleasedPhysicalOwner>;
}

impl<A: PhysicalFamilyAdapter> LiveFrameLanes for PhysicalAdapterLane<A> {
    fn abandon_frame(&self) {
        self.abandon();
    }

    fn released_owners(&self) -> Vec<ReleasedPhysicalOwner> {
        self.released()
    }
}

impl LiveFrameLanes for OpticsLanes {
    fn abandon_frame(&self) {
        self.abandon();
    }

    fn released_owners(&self) -> Vec<ReleasedPhysicalOwner> {
        self.released()
    }
}

/// Finalize a prepared Live frame through the existing engine finalizer.
///
/// Every token check and the lanes' shared-control verification run before
/// `render_static_family_frame`; the render is the last fallible step; lane continuity commits
/// only after it succeeds. On any error nothing is published and every lane of the set is
/// abandoned, keeping its previous accepted state.
pub(in crate::runtime::output_scheduler::dynamic_projection) fn finalize_live_lanes_frame<T, L>(
    engine: &Engine,
    capture: &PreparedOutputFrame,
    lanes: &L,
    prepared: PreparedHybridFrame<T>,
) -> Result<PublishedLiveFrame<T>, DynamicRuntimeError>
where
    T: LiveFrameSidecar,
    L: LiveFrameLanes + ?Sized,
{
    let invalid = |error: &dyn std::fmt::Display| {
        lanes.abandon_frame();
        DynamicRuntimeError::InvalidSample(error.to_string())
    };
    let token = capture.frame_token();
    if prepared.frame_token != token {
        return Err(invalid(
            &"prepared hybrid frame belongs to another capture or lane",
        ));
    }
    if prepared
        .family_sidecars
        .iter()
        .any(|result| *result.frame_token() != token)
    {
        return Err(invalid(
            &"physical sidecar carries a mixed or stale frame token",
        ));
    }
    lanes
        .verify_frame(&token)
        .map_err(|error| invalid(&error))?;
    let PreparedHybridFrame {
        token: static_token,
        geometry,
        sampled,
        family_sidecars,
        requirements,
        ..
    } = prepared;
    let rendered = engine
        .render_static_family_frame(capture, static_token)
        .map_err(|error| invalid(&error))?;
    let accepted = lanes.accept_frame(&token);
    debug_assert!(accepted, "verified Live token is accepted");
    Ok(PublishedLiveFrame {
        rendered,
        token,
        geometry,
        sampled,
        results: family_sidecars,
        requirements,
        released: lanes.released_owners(),
    })
}
