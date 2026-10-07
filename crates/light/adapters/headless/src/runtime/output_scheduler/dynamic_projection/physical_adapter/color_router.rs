//! TL-612 Color routing adapter: one `PhysicalFamilyAdapter` for the Color owner that sends
//! each programming target to exactly one of the two existing Color adapters, so a single outer
//! [`PhysicalAdapterLane`] (Live) or one ordinary [`PhysicalPreloadLanes`] pair can carry lamps
//! and Media Server heads together.
//!
//! This is a router, not a second Color solver:
//! - Lamp targets delegate every call unchanged to [`ColorAdapter`] (TL-592/TL-557/TL-559):
//!   calibration, White Blend envelope, fitting, Direct replay, adoption and transitions.
//! - Media targets delegate every call unchanged to [`MediaColorAdapter`] (TL-593): tint plus
//!   White Blend as source desaturation (Grayscale); never lamp calibration or the lamp White
//!   Blend envelope. Temperature/Duv/UV stay the Media adapter's passive limitations.
//!
//! Classification reads only the captured snapshot handed to `compile` (the lane compiles once
//! per captured runtime generation): `profile_head_destinations` and the immutable profile's
//! reserved Media identities (`has_media_color_identity`). It never looks at displayed color,
//! live globals or another lane.
//! - No head of the target carries a Media identity: [`ColorRoute::Lamp`].
//! - A Media identity reserves the target for Media. It must be exactly one profile head on one
//!   root destination without multipatch copies; that head is delegated to
//!   `MediaColorAdapter::compile`. An unsupported Media personality, an unsplit/multi-head root,
//!   a mixed lamp+Media target or a reserved head with copies is passive (`Ok(None)`, the lane
//!   reports the existing Color requirement). Nothing reserved ever falls through to lamp
//!   fitting, and no supported subset is accepted while another reserved head is dropped.
//! - A target spanning several root fixtures stays the existing corruption error.
//!
//! Continuity is tagged by route. The outer lane keeps it per `(target, owner)` and passes the
//! last accepted value back; a value of the other route (fixture/profile replacement) is
//! discarded here so the delegate starts from the actual captured scalar Current. The router
//! accepts no continuity itself, creates no child lane, fits once per head, never touches
//! intensity/master/blackout and keeps no descriptor cache (the lane's generation cache is the
//! only authority).
//!
//! Production runs the semantic programming contract (contract 1, TL-552) and installs this
//! adapter on the Live and retained Preload family lanes (TL-548 C3). Physical/cadence acceptance
//! remains with TL-523 and TL-553.
use super::color::{
    AchievedColor, ColorAdapter, ColorContinuity, ColorDescriptor, ColorQuality, ColorRequest,
};
use super::media_color::{
    MediaColorAdapter, MediaColorDescriptor, MediaColorQuality, has_media_color_identity,
};
use super::*;
use light_core::programming::ColorIntent;
use light_engine::profile_head_destinations;
use light_fixture::media_color::MediaColorControls;

#[cfg(test)]
mod tests;

/// Which existing Color adapter owns a target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) enum ColorRoute {
    Lamp,
    Media,
}

/// The delegate's own compiled descriptor, owned unchanged (shared fitters stay `Arc`s inside).
pub(in crate::runtime) enum RoutedColorDescriptor {
    Lamp(ColorDescriptor),
    Media(MediaColorDescriptor),
}

impl RoutedColorDescriptor {
    pub fn route(&self) -> ColorRoute {
        match self {
            Self::Lamp(_) => ColorRoute::Lamp,
            Self::Media(_) => ColorRoute::Media,
        }
    }

    pub fn lamp(&self) -> Option<&ColorDescriptor> {
        match self {
            Self::Lamp(descriptor) => Some(descriptor),
            Self::Media(_) => None,
        }
    }

    pub fn media(&self) -> Option<&MediaColorDescriptor> {
        match self {
            Self::Media(descriptor) => Some(descriptor),
            Self::Lamp(_) => None,
        }
    }
}

/// Lane-owned continuity, tagged with the route that produced it.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::runtime) enum RoutedColorContinuity {
    Lamp(ColorContinuity),
    /// The Media adapter keeps no continuity; the tag only records the route.
    Media(()),
}

impl RoutedColorContinuity {
    pub fn route(&self) -> ColorRoute {
        match self {
            Self::Lamp(_) => ColorRoute::Lamp,
            Self::Media(()) => ColorRoute::Media,
        }
    }
}

/// The delegate's requested value, unchanged (the composed request; never fitted output).
#[derive(Clone, Debug, PartialEq)]
pub(in crate::runtime) enum RoutedColorRequest {
    Lamp(ColorRequest),
    Media(ColorIntent),
}

impl RoutedColorRequest {
    /// The semantic intent, for either route; None for a lamp Direct program.
    pub fn semantic(&self) -> Option<&ColorIntent> {
        match self {
            Self::Lamp(request) => request.semantic(),
            Self::Media(intent) => Some(intent),
        }
    }
}

/// The delegate's achieved value: lamp forward evaluation or the controls the Media decoder reads.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::runtime) enum RoutedAchievedColor {
    Lamp(AchievedColor),
    Media(MediaColorControls),
}

/// The delegate's passive quality. Never a notification. Unboxed on purpose: the lamp quality
/// is moved once per head and frame, exactly as the lamp adapter publishes it.
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub(in crate::runtime) enum RoutedColorQuality {
    Lamp(ColorQuality),
    Media(MediaColorQuality),
}

/// Cumulative routing decisions since creation. Counters only; the delegates keep their own.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::runtime) struct RoutingColorAdapterCounters {
    pub lamp_routes: u64,
    pub media_routes: u64,
    /// Reserved Media targets kept passive: unsupported personality, mixed lamp+Media, several
    /// heads or multipatch copies.
    pub reserved_passive: u64,
    /// Lane continuity of the other route handed back after a replacement, dropped here.
    pub discarded_continuity: u64,
}

super::counters::counter_sum!(RoutingColorAdapterCounters {
    lamp_routes,
    media_routes,
    reserved_passive,
    discarded_continuity,
});

/// Routes the Color owner per target between the existing lamp and Media adapters.
#[derive(Default)]
pub(in crate::runtime) struct RoutingColorAdapter {
    lamp: ColorAdapter,
    media: MediaColorAdapter,
    /// Shared by parallel frame workers (TL-639 round 5).
    counters: super::counters::ShardedCounters<RoutingColorAdapterCounters>,
}

/// Route decision for one target of one captured snapshot.
enum Classification {
    Lamp,
    Media,
    Passive,
}

impl RoutingColorAdapter {
    pub fn lamp(&self) -> &ColorAdapter {
        &self.lamp
    }

    pub fn media(&self) -> &MediaColorAdapter {
        &self.media
    }

    pub fn counters(&self) -> RoutingColorAdapterCounters {
        self.counters.total()
    }

    fn count(&self, update: impl FnOnce(&mut RoutingColorAdapterCounters)) {
        self.counters.update(update);
    }

    /// Classify from the captured profile only.
    fn classify(
        snapshot: &EngineSnapshot,
        target: FixtureId,
    ) -> Result<Classification, TransitionError> {
        let heads = profile_head_destinations(snapshot, target);
        let reserved = heads.iter().any(|head| {
            has_media_color_identity(&snapshot.fixtures[head.fixture_index], head.head_id)
        });
        if !reserved {
            return Ok(Classification::Lamp);
        }
        if heads
            .iter()
            .any(|head| head.fixture_index != heads[0].fixture_index)
        {
            return Err(super::color::invalid(
                "one Color target spans several root fixtures",
            ));
        }
        let [head] = heads.as_slice() else {
            return Ok(Classification::Passive);
        };
        if !snapshot.fixtures[head.fixture_index].multipatch.is_empty() {
            return Ok(Classification::Passive);
        }
        Ok(Classification::Media)
    }

    fn lamp_previous<'a>(
        &self,
        previous: Option<&'a RoutedColorContinuity>,
    ) -> Option<&'a ColorContinuity> {
        match previous? {
            RoutedColorContinuity::Lamp(continuity) => Some(continuity),
            RoutedColorContinuity::Media(()) => {
                self.count(|c| c.discarded_continuity += 1);
                None
            }
        }
    }

    fn media_previous(&self, previous: Option<&RoutedColorContinuity>) -> Option<&'static ()> {
        match previous? {
            RoutedColorContinuity::Media(()) => Some(&()),
            RoutedColorContinuity::Lamp(_) => {
                self.count(|c| c.discarded_continuity += 1);
                None
            }
        }
    }
}

impl PhysicalFamilyAdapter for RoutingColorAdapter {
    type Descriptor = RoutedColorDescriptor;
    type Continuity = RoutedColorContinuity;
    type Requested = RoutedColorRequest;
    type Achieved = RoutedAchievedColor;
    type Quality = RoutedColorQuality;

    fn owns(&self, owner: ProgrammingOwner) -> bool {
        owner == ProgrammingOwner::Color
    }

    fn begin_lane_frame(&self, token: &CapturedFrameToken) -> Result<(), TransitionError> {
        self.lamp.begin_lane_frame(token)?;
        self.media.begin_lane_frame(token)
    }

    fn abandon_lane_frame(&self) {
        self.lamp.abandon_lane_frame();
        self.media.abandon_lane_frame();
    }

    fn verify_lane_frame(&self, token: &CapturedFrameToken) -> Result<(), TransitionError> {
        self.lamp.verify_lane_frame(token)?;
        self.media.verify_lane_frame(token)
    }

    fn accept_lane_frame(&self, token: &CapturedFrameToken) {
        self.lamp.accept_lane_frame(token);
        self.media.accept_lane_frame(token);
    }

    fn compile(
        &self,
        snapshot: &EngineSnapshot,
        target: FixtureId,
    ) -> Result<Option<RoutedColorDescriptor>, TransitionError> {
        match Self::classify(snapshot, target)? {
            Classification::Lamp => {
                let compiled = self.lamp.compile(snapshot, target)?;
                if compiled.is_some() {
                    self.count(|c| c.lamp_routes += 1);
                }
                Ok(compiled.map(RoutedColorDescriptor::Lamp))
            }
            Classification::Media => {
                let compiled = self.media.compile(snapshot, target)?;
                if compiled.is_some() {
                    self.count(|c| c.media_routes += 1);
                } else {
                    self.count(|c| c.reserved_passive += 1);
                }
                Ok(compiled.map(RoutedColorDescriptor::Media))
            }
            Classification::Passive => {
                self.count(|c| c.reserved_passive += 1);
                Ok(None)
            }
        }
    }

    fn footprint<'d>(&self, descriptor: &'d RoutedColorDescriptor) -> &'d [NativeControlSlot] {
        match descriptor {
            RoutedColorDescriptor::Lamp(descriptor) => self.lamp.footprint(descriptor),
            RoutedColorDescriptor::Media(descriptor) => self.media.footprint(descriptor),
        }
    }

    fn resolve(
        &self,
        request: PhysicalRequest<'_, Self>,
    ) -> Result<PhysicalResolution<Self>, TransitionError> {
        let PhysicalRequest {
            frame,
            target,
            owner,
            descriptor,
            value,
            previous,
        } = request;
        match descriptor {
            RoutedColorDescriptor::Lamp(descriptor) => {
                let resolved = self.lamp.resolve(PhysicalRequest {
                    frame,
                    target,
                    owner,
                    descriptor,
                    value,
                    previous: self.lamp_previous(previous),
                })?;
                Ok(PhysicalResolution {
                    writes: resolved.writes,
                    requested: RoutedColorRequest::Lamp(resolved.requested),
                    achieved: RoutedAchievedColor::Lamp(resolved.achieved),
                    quality: RoutedColorQuality::Lamp(resolved.quality),
                    continuity: RoutedColorContinuity::Lamp(resolved.continuity),
                })
            }
            RoutedColorDescriptor::Media(descriptor) => {
                let resolved = self.media.resolve(PhysicalRequest {
                    frame,
                    target,
                    owner,
                    descriptor,
                    value,
                    previous: self.media_previous(previous),
                })?;
                Ok(PhysicalResolution {
                    writes: resolved.writes,
                    requested: RoutedColorRequest::Media(resolved.requested),
                    achieved: RoutedAchievedColor::Media(resolved.achieved),
                    quality: RoutedColorQuality::Media(resolved.quality),
                    continuity: {
                        let () = resolved.continuity;
                        RoutedColorContinuity::Media(())
                    },
                })
            }
        }
    }

    fn adopt(
        &self,
        frame: HybridFrameContext<'_>,
        descriptor: &RoutedColorDescriptor,
        target: FixtureId,
        original: &AttributeValue,
        address: &DynamicValueAddress,
    ) -> Result<AttributeValue, TransitionError> {
        match descriptor {
            RoutedColorDescriptor::Lamp(descriptor) => self
                .lamp
                .adopt(frame, descriptor, target, original, address),
            RoutedColorDescriptor::Media(descriptor) => self
                .media
                .adopt(frame, descriptor, target, original, address),
        }
    }

    fn adopt_with_continuity(
        &self,
        frame: HybridFrameContext<'_>,
        descriptor: &RoutedColorDescriptor,
        target: FixtureId,
        original: &AttributeValue,
        address: &DynamicValueAddress,
        previous: Option<&RoutedColorContinuity>,
    ) -> Result<AttributeValue, TransitionError> {
        match descriptor {
            RoutedColorDescriptor::Lamp(descriptor) => self.lamp.adopt_with_continuity(
                frame,
                descriptor,
                target,
                original,
                address,
                self.lamp_previous(previous),
            ),
            RoutedColorDescriptor::Media(descriptor) => self.media.adopt_with_continuity(
                frame,
                descriptor,
                target,
                original,
                address,
                self.media_previous(previous),
            ),
        }
    }

    fn transition(
        &self,
        frame: HybridFrameContext<'_>,
        descriptor: &RoutedColorDescriptor,
        target: FixtureId,
        requirement: TransitionRequirement,
        from: &AttributeValue,
        to: &AttributeValue,
        operation: FamilyExpressionOperation,
    ) -> Result<(AttributeValue, Option<ProgrammingTransitionTrace>), TransitionError> {
        match descriptor {
            RoutedColorDescriptor::Lamp(descriptor) => {
                self.lamp
                    .transition(frame, descriptor, target, requirement, from, to, operation)
            }
            RoutedColorDescriptor::Media(descriptor) => {
                self.media
                    .transition(frame, descriptor, target, requirement, from, to, operation)
            }
        }
    }

    /// Neither delegate overrides the field/metadata hooks (both use the trait defaults), and
    /// the lane calls them without a descriptor, so the lamp delegate's answer is the Media one.
    fn consumed_fields(
        &self,
        owner: ProgrammingOwner,
        value: &AttributeValue,
    ) -> Result<ProgrammingFieldScope, TransitionError> {
        self.lamp.consumed_fields(owner, value)
    }

    fn projection_metadata(
        &self,
        owner: ProgrammingOwner,
        provenance: &PhysicalProvenance,
    ) -> FamilyProjectionMetadata {
        self.lamp.projection_metadata(owner, provenance)
    }
}
