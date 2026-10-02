//! TL-593 Media Color adapter: the composed semantic Color intent on a Media Server head,
//! translated by `light_fixture::media_color` into the personality's existing tint and White
//! Blend (Grayscale) controls, on the TL-590 captured-frame adapter seam.
//!
//! Media is not a lamp. Its White Blend is source desaturation with the RGB tint kept active, so
//! it never goes through the lamp fitter or the lamp White Blend envelope: the lamp
//! [`super::ColorAdapter`] declines all heads with Media-native color identities, including
//! unsupported wire personalities. This adapter accepts only the supported Media wire contract. The Media Server decodes the written bytes into `LayerState`/`MasterState`
//! and renders them with the existing TL-569 color stage (`MediaColor`, `layer.wgsl`).
//!
//! Per head and frame the adapter writes the complete footprint (cyan, magenta, yellow and, on a
//! layer, Grayscale) from the composed value alone. It reads no current native value and keeps no
//! continuity: the Media controls are a pure function of the request. Layer/master dimmer, alpha,
//! masters and blackout stay with Intensity and the single final render.
use super::*;
use light_core::programming::ColorIntent;
use light_engine::profile_head_destinations;
use light_fixture::PatchedFixture;
use light_fixture::media_color::{
    MediaColorControls, MediaColorHead, MediaColorLimitations, MediaColorSurface,
};
use std::cell::Cell;
use uuid::Uuid;

#[cfg(test)]
pub(in crate::runtime) mod tests;

/// Compiled Media color destination of one head.
#[derive(Clone, Debug)]
pub(in crate::runtime) struct MediaColorDescriptor {
    /// Root patched fixture whose mode channels the footprint addresses.
    pub destination: FixtureId,
    pub head_id: Uuid,
    pub head: MediaColorHead,
    /// Cyan, magenta, yellow, then Grayscale on a layer.
    pub footprint: Box<[NativeControlSlot]>,
}

/// Passive status of one Media head. Never a notification.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::runtime) struct MediaColorQuality {
    pub surface: MediaColorSurface,
    /// Derived controls before wire quantization; `achieved` is what the decoder reads.
    pub derived: MediaColorControls,
    pub limitations: MediaColorLimitations,
}

/// Cumulative adapter work since creation. Counters only.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::runtime) struct MediaColorAdapterCounters {
    pub descriptor_compiles: u64,
    /// Targets owning more than one Media color head (for example an unsplit root fixture).
    pub multi_head_targets: u64,
    pub resolves: u64,
}

#[derive(Default)]
pub(in crate::runtime) struct MediaColorAdapter {
    counters: Cell<MediaColorAdapterCounters>,
}

/// The Media color head of one patched fixture head, if that head is one.
pub(in crate::runtime) fn media_color_head(
    fixture: &PatchedFixture,
    head_id: Uuid,
) -> Option<MediaColorHead> {
    let profile = fixture.definition.profile_snapshot.as_deref()?;
    let mode_id = fixture.definition.mode_id?;
    let mode = profile.modes.iter().find(|mode| mode.id == mode_id)?;
    MediaColorHead::from_mode(mode, head_id)
}

/// Media-native identities reserve the Color owner even for an unsupported Media personality.
pub(in crate::runtime) fn has_media_color_identity(
    fixture: &PatchedFixture,
    head_id: Uuid,
) -> bool {
    let Some(profile) = fixture.definition.profile_snapshot.as_deref() else {
        return false;
    };
    let Some(mode_id) = fixture.definition.mode_id else {
        return false;
    };
    profile
        .modes
        .iter()
        .find(|mode| mode.id == mode_id)
        .is_some_and(|mode| light_fixture::media_color::has_media_color_identity(mode, head_id))
}

impl MediaColorAdapter {
    pub fn counters(&self) -> MediaColorAdapterCounters {
        self.counters.get()
    }

    fn count(&self, update: impl FnOnce(&mut MediaColorAdapterCounters)) {
        let mut counters = self.counters.get();
        update(&mut counters);
        self.counters.set(counters);
    }
}

impl PhysicalFamilyAdapter for MediaColorAdapter {
    type Descriptor = MediaColorDescriptor;
    type Continuity = ();
    type Requested = ColorIntent;
    /// What the Media decoder reads from the written raws.
    type Achieved = MediaColorControls;
    type Quality = MediaColorQuality;

    fn owns(&self, owner: ProgrammingOwner) -> bool {
        owner == ProgrammingOwner::Color
    }

    fn compile(
        &self,
        snapshot: &EngineSnapshot,
        target: FixtureId,
    ) -> Result<Option<MediaColorDescriptor>, TransitionError> {
        self.count(|c| c.descriptor_compiles += 1);
        let mut found = None;
        for head in profile_head_destinations(snapshot, target) {
            let fixture = &snapshot.fixtures[head.fixture_index];
            let Some(media) = media_color_head(fixture, head.head_id) else {
                continue;
            };
            if found.is_some() {
                self.count(|c| c.multi_head_targets += 1);
                return Ok(None);
            }
            found = Some((head, media));
        }
        let Some((head, media)) = found else {
            return Ok(None);
        };
        let footprint = media
            .controls()
            .iter()
            .map(|control| NativeControlSlot {
                destination: head.destination,
                channel_index: control.channel_index,
                split: control.split,
            })
            .collect();
        Ok(Some(MediaColorDescriptor {
            destination: head.destination,
            head_id: head.head_id,
            head: media,
            footprint,
        }))
    }

    fn footprint<'d>(&self, descriptor: &'d MediaColorDescriptor) -> &'d [NativeControlSlot] {
        &descriptor.footprint
    }

    fn resolve(
        &self,
        request: PhysicalRequest<'_, Self>,
    ) -> Result<PhysicalResolution<Self>, TransitionError> {
        let intent = super::color::semantic_intent(request.value)?;
        let descriptor = request.descriptor;
        let resolved = descriptor.head.resolve(intent)?;
        self.count(|c| c.resolves += 1);
        let writes = descriptor
            .head
            .controls()
            .iter()
            .zip(descriptor.footprint.iter())
            .zip(&resolved.raws)
            .map(|((control, slot), raw)| NativeControlWrite {
                slot: *slot,
                channel_id: control.channel_id,
                function_id: None,
                raw: *raw,
                parked: false,
            })
            .collect();
        Ok(PhysicalResolution {
            writes,
            requested: intent.clone(),
            achieved: resolved.achieved,
            quality: MediaColorQuality {
                surface: descriptor.head.surface,
                derived: resolved.requested,
                limitations: resolved.limitations,
            },
            continuity: (),
        })
    }
}
