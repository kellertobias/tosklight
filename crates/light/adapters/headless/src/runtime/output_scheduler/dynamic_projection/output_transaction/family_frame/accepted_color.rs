//! TL-594 track B: the Color results of accepted Live family frames, for the colour report.
//!
//! When the family adapters are engaged, every accepted Live frame records the encoded
//! quality of each Color sidecar it published, keyed by the frame's runtime generation and
//! sample instant. The colour report then names the heads of the *published* output frame from
//! these records, instead of re-resolving the current target (or an invented D65 white) through
//! `mode.resolve_intent`. A head without an active requested colour has no sidecar, and so no
//! record and no row: nothing is reported against an invented white.
//!
//! Bounded: a ring of the last [`RETAINED_FRAMES`] accepted frames, so the frame the output
//! publication hub currently holds (or a displayed-source lease names) is still found while
//! later frames are being accepted.
//!
//! TL-554: each lamp Color sidecar's published output (token, composed value and every native
//! write) is moved here too, so the first native edit can seed exactly the premaster output of
//! the frame the operator saw, and each head's passive Direct replay status is kept for the
//! report. The sidecars are moved, not cloned: the frame no longer needs them.
use super::super::super::physical_adapter::NativeControlWrite;
use super::super::super::physical_adapter::color::{ColorQuality, DirectColorStatus};
use super::super::super::physical_adapter::color_router::RoutedColorQuality;
use super::super::super::physical_adapter::family_lanes::FamilySidecar;
use chrono::{DateTime, Utc};
use light_core::{AttributeValue, ColorResolutionQuality, FixtureId};
use light_engine::CapturedFrameToken;
use light_fixture::PhysicalDataQuality;
use light_fixture::forward::{ColorMatch, UvFitStatus};
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::Arc;
use uuid::Uuid;

/// Accepted frames kept for lookup by the published frame identity.
pub(in crate::runtime) const RETAINED_FRAMES: usize = 32;

/// One head's encoded Color result in an accepted frame.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::runtime) struct AcceptedColorHead {
    /// The sidecar's programmed owner (the fixture or one of its logical heads).
    pub target: FixtureId,
    /// The destination head's identity, when the sidecar reports per-head outcomes.
    pub destination: FixtureId,
    pub head_id: Option<Uuid>,
    pub quality: ColorResolutionQuality,
    pub delta_uv: Option<f32>,
    /// TL-550: the lamp fitter's UV result, kept apart from the visible match. `None` for Media.
    pub uv: Option<AcceptedColorUv>,
    /// TL-554: the head's Direct replay status (Direct values only).
    pub direct: Option<DirectColorStatus>,
}

/// TL-554: one lamp Color sidecar's published output, exactly as the frame wrote it.
#[derive(Clone, Debug)]
pub(in crate::runtime) struct PublishedColorOutput {
    pub token: CapturedFrameToken,
    pub target: FixtureId,
    /// The composed family value the frame resolved.
    pub value: AttributeValue,
    /// Every native write of the target's complete footprint (premaster raw values).
    pub writes: Vec<NativeControlWrite>,
}

/// One lamp head's UV result: the fitter's status and whether the drive range limited it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::runtime) struct AcceptedColorUv {
    pub status: UvFitStatus,
    pub clipped: bool,
}

fn lamp_uv(quality: &ColorQuality) -> Option<AcceptedColorUv> {
    Some(AcceptedColorUv {
        status: quality.uv,
        clipped: quality.uv_clipped,
    })
}

/// The Color results of one accepted Live frame.
#[derive(Debug)]
pub(in crate::runtime) struct AcceptedColorFrame {
    pub generation: u64,
    pub sampled_at: DateTime<Utc>,
    pub heads: Vec<AcceptedColorHead>,
    /// TL-554: the lamp Color outputs of this frame, one per programming target.
    pub outputs: Vec<PublishedColorOutput>,
    /// TL-552: Color targets this frame composed but held without a sidecar (for example a
    /// fixture whose colour has no model). The report names them with the reason.
    pub held: Vec<FixtureId>,
}

impl AcceptedColorFrame {
    /// TL-552: this frame held a requested colour of `fixture` or `owner` without a sidecar.
    pub(in crate::runtime) fn held(&self, fixture: FixtureId, owner: FixtureId) -> bool {
        self.held
            .iter()
            .any(|target| *target == owner || *target == fixture)
    }

    /// TL-554: the published output of one programming target in this frame.
    pub(in crate::runtime) fn output(&self, target: FixtureId) -> Option<&PublishedColorOutput> {
        self.outputs.iter().find(|output| output.target == target)
    }

    /// The encoded result for a report head: its own per-head outcome first, then a sidecar
    /// whose owner is the head's owner or its fixture.
    pub(in crate::runtime) fn head(
        &self,
        fixture: FixtureId,
        owner: FixtureId,
        head_id: Uuid,
    ) -> Option<&AcceptedColorHead> {
        let names = |id: FixtureId| id == owner || id == fixture;
        self.heads
            .iter()
            .find(|row| row.head_id == Some(head_id) && names(row.destination))
            .or_else(|| {
                self.heads
                    .iter()
                    .find(|row| row.head_id.is_none() && names(row.target))
            })
    }
}

/// The lamp fitter's passive status as the operator-facing report quality. Measured data is
/// required for `Exact`; an exact match against nominal data is `Approximate` (the legacy
/// report's definition). Without known data provenance (an uncalibrated profile resolved from
/// its channel names, or an unknown contribution) every result is `Uncalibrated`, so an
/// estimate never reads as a precise score. A head that shows colour only through discrete
/// wheel slots reports an inexact known match as `WheelLimited`. An unknown match with known
/// data is `Unsupported`.
pub(in crate::runtime) fn lamp_quality(quality: &ColorQuality) -> ColorResolutionQuality {
    match quality.color_match {
        _ if quality.data_quality == PhysicalDataQuality::Unknown => {
            ColorResolutionQuality::Uncalibrated
        }
        ColorMatch::Approximate | ColorMatch::OutOfGamut if quality.discrete => {
            ColorResolutionQuality::WheelLimited
        }
        ColorMatch::Exact if quality.nominal => ColorResolutionQuality::Approximate,
        ColorMatch::Exact => ColorResolutionQuality::Exact,
        ColorMatch::Approximate => ColorResolutionQuality::Approximate,
        ColorMatch::OutOfGamut => ColorResolutionQuality::OutOfGamut,
        ColorMatch::Unknown => ColorResolutionQuality::Unsupported,
    }
}

/// The visible distance is reported only against known data: an uncalibrated estimate never
/// shows a precise-looking Δu′v′.
fn lamp_delta_uv(quality: &ColorQuality) -> Option<f32> {
    (lamp_quality(quality) != ColorResolutionQuality::Uncalibrated)
        .then_some(quality.delta_uv)
        .flatten()
        .map(|delta| delta as f32)
}

fn rows(sidecar: &FamilySidecar) -> Vec<AcceptedColorHead> {
    let Some(color) = sidecar.color() else {
        return Vec::new();
    };
    let target = color.target;
    match &color.quality {
        RoutedColorQuality::Lamp(quality) if !quality.heads.is_empty() => quality
            .heads
            .iter()
            .map(|head| AcceptedColorHead {
                target,
                destination: head.destination,
                head_id: Some(head.head_id),
                quality: lamp_quality(&head.quality),
                delta_uv: lamp_delta_uv(&head.quality),
                uv: lamp_uv(&head.quality),
                direct: head.quality.direct.clone(),
            })
            .collect(),
        RoutedColorQuality::Lamp(quality) => vec![AcceptedColorHead {
            target,
            destination: target,
            head_id: None,
            quality: lamp_quality(quality),
            delta_uv: lamp_delta_uv(quality),
            uv: lamp_uv(quality),
            direct: quality.direct.clone(),
        }],
        RoutedColorQuality::Media(quality) => vec![AcceptedColorHead {
            target,
            destination: target,
            head_id: None,
            quality: if quality.limitations.tint_gamut_mapped {
                ColorResolutionQuality::OutOfGamut
            } else {
                ColorResolutionQuality::Exact
            },
            delta_uv: None,
            uv: None,
            direct: None,
        }],
    }
}

/// The bounded ring of accepted frames' Color results.
#[derive(Default)]
pub(in crate::runtime) struct AcceptedColorFrames {
    frames: Mutex<VecDeque<Arc<AcceptedColorFrame>>>,
}

impl AcceptedColorFrames {
    /// Record the Color sidecars of one accepted frame. Called only after every lane accepted.
    pub(in crate::runtime) fn record(
        &self,
        token: &CapturedFrameToken,
        results: Vec<FamilySidecar>,
        held: Vec<FixtureId>,
    ) {
        let heads = results.iter().flat_map(rows).collect();
        let outputs = results
            .into_iter()
            .filter_map(|sidecar| match sidecar {
                FamilySidecar::Color(color)
                    if matches!(color.quality, RoutedColorQuality::Lamp(_)) =>
                {
                    Some(PublishedColorOutput {
                        token: color.token,
                        target: color.target,
                        value: color.value,
                        writes: color.writes,
                    })
                }
                _ => None,
            })
            .collect();
        let frame = Arc::new(AcceptedColorFrame {
            generation: token.generation(),
            sampled_at: token.sampled_at(),
            heads,
            outputs,
            held,
        });
        let mut frames = self.frames.lock();
        if frames.len() == RETAINED_FRAMES {
            frames.pop_front();
        }
        frames.push_back(frame);
    }

    /// The accepted frame with exactly this identity, if it is still retained.
    pub(in crate::runtime) fn find(
        &self,
        generation: u64,
        sampled_at: DateTime<Utc>,
    ) -> Option<Arc<AcceptedColorFrame>> {
        self.frames
            .lock()
            .iter()
            .rev()
            .find(|frame| frame.generation == generation && frame.sampled_at == sampled_at)
            .cloned()
    }

    /// The newest retained frame (TL-554 latest-accepted adoption without a displayed source).
    pub(in crate::runtime) fn latest(&self) -> Option<Arc<AcceptedColorFrame>> {
        self.frames.lock().back().cloned()
    }

    /// Show activation: nothing of the previous show's frames stays readable.
    pub(in crate::runtime) fn clear(&self) {
        self.frames.lock().clear();
    }
}
