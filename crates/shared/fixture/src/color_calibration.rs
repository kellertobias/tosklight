//! Per-physical-instance optical observations. These never alter a library profile or a cue.
use crate::{
    ColorRecipeMeasurement, FixtureProfile, NativeColorIdentity, OpticalProvenance, OpticalSource,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

/// Absence means no installed correction. Physical copies do not inherit a root's measurements.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InstalledColorCalibration {
    pub version: u16,
    #[serde(default)]
    pub revision: u32,
    pub paths: Vec<InstalledColorPathCalibration>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InstalledColorPathCalibration {
    /// Exact authoring identity, retained unchanged when the patched profile changes.
    pub source_identity: NativeColorIdentity,
    #[serde(default)]
    pub emitters: Vec<InstalledEmitterCalibration>,
    /// Complete-path observations; never interpreted as individual CMY/filter transmission.
    #[serde(default)]
    pub measurements: Vec<ColorRecipeMeasurement>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InstalledEmitterCalibration {
    pub emitter_id: Uuid,
    /// Multiplies source XYZ and every spectral sample, not the native DMX drive. Zero is valid.
    /// This describes output gain only, not a chromaticity shift or a fabricated spectrum.
    pub output_gain: f32,
    #[serde(default)]
    pub provenance: OpticalProvenance,
}

/// Derived at configuration time, never persisted as the operator's intent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstalledColorCalibrationStatus {
    Current,
    Stale { reason: String },
}

fn hash_valid(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
}

impl InstalledColorCalibration {
    /// Structural validation deliberately does not require the old profile to be installed.
    /// Replacement must retain stale observations without preventing show load or unrelated edits.
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("unsupported installed Color calibration version".into());
        }
        if self.paths.is_empty() || self.paths.len() > 512 {
            return Err("installed Color calibration requires 1-512 paths".into());
        }
        let mut heads = HashSet::new();
        let mut paths = HashSet::new();
        let mut entries = 0usize;
        let mut source = None;
        for path in &self.paths {
            let id = &path.source_identity;
            if [id.profile_id, id.mode_id, id.head_id, id.path_id]
                .iter()
                .any(Uuid::is_nil)
                || !hash_valid(&id.profile_digest)
                || !hash_valid(&id.native_layout_signature)
                || !heads.insert(id.head_id)
                || !paths.insert(id.path_id)
            {
                return Err("installed Color calibration requires unique valid path identities and SHA-256 digests".into());
            }
            let profile = (
                id.profile_id,
                id.profile_revision,
                &id.profile_digest,
                id.mode_id,
                id.model_revision,
            );
            if source.as_ref().is_some_and(|s| *s != profile) {
                return Err("installed Color calibration paths must refer to one profile, mode and optical model revision".into());
            }
            source = Some(profile);
            if path.emitters.is_empty() && path.measurements.is_empty() {
                return Err("installed Color calibration path has no observations".into());
            }
            let mut emitters = HashSet::new();
            for emitter in &path.emitters {
                entries = entries.saturating_add(1);
                if emitter.emitter_id.is_nil()
                    || !emitters.insert(emitter.emitter_id)
                    || !emitter.output_gain.is_finite()
                    || emitter.output_gain < 0.0
                {
                    return Err("installed emitter gains require unique identities and finite nonnegative output".into());
                }
                emitter.provenance.validate()?;
            }
            let mut recipes = HashSet::new();
            for measurement in &path.measurements {
                entries = entries.saturating_add(1 + measurement.recipe.len());
                if ![measurement.xyz.x, measurement.xyz.y, measurement.xyz.z]
                    .iter()
                    .all(|v| v.is_finite() && *v >= 0.0)
                {
                    return Err(
                        "installed Color measurement XYZ must be finite nonnegative output".into(),
                    );
                }
                measurement.provenance.validate()?;
                let mut controls = HashSet::new();
                let mut recipe = Vec::new();
                for value in &measurement.recipe {
                    if value.channel_id.is_nil()
                        || value.function_id.is_nil()
                        || !controls.insert(value.channel_id)
                    {
                        return Err("installed Color recipes need unique valid channel and function identities".into());
                    }
                    recipe.push((value.channel_id, value.function_id, value.raw));
                }
                recipe.sort_unstable();
                if !recipes.insert(recipe) {
                    return Err("installed Color calibration repeats a native recipe".into());
                }
            }
            if entries > 65_536 {
                return Err("installed Color calibration exceeds 65536 observation entries".into());
            }
        }
        Ok(())
    }

    /// Require exact matching when adding/editing calibration. Unchanged stale data is allowed
    /// through unrelated edits; callers must not silently rebind it to a new profile.
    pub fn validate_for_profile(
        &self,
        profile: &FixtureProfile,
        mode_id: Uuid,
    ) -> Result<(), String> {
        self.validate_for_context(&ColorCalibrationContext::new(profile, mode_id)?)
    }

    pub fn validate_for_context(&self, context: &ColorCalibrationContext) -> Result<(), String> {
        self.validate()?;
        let mode = &context.mode;
        for path in &self.paths {
            let current = context
                .identities
                .iter()
                .find(|id| id.head_id == path.source_identity.head_id)
                .ok_or("installed Color calibration head path is missing")?;
            if *current != path.source_identity {
                return Err("installed Color calibration is stale for this profile, mode or optical path; measure or explicitly author a new calibration".into());
            }
            let optical = mode
                .color_physical
                .as_ref()
                .unwrap()
                .paths
                .iter()
                .find(|p| p.id == current.path_id)
                .unwrap();
            for correction in &path.emitters {
                let emitter = match &optical.source {
                    OpticalSource::Additive { emitters } => {
                        emitters.iter().find(|e| e.id == correction.emitter_id)
                    }
                    _ => None,
                }
                .ok_or("installed Color gain references a missing additive emitter")?;
                if emitter.xyz.is_some_and(|xyz| {
                    [xyz.x, xyz.y, xyz.z]
                        .iter()
                        .any(|v| !(v * correction.output_gain).is_finite())
                }) || emitter
                    .spectrum
                    .iter()
                    .any(|s| !(s.value * correction.output_gain).is_finite())
                {
                    return Err("installed Color gain overflows the authored emitter output".into());
                }
            }
            for sample in &path.measurements {
                mode.validate_native_color_recipe(optical, &sample.recipe)
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    /// Call once when configuration changes. A stale result disables every correction in this
    /// calibration; it does not block loading or mutate the recorded observations.
    pub fn status(
        &self,
        profile: &FixtureProfile,
        mode_id: Uuid,
    ) -> InstalledColorCalibrationStatus {
        match self.validate_for_profile(profile, mode_id) {
            Ok(()) => InstalledColorCalibrationStatus::Current,
            Err(reason) => InstalledColorCalibrationStatus::Stale { reason },
        }
    }
}

/// Compact configuration-time context. It retains no source archive or model assets.
#[derive(Clone, Debug)]
pub struct ColorCalibrationContext {
    mode: crate::FixtureMode,
    identities: Vec<NativeColorIdentity>,
}
impl ColorCalibrationContext {
    pub fn new(profile: &FixtureProfile, mode_id: Uuid) -> Result<Self, String> {
        let identities = profile
            .native_color_identities(mode_id)
            .map_err(|e| e.to_string())?;
        Ok(Self {
            mode: profile.mode(mode_id).unwrap().clone(),
            identities,
        })
    }
    /// Verify the selected optical interpretation without rehashing a compact runtime profile.
    /// Unrelated compatibility repairs (such as shutter functions) do not change Color identity.
    pub fn validate_runtime_profile(
        &self,
        profile: &FixtureProfile,
        mode_id: Uuid,
    ) -> Result<(), String> {
        let mode = profile
            .mode(mode_id)
            .ok_or("runtime Color mode is missing")?;
        if self.identities.iter().any(|id| {
            id.profile_id != profile.id.0
                || id.profile_revision != profile.revision
                || id.mode_id != mode_id
        }) || self.mode.id != mode_id
            || serde_json::to_value(&self.mode.color_physical).map_err(|e| e.to_string())?
                != serde_json::to_value(&mode.color_physical).map_err(|e| e.to_string())?
        {
            return Err("runtime Color model differs from its authoritative source".into());
        }
        let Some(model) = &self.mode.color_physical else {
            return Err("authoritative Color context has no model".into());
        };
        let controls = model
            .paths
            .iter()
            .flat_map(|p| p.controls.iter())
            .copied()
            .collect::<std::collections::HashSet<_>>();
        for id in controls {
            let original = self.mode.channels.iter().find(|c| c.id == id);
            let current = mode.channels.iter().find(|c| c.id == id);
            if original.is_none()
                || current.is_none()
                || serde_json::to_value(original).map_err(|e| e.to_string())?
                    != serde_json::to_value(current).map_err(|e| e.to_string())?
            {
                return Err(
                    "runtime native Color control differs from its authoritative source".into(),
                );
            }
        }
        Ok(())
    }
    pub fn identities(&self) -> &[NativeColorIdentity] {
        &self.identities
    }
    pub fn mode(&self) -> &crate::FixtureMode {
        &self.mode
    }
}

pub fn validate_color_calibration(value: Option<&InstalledColorCalibration>) -> Result<(), String> {
    value.map_or(Ok(()), InstalledColorCalibration::validate)
}
