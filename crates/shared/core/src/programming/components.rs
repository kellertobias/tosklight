use super::{ScalarDomain, ScalarInterpolation};
use crate::{AttributeBounds, AttributeKey, NativeColorBinding};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProgrammingOwner {
    Color,
    Position,
    Focus,
    Zoom,
}
impl ProgrammingOwner {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Color => "color",
            Self::Position => "position",
            Self::Focus => "focus",
            Self::Zoom => "zoom",
        }
    }
    pub fn key(self) -> AttributeKey {
        AttributeKey(self.id().into())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorComponent {
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

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(tag = "kind", content = "component", rename_all = "snake_case")]
pub enum ProgrammingComponent {
    Color(ColorComponent),
    ColorWheel(u16),
    NativeColor(NativeColorBinding),
    Pan,
    Tilt,
    TargetReference,
    TargetX,
    TargetY,
    TargetZ,
    Focus,
    Zoom,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentRole {
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
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentUnit {
    Percent,
    Degrees,
    Metres,
    Kelvin,
    Duv,
    Factor,
    NativeInteger,
    Selection,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthoringCapability {
    /// The request can be authored even when a selected lamp cannot reproduce it.
    SemanticIntent,
    VerifiedNativeControl,
    FocusParameter,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct ComponentDescriptor {
    pub owner: ProgrammingOwner,
    pub role: ComponentRole,
    pub unit: ComponentUnit,
    pub domain: Option<ScalarDomain>,
    pub step: f32,
    pub fine_step: f32,
    pub display_scale: f32,
    pub interpolation: ScalarInterpolation,
    pub capability: AuthoringCapability,
    pub spread: bool,
    pub align: bool,
    pub dynamics: bool,
}
impl ProgrammingComponent {
    pub const fn owner(self) -> ProgrammingOwner {
        match self {
            Self::Color(_) | Self::ColorWheel(_) | Self::NativeColor(_) => ProgrammingOwner::Color,
            Self::Pan
            | Self::Tilt
            | Self::TargetReference
            | Self::TargetX
            | Self::TargetY
            | Self::TargetZ => ProgrammingOwner::Position,
            Self::Focus => ProgrammingOwner::Focus,
            Self::Zoom => ProgrammingOwner::Zoom,
        }
    }
    pub fn descriptor(self) -> ComponentDescriptor {
        use ColorComponent as C;
        use ComponentRole as R;
        use ComponentUnit as U;
        let (role, unit, domain, step, fine_step, interpolation) = match self {
            Self::Color(C::Red | C::Green | C::Blue | C::Amber) => (
                R::ColorRecipe,
                U::Percent,
                Some(ScalarDomain::UNIT),
                0.01,
                0.001,
                ScalarInterpolation::Linear,
            ),
            Self::Color(C::Hue) => (
                R::ColorCoordinate,
                U::Degrees,
                Some(ScalarDomain::DEGREES),
                1.0,
                0.1,
                ScalarInterpolation::ShortestArc,
            ),
            Self::Color(C::Saturation) => (
                R::ColorCoordinate,
                U::Percent,
                Some(ScalarDomain::UNIT),
                0.01,
                0.001,
                ScalarInterpolation::Linear,
            ),
            Self::Color(C::WhiteBlend | C::Uv) => (
                R::ColorOrthogonal,
                U::Percent,
                Some(ScalarDomain::UNIT),
                0.01,
                0.001,
                ScalarInterpolation::Linear,
            ),
            Self::Color(C::Temperature) => (
                R::ColorOrthogonal,
                U::Kelvin,
                Some(ScalarDomain::KELVIN),
                100.0,
                10.0,
                ScalarInterpolation::Reciprocal,
            ),
            Self::Color(C::Duv) => (
                R::ColorOrthogonal,
                U::Duv,
                Some(ScalarDomain::DUV),
                0.001,
                0.0001,
                ScalarInterpolation::Linear,
            ),
            Self::Color(C::RelativeOutput) => (
                R::ColorOrthogonal,
                U::Factor,
                Some(ScalarDomain::Bounded {
                    bounds: AttributeBounds {
                        min: 0.0,
                        max: f32::MAX,
                    },
                }),
                0.01,
                0.001,
                ScalarInterpolation::Linear,
            ),
            Self::Pan | Self::Tilt => (
                R::Angle,
                U::Degrees,
                Some(ScalarDomain::Finite),
                1.0,
                0.1,
                ScalarInterpolation::Linear,
            ),
            Self::TargetX | Self::TargetY | Self::TargetZ => (
                R::Target,
                U::Metres,
                Some(ScalarDomain::Finite),
                0.1,
                0.01,
                ScalarInterpolation::Linear,
            ),
            Self::TargetReference => (
                R::Target,
                U::Selection,
                None,
                1.0,
                1.0,
                ScalarInterpolation::Linear,
            ),
            Self::Focus => (
                R::Focus,
                U::Percent,
                Some(ScalarDomain::UNIT),
                0.01,
                0.001,
                ScalarInterpolation::Linear,
            ),
            Self::Zoom => (
                R::Zoom,
                U::Degrees,
                Some(ScalarDomain::Bounded {
                    bounds: AttributeBounds {
                        min: 0.0,
                        max: 180.0,
                    },
                }),
                1.0,
                0.1,
                ScalarInterpolation::Linear,
            ),
            Self::ColorWheel(_) => (
                R::ColorWheel,
                U::Selection,
                None,
                1.0,
                1.0,
                ScalarInterpolation::Linear,
            ),
            // Native integer domains/continuous eligibility come from the verified function,
            // never a lossy float percentage or the spelling of a canonical alias.
            Self::NativeColor(_) => (
                R::NativeColor,
                U::NativeInteger,
                None,
                1.0,
                1.0,
                ScalarInterpolation::Linear,
            ),
        };
        ComponentDescriptor {
            owner: self.owner(),
            role,
            unit,
            domain,
            step,
            fine_step,
            display_scale: if unit == U::Percent { 100.0 } else { 1.0 },
            interpolation,
            capability: match self {
                Self::NativeColor(_) | Self::ColorWheel(_) => {
                    AuthoringCapability::VerifiedNativeControl
                }
                Self::Focus => AuthoringCapability::FocusParameter,
                _ => AuthoringCapability::SemanticIntent,
            },
            spread: domain.is_some(),
            align: domain.is_some(),
            dynamics: domain.is_some(),
        }
    }
}

/// The same word can describe a moving head's aiming control or a Point/Media transform.
/// Callers supply the target role instead of merging everything named position.* into one owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgrammingTargetRole {
    LightHead,
    Point,
    Media,
}

pub fn programming_component(
    id: &str,
    target: ProgrammingTargetRole,
) -> Option<ProgrammingComponent> {
    use ColorComponent as C;
    use ProgrammingComponent as P;
    if target == ProgrammingTargetRole::Point {
        return None;
    }
    let color = match id {
        "color.red" => Some(C::Red),
        "color.green" => Some(C::Green),
        "color.blue" => Some(C::Blue),
        "color.amber" => Some(C::Amber),
        "color.hue" => Some(C::Hue),
        "color.saturation" => Some(C::Saturation),
        "color.white" | "color.white_blend" => Some(C::WhiteBlend),
        "color.temperature" => Some(C::Temperature),
        "color.tint" | "color.duv" => Some(C::Duv),
        "color.uv" => Some(C::Uv),
        "color.relative_output" => Some(C::RelativeOutput),
        _ => None,
    };
    if let Some(color) = color {
        return Some(P::Color(color));
    }
    if target == ProgrammingTargetRole::Media {
        return None;
    }
    match id {
        "pan" | "position.pan" => Some(P::Pan),
        "tilt" | "position.tilt" => Some(P::Tilt),
        "position.target" => Some(P::TargetReference),
        "position.target.x" => Some(P::TargetX),
        "position.target.y" => Some(P::TargetY),
        "position.target.z" => Some(P::TargetZ),
        "color.wheel.1" => Some(P::ColorWheel(0)),
        "color.wheel.2" => Some(P::ColorWheel(1)),
        "focus" => Some(P::Focus),
        "zoom" => Some(P::Zoom),
        _ => None,
    }
}
