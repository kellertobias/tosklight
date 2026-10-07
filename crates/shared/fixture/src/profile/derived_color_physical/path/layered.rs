//! A hue/saturation engine layered over direct emitters behind an activation gate (ETC Source
//! Four LED "HSI Plus 7" / "HSIC Plus 7").
//!
//! ETC documents the seven native channels only as fine-tuning the HSI mix while the Plus Seven
//! control is above 51%; how the two engines combine is not published. The derived model takes
//! the honest-but-narrow reading that can be fitted and checked:
//!
//! - the direct emitters are the continuous engine (uncalibrated, named from their channels);
//! - the activation gate is parked **on** (its "activated" range), so the direct channels act;
//! - the HSI engine is parked at saturation 0 (no tint; hue held at its default, where it has no
//!   effect) and the Intensity channel stays Intensity's.
//!
//! Every parked state carries [`DERIVED_LAYERED_SOURCE`] with Unknown quality. Any other gate,
//! hue or saturation position is unmodelled, so the forward prediction becomes unknown.
use super::super::super::{
    ChannelFunctionBehavior, ColorCalibrationStatus, ColorSystem, ColorSystemCalibration,
    EmitterBinding, FixtureMode, HeadColorSystem, OpticalProvenance, PhysicalDataQuality,
};
use super::super::DERIVED_LAYERED_SOURCE;
use super::{PathBuilder, continuous_function, neutral_state, normalized_raw};
use crate::srgb_to_xyz;
use uuid::Uuid;

/// Semantic id of the function that activates the direct channels (ETC "Plus Seven activated").
const GATE_ON: &str = "plus_seven_on";
/// Fewer direct emitters than this are not a colour engine of their own.
const MIN_DIRECT_EMITTERS: usize = 3;

pub(super) struct LayeredEngine {
    /// The direct emitters as one uncalibrated additive system.
    pub direct: HeadColorSystem,
    pub hue: Uuid,
    pub saturation: Uuid,
    /// The non-colour channel whose "activated" range lets the direct channels act.
    pub gate: Uuid,
}

impl LayeredEngine {
    /// The layered reading of `system` on `head`, when the head has an activation gate and at
    /// least three direct visible emitters outside the hue/saturation engine.
    pub fn find(mode: &FixtureMode, head: Uuid, system: &HeadColorSystem) -> Option<Self> {
        let ColorSystem::HueSaturation {
            hue_channel_id,
            saturation_channel_id,
            ..
        } = system.system
        else {
            return None;
        };
        let gate = mode
            .channels
            .iter()
            .filter(|c| c.head_id == head)
            .find(|c| c.functions.iter().any(is_gate_on))?
            .id;
        // The direct emitters as the inferred system would read them without the authored HSI.
        let mut probe = mode.clone();
        probe.color_systems.retain(|s| s.head_id != head);
        let mut emitters: Vec<EmitterBinding> = probe
            .intent_color_systems(head)
            .iter()
            .filter_map(|s| match &s.system {
                ColorSystem::Additive { emitters } => Some(emitters.clone()),
                _ => None,
            })
            .flatten()
            .filter(|e| e.channel_id != hue_channel_id && e.channel_id != saturation_channel_id)
            .collect();
        // A native Cyan emitter that a legacy alias drives through the Red attribute.
        let cyan: Vec<_> = mode
            .channels
            .iter()
            .filter(|c| {
                c.head_id == head
                    && &*c.fixture_attribute.0 == "color.cyan"
                    && !emitters.iter().any(|e| e.channel_id == c.id)
                    && continuous_function(c).is_some()
            })
            .collect();
        for channel in cyan {
            emitters.push(EmitterBinding {
                channel_id: channel.id,
                name: "Cyan".into(),
                xyz: srgb_to_xyz(0.0, 1.0, 1.0),
                maximum_level: 1.0,
                response_curve: 1.0,
                visible: true,
            });
        }
        if emitters.iter().filter(|e| e.visible).count() < MIN_DIRECT_EMITTERS {
            return None;
        }
        Some(Self {
            direct: HeadColorSystem {
                head_id: head,
                correction_matrix: super::super::super::color_model::identity_color_correction(),
                system: ColorSystem::Additive { emitters },
                calibration: ColorSystemCalibration {
                    status: ColorCalibrationStatus::Uncalibrated,
                    revision: 0,
                    source: None,
                },
            },
            hue: hue_channel_id,
            saturation: saturation_channel_id,
            gate,
        })
    }
}

fn is_gate_on(function: &super::super::super::ChannelFunction) -> bool {
    matches!(
        &function.behavior,
        ChannelFunctionBehavior::Fixed { semantic_id, .. } if semantic_id == GATE_ON
    )
}

impl PathBuilder<'_> {
    /// Park the layered hue/saturation engine at saturation 0 (hue at its default) and hold the
    /// activation gate on.
    pub(super) fn park_layered(&mut self, layered: &LayeredEngine) -> Result<(), String> {
        let provenance = OpticalProvenance {
            quality: PhysicalDataQuality::Unknown,
            source: Some(DERIVED_LAYERED_SOURCE.into()),
            revision: 0,
        };
        let missing = |what: &str| format!("the layered engine's {what} has no state to park");
        let hue = self.channel(layered.hue)?;
        let (function, from, to, _) = neutral_state(hue).ok_or_else(|| missing("hue"))?;
        self.park_state(hue, function, (from, to), provenance.clone())?;

        let saturation = self.channel(layered.saturation)?;
        let raw = normalized_raw(saturation, 0.0);
        let function = saturation
            .functions
            .iter()
            .find(|f| {
                matches!(f.behavior, ChannelFunctionBehavior::Continuous { .. })
                    && (f.dmx_from..=f.dmx_to).contains(&raw)
            })
            .ok_or_else(|| missing("saturation"))?;
        self.park_state(saturation, function, (raw, raw), provenance.clone())?;

        let gate = self.channel(layered.gate)?;
        let function = gate
            .functions
            .iter()
            .find(|f| is_gate_on(f))
            .ok_or_else(|| missing("activation gate"))?;
        self.park_state(
            gate,
            function,
            (function.dmx_from, function.dmx_to),
            provenance,
        )
    }
}
