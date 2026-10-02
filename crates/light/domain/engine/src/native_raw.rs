//! Pre-render native raw values of one destination fixture, read from a scalar-resolved
//! captured frame (TL-592).
//!
//! Physical family fitters (Color, Focus/Zoom) take the destination's current native values so
//! they keep unrelated channels, read the active function of shared channels and prefer stable
//! discrete states. This module supplies those values from exactly the immutable static
//! baseline a hybrid frame composes over, bound to the frame's [`CapturedFrameToken`].
//!
//! The values are *pre-master*: every channel of the destination mode is resolved by the same
//! compiled resolution plan the renderer uses, with neutral [`ChannelScales`] (no virtual
//! intensity, sequence, group or grand master), no blackout, Highlight, control-loss, Freeze or
//! axis-inversion overlay. Those overlays and masters stay in the final render, so a fitter
//! working in this domain never applies them a second time. Semantic family values (for
//! example the composed `color` owner) are not expanded into channels here; the family adapter
//! that owns those channels replaces them completely.
//!
//! Reading never mutates the capture, the token, Live continuity or any queued projection.
use crate::{
    CapturedFrameToken, EngineError, EngineSnapshot, PreparedOutputFrame,
    PreparedStaticFamilyFrame, ProfileValueIndex,
};
use light_core::FixtureId;
use light_fixture::ChannelScales;
use std::sync::Arc;
use uuid::Uuid;

/// One destination fixture's complete native raw vector (one full-width value per mode
/// channel, in mode channel order), with the captured frame it was read from.
#[derive(Clone, Debug, Default)]
pub struct CapturedNativeRaw {
    token: Option<CapturedFrameToken>,
    destination: Option<FixtureId>,
    instance_id: Option<Uuid>,
    raw: Vec<u32>,
}

impl CapturedNativeRaw {
    /// The captured frame and lane these values belong to.
    pub fn token(&self) -> Option<&CapturedFrameToken> {
        self.token.as_ref()
    }
    /// Root patched fixture whose mode channels `raw` indexes.
    pub fn destination(&self) -> Option<FixtureId> {
        self.destination
    }
    /// Physical root or copy whose installation inversion is included. `None` for the
    /// family-agnostic, pre-inversion capture.
    pub fn instance_id(&self) -> Option<Uuid> {
        self.instance_id
    }
    pub fn raw(&self) -> &[u32] {
        &self.raw
    }
}

/// One profile head of a patched fixture owned by a programming target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProfileHeadDestination {
    /// Root patched fixture (the native channel space of the mode).
    pub destination: FixtureId,
    /// Position of the root fixture in `EngineSnapshot::fixtures`.
    pub fixture_index: usize,
    /// Head position in the fixture's mode.
    pub head_index: usize,
    pub head_id: Uuid,
}

/// Every profile head a programming target owns, in snapshot and mode order. A root fixture
/// target can own several heads (its own and master-shared heads); a logical head owns one.
/// Configuration-time lookup: cache the result per runtime generation.
pub fn profile_head_destinations(
    snapshot: &EngineSnapshot,
    target: FixtureId,
) -> Vec<ProfileHeadDestination> {
    let mut found = Vec::new();
    for (fixture_index, fixture) in snapshot.fixtures.iter().enumerate() {
        let Some(mode) = crate::fixture::profile_mode(fixture) else {
            continue;
        };
        for (head_index, head) in mode.heads.iter().enumerate() {
            if crate::fixture::profile_head_owner(fixture, head_index, head) == target {
                found.push(ProfileHeadDestination {
                    destination: fixture.fixture_id,
                    fixture_index,
                    head_index,
                    head_id: head.id,
                });
            }
        }
    }
    found
}

impl PreparedStaticFamilyFrame {
    /// Pre-master native raw values of the fixture that owns `target`, read from this immutable
    /// scalar baseline. `capture` and `token` must be this token's own capture and lane.
    pub fn native_raw(
        &self,
        capture: &PreparedOutputFrame,
        token: &CapturedFrameToken,
        target: FixtureId,
    ) -> Result<CapturedNativeRaw, EngineError> {
        let mut out = CapturedNativeRaw::default();
        self.native_raw_into(capture, token, target, &mut out)?;
        Ok(out)
    }

    /// Allocation-reusing form of [`Self::native_raw`]. On error `out` is left empty.
    pub fn native_raw_into(
        &self,
        capture: &PreparedOutputFrame,
        token: &CapturedFrameToken,
        target: FixtureId,
        out: &mut CapturedNativeRaw,
    ) -> Result<(), EngineError> {
        self.capture_native_raw_into(capture, token, target, None, out)
    }

    /// Pre-master Position baseline for one physical root or multipatch copy. Installation
    /// pan/tilt inversion is applied to normalized inputs exactly as in the ordinary renderer,
    /// including motor aliases bound to the Pan/Tilt role by the compiled Position model;
    /// explicit raw values remain native. Profile channel inversion and function conversion
    /// then run once through the existing compiled resolution plan. Other render overlays and
    /// masters are excluded. This does not apply calibration or solve a Position intent.
    ///
    /// `target` must be the physical root or a profile-head owner of the root containing
    /// `instance_id`. Native destination lookup does not grant semantic emitter ownership.
    /// On error `out` is empty. This domain must not replace Color/Optics capture.
    pub fn native_position_raw_into(
        &self,
        capture: &PreparedOutputFrame,
        token: &CapturedFrameToken,
        target: FixtureId,
        instance_id: Uuid,
        out: &mut CapturedNativeRaw,
    ) -> Result<(), EngineError> {
        self.capture_native_raw_into(capture, token, target, Some(instance_id), out)
    }

    fn capture_native_raw_into(
        &self,
        capture: &PreparedOutputFrame,
        token: &CapturedFrameToken,
        target: FixtureId,
        instance_id: Option<Uuid>,
        out: &mut CapturedNativeRaw,
    ) -> Result<(), EngineError> {
        out.token = None;
        out.destination = None;
        out.instance_id = None;
        out.raw.clear();
        let invalid = |message: &str| EngineError::Invalid(message.into());
        if !Arc::ptr_eq(&self.capture_identity, &capture.identity) {
            return Err(EngineError::StalePreparedFrame);
        }
        if !token.matches_static_frame(self) || token.generation() != capture.generation() {
            return Err(invalid(
                "native raw values requested with another frame token",
            ));
        }
        let generation = &capture.generation;
        let (destination, fixture_index) = generation
            .profile_owner(target)
            .ok_or_else(|| invalid("native raw target has no profile destination"))?;
        let fixture = &generation.snapshot().fixtures[fixture_index];
        let inversion = match instance_id {
            None => crate::profile_projection::AxisInversion::default(),
            Some(id) if id == destination.0 => crate::profile_projection::AxisInversion {
                pan: fixture.invert_pan,
                tilt: fixture.invert_tilt,
            },
            Some(id) => {
                let copy = fixture
                    .multipatch
                    .iter()
                    .find(|copy| copy.id == id)
                    .ok_or_else(|| {
                        invalid("native Position instance does not belong to destination")
                    })?;
                crate::profile_projection::AxisInversion {
                    pan: copy.invert_pan,
                    tilt: copy.invert_tilt,
                }
            }
        };
        // Match the renderer: visual-only/non-DMX profiles do not apply patch inversion,
        // whereas an unpatched DMX fixture still does. Validate instance ownership above
        // regardless of whether the profile is currently patched or emits DMX.
        let inversion = if fixture
            .definition
            .profile_snapshot
            .as_deref()
            .is_some_and(|profile| profile.patch_policy == light_fixture::PatchPolicy::Dmx)
        {
            inversion
        } else {
            crate::profile_projection::AxisInversion::default()
        };
        let mode = crate::fixture::profile_mode(fixture)
            .ok_or_else(|| invalid("native raw destination has no profile mode"))?;
        let projection = generation
            .profile_projection(destination)
            .ok_or_else(|| invalid("native raw destination projection plan is missing"))?;
        let resolution = projection
            .resolution()
            .bind(mode)
            .map_err(|error| EngineError::Invalid(error.to_string()))?;
        let frame = self
            .resolved
            .frame
            .as_ref()
            .ok_or_else(|| invalid("prepared static resolution has no dense frame"))?;
        let values = ProfileValueIndex::Dense {
            frame,
            channels: generation.channel_slots(),
        };
        out.raw.resize(mode.channels.len(), 0);
        for head in projection.heads() {
            let read = values.head_read(head.owner);
            // Mirror the renderer's input transformation, including canonical aliases, role-bound
            // Position motor aliases of this head and selected channel functions. Reflecting a finished raw value would be incorrect
            // for explicit Raw inputs, channel inversion and nontrivial function ranges.
            let inverted = (inversion.pan || inversion.tilt).then(|| {
                let mut head_values = values.values(head.owner);
                crate::profile_projection::apply_axis_inversion(inversion, head, &mut head_values);
                head_values
            });
            for &channel_index in head.channel_indices.iter() {
                let resolved = resolution.resolve_channel_with(
                    channel_index,
                    |which, attribute| match &inverted {
                        Some(head_values) => head_values.get(attribute),
                        None => values.value_at(read, channel_index, which, attribute),
                    },
                    false,
                    None,
                    |_| ChannelScales::default(),
                );
                out.raw[channel_index] = resolved.raw;
            }
        }
        out.token = Some(token.clone());
        out.destination = Some(destination);
        out.instance_id = instance_id;
        Ok(())
    }
}
