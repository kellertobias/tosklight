//! Effective fields carried by a value, independently of the operator's original edit footprint.
//! Transfers describe structural dependence, never inferred authorship from equal numeric values.
use super::*;
use crate::AttributeValue;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ProgrammingTraceField {
    ColorXyz,
    ColorRecipeRed,
    ColorRecipeGreen,
    ColorRecipeBlue,
    ColorRecipeAmber,
    WhiteBlend,
    Temperature,
    Duv,
    Uv,
    RelativeOutput,
    Allocation,
    /// The complete constraint collection, including an intentionally empty collection.
    ColorWheels,
    ColorWheel(u16),
    Pan,
    Tilt,
    TargetReference,
    TargetX,
    TargetY,
    TargetZ,
    Focus,
    Zoom,
    ZoomConvention,
    /// Includes the selected function and its native integer value on this channel.
    NativeColorChannel(Uuid),
    NativeColorIdentity,
    NativePrediction,
}

impl ProgrammingTraceField {
    pub const fn owner(self) -> ProgrammingOwner {
        match self {
            Self::Pan
            | Self::Tilt
            | Self::TargetReference
            | Self::TargetX
            | Self::TargetY
            | Self::TargetZ => ProgrammingOwner::Position,
            Self::Focus => ProgrammingOwner::Focus,
            Self::Zoom | Self::ZoomConvention => ProgrammingOwner::Zoom,
            _ => ProgrammingOwner::Color,
        }
    }
}

mod scope;
pub use scope::{Fields as ProgrammingFields, ProgrammingFieldScope};

impl<'de> Deserialize<'de> for ProgrammingFieldScope {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let scope = Self::new(Vec::<ProgrammingTraceField>::deserialize(deserializer)?);
        if scope.fields().any(|field| {
            matches!(field,
            ProgrammingTraceField::NativeColorChannel(id) if id.is_nil())
        }) {
            return Err(serde::de::Error::custom(
                "native trace field requires a stable channel UUID",
            ));
        }
        Ok(scope)
    }
}

impl ProgrammingFieldScope {
    pub fn validate(&self, owner: ProgrammingOwner) -> Result<(), IntentError> {
        require(
            self.owned_by(owner),
            "trace field belongs to a different programming owner",
        )?;
        require(
            self.parameterized().iter().all(|field| {
                !matches!(field,
            ProgrammingTraceField::NativeColorChannel(id) if id.is_nil())
            }),
            "native trace field requires a stable channel UUID",
        )
    }

    pub fn from_component(component: ProgrammingComponent) -> Self {
        use ColorComponent as C;
        use ProgrammingComponent as P;
        use ProgrammingTraceField as F;
        match component {
            P::Color(C::Red) => Self::from_bits(&[F::ColorXyz, F::ColorRecipeRed]),
            P::Color(C::Green) => Self::from_bits(&[F::ColorXyz, F::ColorRecipeGreen]),
            P::Color(C::Blue) => Self::from_bits(&[F::ColorXyz, F::ColorRecipeBlue]),
            P::Color(C::Amber) => Self::from_bits(&[F::ColorXyz, F::ColorRecipeAmber]),
            P::Color(C::Hue | C::Saturation) => Self::from_bits(&[
                F::ColorXyz,
                F::ColorRecipeRed,
                F::ColorRecipeGreen,
                F::ColorRecipeBlue,
            ]),
            P::Color(C::WhiteBlend) => Self::from_bits(&[F::WhiteBlend]),
            P::Color(C::Temperature) => Self::from_bits(&[F::Temperature]),
            P::Color(C::Duv) => Self::from_bits(&[F::Duv]),
            P::Color(C::Uv) => Self::from_bits(&[F::Uv]),
            P::Color(C::RelativeOutput) => Self::from_bits(&[F::RelativeOutput]),
            P::ColorWheel(wheel) => Self::new([F::ColorWheel(wheel)]),
            P::NativeColor(binding) => Self::new([F::NativeColorChannel(binding.channel_id)]),
            P::Pan => Self::from_bits(&[F::Pan]),
            P::Tilt => Self::from_bits(&[F::Tilt]),
            P::TargetReference => Self::from_bits(&[F::TargetReference]),
            P::TargetX => Self::from_bits(&[F::TargetX]),
            P::TargetY => Self::from_bits(&[F::TargetY]),
            P::TargetZ => Self::from_bits(&[F::TargetZ]),
            P::Focus => Self::from_bits(&[F::Focus]),
            P::Zoom => Self::from_bits(&[F::Zoom]),
        }
    }

    /// Materialize Whole against the actual endpoint representation. This is not a union of
    /// every possible representation of the owner (for example, Angle has no Target fields).
    pub fn for_value(owner: ProgrammingOwner, value: &AttributeValue) -> Result<Self, IntentError> {
        use ProgrammingTraceField as F;
        value.validate_programming_address(owner.key_ref())?;
        require(
            value.spread_control_points() == 0,
            "trace scope requires materialized programming values",
        )?;
        Ok(match (owner, value) {
            (ProgrammingOwner::Color, AttributeValue::ColorProgram(program)) => {
                match program.as_ref() {
                    ColorProgram::Semantic { .. } => Self::from_bits(&[
                        F::ColorXyz,
                        F::ColorRecipeRed,
                        F::ColorRecipeGreen,
                        F::ColorRecipeBlue,
                        F::ColorRecipeAmber,
                        F::WhiteBlend,
                        F::Temperature,
                        F::Duv,
                        F::Uv,
                        F::RelativeOutput,
                        F::Allocation,
                        F::ColorWheels,
                    ]),
                    ColorProgram::Direct { recipe, .. } => {
                        let mut fields = vec![F::NativeColorIdentity, F::NativePrediction];
                        fields.extend(
                            recipe
                                .channels
                                .iter()
                                .map(|channel| F::NativeColorChannel(channel.channel_id)),
                        );
                        Self::new(fields)
                    }
                }
            }
            (ProgrammingOwner::Color, AttributeValue::ColorXyz(xyz)) => {
                require(
                    valid_xyz(*xyz),
                    "Color trace coordinates must be finite and nonnegative",
                )?;
                Self::from_bits(&[F::ColorXyz])
            }
            (ProgrammingOwner::Position, AttributeValue::Position(position)) => {
                match position.as_ref() {
                    PositionIntent::Angles { .. } => Self::from_bits(&[F::Pan, F::Tilt]),
                    PositionIntent::Target { .. } => {
                        Self::from_bits(&[F::TargetReference, F::TargetX, F::TargetY, F::TargetZ])
                    }
                }
            }
            (ProgrammingOwner::Focus, AttributeValue::Normalized(value)) => {
                require(
                    ScalarDomain::UNIT.contains(*value),
                    "Focus trace value is outside 0-1",
                )?;
                Self::from_bits(&[F::Focus])
            }
            (ProgrammingOwner::Zoom, AttributeValue::Zoom(_)) => {
                Self::from_bits(&[F::Zoom, F::ZoomConvention])
            }
            _ => {
                return Err(IntentError(
                    "value has no materialized trace fields for this owner".into(),
                ));
            }
        })
    }
}

/// A known transfer. Empty means no input participates; unresolved conversions are errors at
/// trace construction, not empty transfers. Remap pairs are (input field, output field).
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProgrammingFieldTransfer {
    pub identity: ProgrammingFieldScope,
    pub remap: Arc<[(ProgrammingTraceField, ProgrammingTraceField)]>,
}

impl ProgrammingFieldTransfer {
    pub fn forward(&self, input: &ProgrammingFieldScope) -> ProgrammingFieldScope {
        self.identity
            .intersection(input)
            .union(&ProgrammingFieldScope::new(
                self.remap
                    .iter()
                    .filter(|(from, _)| input.overlaps(*from))
                    .map(|(_, to)| *to),
            ))
    }

    pub fn reverse(&self, query: &ProgrammingFieldScope) -> ProgrammingFieldScope {
        self.identity
            .intersection(query)
            .union(&ProgrammingFieldScope::new(
                self.remap
                    .iter()
                    .filter(|(_, to)| query.overlaps(*to))
                    .map(|(from, _)| *from),
            ))
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProgrammingTransitionTrace {
    pub from: ProgrammingFieldTransfer,
    pub to: ProgrammingFieldTransfer,
}

/// Trace the same successful evaluator path as interpolation. For value and trace together,
/// prefer CompiledProgrammingTransition::sample_with_trace to avoid evaluating twice.
pub fn interpolate_programming_trace(
    owner: ProgrammingOwner,
    from: &AttributeValue,
    to: &AttributeValue,
    progress: f32,
) -> Result<ProgrammingTransitionTrace, TransitionError> {
    interpolate_programming_value(from, to, progress)?;
    successful_transition_trace(owner, from, to, progress, false, None)
}

/// Called only after successful sampling. Native descriptors come from the pinned compiler.
pub(super) fn successful_transition_trace(
    owner: ProgrammingOwner,
    from: &AttributeValue,
    to: &AttributeValue,
    amount: f32,
    scale: bool,
    native_channels: Option<&[(Uuid, bool)]>,
) -> Result<ProgrammingTransitionTrace, TransitionError> {
    use ProgrammingTraceField as F;
    if amount <= 0.0 {
        return Ok(ProgrammingTransitionTrace {
            from: identity(ProgrammingFieldScope::for_value(owner, from)?),
            to: Default::default(),
        });
    }
    if amount == 1.0 || (!scale && amount >= 1.0) {
        return Ok(ProgrammingTransitionTrace {
            from: Default::default(),
            to: identity(ProgrammingFieldScope::for_value(owner, to)?),
        });
    }
    let from_scope = ProgrammingFieldScope::for_value(owner, from)?;
    let to_scope = ProgrammingFieldScope::for_value(owner, to)?;
    let mut trace = match (from, to) {
        (AttributeValue::Normalized(_), AttributeValue::Normalized(_)) => both(vec![F::Focus]),
        (AttributeValue::ColorXyz(_), AttributeValue::ColorXyz(_)) => {
            if scale {
                // Legacy ColorXyz has no extrapolating scale evaluator; it retains from.
                ProgrammingTransitionTrace {
                    from: identity(from_scope.clone()),
                    to: Default::default(),
                }
            } else {
                both(vec![F::ColorXyz])
            }
        }
        (AttributeValue::Position(a), AttributeValue::Position(b)) => {
            match (a.as_ref(), b.as_ref()) {
                (PositionIntent::Angles { .. }, PositionIntent::Angles { .. }) => {
                    both(vec![F::Pan, F::Tilt])
                }
                (
                    PositionIntent::Target { reference: a, .. },
                    PositionIntent::Target { reference: b, .. },
                ) if a == b => {
                    let mut trace = both(vec![F::TargetX, F::TargetY, F::TargetZ]);
                    trace.from.identity = trace
                        .from
                        .identity
                        .union(&ProgrammingFieldScope::new([F::TargetReference]));
                    trace
                }
                (PositionIntent::Target { .. }, PositionIntent::Target { .. }) => {
                    return Err(TransitionError::Requires(
                        TransitionRequirement::LiveTargetPoints,
                    ));
                }
                _ => {
                    return Err(TransitionError::Requires(
                        TransitionRequirement::LiveJointAngles,
                    ));
                }
            }
        }
        (AttributeValue::Zoom(a), AttributeValue::Zoom(b)) if a.convention == b.convention => {
            let mut trace = both(vec![F::Zoom]);
            trace.from.identity = trace
                .from
                .identity
                .union(&ProgrammingFieldScope::new([F::ZoomConvention]));
            trace
        }
        (AttributeValue::Zoom(_), AttributeValue::Zoom(_)) => {
            return Err(TransitionError::Requires(
                TransitionRequirement::ZoomConvention,
            ));
        }
        (AttributeValue::ColorProgram(a), AttributeValue::ColorProgram(b)) => color_program_trace(
            (a.as_ref(), b.as_ref()),
            (from, to),
            amount,
            scale,
            native_channels,
        )?,
        _ => {
            return Err(TransitionError::Requires(
                TransitionRequirement::CompatibleOwners,
            ));
        }
    };
    // Validation above proves ownership; intersect also prevents a transfer naming nonexistent
    // endpoint fields if a future representation extends this dispatch.
    trace.from.identity = trace.from.identity.intersection(&from_scope);
    trace.to.identity = trace.to.identity.intersection(&to_scope);
    Ok(trace)
}

fn identity(scope: ProgrammingFieldScope) -> ProgrammingFieldTransfer {
    ProgrammingFieldTransfer {
        identity: scope,
        remap: Arc::default(),
    }
}

fn both(fields: Vec<ProgrammingTraceField>) -> ProgrammingTransitionTrace {
    let transfer = identity(ProgrammingFieldScope::new(fields));
    ProgrammingTransitionTrace {
        from: transfer.clone(),
        to: transfer,
    }
}

/// Trace one ColorProgram transition between the programs of the `from` and `to` endpoints.
fn color_program_trace(
    programs: (&ColorProgram, &ColorProgram),
    (from, to): (&AttributeValue, &AttributeValue),
    amount: f32,
    scale: bool,
    native_channels: Option<&[(Uuid, bool)]>,
) -> Result<ProgrammingTransitionTrace, TransitionError> {
    use ProgrammingTraceField as F;
    Ok(match programs {
        (ColorProgram::Semantic { intent: a }, ColorProgram::Semantic { intent: b }) => {
            let mut trace = both(vec![
                F::ColorXyz,
                F::WhiteBlend,
                F::Temperature,
                F::Duv,
                F::Uv,
                F::RelativeOutput,
            ]);
            if a.base_xyz != b.base_xyz || a.recipe != b.recipe {
                let remap: Arc<[_]> = vec![
                    (F::ColorXyz, F::ColorRecipeRed),
                    (F::ColorXyz, F::ColorRecipeGreen),
                    (F::ColorXyz, F::ColorRecipeBlue),
                ]
                .into();
                trace.from.remap = remap.clone();
                trace.to.remap = remap;
                // set_coordinates generates Amber=0. Its prior recipe field is not an input.
            } else {
                let recipe = ProgrammingFieldScope::new([
                    F::ColorRecipeRed,
                    F::ColorRecipeGreen,
                    F::ColorRecipeBlue,
                    F::ColorRecipeAmber,
                ]);
                trace.from.identity = trace.from.identity.union(&recipe);
                trace.to.identity = trace.to.identity.union(&recipe);
            }
            let held = if scale && amount > 1.0 {
                &mut trace.to
            } else {
                &mut trace.from
            };
            held.identity = held
                .identity
                .union(&ProgrammingFieldScope::new([F::Allocation, F::ColorWheels]));
            trace
        }
        (ColorProgram::Direct { recipe: a, .. }, ColorProgram::Direct { recipe: b, .. })
            if a.source == b.source =>
        {
            let channels = native_channels.ok_or(TransitionError::Requires(
                TransitionRequirement::NativeColorModel,
            ))?;
            let mut trace = ProgrammingTransitionTrace::default();
            trace.from.identity = ProgrammingFieldScope::new([F::NativeColorIdentity]);
            let mut from_fields = vec![];
            let mut to_fields = vec![];
            for &(channel, continuous) in channels {
                let field = F::NativeColorChannel(channel);
                if continuous || !(scale && amount > 1.0) {
                    from_fields.push(field);
                }
                if continuous || (scale && amount > 1.0) {
                    to_fields.push(field);
                }
            }
            trace.from.identity = trace
                .from
                .identity
                .union(&ProgrammingFieldScope::new(from_fields));
            trace.to.identity = ProgrammingFieldScope::new(to_fields);
            if from == to {
                // The value optimization retained the portable estimate instead of predicting.
                let prediction = ProgrammingFieldScope::new([F::NativePrediction]);
                trace.from.identity = trace.from.identity.union(&prediction);
                trace.to.identity = trace.to.identity.union(&prediction);
            } else {
                for transfer in [&mut trace.from, &mut trace.to] {
                    transfer.remap = transfer
                        .identity
                        .fields()
                        .map(|field| (field, F::NativePrediction))
                        .collect();
                }
            }
            trace
        }
        _ => {
            return Err(TransitionError::Requires(
                TransitionRequirement::ColorAppearance,
            ));
        }
    })
}

#[cfg(test)]
mod tests;
