//! One head's derived optical path: its engine, its parked colour controls and observations.
use super::super::color_model::SEMANTIC_WHITE_XYZ;
use super::super::{
    AngularMotionKind, CanonicalTransform, ChannelFunction, ChannelFunctionBehavior,
    ColorCalibrationStatus, ColorRecipeMeasurement, ColorSystem, ColorSystemCalibration,
    FilterSpectrum, FixtureChannel, FixtureMode, HeadColorSystem, HeadOpticalPath,
    NativeColorBinding, NativeColorValue, OpticalEmitter, OpticalEmitterBand, OpticalFilter,
    OpticalProvenance, OpticalSource, OpticalTransmission, PhysicalDataQuality, SpectrumSample,
    SubtractiveCalibration, native_color_function_allowed,
};
use super::{
    DERIVED_HUE_SATURATION_SOURCE, DERIVED_NOMINAL_SOURCE, DERIVED_PARKED_SOURCE,
    DERIVED_UNCALIBRATED_SOURCE,
};
use crate::srgb_to_xyz;
use light_core::Xyz;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use uuid::Uuid;

const DERIVED_MEASURED_SOURCE: &str = "Measured colour system of the profile";
/// Hue steps of the nominal hue/saturation grid (3°) and its saturation rings.
const HUE_STEPS: u32 = 120;
const SATURATIONS: [f32; 7] = [1.0, 0.85, 0.7, 0.55, 0.4, 0.25, 0.12];
/// Colour names whose emitter role is unambiguous.
const COLOUR_NAMES: [&str; 11] = [
    "red", "green", "blue", "white", "amber", "lime", "indigo", "cyan", "magenta", "yellow", "uv",
];

fn is_color(attribute: &str) -> bool {
    attribute == "color" || attribute.starts_with("color.")
}

pub(super) fn is_color_channel(channel: &FixtureChannel) -> bool {
    is_color(&channel.fixture_attribute.0)
        || is_color(&channel.attribute.0)
        || channel.functions.iter().any(|f| is_color(&f.attribute.0))
}

/// The head brings continuously driven light of its own (visible emitters or CMY flags).
pub(super) fn has_continuous_engine(mode: &FixtureMode, head: Uuid) -> bool {
    mode.intent_color_systems(head)
        .iter()
        .any(|system| match &system.system {
            ColorSystem::Additive { emitters } => emitters.iter().any(|e| e.visible),
            ColorSystem::Subtractive { .. } => true,
            _ => false,
        })
}

fn derived_id(tag: &str, first: Uuid, second: Uuid) -> Uuid {
    let digest = Sha256::digest(
        [
            b"tosklight.derived-color-physical.v1:".as_slice(),
            tag.as_bytes(),
            first.as_bytes(),
            second.as_bytes(),
        ]
        .concat(),
    );
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    // RFC 4122 variant, version 8 (vendor-specific): stable and never nil.
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

/// Provenance of derived data, by the colour system's declared calibration.
fn provenance(calibration: &ColorSystemCalibration, inferred: bool) -> OpticalProvenance {
    let (quality, source) = match calibration.status {
        _ if inferred => (
            PhysicalDataQuality::Unknown,
            DERIVED_UNCALIBRATED_SOURCE.into(),
        ),
        ColorCalibrationStatus::Uncalibrated => (
            PhysicalDataQuality::Unknown,
            DERIVED_UNCALIBRATED_SOURCE.into(),
        ),
        ColorCalibrationStatus::Nominal => (
            PhysicalDataQuality::Estimated,
            DERIVED_NOMINAL_SOURCE.into(),
        ),
        ColorCalibrationStatus::Measured => (
            PhysicalDataQuality::Measured,
            calibration
                .source
                .clone()
                .filter(|s| !s.trim().is_empty() && s.len() <= 1024)
                .unwrap_or_else(|| DERIVED_MEASURED_SOURCE.into()),
        ),
    };
    OpticalProvenance {
        quality,
        source: Some(source),
        revision: calibration.revision,
    }
}

/// Lower a provenance to at most `cap` (a derived approximation never claims more).
fn capped(mut value: OpticalProvenance, cap: PhysicalDataQuality) -> OpticalProvenance {
    let rank = |q| match q {
        PhysicalDataQuality::Unknown => 0,
        PhysicalDataQuality::Estimated => 1,
        PhysicalDataQuality::Manufacturer => 2,
        PhysicalDataQuality::Measured => 3,
    };
    if rank(value.quality) > rank(cap) {
        value.quality = cap;
        value.source = Some(DERIVED_NOMINAL_SOURCE.into());
    }
    value
}

/// A correction matrix re-maps the target, not the emitters: keep the data but never present
/// it as measured output.
fn system_evidence(system: &HeadColorSystem, inferred: bool) -> OpticalProvenance {
    let evidence = provenance(&system.calibration, inferred);
    if system.correction_matrix == super::super::color_model::identity_color_correction() {
        evidence
    } else {
        capped(evidence, PhysicalDataQuality::Estimated)
    }
}

/// The continuous native function an emitter or flag drives: the widest allowed one.
fn continuous_function(channel: &FixtureChannel) -> Option<&ChannelFunction> {
    channel
        .functions
        .iter()
        .filter(|f| {
            matches!(f.behavior, ChannelFunctionBehavior::Continuous { .. })
                && f.dmx_from < f.dmx_to
                && native_color_function_allowed(channel, f)
        })
        .max_by_key(|f| (f.dmx_to - f.dmx_from, std::cmp::Reverse(f.dmx_from)))
}

fn emitter_band(name: &str) -> OpticalEmitterBand {
    let name = name.trim().to_ascii_lowercase();
    if name == "ir" || name.contains("infrared") {
        OpticalEmitterBand::Infrared
    } else if name == "uv" || name.contains("ultraviolet") {
        OpticalEmitterBand::Ultraviolet
    } else {
        OpticalEmitterBand::OtherNonVisible
    }
}

fn words(value: &str) -> String {
    let lower = value.to_ascii_lowercase();
    let spaced: String = lower
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { ' ' })
        .collect();
    spaced.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A function name that states the control does nothing to the beam.
fn names_neutral(value: &str) -> bool {
    matches!(
        words(value).as_str(),
        "no function"
            | "open"
            | "off"
            | "none"
            | "neutral"
            | "no correction"
            | "no effect"
            | "no color"
            | "no colour"
            | "color open"
            | "colour open"
    )
}

/// Moving or effect ranges are never a parking state, even when they hold the default.
fn names_motion(value: &str) -> bool {
    let value = words(value);
    [
        "rainbow", "effect", "random", "scroll", "rotat", "spin", "strobe", "cycle", "chase",
        "auto", "fade",
    ]
    .iter()
    .any(|word| value.contains(word))
}

fn function_names(function: &ChannelFunction) -> [&str; 3] {
    match &function.behavior {
        ChannelFunctionBehavior::Fixed {
            semantic_id, label, ..
        }
        | ChannelFunctionBehavior::Indexed {
            semantic_id, label, ..
        } => [&function.name, label, semantic_id],
        _ => [&function.name, "", ""],
    }
}

/// The neutral state of a colour control: a function named neutral, else the profile default
/// (one raw value of a continuous function, or the whole steady function holding it).
fn neutral_state(channel: &FixtureChannel) -> Option<(&ChannelFunction, u32, u32, bool)> {
    let max = channel.resolution.max_raw();
    let usable = |f: &&ChannelFunction| {
        native_color_function_allowed(channel, f)
            && !matches!(f.behavior, ChannelFunctionBehavior::Control { .. })
            && f.dmx_from <= f.dmx_to
            && f.dmx_to <= max
            && f.angular_motion
                .is_none_or(|m| m.kind != AngularMotionKind::AngularVelocity)
    };
    if let Some(function) = channel
        .functions
        .iter()
        .filter(usable)
        .find(|f| function_names(f).iter().any(|name| names_neutral(name)))
    {
        return Some((function, function.dmx_from, function.dmx_to, true));
    }
    let raw = channel.default_raw.min(max);
    let function = channel.functions.iter().filter(usable).find(|f| {
        (f.dmx_from..=f.dmx_to).contains(&raw)
            && !function_names(f).iter().any(|name| names_motion(name))
    })?;
    Some(match function.behavior {
        ChannelFunctionBehavior::Continuous { .. } => (function, raw, raw, false),
        _ => (function, function.dmx_from, function.dmx_to, false),
    })
}

/// The control's raw for a normalized level, honouring channel inversion.
fn normalized_raw(channel: &FixtureChannel, level: f32) -> u32 {
    let max = channel.resolution.max_raw();
    let raw = (f64::from(level.clamp(0.0, 1.0)) * f64::from(max)).round() as u32;
    if channel.invert {
        max.saturating_sub(raw)
    } else {
        raw
    }
}

/// HSV at full value in encoded sRGB, the inverse of Color Intent's hue/saturation mapping.
fn hsv_srgb(hue: f32, saturation: f32) -> [f32; 3] {
    let sector = (hue.rem_euclid(1.0) * 6.0).min(5.999_999);
    let fraction = sector.fract();
    let (p, q, t) = (
        1.0 - saturation,
        1.0 - saturation * fraction,
        1.0 - saturation * (1.0 - fraction),
    );
    match sector as u32 {
        0 => [1.0, t, p],
        1 => [q, 1.0, p],
        2 => [p, 1.0, t],
        3 => [p, q, 1.0],
        4 => [t, p, 1.0],
        _ => [1.0, p, q],
    }
}

/// The head's chosen engine.
enum Engine<'s> {
    Continuous,
    HueSaturation(&'s HeadColorSystem),
    Wheel(&'s HeadColorSystem),
    White,
}

struct PathBuilder<'a> {
    mode: &'a FixtureMode,
    head: Uuid,
    inferred: bool,
    emitters: Vec<OpticalEmitter>,
    filters: Vec<OpticalFilter>,
    measurements: Vec<ColorRecipeMeasurement>,
    covered: HashSet<Uuid>,
    /// Neutral native value of every parked control, part of every whole-path observation.
    parked: Vec<NativeColorValue>,
    /// The chosen engine, for reasons.
    engine_name: &'static str,
}

/// One head's path from its most capable engine; every other colour control is parked.
pub(super) fn derive_head_path(
    mode: &FixtureMode,
    head: Uuid,
) -> Result<(HeadOpticalPath, u32), String> {
    let controls: Vec<Uuid> = mode
        .channels
        .iter()
        .filter(|c| c.head_id == head && is_color_channel(c))
        .map(|c| c.id)
        .collect();
    let systems = mode.intent_color_systems(head);
    let revision = systems
        .iter()
        .map(|s| s.calibration.revision)
        .max()
        .unwrap_or(0);
    let mut builder = PathBuilder {
        mode,
        head,
        inferred: !mode.color_systems.iter().any(|s| s.head_id == head),
        emitters: Vec::new(),
        filters: Vec::new(),
        measurements: Vec::new(),
        covered: HashSet::new(),
        parked: Vec::new(),
        engine_name: "white-only",
    };
    let engine = builder.engine(&systems, &controls)?;
    builder.engine_name = match engine {
        Engine::Continuous => "emitter",
        Engine::HueSaturation(_) => "hue/saturation",
        Engine::Wheel(_) => "colour wheel",
        Engine::White => "white-only",
    };
    for id in &controls {
        if !builder.covered.contains(id) {
            builder.park(*id)?;
        }
    }
    if builder.covered.iter().any(|id| !controls.contains(id)) {
        return Err("a colour system names a channel of another head".into());
    }
    let source = match engine {
        Engine::Continuous => OpticalSource::Additive {
            emitters: std::mem::take(&mut builder.emitters),
        },
        Engine::HueSaturation(system) => builder.hue_saturation(system)?,
        Engine::Wheel(system) => builder.wheel(system)?,
        // The beam of a head without a colour engine: a nominal white, never fixture data.
        Engine::White => OpticalSource::Fixed {
            xyz: Some(SEMANTIC_WHITE_XYZ),
            spectrum: Vec::new(),
            provenance: OpticalProvenance {
                quality: PhysicalDataQuality::Unknown,
                source: Some(DERIVED_UNCALIBRATED_SOURCE.into()),
                revision: 0,
            },
        },
    };
    Ok((
        HeadOpticalPath {
            id: derived_id("path", mode.id, head),
            head_id: head,
            controls,
            source,
            filters: builder.filters,
            measurements: builder.measurements,
        },
        revision,
    ))
}

impl<'a> PathBuilder<'a> {
    fn channel(&self, id: Uuid) -> Result<&'a FixtureChannel, String> {
        self.mode
            .channels
            .iter()
            .find(|c| c.id == id)
            .ok_or_else(|| "a colour system names a missing channel".into())
    }

    fn claim(&mut self, id: Uuid) -> Result<(), String> {
        self.channel(id)?;
        if self.covered.insert(id) {
            Ok(())
        } else {
            Err("one channel belongs to two colour systems".into())
        }
    }

    /// Most capable engine first: continuous emitters/flags, hue/saturation, the wheel with the
    /// most coloured steady slots, else white only.
    fn engine<'s>(
        &mut self,
        systems: &'s [HeadColorSystem],
        controls: &[Uuid],
    ) -> Result<Engine<'s>, String> {
        let continuous = |s: &&HeadColorSystem| {
            matches!(
                s.system,
                ColorSystem::Additive { .. } | ColorSystem::Subtractive { .. }
            )
        };
        let hue_saturation: Vec<_> = systems
            .iter()
            .filter(|s| matches!(s.system, ColorSystem::HueSaturation { .. }))
            .collect();
        if systems.iter().any(|s| continuous(&s)) {
            if !hue_saturation.is_empty() {
                return Err(
                    "a hue/saturation engine is layered over direct emitters; how \
                            they combine is not described"
                        .into(),
                );
            }
            for system in systems.iter().filter(continuous) {
                self.continuous(system)?;
            }
            self.check_emitters()?;
            return Ok(Engine::Continuous);
        }
        match hue_saturation.as_slice() {
            [] => {}
            [system] => {
                let ColorSystem::HueSaturation {
                    hue_channel_id,
                    saturation_channel_id,
                    intensity_channel_id,
                } = &system.system
                else {
                    unreachable!()
                };
                self.claim(*hue_channel_id)?;
                self.claim(*saturation_channel_id)?;
                // An HSI intensity on the fixture dimmer is Intensity's, not a colour control.
                if let Some(id) = intensity_channel_id.filter(|id| controls.contains(id)) {
                    self.claim(id)?;
                }
                return Ok(Engine::HueSaturation(system));
            }
            _ => return Err("several hue/saturation engines on one head".into()),
        }
        let observable = |system: &HeadColorSystem| match &system.system {
            ColorSystem::DiscreteWheel { slots, .. } => slots
                .iter()
                .filter(|slot| slot.is_steady() && slot.display_xyz().is_some())
                .count(),
            _ => 0,
        };
        let wheel = systems
            .iter()
            .filter(|s| matches!(s.system, ColorSystem::DiscreteWheel { .. }))
            .enumerate()
            .max_by_key(|(index, s)| (observable(s), std::cmp::Reverse(*index)))
            .map(|(_, s)| s);
        Ok(match wheel {
            Some(system) => {
                let ColorSystem::DiscreteWheel { channel_id, .. } = &system.system else {
                    unreachable!()
                };
                self.claim(*channel_id)?;
                Engine::Wheel(system)
            }
            None => Engine::White,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn emitter(
        &mut self,
        channel_id: Uuid,
        name: &str,
        xyz: Option<Xyz>,
        band: OpticalEmitterBand,
        reversed_by_flag: bool,
        maximum_level: f32,
        response_exponent: f32,
        provenance: OpticalProvenance,
    ) -> Result<(), String> {
        let channel = self.channel(channel_id)?;
        let function = continuous_function(channel)
            .ok_or_else(|| format!("{name} has no continuous function to drive"))?;
        self.claim(channel.id)?;
        self.emitters.push(OpticalEmitter {
            id: derived_id("emitter", self.head, channel.id),
            name: if name.trim().is_empty() {
                function.name.clone()
            } else {
                name.into()
            },
            binding: NativeColorBinding {
                channel_id: channel.id,
                function_id: function.id,
            },
            xyz,
            spectrum: Vec::new(),
            band,
            // Raw values are final channel bytes: an inverted channel reaches full at its start,
            // and a CMY flag fully in removes its complement.
            native_reversed: channel.invert != reversed_by_flag,
            maximum_level: if maximum_level.is_finite() && maximum_level > 0.0 {
                maximum_level.min(1.0)
            } else {
                1.0
            },
            response_exponent: if response_exponent.is_finite() && response_exponent > 0.0 {
                response_exponent
            } else {
                1.0
            },
            provenance,
        });
        Ok(())
    }

    /// Additive emitters or CMY flags.
    fn continuous(&mut self, system: &HeadColorSystem) -> Result<(), String> {
        let evidence = system_evidence(system, self.inferred);
        match &system.system {
            ColorSystem::Additive { emitters } => {
                for emitter in emitters {
                    let (xyz, band, quality) = if emitter.visible {
                        (
                            Some(emitter.xyz),
                            OpticalEmitterBand::Visible,
                            evidence.clone(),
                        )
                    } else {
                        // Visible leakage of a UV/IR emitter is not described: unknown, not black.
                        (
                            None,
                            emitter_band(&emitter.name),
                            OpticalProvenance {
                                quality: PhysicalDataQuality::Unknown,
                                ..evidence.clone()
                            },
                        )
                    };
                    self.emitter(
                        emitter.channel_id,
                        &emitter.name,
                        xyz,
                        band,
                        false,
                        emitter.maximum_level,
                        emitter.response_curve,
                        quality,
                    )?;
                }
            }
            ColorSystem::Subtractive {
                cyan_channel_id,
                magenta_channel_id,
                yellow_channel_id,
                filters,
            } => self.flags(
                [*cyan_channel_id, *magenta_channel_id, *yellow_channel_id],
                filters.as_ref(),
                evidence,
            )?,
            _ => unreachable!("only continuous systems"),
        }
        Ok(())
    }

    fn flags(
        &mut self,
        channels: [Uuid; 3],
        filters: Option<&SubtractiveCalibration>,
        evidence: OpticalProvenance,
    ) -> Result<(), String> {
        let primaries = [
            srgb_to_xyz(1.0, 0.0, 0.0),
            srgb_to_xyz(0.0, 1.0, 0.0),
            srgb_to_xyz(0.0, 0.0, 1.0),
        ];
        let (absorbed, evidence) = match filters {
            None => (primaries, evidence),
            // Measured flags: each flag removes what it absorbs from the open beam. The
            // independent-flag reading is itself an estimate.
            Some(SubtractiveCalibration {
                open_xyz,
                cyan_xyz,
                magenta_xyz,
                yellow_xyz,
            }) => {
                let less = |open: Xyz, flag: Xyz| Xyz {
                    x: (open.x - flag.x).max(0.0),
                    y: (open.y - flag.y).max(0.0),
                    z: (open.z - flag.z).max(0.0),
                };
                (
                    [
                        less(*open_xyz, *cyan_xyz),
                        less(*open_xyz, *magenta_xyz),
                        less(*open_xyz, *yellow_xyz),
                    ],
                    capped(evidence, PhysicalDataQuality::Estimated),
                )
            }
        };
        for ((channel, name), xyz) in channels
            .into_iter()
            .zip(["Cyan", "Magenta", "Yellow"])
            .zip(absorbed)
        {
            self.emitter(
                channel,
                name,
                Some(xyz),
                OpticalEmitterBand::Visible,
                true,
                1.0,
                1.0,
                evidence.clone(),
            )?;
        }
        Ok(())
    }

    /// Emitters read from channel names must name distinct roles, and the fitter bounds how
    /// many visible emitters one head may solve.
    fn check_emitters(&self) -> Result<(), String> {
        let visible: Vec<&OpticalEmitter> = self
            .emitters
            .iter()
            .filter(|e| e.band == OpticalEmitterBand::Visible)
            .collect();
        if self.inferred {
            // A channel whose own colour name differs from the role it is driven as (a Cyan
            // channel on the Red attribute) has an ambiguous role: never guess which it is.
            for emitter in &visible {
                let channel = self.channel(emitter.binding.channel_id)?;
                // CMY flags deliberately drive the complement's attribute.
                if channel.canonical_transform != CanonicalTransform::Identity {
                    continue;
                }
                let role = |attribute: &str| {
                    let name = attribute.strip_prefix("color.")?;
                    COLOUR_NAMES.contains(&name).then_some(name.to_owned())
                };
                if let (Some(own), Some(driven)) = (
                    role(&channel.fixture_attribute.0),
                    role(&channel.attribute.0),
                ) && own != driven
                {
                    return Err(format!(
                        "the {own} channel is driven as {driven}; its colour role is ambiguous"
                    ));
                }
            }
        }
        let limit = crate::forward::COLOR_FIT_MAX_VISIBLE_EMITTERS;
        if visible.len() > limit {
            return Err(format!(
                "{} visible emitters exceed the fitter's {limit}",
                visible.len()
            ));
        }
        Ok(())
    }

    /// Hold a colour control at its neutral state: a filter whose only known range is that
    /// state, with a unit transmission. Its other ranges stay unmodelled (unknown output).
    fn park(&mut self, id: Uuid) -> Result<(), String> {
        let channel = self.channel(id)?;
        let (function, from, to, named) = neutral_state(channel).ok_or_else(|| {
            format!(
                "{} has no neutral state to park",
                channel.fixture_attribute.0
            )
        })?;
        // A direct emitter outside the engine is only neutral when it is off; lit at its
        // default it adds light the engine does not describe.
        let off = if channel.invert {
            channel.resolution.max_raw()
        } else {
            0
        };
        let role = channel
            .attribute
            .0
            .strip_prefix("color.")
            .filter(|name| COLOUR_NAMES.contains(name));
        if let Some(role) = role
            && !named
            && (from, to) != (off, off)
        {
            return Err(format!(
                "a direct {role} emitter is layered over the {} engine; how they combine is \
                 not described",
                self.engine_name
            ));
        }
        self.claim(id)?;
        let unit = |wavelength_nm| SpectrumSample {
            wavelength_nm,
            value: 1.0,
        };
        self.filters.push(OpticalFilter {
            id: derived_id("parked", channel.id, function.id),
            name: if function.name.trim().is_empty() {
                channel.fixture_attribute.0.to_string()
            } else {
                function.name.clone()
            },
            binding: NativeColorBinding {
                channel_id: channel.id,
                function_id: function.id,
            },
            transmission: OpticalTransmission::Spectral {
                samples: vec![FilterSpectrum {
                    raw_from: from,
                    raw_to: to,
                    spectrum: vec![unit(360.0), unit(830.0)],
                }],
            },
            provenance: OpticalProvenance {
                quality: PhysicalDataQuality::Estimated,
                source: Some(DERIVED_PARKED_SOURCE.into()),
                revision: 0,
            },
        });
        self.parked.push(NativeColorValue {
            channel_id: channel.id,
            function_id: function.id,
            raw: from,
        });
        Ok(())
    }

    /// Every allowed function of a control becomes a filter of unknown transmission; returns
    /// the bound functions.
    fn unknown_filters(
        &mut self,
        channel: &FixtureChannel,
        evidence: &OpticalProvenance,
        fallback: &str,
    ) -> Vec<Uuid> {
        let mut bound = Vec::new();
        for function in channel
            .functions
            .iter()
            .filter(|f| native_color_function_allowed(channel, f))
        {
            if bound.contains(&function.id) {
                continue;
            }
            bound.push(function.id);
            self.filters.push(OpticalFilter {
                id: derived_id("filter", channel.id, function.id),
                name: if function.name.trim().is_empty() {
                    fallback.into()
                } else {
                    function.name.clone()
                },
                binding: NativeColorBinding {
                    channel_id: channel.id,
                    function_id: function.id,
                },
                transmission: OpticalTransmission::Unknown,
                provenance: OpticalProvenance {
                    quality: PhysicalDataQuality::Unknown,
                    ..evidence.clone()
                },
            });
        }
        bound
    }

    /// One native value at `raw` through a bound function, or None outside every one.
    fn value(channel: &FixtureChannel, bound: &[Uuid], raw: u32) -> Option<NativeColorValue> {
        let function = channel
            .functions
            .iter()
            .find(|f| (f.dmx_from..=f.dmx_to).contains(&raw) && bound.contains(&f.id))?;
        (raw <= channel.resolution.max_raw()).then_some(NativeColorValue {
            channel_id: channel.id,
            function_id: function.id,
            raw,
        })
    }

    /// A whole-path observation: the engine's values plus every parked control's neutral value.
    fn observe(&mut self, values: Vec<NativeColorValue>, xyz: Xyz, provenance: OpticalProvenance) {
        let mut recipe = values;
        recipe.extend(self.parked.iter().cloned());
        self.measurements.push(ColorRecipeMeasurement {
            recipe,
            xyz,
            provenance,
        });
    }

    /// The slots of the engine wheel: a nominal D65 source, unknown filters per slot function
    /// and one observation per steady slot whose colour is declared (or, uncalibrated, named).
    fn wheel(&mut self, system: &HeadColorSystem) -> Result<OpticalSource, String> {
        let ColorSystem::DiscreteWheel { channel_id, slots } = &system.system else {
            unreachable!()
        };
        let evidence = system_evidence(system, self.inferred);
        let channel = self.channel(*channel_id)?;
        let bound = self.unknown_filters(channel, &evidence, "Wheel");
        let mut seen = HashSet::new();
        for slot in slots.iter().filter(|slot| slot.is_steady()) {
            let (xyz, provenance) = match (slot.measured_xyz, slot.display_xyz()) {
                (Some(xyz), _) => (xyz, evidence.clone()),
                // A colour read from the slot's name is a guess whatever the system declares.
                (None, Some(xyz)) => (
                    xyz,
                    OpticalProvenance {
                        quality: PhysicalDataQuality::Unknown,
                        source: Some(DERIVED_UNCALIBRATED_SOURCE.into()),
                        revision: evidence.revision,
                    },
                ),
                (None, None) => continue,
            };
            if slot.dmx_from > slot.dmx_to {
                continue;
            }
            let raw = slot.dmx_from + (slot.dmx_to - slot.dmx_from) / 2;
            let Some(value) = Self::value(channel, &bound, raw) else {
                continue;
            };
            if seen.insert(raw) {
                self.observe(vec![value], xyz, provenance);
            }
        }
        // The open beam of a lamp behind a wheel is a nominal D65 white.
        Ok(OpticalSource::Fixed {
            xyz: Some(SEMANTIC_WHITE_XYZ),
            spectrum: Vec::new(),
            provenance: capped(evidence, PhysicalDataQuality::Estimated),
        })
    }

    /// A hue/saturation engine as a nominal sRGB HSV grid of whole-path observations (3° hue
    /// steps, seven saturation rings, white) over a D65 source, estimated at best.
    fn hue_saturation(&mut self, system: &HeadColorSystem) -> Result<OpticalSource, String> {
        let ColorSystem::HueSaturation {
            hue_channel_id,
            saturation_channel_id,
            intensity_channel_id,
        } = &system.system
        else {
            unreachable!()
        };
        let evidence = capped(
            system_evidence(system, self.inferred),
            PhysicalDataQuality::Estimated,
        );
        let grid = OpticalProvenance {
            source: Some(DERIVED_HUE_SATURATION_SOURCE.into()),
            ..evidence.clone()
        };
        let hue = self.channel(*hue_channel_id)?;
        let saturation = self.channel(*saturation_channel_id)?;
        let intensity = intensity_channel_id
            .filter(|id| self.covered.contains(id))
            .map(|id| self.channel(id))
            .transpose()?;
        let hue_bound = self.unknown_filters(hue, &grid, "Hue");
        let saturation_bound = self.unknown_filters(saturation, &grid, "Saturation");
        let intensity_bound =
            intensity.map(|channel| self.unknown_filters(channel, &grid, "Intensity"));
        let mut seen = HashSet::new();
        let grid_points = (0..HUE_STEPS)
            .flat_map(|step| SATURATIONS.map(|s| (step as f32 / HUE_STEPS as f32, s)))
            .chain(std::iter::once((0.0, 0.0)));
        for (h, s) in grid_points {
            let mut values = vec![
                Self::value(hue, &hue_bound, normalized_raw(hue, h)),
                Self::value(saturation, &saturation_bound, normalized_raw(saturation, s)),
            ];
            // An HSI engine's own intensity stays full: brightness is Intensity's.
            if let (Some(channel), Some(bound)) = (intensity, &intensity_bound) {
                values.push(Self::value(channel, bound, normalized_raw(channel, 1.0)));
            }
            let Some(values) = values.into_iter().collect::<Option<Vec<_>>>() else {
                continue;
            };
            if !seen.insert(values.iter().map(|v| v.raw).collect::<Vec<_>>()) {
                continue;
            }
            let [red, green, blue] = hsv_srgb(h, s);
            self.observe(values, srgb_to_xyz(red, green, blue), grid.clone());
        }
        if self.measurements.is_empty() {
            return Err("the hue/saturation controls have no usable function".into());
        }
        Ok(OpticalSource::Fixed {
            xyz: Some(SEMANTIC_WHITE_XYZ),
            spectrum: Vec::new(),
            provenance: evidence,
        })
    }
}
