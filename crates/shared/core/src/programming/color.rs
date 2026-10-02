use super::{
    ColorComponent, IntentError, ProgrammingComponent, ScalarDomain, ScalarIntent, require,
};
use crate::{NativeColorBinding, NativeColorIdentity, NativeColorValue, PhysicalDataQuality, Xyz};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Virtual authoring levels, never the selected fixture's actual emitter drives. RGB is sRGB
/// encoded at the editing boundary. The version pins the virtual engine used to produce XYZ.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VirtualColorRecipe {
    pub version: u16,
    pub rgb: [f32; 3],
    pub amber: f32,
    /// Advanced coordinates can be retained while the Easy controls show an approximation.
    pub approximate: bool,
}
impl Default for VirtualColorRecipe {
    fn default() -> Self {
        Self {
            version: 1,
            rgb: [1.0; 3],
            amber: 0.0,
            approximate: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WhiteTarget {
    pub kelvin: f32,
    pub duv: f32,
}
impl Default for WhiteTarget {
    fn default() -> Self {
        Self {
            kelvin: 6500.0,
            duv: 0.0,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct UvIntent {
    pub amount: f32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorAllocation {
    #[default]
    PreserveRecipe,
    PreferWhite,
    PreferColoredEmitters,
}

/// A deliberate native wheel constraint remains pinned to its source. Adapters must report an
/// incompatible constraint; they must not reinterpret the raw slot as a destination wheel index.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColorWheelConstraint {
    pub source: NativeColorIdentity,
    pub value: NativeColorValue,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColorComponentSpread {
    pub component: ColorComponent,
    pub points: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColorIntent {
    /// Authoritative base XYZ in the pinned virtual engine. Zero is black, not D65 white.
    pub base_xyz: Xyz,
    pub recipe: VirtualColorRecipe,
    pub white_blend: f32,
    pub white_target: WhiteTarget,
    #[serde(default)]
    pub uv: UvIntent,
    #[serde(default = "one")]
    pub relative_output: f32,
    #[serde(default)]
    pub allocation: ColorAllocation,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wheel_constraints: Vec<ColorWheelConstraint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spreads: Vec<ColorComponentSpread>,
}
fn one() -> f32 {
    1.0
}
impl Default for ColorIntent {
    fn default() -> Self {
        Self {
            base_xyz: crate::color_intent::D65_WHITE,
            recipe: Default::default(),
            white_blend: 0.0,
            white_target: Default::default(),
            uv: Default::default(),
            relative_output: 1.0,
            allocation: Default::default(),
            wheel_constraints: vec![],
            spreads: vec![],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NativeColorRecipe {
    pub source: NativeColorIdentity,
    pub channels: Vec<NativeColorValue>,
    /// Exact endpoints for profile-declared continuous native controls. Resolve before source
    /// prediction so each destination fits the varying recipe rather than a frozen estimate.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spreads: Vec<NativeColorSpread>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NativeColorSpread {
    pub binding: NativeColorBinding,
    pub points: Vec<u32>,
}

/// Profile-compiled native metadata retains integer width; generic scalar descriptors never
/// reinterpret a 32-bit channel as a float percentage. Wheel/function selection stays discrete.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NativeColorComponentDescriptor {
    pub binding: NativeColorBinding,
    pub raw_from: u32,
    pub raw_to: u32,
    pub continuous: bool,
}
impl NativeColorSpread {
    pub fn validate(&self, descriptor: NativeColorComponentDescriptor) -> Result<(), IntentError> {
        require(
            descriptor.binding == self.binding && descriptor.continuous,
            "native spread requires its verified continuous function",
        )?;
        require(
            (2..=4096).contains(&self.points.len()),
            "native spread requires 2-4096 control points",
        )?;
        let range =
            descriptor.raw_from.min(descriptor.raw_to)..=descriptor.raw_from.max(descriptor.raw_to);
        require(
            self.points.iter().all(|raw| range.contains(raw)),
            "native spread endpoint is outside its function",
        )
    }

    /// Compile using the same ordered anchor rule as scalar spreads, with exact integer math.
    /// Floating point conversion would lose low bytes on 24/32-bit fixtures.
    pub fn resolve(&self, count: usize) -> Result<Vec<u32>, IntentError> {
        self.resolve_selected(count, &(0..count).collect::<Vec<_>>())
    }
    pub fn resolve_selected(&self, count: usize, ranks: &[usize]) -> Result<Vec<u32>, IntentError> {
        require(
            (2..=4096).contains(&self.points.len()),
            "native spread requires 2-4096 control points",
        )?;
        require(
            ranks.iter().all(|rank| *rank < count),
            "native spread rank is outside its selection",
        )?;
        let layout = super::ranks::SpreadRankLayout::new(self.points.len(), count);
        Ok(ranks
            .iter()
            .map(|rank| {
                let (left, right, step, span) = layout.weights(*rank);
                let numerator = u128::from(self.points[left]) * (span - step)
                    + u128::from(self.points[right]) * step;
                ((numerator + span / 2) / span) as u32
            })
            .collect())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PortableVisibleColor {
    pub xyz: Xyz,
    pub relative_output: f32,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PortableUv {
    pub amount: f32,
    pub quality: PhysicalDataQuality,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PortableColorEstimate {
    pub model_revision: u32,
    /// None means unknown, independently of the other component. Some(zero) means known black.
    pub visible: Option<PortableVisibleColor>,
    pub uv: Option<PortableUv>,
    pub quality: PhysicalDataQuality,
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ColorProgram {
    Semantic {
        intent: ColorIntent,
    },
    /// The complete recipe and its pinned source prediction form one atomic recorded value.
    Direct {
        recipe: NativeColorRecipe,
        portable: PortableColorEstimate,
    },
}

pub(super) fn valid_xyz(xyz: Xyz) -> bool {
    [xyz.x, xyz.y, xyz.z]
        .into_iter()
        .all(|v| v.is_finite() && v >= 0.0)
}
fn validate_identity(source: &NativeColorIdentity) -> Result<(), IntentError> {
    require(
        [
            source.profile_id,
            source.mode_id,
            source.head_id,
            source.path_id,
        ]
        .into_iter()
        .all(|id| !id.is_nil()),
        "native source identity requires stable UUIDs",
    )?;
    require(
        [&source.profile_digest, &source.native_layout_signature]
            .into_iter()
            .all(|s| !s.is_empty() && s.len() <= 256 && !s.chars().any(char::is_control)),
        "native source digests must contain 1-256 printable bytes",
    )
}
impl NativeColorIdentity {
    pub fn validate(&self) -> Result<(), IntentError> {
        validate_identity(self)
    }
}
impl ColorIntent {
    pub fn validate(&self) -> Result<(), IntentError> {
        require(
            valid_xyz(self.base_xyz),
            "base XYZ must be finite and nonnegative",
        )?;
        require(
            self.recipe.version == 1,
            "unsupported virtual Color recipe version",
        )?;
        require(
            self.recipe
                .rgb
                .into_iter()
                .chain([self.recipe.amber, self.white_blend, self.uv.amount])
                .all(|v| ScalarDomain::UNIT.contains(v)),
            "Color levels must be finite and within 0-1",
        )?;
        require(
            ScalarDomain::KELVIN.contains(self.white_target.kelvin)
                && ScalarDomain::DUV.contains(self.white_target.duv),
            "white target is outside the Kelvin or Duv domain",
        )?;
        require(
            self.relative_output.is_finite() && self.relative_output >= 0.0,
            "relative visible output must be finite and nonnegative",
        )?;
        if !self.recipe.approximate {
            let expected = super::VirtualColorAuthoringV1::recipe_xyz(&self.recipe)?;
            require(
                (self.base_xyz.x - expected.x).abs() <= 0.0001
                    && (self.base_xyz.y - expected.y).abs() <= 0.0001
                    && (self.base_xyz.z - expected.z).abs() <= 0.0001,
                "exact virtual recipe disagrees with authoritative Color coordinates",
            )?;
        }
        require(self.spreads.len() <= 11, "too many Color component spreads")?;
        let mut components = HashSet::new();
        let mut recipe = false;
        let mut coordinates = false;
        for spread in &self.spreads {
            require(
                components.insert(spread.component),
                "duplicate Color component spread",
            )?;
            let descriptor = ProgrammingComponent::Color(spread.component).descriptor();
            recipe |= descriptor.role == super::ComponentRole::ColorRecipe;
            coordinates |= descriptor.role == super::ComponentRole::ColorCoordinate;
            ScalarIntent::Spread(spread.points.clone())
                .validate(descriptor.domain.expect("Color scalar domain"))?;
        }
        require(
            !(recipe && coordinates),
            "Color recipe and coordinate spreads cannot write the same base",
        )?;
        require(
            self.wheel_constraints.len() <= 32,
            "too many Color wheel constraints",
        )?;
        let mut wheels = HashSet::new();
        for constraint in &self.wheel_constraints {
            validate_identity(&constraint.source)?;
            require(
                !constraint.value.channel_id.is_nil() && !constraint.value.function_id.is_nil(),
                "wheel constraint requires channel/function UUIDs",
            )?;
            require(
                wheels.insert((constraint.source.path_id, constraint.value.channel_id)),
                "duplicate wheel constraint",
            )?;
        }
        Ok(())
    }

    /// Visible-only envelope; independent UV and Intensity are deliberately absent.
    pub fn blend_visible(&self, white_xyz: Xyz) -> Result<Xyz, IntentError> {
        require(
            valid_xyz(white_xyz),
            "white target XYZ must be finite and nonnegative",
        )?;
        let colored = (2.0 * (1.0 - f64::from(self.white_blend))).min(1.0);
        let white = (2.0 * f64::from(self.white_blend)).min(1.0);
        let mix = |base: f32, target: f32| {
            (f64::from(self.relative_output)
                * (colored * f64::from(base) + white * f64::from(target))) as f32
        };
        let xyz = Xyz {
            x: mix(self.base_xyz.x, white_xyz.x),
            y: mix(self.base_xyz.y, white_xyz.y),
            z: mix(self.base_xyz.z, white_xyz.z),
        };
        require(
            valid_xyz(xyz),
            "visible Color result exceeds the finite output domain",
        )?;
        Ok(xyz)
    }
}
impl NativeColorRecipe {
    pub fn validate(&self) -> Result<(), IntentError> {
        validate_identity(&self.source)?;
        require(
            (1..=512).contains(&self.channels.len()),
            "native Color recipe requires 1-512 channels",
        )?;
        let mut channels = HashSet::new();
        for value in &self.channels {
            require(
                !value.channel_id.is_nil() && !value.function_id.is_nil(),
                "native Color value requires channel/function UUIDs",
            )?;
            require(
                channels.insert(value.channel_id),
                "native Color recipe repeats a channel",
            )?;
        }
        require(
            self.spreads.len() <= self.channels.len(),
            "too many native Color spreads",
        )?;
        let mut spread_channels = HashSet::new();
        for spread in &self.spreads {
            require(
                (2..=4096).contains(&spread.points.len()),
                "native spread requires 2-4096 control points",
            )?;
            require(
                spread_channels.insert(spread.binding.channel_id),
                "duplicate native Color spread",
            )?;
            require(
                self.channels.iter().any(|value| {
                    value.channel_id == spread.binding.channel_id
                        && value.function_id == spread.binding.function_id
                }),
                "native spread must belong to the complete reference recipe",
            )?;
        }
        // Exact complete ownership, continuous eligibility and raw ranges need the pinned adapter.
        Ok(())
    }
}
impl PortableColorEstimate {
    pub fn validate(&self) -> Result<(), IntentError> {
        if let Some(visible) = self.visible {
            require(
                valid_xyz(visible.xyz)
                    && visible.relative_output.is_finite()
                    && visible.relative_output >= 0.0,
                "portable visible estimate must preserve finite nonnegative XYZ/output",
            )?;
        }
        if let Some(uv) = self.uv {
            require(
                ScalarDomain::UNIT.contains(uv.amount),
                "portable UV amount must be within 0-1",
            )?;
        }
        require(
            self.limitations.len() <= 64 && self.limitations.iter().all(|s| s.len() <= 1024),
            "portable Color limitations exceed the contract limit",
        )
    }
}
impl ColorProgram {
    pub fn validate(&self) -> Result<(), IntentError> {
        match self {
            Self::Semantic { intent } => intent.validate(),
            Self::Direct { recipe, portable } => {
                recipe.validate()?;
                require(
                    recipe.source.model_revision == portable.model_revision,
                    "portable Color estimate must use the pinned source model revision",
                )?;
                portable.validate()
            }
        }
    }
}
