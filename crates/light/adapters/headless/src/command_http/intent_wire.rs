//! Explicit DTO conversion at the adapter boundary, shared by Normal and Preload values.
use light_core::programming as domain;
use light_wire::v2::programming_intent as wire;

pub(super) trait IntoIntentDomain {
    type Domain;
    fn into_intent_domain(self) -> Self::Domain;
}
pub(crate) trait ToIntentWire {
    type Wire;
    fn to_intent_wire(&self) -> Self::Wire;
}
macro_rules! identity {
    ($($ty:ty),*) => { $(
        impl IntoIntentDomain for $ty { type Domain = Self; fn into_intent_domain(self) -> Self { self } }
        impl ToIntentWire for $ty { type Wire = Self; fn to_intent_wire(&self) -> Self { self.clone() } }
    )* };
}
identity!(f32, u16, u32, bool, String, uuid::Uuid);
impl<T: IntoIntentDomain> IntoIntentDomain for Vec<T> {
    type Domain = Vec<T::Domain>;
    fn into_intent_domain(self) -> Self::Domain {
        self.into_iter().map(T::into_intent_domain).collect()
    }
}
impl<T: ToIntentWire> ToIntentWire for Vec<T> {
    type Wire = Vec<T::Wire>;
    fn to_intent_wire(&self) -> Self::Wire {
        self.iter().map(T::to_intent_wire).collect()
    }
}
impl<T: IntoIntentDomain> IntoIntentDomain for Option<T> {
    type Domain = Option<T::Domain>;
    fn into_intent_domain(self) -> Self::Domain {
        self.map(T::into_intent_domain)
    }
}
impl<T: ToIntentWire> ToIntentWire for Option<T> {
    type Wire = Option<T::Wire>;
    fn to_intent_wire(&self) -> Self::Wire {
        self.as_ref().map(T::to_intent_wire)
    }
}
impl<T: IntoIntentDomain, const N: usize> IntoIntentDomain for [T; N] {
    type Domain = [T::Domain; N];
    fn into_intent_domain(self) -> Self::Domain {
        self.map(T::into_intent_domain)
    }
}
impl<T: ToIntentWire, const N: usize> ToIntentWire for [T; N] {
    type Wire = [T::Wire; N];
    fn to_intent_wire(&self) -> Self::Wire {
        std::array::from_fn(|i| self[i].to_intent_wire())
    }
}
macro_rules! fields {
    ($wire:ty, $domain:ty, $($field:ident),+ $(,)?) => {
        impl IntoIntentDomain for $wire {
            type Domain = $domain;
            fn into_intent_domain(self) -> Self::Domain { Self::Domain { $($field: self.$field.into_intent_domain()),+ } }
        }
        impl ToIntentWire for $domain {
            type Wire = $wire;
            fn to_intent_wire(&self) -> Self::Wire { Self::Wire { $($field: self.$field.to_intent_wire()),+ } }
        }
    };
}
macro_rules! choices {
    ($wire:ty, $domain:ty, $($variant:ident),+ $(,)?) => {
        impl IntoIntentDomain for $wire { type Domain = $domain; fn into_intent_domain(self) -> Self::Domain { match self { $(Self::$variant => Self::Domain::$variant),+ } } }
        impl ToIntentWire for $domain { type Wire = $wire; fn to_intent_wire(&self) -> Self::Wire { match self { $(Self::$variant => Self::Wire::$variant),+ } } }
    };
}
macro_rules! records {
    ($wire:ty, $domain:ty, $($variant:ident { $($field:ident),+ }),+ $(,)?) => {
        impl IntoIntentDomain for $wire { type Domain = $domain; fn into_intent_domain(self) -> Self::Domain { match self { $(Self::$variant { $($field),+ } => Self::Domain::$variant { $($field: $field.into_intent_domain()),+ }),+ } } }
        impl ToIntentWire for $domain { type Wire = $wire; fn to_intent_wire(&self) -> Self::Wire { match self { $(Self::$variant { $($field),+ } => Self::Wire::$variant { $($field: $field.to_intent_wire()),+ }),+ } } }
    };
}
fields!(
    light_wire::v2::programming::ProgrammingColorXyz,
    light_core::Xyz,
    x,
    y,
    z
);
fields!(
    wire::ProgrammingNativeColorBinding,
    light_core::NativeColorBinding,
    channel_id,
    function_id
);
fields!(
    wire::ProgrammingNativeColorValue,
    light_core::NativeColorValue,
    channel_id,
    function_id,
    raw
);
fields!(
    wire::ProgrammingNativeColorIdentity,
    light_core::NativeColorIdentity,
    profile_id,
    profile_revision,
    profile_digest,
    mode_id,
    head_id,
    path_id,
    model_revision,
    native_layout_signature
);
fields!(
    wire::ProgrammingVirtualColorRecipe,
    domain::VirtualColorRecipe,
    version,
    rgb,
    amber,
    approximate
);
fields!(
    wire::ProgrammingWhiteTarget,
    domain::WhiteTarget,
    kelvin,
    duv
);
fields!(wire::ProgrammingUvIntent, domain::UvIntent, amount);
fields!(
    wire::ProgrammingColorWheelConstraint,
    domain::ColorWheelConstraint,
    source,
    value
);
fields!(
    wire::ProgrammingColorComponentSpread,
    domain::ColorComponentSpread,
    component,
    points
);
fields!(
    wire::ProgrammingColorIntent,
    domain::ColorIntent,
    base_xyz,
    recipe,
    white_blend,
    white_target,
    uv,
    relative_output,
    allocation,
    wheel_constraints,
    spreads
);
fields!(
    wire::ProgrammingNativeColorRecipe,
    domain::NativeColorRecipe,
    source,
    channels,
    spreads
);
fields!(
    wire::ProgrammingNativeColorSpread,
    domain::NativeColorSpread,
    binding,
    points
);
fields!(
    wire::ProgrammingPortableVisibleColor,
    domain::PortableVisibleColor,
    xyz,
    relative_output
);
fields!(
    wire::ProgrammingPortableUv,
    domain::PortableUv,
    amount,
    quality
);
fields!(
    wire::ProgrammingPortableColorEstimate,
    domain::PortableColorEstimate,
    model_revision,
    visible,
    uv,
    quality,
    limitations
);
fields!(
    wire::ProgrammingZoomIntent,
    domain::ZoomIntent,
    opening_degrees,
    convention
);
choices!(
    wire::ProgrammingPhysicalDataQuality,
    light_core::PhysicalDataQuality,
    Unknown,
    Estimated,
    Manufacturer,
    Measured
);
choices!(
    wire::ProgrammingOpeningConvention,
    light_core::OpeningConvention,
    Beam,
    Field
);
choices!(
    wire::ProgrammingColorAllocation,
    domain::ColorAllocation,
    PreserveRecipe,
    PreferWhite,
    PreferColoredEmitters
);
choices!(
    wire::ProgrammingColorComponent,
    domain::ColorComponent,
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
    RelativeOutput
);
records!(
    wire::ProgrammingColorProgram,
    domain::ColorProgram,
    Semantic { intent },
    Direct { recipe, portable }
);
records!(
    wire::ProgrammingPositionIntent,
    domain::PositionIntent,
    Angles {
        pan_degrees,
        tilt_degrees
    },
    Target {
        reference,
        offset_metres
    }
);
impl IntoIntentDomain for wire::ProgrammingTargetReference {
    type Domain = domain::TargetReference;
    fn into_intent_domain(self) -> Self::Domain {
        match self {
            Self::Origin => Self::Domain::Origin,
            Self::Point { point_id } => Self::Domain::Point { point_id },
        }
    }
}
impl ToIntentWire for domain::TargetReference {
    type Wire = wire::ProgrammingTargetReference;
    fn to_intent_wire(&self) -> Self::Wire {
        match *self {
            Self::Origin => Self::Wire::Origin,
            Self::Point { point_id } => Self::Wire::Point { point_id },
        }
    }
}
impl IntoIntentDomain for wire::ProgrammingScalarIntent {
    type Domain = domain::ScalarIntent;
    fn into_intent_domain(self) -> Self::Domain {
        match self {
            Self::Value(v) => Self::Domain::Value(v),
            Self::Spread(v) => Self::Domain::Spread(v),
        }
    }
}
impl ToIntentWire for domain::ScalarIntent {
    type Wire = wire::ProgrammingScalarIntent;
    fn to_intent_wire(&self) -> Self::Wire {
        match self {
            Self::Value(v) => Self::Wire::Value(*v),
            Self::Spread(v) => Self::Wire::Spread(v.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use light_core::{
        AttributeValue, NativeColorIdentity, NativeColorValue, OpeningConvention,
        PhysicalDataQuality,
    };
    use std::sync::Arc;
    use uuid::Uuid;

    #[test]
    fn complete_owners_round_trip_identically_through_normal_and_preload_wire() {
        let values = [
            AttributeValue::GroupFamily(Arc::new(domain::GroupFamilyAssignment {
                owner: domain::ProgrammingOwner::Position,
                template: AttributeValue::Position(Arc::new(domain::PositionIntent::angles(
                    -720.0, 30.0,
                ))),
                members: std::collections::BTreeMap::from([(
                    Uuid::from_u128(42),
                    AttributeValue::Position(Arc::new(domain::PositionIntent::target(
                        domain::TargetReference::Origin,
                        [1.0, 2.0, 3.0],
                    ))),
                )]),
            })),
            AttributeValue::ColorProgram(Arc::new(domain::ColorProgram::Semantic {
                intent: domain::ColorIntent {
                    uv: domain::UvIntent { amount: 0.75 },
                    white_blend: 0.5,
                    ..Default::default()
                },
            })),
            AttributeValue::ColorProgram(Arc::new(domain::ColorProgram::Direct {
                recipe: domain::NativeColorRecipe {
                    source: NativeColorIdentity {
                        profile_id: Uuid::from_u128(1),
                        profile_revision: 2,
                        profile_digest: "digest".into(),
                        mode_id: Uuid::from_u128(3),
                        head_id: Uuid::from_u128(4),
                        path_id: Uuid::from_u128(5),
                        model_revision: 6,
                        native_layout_signature: "layout".into(),
                    },
                    channels: vec![NativeColorValue {
                        channel_id: Uuid::from_u128(7),
                        function_id: Uuid::from_u128(8),
                        raw: u32::MAX - 1,
                    }],
                    spreads: vec![],
                },
                portable: domain::PortableColorEstimate {
                    model_revision: 6,
                    visible: None,
                    uv: Some(domain::PortableUv {
                        amount: 0.8,
                        quality: PhysicalDataQuality::Estimated,
                    }),
                    quality: PhysicalDataQuality::Unknown,
                    limitations: vec!["Visible appearance unavailable".into()],
                },
            })),
            AttributeValue::Position(Arc::new(domain::PositionIntent::angles(-720.0, 270.0))),
            AttributeValue::Position(Arc::new(domain::PositionIntent::target(
                domain::TargetReference::Point {
                    point_id: Uuid::from_u128(9),
                },
                [-3.0, 2.0, 8.0],
            ))),
            AttributeValue::Zoom(Arc::new(domain::ZoomIntent {
                opening_degrees: domain::ScalarIntent::Spread(vec![4.0, 60.0]),
                convention: OpeningConvention::Beam,
            })),
        ];
        for value in values {
            let normal = super::super::values_wire::attribute_value(&value);
            let preload = super::super::preload_values_wire::attribute_value(&value);
            assert_eq!(
                serde_json::to_value(&normal).unwrap(),
                serde_json::to_value(&preload).unwrap()
            );
            assert_eq!(
                serde_json::to_value(&normal).unwrap(),
                serde_json::to_value(&value).unwrap()
            );
            assert_eq!(super::super::values_wire::application_value(normal), value);
            assert_eq!(
                super::super::preload_values_wire::application_value(preload),
                value
            );
        }
    }
}

impl IntoIntentDomain for wire::ProgrammingScalarEdit {
    type Domain = domain::ScalarEdit;
    fn into_intent_domain(self) -> Self::Domain {
        match self {
            Self::Set(v) => Self::Domain::Set(v.into_intent_domain()),
            Self::Relative(v) => Self::Domain::Relative(v),
        }
    }
}
impl ToIntentWire for domain::ScalarEdit {
    type Wire = wire::ProgrammingScalarEdit;
    fn to_intent_wire(&self) -> Self::Wire {
        match self {
            Self::Set(v) => Self::Wire::Set(v.to_intent_wire()),
            Self::Relative(v) => Self::Wire::Relative(*v),
        }
    }
}
impl IntoIntentDomain for wire::ProgrammingNativeColorEdit {
    type Domain = domain::NativeColorEdit;
    fn into_intent_domain(self) -> Self::Domain {
        match self {
            Self::Set(v) => Self::Domain::Set(v),
            Self::Spread(v) => Self::Domain::Spread(v),
            Self::Relative(v) => Self::Domain::Relative(v),
        }
    }
}
impl ToIntentWire for domain::NativeColorEdit {
    type Wire = wire::ProgrammingNativeColorEdit;
    fn to_intent_wire(&self) -> Self::Wire {
        match self {
            Self::Set(v) => Self::Wire::Set(*v),
            Self::Spread(v) => Self::Wire::Spread(v.clone()),
            Self::Relative(v) => Self::Wire::Relative(*v),
        }
    }
}
impl IntoIntentDomain for wire::ProgrammingComponent {
    type Domain = domain::ProgrammingComponent;
    fn into_intent_domain(self) -> Self::Domain {
        match self {
            Self::Color(v) => Self::Domain::Color(v.into_intent_domain()),
            Self::ColorWheel(v) => Self::Domain::ColorWheel(v),
            Self::NativeColor(v) => Self::Domain::NativeColor(v.into_intent_domain()),
            Self::Pan => Self::Domain::Pan,
            Self::Tilt => Self::Domain::Tilt,
            Self::TargetReference => Self::Domain::TargetReference,
            Self::TargetX => Self::Domain::TargetX,
            Self::TargetY => Self::Domain::TargetY,
            Self::TargetZ => Self::Domain::TargetZ,
            Self::Focus => Self::Domain::Focus,
            Self::Zoom => Self::Domain::Zoom,
        }
    }
}
impl ToIntentWire for domain::ProgrammingComponent {
    type Wire = wire::ProgrammingComponent;
    fn to_intent_wire(&self) -> Self::Wire {
        match self {
            Self::Color(v) => Self::Wire::Color(v.to_intent_wire()),
            Self::ColorWheel(v) => Self::Wire::ColorWheel(*v),
            Self::NativeColor(v) => Self::Wire::NativeColor(v.to_intent_wire()),
            Self::Pan => Self::Wire::Pan,
            Self::Tilt => Self::Wire::Tilt,
            Self::TargetReference => Self::Wire::TargetReference,
            Self::TargetX => Self::Wire::TargetX,
            Self::TargetY => Self::Wire::TargetY,
            Self::TargetZ => Self::Wire::TargetZ,
            Self::Focus => Self::Wire::Focus,
            Self::Zoom => Self::Wire::Zoom,
        }
    }
}
impl IntoIntentDomain for wire::ProgrammingComponentEdit {
    type Domain = domain::ComponentEdit;
    fn into_intent_domain(self) -> Self::Domain {
        match self {
            Self::ActivateAngles => Self::Domain::ActivateAngles,
            Self::Scalar {
                component,
                operation,
            } => Self::Domain::Scalar {
                component: component.into_intent_domain(),
                operation: operation.into_intent_domain(),
            },
            Self::Target { reference } => Self::Domain::Target {
                reference: reference.into_intent_domain(),
            },
            Self::Coordinates { xyz } => Self::Domain::Coordinates {
                xyz: xyz.into_intent_domain(),
            },
            Self::Native { binding, operation } => Self::Domain::Native {
                binding: binding.into_intent_domain(),
                operation: operation.into_intent_domain(),
            },
        }
    }
}
impl ToIntentWire for domain::ComponentEdit {
    type Wire = wire::ProgrammingComponentEdit;
    fn to_intent_wire(&self) -> Self::Wire {
        match self {
            Self::ActivateAngles => Self::Wire::ActivateAngles,
            Self::Scalar {
                component,
                operation,
            } => Self::Wire::Scalar {
                component: component.to_intent_wire(),
                operation: operation.to_intent_wire(),
            },
            Self::Target { reference } => Self::Wire::Target {
                reference: reference.to_intent_wire(),
            },
            Self::Coordinates { xyz } => Self::Wire::Coordinates {
                xyz: xyz.to_intent_wire(),
            },
            Self::Native { binding, operation } => Self::Wire::Native {
                binding: binding.to_intent_wire(),
                operation: operation.to_intent_wire(),
            },
        }
    }
}

choices!(
    wire::ProgrammingOwner,
    domain::ProgrammingOwner,
    Color,
    Position,
    Focus,
    Zoom
);
impl IntoIntentDomain for wire::ProgrammingGroupFamilyAssignment {
    type Domain = domain::GroupFamilyAssignment;
    fn into_intent_domain(self) -> Self::Domain {
        Self::Domain {
            owner: self.owner.into_intent_domain(),
            template: super::values_wire::application_value(self.template),
            members: self
                .members
                .into_iter()
                .map(|(id, value)| (id, super::values_wire::application_value(value)))
                .collect(),
        }
    }
}
impl ToIntentWire for domain::GroupFamilyAssignment {
    type Wire = wire::ProgrammingGroupFamilyAssignment;
    fn to_intent_wire(&self) -> Self::Wire {
        Self::Wire {
            owner: self.owner.to_intent_wire(),
            template: super::values_wire::attribute_value(&self.template),
            members: self
                .members
                .iter()
                .map(|(id, value)| (*id, super::values_wire::attribute_value(value)))
                .collect(),
        }
    }
}
