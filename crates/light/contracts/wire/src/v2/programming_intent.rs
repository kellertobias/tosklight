//! Versioned wire forms for complete programming owners and their editable components.
//! Domain conversion belongs to the adapter; this crate remains independent of workspace crates.
use super::programming::ProgrammingColorXyz;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

fn one() -> f32 {
    1.0
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProgrammingPhysicalDataQuality {
    #[default]
    Unknown,
    Estimated,
    Manufacturer,
    Measured,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProgrammingOpeningConvention {
    Beam,
    Field,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct ProgrammingNativeColorBinding {
    pub channel_id: Uuid,
    pub function_id: Uuid,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct ProgrammingNativeColorValue {
    pub channel_id: Uuid,
    pub function_id: Uuid,
    /// Premaster native value. u32 preserves every supported channel width exactly.
    pub raw: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct ProgrammingNativeColorIdentity {
    pub profile_id: Uuid,
    pub profile_revision: u32,
    pub profile_digest: String,
    pub mode_id: Uuid,
    pub head_id: Uuid,
    pub path_id: Uuid,
    pub model_revision: u32,
    pub native_layout_signature: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProgrammingScalarDomain {
    /// Unknown fixture limits remain unknown. Signed angles and metres are not percentages.
    Finite,
    Bounded {
        bounds: ProgrammingAttributeBounds,
    },
    Cyclic {
        bounds: ProgrammingAttributeBounds,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProgrammingScalarInterpolation {
    Linear,
    ShortestArc,
    Reciprocal,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ProgrammingScalarIntent {
    Value(f32),
    Spread(Vec<f32>),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProgrammingOwner {
    Color,
    Position,
    Focus,
    Zoom,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProgrammingColorComponent {
    Red,
    Green,
    Blue,
    Amber,
    Hue,
    Saturation,
    WhiteBlend,
    Temperature,
    Duv,
    Uv,
    RelativeOutput,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "kind", content = "component", rename_all = "snake_case")]
pub enum ProgrammingComponent {
    Color(ProgrammingColorComponent),
    ColorWheel(u16),
    NativeColor(ProgrammingNativeColorBinding),
    Pan,
    Tilt,
    TargetReference,
    TargetX,
    TargetY,
    TargetZ,
    Focus,
    Zoom,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProgrammingComponentRole {
    ColorRecipe,
    ColorCoordinate,
    ColorOrthogonal,
    ColorWheel,
    NativeColor,
    Angle,
    Target,
    Focus,
    Zoom,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProgrammingComponentUnit {
    Percent,
    Degrees,
    Metres,
    Kelvin,
    Duv,
    Factor,
    NativeInteger,
    Selection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProgrammingAuthoringCapability {
    /// The request can be authored even when a selected lamp cannot reproduce it.
    SemanticIntent,
    VerifiedNativeControl,
    FocusParameter,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, JsonSchema, TS, Deserialize)]
pub struct ProgrammingComponentDescriptor {
    pub owner: ProgrammingOwner,
    pub role: ProgrammingComponentRole,
    pub unit: ProgrammingComponentUnit,
    pub domain: Option<ProgrammingScalarDomain>,
    pub step: f32,
    pub fine_step: f32,
    pub display_scale: f32,
    pub interpolation: ProgrammingScalarInterpolation,
    pub capability: ProgrammingAuthoringCapability,
    pub spread: bool,
    pub align: bool,
    pub dynamics: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct ProgrammingVirtualColorRecipe {
    pub version: u16,
    pub rgb: [f32; 3],
    pub amber: f32,
    /// Advanced coordinates can be retained while the Easy controls show an approximation.
    pub approximate: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct ProgrammingWhiteTarget {
    pub kelvin: f32,
    pub duv: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct ProgrammingUvIntent {
    pub amount: f32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProgrammingColorAllocation {
    #[default]
    PreserveRecipe,
    PreferWhite,
    PreferColoredEmitters,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct ProgrammingColorWheelConstraint {
    pub source: ProgrammingNativeColorIdentity,
    pub value: ProgrammingNativeColorValue,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct ProgrammingColorComponentSpread {
    pub component: ProgrammingColorComponent,
    pub points: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct ProgrammingColorIntent {
    /// Authoritative base XYZ in the pinned virtual engine. Zero is black, not D65 white.
    pub base_xyz: ProgrammingColorXyz,
    pub recipe: ProgrammingVirtualColorRecipe,
    pub white_blend: f32,
    pub white_target: ProgrammingWhiteTarget,
    #[serde(default)]
    pub uv: ProgrammingUvIntent,
    #[serde(default = "one")]
    pub relative_output: f32,
    #[serde(default)]
    pub allocation: ProgrammingColorAllocation,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wheel_constraints: Vec<ProgrammingColorWheelConstraint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spreads: Vec<ProgrammingColorComponentSpread>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct ProgrammingNativeColorRecipe {
    pub source: ProgrammingNativeColorIdentity,
    pub channels: Vec<ProgrammingNativeColorValue>,
    /// Exact endpoints for profile-declared continuous native controls. Resolve before source
    /// prediction so each destination fits the varying recipe rather than a frozen estimate.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spreads: Vec<ProgrammingNativeColorSpread>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct ProgrammingNativeColorSpread {
    pub binding: ProgrammingNativeColorBinding,
    pub points: Vec<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct ProgrammingNativeColorComponentDescriptor {
    pub binding: ProgrammingNativeColorBinding,
    pub raw_from: u32,
    pub raw_to: u32,
    pub continuous: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct ProgrammingPortableVisibleColor {
    pub xyz: ProgrammingColorXyz,
    pub relative_output: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct ProgrammingPortableUv {
    pub amount: f32,
    pub quality: ProgrammingPhysicalDataQuality,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct ProgrammingPortableColorEstimate {
    pub model_revision: u32,
    /// None means unknown, independently of the other component. Some(zero) means known black.
    pub visible: Option<ProgrammingPortableVisibleColor>,
    pub uv: Option<ProgrammingPortableUv>,
    pub quality: ProgrammingPhysicalDataQuality,
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProgrammingColorProgram {
    Semantic {
        intent: ProgrammingColorIntent,
    },
    /// The complete recipe and its pinned source prediction form one atomic recorded value.
    Direct {
        recipe: ProgrammingNativeColorRecipe,
        portable: ProgrammingPortableColorEstimate,
    },
}

#[derive(
    Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize, JsonSchema, TS,
)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProgrammingTargetReference {
    #[default]
    Origin,
    Point {
        point_id: Uuid,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProgrammingPositionIntent {
    Angles {
        pan_degrees: ProgrammingScalarIntent,
        tilt_degrees: ProgrammingScalarIntent,
    },
    Target {
        reference: ProgrammingTargetReference,
        offset_metres: [ProgrammingScalarIntent; 3],
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct ProgrammingZoomIntent {
    pub opening_degrees: ProgrammingScalarIntent,
    pub convention: ProgrammingOpeningConvention,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct ProgrammingAttributeBounds {
    pub min: f32,
    pub max: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ProgrammingScalarEdit {
    Set(ProgrammingScalarIntent),
    Relative(f32),
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ProgrammingNativeColorEdit {
    Set(u32),
    Spread(Vec<u32>),
    Relative(#[ts(type = "number")] i64),
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProgrammingComponentEdit {
    ActivateAngles,
    Scalar {
        component: ProgrammingComponent,
        operation: ProgrammingScalarEdit,
    },
    Target {
        reference: ProgrammingTargetReference,
    },
    Coordinates {
        xyz: ProgrammingColorXyz,
    },
    Native {
        binding: ProgrammingNativeColorBinding,
        operation: ProgrammingNativeColorEdit,
    },
}

/// Complete member exceptions remain scoped to the live Group. Nested assignments are invalid.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ProgrammingGroupFamilyAssignment {
    pub owner: ProgrammingOwner,
    pub template: super::programming::ProgrammingAttributeValue,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub members: std::collections::BTreeMap<Uuid, super::programming::ProgrammingAttributeValue>,
}
