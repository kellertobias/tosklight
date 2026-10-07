use light_core::{AttributeValue, NativeColorIdentity, OpeningConvention, programming::*};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DynamicSemanticColorBasis {
    /// Orthogonal-only effects retain whichever semantic base wins composition.
    Retain,
    Recipe,
    HueSaturation,
    Whole,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DynamicFamilyRepresentation {
    Angles,
    Target {
        /// Component offsets require one declared frame. Whole Target keyframes may vary it;
        /// their interpolation then requires live world points from the coherent frame.
        reference: Option<TargetReference>,
    },
    SemanticColor {
        basis: DynamicSemanticColorBasis,
    },
    DirectColor {
        source: NativeColorIdentity,
    },
    Focus,
    Zoom {
        convention: OpeningConvention,
    },
}

impl DynamicFamilyRepresentation {
    pub fn owner(&self) -> ProgrammingOwner {
        match self {
            Self::Angles | Self::Target { .. } => ProgrammingOwner::Position,
            Self::SemanticColor { .. } | Self::DirectColor { .. } => ProgrammingOwner::Color,
            Self::Focus => ProgrammingOwner::Focus,
            Self::Zoom { .. } => ProgrammingOwner::Zoom,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DynamicValueAddress {
    pub representation: DynamicFamilyRepresentation,
    /// None addresses a whole family keyframe/mask. This never stands for a fixture channel.
    pub component: Option<ProgrammingComponent>,
}

impl DynamicValueAddress {
    /// Preserve the actual representation of a whole-owner baseline. In particular,
    /// Size over an Angle/Target or Semantic/Direct takeover must retain both kinds.
    pub fn whole_family(
        owner: ProgrammingOwner,
        value: &AttributeValue,
    ) -> Result<Self, IntentError> {
        value.validate_programming_address(owner.key_ref())?;
        let representation = match value {
            AttributeValue::Position(position) => match position.as_ref() {
                PositionIntent::Angles { .. } => DynamicFamilyRepresentation::Angles,
                PositionIntent::Target { .. } => {
                    DynamicFamilyRepresentation::Target { reference: None }
                }
            },
            AttributeValue::ColorProgram(color) => match color.as_ref() {
                ColorProgram::Semantic { .. } => DynamicFamilyRepresentation::SemanticColor {
                    basis: DynamicSemanticColorBasis::Whole,
                },
                ColorProgram::Direct { recipe, .. } => DynamicFamilyRepresentation::DirectColor {
                    source: recipe.source.clone(),
                },
            },
            AttributeValue::Zoom(zoom) => DynamicFamilyRepresentation::Zoom {
                convention: zoom.convention,
            },
            AttributeValue::Normalized(_) if owner == ProgrammingOwner::Focus => {
                DynamicFamilyRepresentation::Focus
            }
            _ => {
                return Err(IntentError(
                    "Dynamic baseline requires a materialized programming family".into(),
                ));
            }
        };
        let address = Self {
            representation,
            component: None,
        };
        address.validate_family(value)?;
        Ok(address)
    }

    /// Storage validation without a fixture adapter. A native value retains its integer shape;
    /// exact function bounds are checked again when its pinned model is compiled.
    pub fn validate_value_shape(&self, value: &super::DynamicValue) -> Result<(), IntentError> {
        use super::DynamicValue as V;
        self.validate()?;
        match (self.component, value) {
            (None, V::Family(value)) => self.validate_family(value),
            (Some(ProgrammingComponent::NativeColor(_)), V::Native(_)) => Ok(()),
            (Some(component), V::Scalar(value)) => {
                let descriptor = component.descriptor();
                ensure(
                    descriptor.dynamics
                        && descriptor
                            .domain
                            .is_some_and(|domain| domain.contains(*value)),
                    "Dynamic scalar is outside its declared component domain",
                )
            }
            _ => Err(IntentError(
                "Dynamic value has the wrong component/family type".into(),
            )),
        }
    }

    pub fn owner(&self) -> ProgrammingOwner {
        self.representation.owner()
    }

    pub fn validate(&self) -> Result<(), IntentError> {
        use DynamicFamilyRepresentation as R;
        use DynamicSemanticColorBasis as B;
        use ProgrammingComponent as C;
        if let R::DirectColor { source } = &self.representation {
            source.validate()?;
        }
        if let R::Target {
            reference: Some(TargetReference::Point { point_id }),
        } = &self.representation
        {
            ensure(
                !point_id.is_nil(),
                "Dynamic target requires a stable Point UUID",
            )?;
        }
        let Some(component) = self.component else {
            return ensure(
                !matches!(
                    self.representation,
                    R::SemanticColor {
                        basis: B::Retain | B::Recipe | B::HueSaturation
                    }
                ),
                "whole Color lanes require the Whole basis",
            );
        };
        ensure(
            component.owner() == self.owner(),
            "Dynamic component belongs to a different owner",
        )?;
        let compatible = match (&self.representation, component) {
            (R::Angles, C::Pan | C::Tilt) => true,
            (R::Target { reference: Some(_) }, C::TargetX | C::TargetY | C::TargetZ) => true,
            (R::SemanticColor { basis }, C::Color(color)) => match color {
                ColorComponent::Red
                | ColorComponent::Green
                | ColorComponent::Blue
                | ColorComponent::Amber => *basis == B::Recipe,
                ColorComponent::Hue | ColorComponent::Saturation => *basis == B::HueSaturation,
                _ => true,
            },
            (R::DirectColor { .. }, C::NativeColor(binding)) => {
                !binding.channel_id.is_nil() && !binding.function_id.is_nil()
            }
            (R::Focus, C::Focus) | (R::Zoom { .. }, C::Zoom) => true,
            _ => false,
        };
        ensure(
            compatible,
            "Dynamic component is incompatible with its declared representation",
        )
    }

    pub fn validate_family(&self, value: &AttributeValue) -> Result<(), IntentError> {
        self.validate()?;
        ensure(
            self.component.is_none(),
            "component lanes cannot contain whole-family values",
        )?;
        value.validate_programming_address(self.owner().key_ref())?;
        ensure(
            value.spread_control_points() == 0,
            "Dynamic source must be materialized for its target",
        )?;
        ensure(
            self.matches_representation(value),
            "Dynamic family source has a different representation or pinned identity",
        )
    }

    /// Authored-scope compatibility, before rank materialization or pinned model verification.
    pub fn matches_authored_source(&self, value: &AttributeValue) -> bool {
        if self.owner() == ProgrammingOwner::Focus
            && let AttributeValue::Spread(points) = value
        {
            return light_core::programming::ScalarIntent::Spread(points.clone())
                .validate(ScalarDomain::UNIT)
                .is_ok();
        }
        if !self.matches_representation(value) {
            return false;
        }
        if let Some(ProgrammingComponent::NativeColor(binding)) = self.component {
            let AttributeValue::ColorProgram(program) = value else {
                return false;
            };
            let ColorProgram::Direct { recipe, .. } = program.as_ref() else {
                return false;
            };
            return recipe.channels.iter().any(|channel| {
                channel.channel_id == binding.channel_id
                    && channel.function_id == binding.function_id
            });
        }
        true
    }

    pub(crate) fn matches_representation(&self, value: &AttributeValue) -> bool {
        match (&self.representation, value) {
            (DynamicFamilyRepresentation::Angles, AttributeValue::Position(value)) => {
                matches!(value.as_ref(), PositionIntent::Angles { .. })
            }
            (
                DynamicFamilyRepresentation::Target { reference },
                AttributeValue::Position(value),
            ) => {
                matches!(value.as_ref(), PositionIntent::Target {reference:actual, ..} if reference.is_none_or(|expected| expected == *actual))
            }
            (
                DynamicFamilyRepresentation::SemanticColor { .. },
                AttributeValue::ColorProgram(value),
            ) => matches!(value.as_ref(), ColorProgram::Semantic { .. }),
            (
                DynamicFamilyRepresentation::DirectColor { source },
                AttributeValue::ColorProgram(value),
            ) => {
                matches!(value.as_ref(), ColorProgram::Direct {recipe, ..} if &recipe.source == source)
            }
            (DynamicFamilyRepresentation::Focus, AttributeValue::Normalized(value)) => {
                ScalarDomain::UNIT.contains(*value)
            }
            (DynamicFamilyRepresentation::Zoom { convention }, AttributeValue::Zoom(value)) => {
                value.convention == *convention
            }
            _ => false,
        }
    }
}

pub(super) fn ensure(condition: bool, message: &str) -> Result<(), IntentError> {
    condition
        .then_some(())
        .ok_or_else(|| IntentError(message.into()))
}

/// Check one contribution's declared representations before editor/API/import/load can install
/// it. Independent instances are arbitrated later; their suppressed clocks continue running.
pub fn validate_dynamic_value_addresses<'a>(
    addresses: impl IntoIterator<Item = &'a DynamicValueAddress>,
) -> Result<(), IntentError> {
    let mut representations =
        std::collections::HashMap::<ProgrammingOwner, &DynamicFamilyRepresentation>::new();
    for address in addresses {
        address.validate()?;
        if let Some(previous) = representations.get(&address.owner()) {
            use DynamicFamilyRepresentation as R;
            use DynamicSemanticColorBasis as B;
            let compatible = match (*previous, &address.representation) {
                (R::SemanticColor { basis: a }, R::SemanticColor { basis: b }) => {
                    *a == B::Retain || *b == B::Retain || a == b
                }
                (a, b) => a == b,
            };
            ensure(
                compatible,
                "one Dynamic contribution cannot mix family representations, target frames or native sources",
            )?;
            // Orthogonal-only lanes do not hide a later base writer from subsequent validation.
            if matches!(previous, R::SemanticColor { basis: B::Retain }) {
                representations.insert(address.owner(), &address.representation);
            }
        } else {
            representations.insert(address.owner(), &address.representation);
        }
    }
    Ok(())
}
