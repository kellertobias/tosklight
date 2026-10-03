//! Authored optical paths and exact native identity. This is fixture data, not a runtime solver.
//! Legacy `color_systems` is deliberately not interpreted as a serial optical path.
use super::{
    ChannelFunction, ChannelFunctionBehavior, ColorSystem, FixtureChannel, FixtureMode,
    FixtureProfile, PhysicalDataQuality, ProfileError,
};
use light_core::Xyz;
pub use light_core::{NativeColorBinding, NativeColorIdentity, NativeColorValue};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ColorPhysicalModel {
    pub version: u16,
    #[serde(default)]
    pub revision: u32,
    pub paths: Vec<HeadOpticalPath>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HeadOpticalPath {
    pub id: Uuid,
    pub head_id: Uuid,
    /// Complete Color ownership, including controls whose appearance is unknown.
    pub controls: Vec<Uuid>,
    pub source: OpticalSource,
    #[serde(default)]
    pub filters: Vec<OpticalFilter>,
    /// Full-path observations for exact native recipes, not independent filter transmission.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub measurements: Vec<ColorRecipeMeasurement>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct OpticalProvenance {
    #[serde(default)]
    pub quality: PhysicalDataQuality,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default)]
    pub revision: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpectrumSample {
    pub wavelength_nm: f32,
    pub value: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OpticalSource {
    Unknown,
    Fixed {
        /// Full source output, in the path's relative-output scale. Black is valid.
        xyz: Option<Xyz>,
        #[serde(default)]
        spectrum: Vec<SpectrumSample>,
        #[serde(default)]
        provenance: OpticalProvenance,
    },
    Additive {
        emitters: Vec<OpticalEmitter>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Declared emitter role. UV/IR identity does not imply zero visible output;
/// optional XYZ and spectrum describe that independently. Missing data stays unknown.
pub enum OpticalEmitterBand {
    Visible,
    Ultraviolet,
    Infrared,
    OtherNonVisible,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OpticalEmitter {
    pub id: Uuid,
    pub name: String,
    pub binding: NativeColorBinding,
    /// Visible output, including any visible component of a UV emitter. None is unknown, not black.
    pub xyz: Option<Xyz>,
    #[serde(default)]
    pub spectrum: Vec<SpectrumSample>,
    pub band: OpticalEmitterBand,
    /// False: native function start is off, end is full. True reverses those endpoints.
    #[serde(default)]
    pub native_reversed: bool,
    pub maximum_level: f32,
    pub response_exponent: f32,
    #[serde(default)]
    pub provenance: OpticalProvenance,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OpticalFilter {
    pub id: Uuid,
    pub name: String,
    pub binding: NativeColorBinding,
    pub transmission: OpticalTransmission,
    #[serde(default)]
    pub provenance: OpticalProvenance,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OpticalTransmission {
    Unknown,
    /// Exact steady raw ranges. Gaps, motion and unmeasured ranges remain unknown.
    Spectral {
        samples: Vec<FilterSpectrum>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FilterSpectrum {
    pub raw_from: u32,
    pub raw_to: u32,
    pub spectrum: Vec<SpectrumSample>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColorRecipeMeasurement {
    /// Exactly one value for every path control, including open/parked controls.
    pub recipe: Vec<NativeColorValue>,
    /// Measured output XYZ preserves brightness and black; it is not chromaticity-normalized.
    pub xyz: Xyz,
    #[serde(default)]
    pub provenance: OpticalProvenance,
}

fn invalid(message: &str) -> ProfileError {
    ProfileError::Invalid(message.into())
}
fn finite_xyz(xyz: Xyz) -> bool {
    [xyz.x, xyz.y, xyz.z]
        .iter()
        .all(|v| v.is_finite() && *v >= 0.0)
}
impl OpticalProvenance {
    pub fn validate(&self) -> Result<(), String> {
        if self.source.as_ref().is_some_and(|s| s.len() > 1024) {
            return Err("optical provenance source must be at most 1024 bytes".into());
        }
        if matches!(
            self.quality,
            PhysicalDataQuality::Manufacturer | PhysicalDataQuality::Measured
        ) && self.source.as_ref().is_none_or(|s| s.trim().is_empty())
        {
            return Err("manufacturer and measured optical data need a source".into());
        }
        Ok(())
    }
}
fn provenance_valid(value: &OpticalProvenance) -> bool {
    value.validate().is_ok()
}
fn spectrum_valid(samples: &[SpectrumSample], transmission: bool) -> bool {
    samples.len() >= 2
        && samples.iter().all(|s| {
            s.wavelength_nm.is_finite()
                && (200.0..=2500.0).contains(&s.wavelength_nm)
                && s.value.is_finite()
                && s.value >= 0.0
                && (!transmission || s.value <= 1.0)
        })
        && samples
            .windows(2)
            .all(|p| p[0].wavelength_nm < p[1].wavelength_nm)
}

fn is_color_attribute(attribute: &str) -> bool {
    attribute == "color" || attribute.starts_with("color.")
}

/// Legacy fixture-control ranges can contain resets authored as ordinary fixed functions.
/// Require explicit Color classification before those ambiguous ranges enter optical recipes.
pub fn native_color_function_allowed(channel: &FixtureChannel, function: &ChannelFunction) -> bool {
    !matches!(function.behavior, ChannelFunctionBehavior::Control { .. })
        && (is_color_attribute(&function.attribute.0)
            || (channel.fixture_attribute.0.as_ref() != "fixture.control"
                && channel.attribute.0.as_ref() != "fixture.control"
                && function.attribute.0.as_ref() != "fixture.control"))
}

/// Validate a path's fixed or additive optical source and its emitter bindings.
fn validate_optical_source<'a, F>(
    source: &OpticalSource,
    identities: &mut HashSet<Uuid>,
    check_binding: &mut F,
) -> Result<(), ProfileError>
where
    F: FnMut(NativeColorBinding) -> Result<&'a ChannelFunction, ProfileError>,
{
    match source {
        OpticalSource::Unknown => {}
        OpticalSource::Fixed {
            xyz,
            spectrum,
            provenance,
        } => {
            if xyz.is_some_and(|v| !finite_xyz(v))
                || (!spectrum.is_empty() && !spectrum_valid(spectrum, false))
                || !provenance_valid(provenance)
            {
                return Err(invalid(
                    "fixed optical source data or provenance is invalid",
                ));
            }
        }
        OpticalSource::Additive { emitters } => {
            if emitters.is_empty() {
                return Err(invalid("additive optical source needs emitters"));
            }
            for emitter in emitters {
                let function = check_binding(emitter.binding)?;
                if !matches!(
                    function.behavior,
                    ChannelFunctionBehavior::Continuous { .. }
                ) || function.dmx_from >= function.dmx_to
                {
                    return Err(invalid(
                        "optical emitter must reference a continuous native function",
                    ));
                }
                if emitter.id.is_nil()
                    || !identities.insert(emitter.id)
                    || emitter.name.trim().is_empty()
                    || !emitter.maximum_level.is_finite()
                    || emitter.maximum_level <= 0.0
                    || emitter.maximum_level > 1.0
                    || !emitter.response_exponent.is_finite()
                    || emitter.response_exponent <= 0.0
                    || emitter.xyz.is_some_and(|v| !finite_xyz(v))
                    || (!emitter.spectrum.is_empty() && !spectrum_valid(&emitter.spectrum, false))
                    || !provenance_valid(&emitter.provenance)
                {
                    return Err(invalid(
                        "optical emitter identity, data or provenance is invalid",
                    ));
                }
            }
        }
    }
    Ok(())
}

impl FixtureMode {
    pub fn validate_color_physical(&self) -> Result<(), ProfileError> {
        let Some(model) = &self.color_physical else {
            return Ok(());
        };
        if model.version != 1 {
            return Err(invalid("unsupported physical color model version"));
        }
        if model.paths.is_empty() {
            return Err(invalid("physical color model needs at least one head path"));
        }
        let mut identities = HashSet::new();
        let mut heads = HashSet::new();
        for path in &model.paths {
            if path.id.is_nil()
                || !identities.insert(path.id)
                || !heads.insert(path.head_id)
                || !self.heads.iter().any(|h| h.id == path.head_id)
            {
                return Err(invalid(
                    "physical color paths need unique identities and existing distinct heads",
                ));
            }
            let controls = self.color_path_controls(path)?;
            let mut modeled = HashSet::new();
            let mut check_binding = |binding: NativeColorBinding| {
                if !controls.contains(&binding.channel_id)
                    || !modeled.insert((binding.channel_id, binding.function_id))
                {
                    return Err(invalid(
                        "optical bindings must name a path control and cannot be modeled twice",
                    ));
                }
                let channel = self
                    .channels
                    .iter()
                    .find(|c| c.id == binding.channel_id)
                    .unwrap();
                let function = channel
                    .functions
                    .iter()
                    .find(|f| f.id == binding.function_id)
                    .ok_or_else(|| {
                        invalid("optical binding references a missing native function")
                    })?;
                if !native_color_function_allowed(channel, function) {
                    return Err(invalid(
                        "service or unclassified fixture-control functions cannot be optical color bindings",
                    ));
                }
                Ok(function)
            };
            validate_optical_source(&path.source, &mut identities, &mut check_binding)?;
            for filter in &path.filters {
                let function = check_binding(filter.binding)?;
                if matches!(function.behavior, ChannelFunctionBehavior::Control { .. }) {
                    return Err(invalid(
                        "service control functions cannot be optical color filters",
                    ));
                }
                if filter.id.is_nil()
                    || !identities.insert(filter.id)
                    || filter.name.trim().is_empty()
                    || !provenance_valid(&filter.provenance)
                {
                    return Err(invalid("optical filter identity or provenance is invalid"));
                }
                if let OpticalTransmission::Spectral { samples } = &filter.transmission {
                    if samples.is_empty()
                        || samples.iter().any(|s| {
                            s.raw_from > s.raw_to
                                || s.raw_from < function.dmx_from
                                || s.raw_to > function.dmx_to
                                || !spectrum_valid(&s.spectrum, true)
                        })
                        || samples.windows(2).any(|s| s[0].raw_to >= s[1].raw_from)
                    {
                        return Err(invalid(
                            "filter spectra need sorted non-overlapping native ranges and valid transmission",
                        ));
                    }
                }
            }
            for sample in &path.measurements {
                if !finite_xyz(sample.xyz) || !provenance_valid(&sample.provenance) {
                    return Err(invalid(
                        "whole-path color measurement or provenance is invalid",
                    ));
                }
                self.validate_native_color_recipe(path, &sample.recipe)?;
            }
        }
        Ok(())
    }

    /// Collect a path's native controls and prove it owns every declared Color channel and
    /// existing color-system control of its head.
    fn color_path_controls(&self, path: &HeadOpticalPath) -> Result<HashSet<Uuid>, ProfileError> {
        let mut controls = HashSet::new();
        for id in &path.controls {
            let channel =
                self.channels.iter().find(|c| c.id == *id).ok_or_else(|| {
                    invalid("physical color control references a missing channel")
                })?;
            let shared = self
                .heads
                .iter()
                .any(|h| h.id == channel.head_id && h.master_shared);
            if !controls.insert(*id) || (channel.head_id != path.head_id && !shared) {
                return Err(invalid(
                    "physical color controls must be distinct and belong to this or the shared head",
                ));
            }
            if channel.functions.is_empty() {
                return Err(invalid("native color controls require explicit functions"));
            }
        }
        // Extra emitters added outside the legacy system list still belong to Color.
        // Shared-head controls require explicit path dependencies: they need not affect
        // every child optical path (for example plate RGB versus white strobe segments).
        if self.channels.iter().any(|channel| {
            channel.head_id == path.head_id
                && (is_color_attribute(&channel.fixture_attribute.0)
                    || is_color_attribute(&channel.attribute.0)
                    || channel
                        .functions
                        .iter()
                        .any(|f| is_color_attribute(&f.attribute.0)))
                && !controls.contains(&channel.id)
        }) {
            return Err(invalid(
                "physical path omits a declared native Color channel",
            ));
        }
        // A new model cannot silently omit already-declared Color ownership.
        for system in self
            .color_systems
            .iter()
            .filter(|s| s.head_id == path.head_id)
        {
            let ids: Vec<Uuid> = match &system.system {
                ColorSystem::Additive { emitters } => {
                    emitters.iter().map(|e| e.channel_id).collect()
                }
                ColorSystem::Subtractive {
                    cyan_channel_id,
                    magenta_channel_id,
                    yellow_channel_id,
                    ..
                } => vec![*cyan_channel_id, *magenta_channel_id, *yellow_channel_id],
                ColorSystem::HueSaturation {
                    hue_channel_id,
                    saturation_channel_id,
                    intensity_channel_id,
                } => [
                    Some(*hue_channel_id),
                    Some(*saturation_channel_id),
                    // An HSI engine driven by the fixture's Intensity channel does not make
                    // that channel Color-owned: brightness stays Intensity's.
                    intensity_channel_id.filter(|id| {
                        self.channels.iter().any(|c| {
                            c.id == *id
                                && (is_color_attribute(&c.fixture_attribute.0)
                                    || is_color_attribute(&c.attribute.0))
                        })
                    }),
                ]
                .into_iter()
                .flatten()
                .collect(),
                ColorSystem::DiscreteWheel { channel_id, .. } => vec![*channel_id],
            };
            if ids.iter().any(|id| !controls.contains(id)) {
                return Err(invalid(
                    "physical path omits an existing color system control",
                ));
            }
        }
        Ok(controls)
    }

    pub fn validate_native_color_recipe(
        &self,
        path: &HeadOpticalPath,
        values: &[NativeColorValue],
    ) -> Result<(), ProfileError> {
        let mut seen = HashSet::new();
        for value in values {
            let channel = self
                .channels
                .iter()
                .find(|c| c.id == value.channel_id)
                .ok_or_else(|| invalid("native color recipe references a missing channel"))?;
            let function = channel
                .functions
                .iter()
                .find(|f| f.id == value.function_id)
                .ok_or_else(|| invalid("native color recipe references a missing function"))?;
            if !native_color_function_allowed(channel, function) {
                return Err(invalid(
                    "service control functions cannot be captured as native color",
                ));
            }
            if !path.controls.contains(&value.channel_id)
                || !seen.insert(value.channel_id)
                || value.raw < function.dmx_from
                || value.raw > function.dmx_to
                || value.raw > channel.resolution.max_raw()
            {
                return Err(invalid(
                    "native color recipe has duplicate, foreign or out-of-function values",
                ));
            }
        }
        if seen.len() != path.controls.len() {
            return Err(invalid(
                "native color recipe must contain every path control",
            ));
        }
        Ok(())
    }
}

impl FixtureProfile {
    pub fn native_color_identity(
        &self,
        mode_id: Uuid,
        head_id: Uuid,
    ) -> Result<NativeColorIdentity, ProfileError> {
        self.native_color_identities(mode_id)?
            .into_iter()
            .find(|id| id.head_id == head_id)
            .ok_or_else(|| invalid("native color head path is missing"))
    }

    /// Derive these once from the complete immutable profile before compact runtime projection.
    pub fn native_color_identities(
        &self,
        mode_id: Uuid,
    ) -> Result<Vec<NativeColorIdentity>, ProfileError> {
        self.validate()?;
        let mode = self
            .mode(mode_id)
            .ok_or_else(|| invalid("native color mode is missing"))?;
        let model = mode
            .color_physical
            .as_ref()
            .ok_or_else(|| invalid("native color identity requires an explicit physical path"))?;
        let bytes =
            serde_json::to_vec(&serde_json::to_value(self).map_err(|e| invalid(&e.to_string()))?)
                .map_err(|e| invalid(&e.to_string()))?;
        let profile_digest = format!("{:x}", Sha256::digest(bytes));
        model
            .paths
            .iter()
            .map(|path| {
                self.native_color_identity_validated(mode_id, path.head_id, &profile_digest)
            })
            .collect()
    }

    fn native_color_identity_validated(
        &self,
        mode_id: Uuid,
        head_id: Uuid,
        profile_digest: &str,
    ) -> Result<NativeColorIdentity, ProfileError> {
        let mode = self
            .modes
            .iter()
            .find(|m| m.id == mode_id)
            .ok_or_else(|| invalid("native color mode is missing"))?;
        let model = mode
            .color_physical
            .as_ref()
            .ok_or_else(|| invalid("native color identity requires an explicit physical path"))?;
        let path = model
            .paths
            .iter()
            .find(|p| p.head_id == head_id)
            .ok_or_else(|| invalid("native color head path is missing"))?;
        let mut controls =
            path.controls
                .iter()
                .map(|id| {
                    let channel = mode.channels.iter().find(|c| c.id == *id).unwrap();
                    let mut functions = channel.functions.iter().map(|f| serde_json::json!({
                "id": f.id, "from": f.dmx_from, "to": f.dmx_to,
                "attribute": f.attribute, "priority": f.priority, "behavior": f.behavior
            })).collect::<Vec<_>>();
                    functions.sort_by_key(|v| v["id"].as_str().unwrap().to_owned());
                    serde_json::json!({ "id": channel.id, "head_id": channel.head_id,
                "fixture_attribute": channel.fixture_attribute, "resolution": channel.resolution,
                "invert": channel.invert, "behavior": channel.behavior,
                "default_raw": channel.default_raw, "functions": functions })
                })
                .collect::<Vec<_>>();
        controls.sort_by_key(|v| v["id"].as_str().unwrap().to_owned());
        let digest = |value: &serde_json::Value| {
            format!(
                "{:x}",
                Sha256::digest(serde_json::to_vec(value).expect("serializable fixture identity"))
            )
        };
        Ok(NativeColorIdentity {
            profile_id: self.id.0,
            profile_revision: self.revision,
            profile_digest: profile_digest.to_owned(),
            mode_id,
            head_id,
            path_id: path.id,
            model_revision: model.revision,
            native_layout_signature: digest(
                &serde_json::json!({"version":1,"mode_id":mode_id,"head_id":head_id,"controls":controls}),
            ),
        })
    }
}
