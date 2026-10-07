//! Portable installation calibration, separate from mounting pose and live control.
use crate::{FixtureProfile, PhysicalDataQuality, PositionAxisRole, PositionCalibrationIdentity};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Per-physical-instance zero correction for the future calibrated Position model.
///
/// `calibrated_degrees = sign * authored_physical_degrees + zero_degrees`, where
/// sign is +1 for normal and -1 for inverted. Mount rotation and bracket angle are
/// composed separately. The existing normalized inversion/output path does not
/// consume this metadata yet; storing it must not change current DMX output.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct InstalledPositionCalibration {
    #[serde(default)]
    pub revision: u32,
    #[serde(default)]
    pub quality: PhysicalDataQuality,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default)]
    pub pan_zero_degrees: f32,
    #[serde(default)]
    pub tilt_zero_degrees: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub axis_overrides: Option<InstalledAxisOverrides>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InstalledAxisOverrides {
    pub version: u16,
    pub source_identity: PositionCalibrationIdentity,
    pub axes: Vec<InstalledAxisCalibration>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InstalledAxisCalibration {
    pub node_id: Uuid,
    pub zero_degrees: f32,
    pub invert: bool,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EffectiveAxisCalibration {
    pub zero_degrees: f64,
    pub invert: bool,
}
impl EffectiveAxisCalibration {
    pub fn physical_to_calibrated(self, degrees: f64) -> f64 {
        self.sign() * degrees + self.zero_degrees
    }
    pub fn calibrated_to_physical(self, degrees: f64) -> f64 {
        self.sign() * (degrees - self.zero_degrees)
    }
    pub fn sign(self) -> f64 {
        if self.invert { -1.0 } else { 1.0 }
    }
}
/// Small immutable context compiled from a complete profile at authoring time.
#[derive(Clone, Debug)]
pub struct PositionCalibrationContext {
    pub identity: PositionCalibrationIdentity,
    pub axis_ids: Vec<Uuid>,
    pub axis_roles: std::collections::HashMap<Uuid, PositionAxisRole>,
}
impl PositionCalibrationContext {
    pub fn new(profile: &FixtureProfile, mode_id: Uuid) -> Result<Option<Self>, String> {
        let Some(identity) = profile
            .position_calibration_identity(mode_id)
            .map_err(|e| e.to_string())?
        else {
            return Ok(None);
        };
        let model = profile
            .mode(mode_id)
            .and_then(|m| m.position_physical.as_ref())
            .ok_or("Position model is missing")?;
        let mut axis_ids = model.bindings.iter().map(|b| b.node_id).collect::<Vec<_>>();
        axis_ids.sort();
        axis_ids.dedup();
        Ok(Some(Self {
            identity,
            axis_ids,
            axis_roles: model.bindings.iter().map(|b| (b.node_id, b.role)).collect(),
        }))
    }
}
impl InstalledAxisOverrides {
    pub fn validate(&self) -> Result<(), String> {
        self.source_identity.validate()?;
        if self.version != 1 || self.axes.is_empty() || self.axes.len() > 4096 {
            return Err("Position axis overrides require version 1 and 1–4096 axes".into());
        }
        let mut seen = std::collections::HashSet::new();
        for axis in &self.axes {
            if axis.node_id.is_nil() || !seen.insert(axis.node_id) || !axis.zero_degrees.is_finite()
            {
                return Err(
                    "Position axis overrides require unique non-nil nodes and finite zeros".into(),
                );
            }
        }
        Ok(())
    }
    pub fn validate_for_context(&self, context: &PositionCalibrationContext) -> Result<(), String> {
        self.validate()?;
        if self.source_identity != context.identity
            || self
                .axes
                .iter()
                .any(|a| !context.axis_ids.contains(&a.node_id))
        {
            return Err(
                "Position axis overrides belong to a different profile, mode or physical geometry"
                    .into(),
            );
        }
        Ok(())
    }
}

impl InstalledPositionCalibration {
    /// An override replaces the complete default pair; offsets and inversion never stack.
    /// A stale set is an error so consumers must surface it before choosing a fallback.
    pub fn effective_axis(
        &self,
        context: &PositionCalibrationContext,
        node_id: Uuid,
        role: PositionAxisRole,
        invert_pan: bool,
        invert_tilt: bool,
    ) -> Result<EffectiveAxisCalibration, String> {
        if context.axis_roles.get(&node_id) != Some(&role) {
            return Err("unknown Position axis or mismatched role".into());
        }
        if let Some(overrides) = &self.axis_overrides {
            overrides.validate_for_context(context)?;
            if let Some(axis) = overrides.axes.iter().find(|a| a.node_id == node_id) {
                return Ok(EffectiveAxisCalibration {
                    zero_degrees: f64::from(axis.zero_degrees),
                    invert: axis.invert,
                });
            }
        }
        Ok(match role {
            PositionAxisRole::Pan => EffectiveAxisCalibration {
                zero_degrees: f64::from(self.pan_zero_degrees),
                invert: invert_pan,
            },
            PositionAxisRole::Tilt => EffectiveAxisCalibration {
                zero_degrees: f64::from(self.tilt_zero_degrees),
                invert: invert_tilt,
            },
        })
    }
    pub fn validate(&self) -> Result<(), String> {
        if let Some(overrides) = &self.axis_overrides {
            overrides.validate()?;
        }
        if !self.pan_zero_degrees.is_finite() || !self.tilt_zero_degrees.is_finite() {
            return Err("position calibration zero offsets must be finite degrees".into());
        }
        if self
            .source
            .as_ref()
            .is_some_and(|source| source.len() > 1024)
        {
            return Err("position calibration source must be at most 1024 bytes".into());
        }
        if matches!(
            self.quality,
            PhysicalDataQuality::Manufacturer | PhysicalDataQuality::Measured
        ) && self
            .source
            .as_ref()
            .is_none_or(|source| source.trim().is_empty())
        {
            return Err("manufacturer and measured position calibration need a source".into());
        }
        Ok(())
    }
}

/// Shared by the portable codec and application updates; absence is an uncalibrated instance.
pub fn validate_position_calibration(
    value: Option<&InstalledPositionCalibration>,
) -> Result<(), String> {
    value.map_or(Ok(()), InstalledPositionCalibration::validate)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn absent_evidence_defaults_to_unknown_and_signed_unwrapped_offsets_are_valid() {
        let mut value: InstalledPositionCalibration = serde_json::from_str("{}").unwrap();
        assert_eq!(value, InstalledPositionCalibration::default());
        value.pan_zero_degrees = -1080.5;
        value.tilt_zero_degrees = 720.25;
        assert!(value.validate().is_ok());
        assert!(validate_position_calibration(None).is_ok());
    }
    #[test]
    fn quality_requires_evidence_and_offsets_must_be_finite() {
        for quality in [
            PhysicalDataQuality::Manufacturer,
            PhysicalDataQuality::Measured,
        ] {
            let mut value = InstalledPositionCalibration {
                quality,
                ..Default::default()
            };
            assert!(value.validate().unwrap_err().contains("source"));
            value.source = Some("  ".into());
            assert!(value.validate().is_err());
            value.source = Some("Commissioning record".into());
            assert!(value.validate().is_ok());
            value.pan_zero_degrees = f32::INFINITY;
            assert!(value.validate().unwrap_err().contains("finite"));
            value.pan_zero_degrees = 0.0;
            value.tilt_zero_degrees = f32::NAN;
            assert!(value.validate().is_err());
        }
        let value = InstalledPositionCalibration {
            source: Some("é".repeat(513)),
            ..Default::default()
        };
        assert!(value.validate().unwrap_err().contains("1024"));
    }
}
