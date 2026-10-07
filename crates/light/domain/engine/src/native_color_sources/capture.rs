//! Direct Color capture bound to one captured frame and its generation's retained originals.
//!
//! The catalogue comes from the capture's own runtime generation, never from the current
//! fixture library or selection. Every observation must carry the identical frame token, so a
//! batch cannot join values from different captures, generations or lanes.

use super::NativeColorSourceCatalog;
use crate::{CapturedFrameToken, PreparedOutputFrame};
use light_core::{
    FixtureId, NativeColorIdentity,
    programming::{
        ColorProgram, DirectColorCapture, DirectDestination, DirectReplay, IntentError,
        NativeColorObservation, capture_direct_color, plan_direct_replay,
    },
};
use light_dynamics::NativeColorModelCapability;
use std::collections::HashSet;

/// One head's premaster native values as sampled by the frame named by `token`.
#[derive(Clone, Debug)]
pub struct DirectColorObservation {
    pub token: CapturedFrameToken,
    pub target: FixtureId,
    pub native: NativeColorObservation,
}

/// A captured Direct value with the frame evidence it was taken from.
#[derive(Clone, Debug)]
pub struct CapturedDirectColor {
    pub token: CapturedFrameToken,
    pub target: FixtureId,
    pub capture: DirectColorCapture,
}

impl NativeColorSourceCatalog {
    /// Capture against this catalogue's exact retained ORIGINAL. An unavailable original cannot
    /// verify complete ownership, so capture fails instead of recording an unverified recipe.
    pub fn capture_direct(
        &self,
        observation: NativeColorObservation,
    ) -> Result<DirectColorCapture, IntentError> {
        let model = self.resolve(&observation.source)?;
        capture_direct_color(model.as_ref(), observation)
    }

    /// Plan exact replay or explicit fallback for a destination head. `None` means the
    /// destination has no native Color path; an unavailable model is Unknown compatibility.
    pub fn plan_direct_replay(
        &self,
        program: &ColorProgram,
        destination: Option<&NativeColorIdentity>,
    ) -> Result<DirectReplay, IntentError> {
        let Some(destination) = destination else {
            return plan_direct_replay(program, &DirectDestination::NoNativeColor);
        };
        match self.resolve_capability(destination)? {
            NativeColorModelCapability::Available(model) => {
                plan_direct_replay(program, &DirectDestination::Verified(model.as_ref()))
            }
            NativeColorModelCapability::Unavailable(unavailable) => {
                plan_direct_replay(program, &DirectDestination::Unverified(unavailable.detail))
            }
        }
    }
}

impl PreparedOutputFrame {
    /// Capture every observation atomically from this frame. All observations must carry one
    /// identical token of this capture and generation (Live or one Preload branch); one head
    /// may appear once. Any failure returns no capture at all.
    pub fn capture_direct_color(
        &self,
        observations: Vec<DirectColorObservation>,
    ) -> Result<Vec<CapturedDirectColor>, IntentError> {
        let Some(first) = observations.first() else {
            return Ok(Vec::new());
        };
        let frame = self.frame_token();
        if !first.token.same_capture(&frame) || first.token.generation() != self.generation() {
            return Err(IntentError(
                "Direct capture observation belongs to a different frame or generation".into(),
            ));
        }
        let token = first.token.clone();
        let catalogue = std::sync::Arc::clone(&self.snapshot().native_color_sources);
        let mut heads = HashSet::new();
        let mut captured = Vec::with_capacity(observations.len());
        for observation in observations {
            if observation.token != token {
                return Err(IntentError(
                    "Direct capture cannot join observations from different frames or lanes".into(),
                ));
            }
            if !heads.insert((observation.target, observation.native.source.head_id)) {
                return Err(IntentError(
                    "Direct capture observed one fixture head twice".into(),
                ));
            }
            captured.push(CapturedDirectColor {
                token: observation.token,
                target: observation.target,
                capture: catalogue.capture_direct(observation.native)?,
            });
        }
        Ok(captured)
    }
}
