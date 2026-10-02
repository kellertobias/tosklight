use super::{
    ChannelFunction, ChannelFunctionBehavior, ChannelResolution, FixtureChannel, ProfileError,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use light_core::{OpeningConvention, PhysicalDataQuality};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PhysicalMappingPoint {
    pub raw: u32,
    pub physical: f32,
}

/// Optional calibration of one continuous function. Its existing physical endpoints
/// remain authoritative. Empty samples mean linear interpolation between those endpoints.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PhysicalMappingCalibration {
    #[serde(default)]
    pub quality: PhysicalDataQuality,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default)]
    pub revision: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub samples: Vec<PhysicalMappingPoint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opening_convention: Option<OpeningConvention>,
}

/// Parsed once when a mapping is compiled. Unrecognized units remain explicit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PhysicalUnit {
    Degrees,
    DegreesPerSecond,
    Percent,
    Normalized,
    Metres,
    Unknown,
    Custom(String),
}

impl PhysicalUnit {
    pub fn parse(unit: Option<&str>) -> Self {
        let Some(unit) = unit.map(str::trim).filter(|unit| !unit.is_empty()) else {
            return Self::Unknown;
        };
        match unit.to_ascii_lowercase().as_str() {
            "deg" | "degree" | "degrees" | "°" => Self::Degrees,
            "deg/s" | "degree/s" | "degrees/s" | "degrees per second" | "°/s" => {
                Self::DegreesPerSecond
            }
            "%" | "percent" | "percentage" => Self::Percent,
            "normalized" | "normalised" | "0..1" => Self::Normalized,
            "m" | "metre" | "metres" | "meter" | "meters" => Self::Metres,
            _ => Self::Custom(unit.to_owned()),
        }
    }
}

/// Native command and its achieved physical result. This is not measured motor feedback.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicalMappingResult {
    pub raw: u32,
    pub physical: f64,
    /// Request was outside the function's range; rounding alone does not set this flag.
    pub clipped: bool,
}

/// Immutable function-local mapping. No unit parsing, allocation or calibration lookup
/// occurs when evaluating it. Raw arithmetic uses f64 to preserve all 32 DMX bits.
#[derive(Clone, Debug)]
pub struct CompiledPhysicalMapping {
    pub channel_id: Uuid,
    pub function_id: Uuid,
    pub resolution: ChannelResolution,
    pub unit: PhysicalUnit,
    pub quality: PhysicalDataQuality,
    pub source: Option<String>,
    pub calibration_revision: u32,
    pub opening_convention: Option<OpeningConvention>,
    points: Box<[PhysicalMappingPoint]>,
}

fn invalid(message: &str) -> ProfileError {
    ProfileError::Invalid(format!("physical mapping: {message}"))
}

impl CompiledPhysicalMapping {
    /// Returns None for a noncontinuous function without physical calibration.
    /// Calibration on an incompatible function is an error, never silently ignored.
    pub fn compile(
        channel: &FixtureChannel,
        function: &ChannelFunction,
    ) -> Result<Option<Self>, ProfileError> {
        let ChannelFunctionBehavior::Continuous {
            physical_min,
            physical_max,
            unit,
        } = &function.behavior
        else {
            return if function.physical_mapping.is_some() {
                Err(invalid("calibration requires a continuous function"))
            } else {
                Ok(None)
            };
        };
        if function.dmx_from >= function.dmx_to || function.dmx_to > channel.resolution.max_raw() {
            return Err(invalid(
                "continuous raw endpoints must increase within the channel resolution",
            ));
        }
        if !physical_min.is_finite() || !physical_max.is_finite() || physical_min == physical_max {
            return Err(invalid("physical endpoints must be finite and different"));
        }
        let unit = PhysicalUnit::parse(unit.as_deref());
        let calibration = function.physical_mapping.as_ref();
        let quality = calibration.map_or(PhysicalDataQuality::Unknown, |value| value.quality);
        let source = calibration.and_then(|value| value.source.clone());
        if matches!(
            quality,
            PhysicalDataQuality::Manufacturer | PhysicalDataQuality::Measured
        ) && source
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
        {
            return Err(invalid(
                "manufacturer and measured mappings require a source",
            ));
        }
        let opening_convention = calibration.and_then(|value| value.opening_convention);
        if opening_convention.is_some()
            && (function.attribute.0.as_ref() != "zoom" || unit != PhysicalUnit::Degrees)
        {
            return Err(invalid(
                "beam/field convention requires a Zoom function with explicit degree units",
            ));
        }
        let first = PhysicalMappingPoint {
            raw: function.dmx_from,
            physical: *physical_min,
        };
        let last = PhysicalMappingPoint {
            raw: function.dmx_to,
            physical: *physical_max,
        };
        let samples = calibration.map_or(&[][..], |value| value.samples.as_slice());
        let points = if samples.is_empty() {
            vec![first, last]
        } else {
            if samples.len() < 2 || samples.first() != Some(&first) || samples.last() != Some(&last)
            {
                return Err(invalid(
                    "samples must include both exact raw and physical function endpoints",
                ));
            }
            let ascending = physical_max > physical_min;
            for pair in samples.windows(2) {
                let [a, b] = pair else { unreachable!() };
                if a.raw >= b.raw
                    || !a.physical.is_finite()
                    || !b.physical.is_finite()
                    || (ascending && a.physical >= b.physical)
                    || (!ascending && a.physical <= b.physical)
                {
                    return Err(invalid(
                        "samples must have increasing raw values and strictly monotonic finite physical values",
                    ));
                }
            }
            samples.to_vec()
        };
        Ok(Some(Self {
            channel_id: channel.id,
            function_id: function.id,
            resolution: channel.resolution,
            unit,
            quality,
            source,
            calibration_revision: calibration.map_or(0, |value| value.revision),
            opening_convention,
            points: points.into_boxed_slice(),
        }))
    }

    pub fn physical_for_raw(&self, requested_raw: u32) -> PhysicalMappingResult {
        let first = self.points[0];
        let last = self.points[self.points.len() - 1];
        let raw = requested_raw.clamp(first.raw, last.raw);
        let index = self
            .points
            .partition_point(|point| point.raw < raw)
            .clamp(1, self.points.len() - 1);
        let a = self.points[index - 1];
        let b = self.points[index];
        let t = f64::from(raw - a.raw) / f64::from(b.raw - a.raw);
        PhysicalMappingResult {
            raw,
            physical: f64::from(a.physical) + t * (f64::from(b.physical) - f64::from(a.physical)),
            clipped: raw != requested_raw,
        }
    }

    pub fn raw_for_physical(&self, requested: f64) -> Result<PhysicalMappingResult, ProfileError> {
        if !requested.is_finite() {
            return Err(invalid("requested value must be finite"));
        }
        let first = f64::from(self.points[0].physical);
        let last = f64::from(self.points[self.points.len() - 1].physical);
        let physical = requested.clamp(first.min(last), first.max(last));
        let ascending = last > first;
        let index = self
            .points
            .partition_point(|point| {
                if ascending {
                    f64::from(point.physical) < physical
                } else {
                    f64::from(point.physical) > physical
                }
            })
            .clamp(1, self.points.len() - 1);
        let a = self.points[index - 1];
        let b = self.points[index];
        let t =
            (physical - f64::from(a.physical)) / (f64::from(b.physical) - f64::from(a.physical));
        let raw = (f64::from(a.raw) + t * f64::from(b.raw - a.raw)).round() as u32;
        let mut achieved = self.physical_for_raw(raw);
        achieved.clipped = physical != requested;
        Ok(achieved)
    }
}
