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

/// Canonical immutable set. The wheel aggregate subsumes individual wheel queries; it does not
/// claim that an empty constraint collection contains a physical wheel.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ProgrammingFieldScope(Arc<[ProgrammingTraceField]>);

impl<'de> Deserialize<'de> for ProgrammingFieldScope {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let scope = Self::new(Vec::<ProgrammingTraceField>::deserialize(deserializer)?);
        if scope.fields().iter().any(|field| {
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
    pub fn new(fields: impl IntoIterator<Item = ProgrammingTraceField>) -> Self {
        let mut fields: Vec<_> = fields.into_iter().collect();
        fields.sort_unstable();
        fields.dedup();
        if fields.contains(&ProgrammingTraceField::ColorWheels) {
            fields.retain(|field| !matches!(field, ProgrammingTraceField::ColorWheel(_)));
        }
        Self(fields.into())
    }

    pub fn empty() -> Self {
        Self::default()
    }

    pub fn fields(&self) -> &[ProgrammingTraceField] {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn contains(&self, field: ProgrammingTraceField) -> bool {
        self.0.binary_search(&field).is_ok()
            || (matches!(field, ProgrammingTraceField::ColorWheel(_))
                && self
                    .0
                    .binary_search(&ProgrammingTraceField::ColorWheels)
                    .is_ok())
    }

    fn overlaps(&self, field: ProgrammingTraceField) -> bool {
        self.contains(field)
            || (field == ProgrammingTraceField::ColorWheels
                && self
                    .0
                    .iter()
                    .any(|field| matches!(field, ProgrammingTraceField::ColorWheel(_))))
    }

    pub fn union(&self, other: &Self) -> Self {
        if self == other || other.is_empty() {
            return self.clone();
        }
        if self.is_empty() {
            return other.clone();
        }
        Self::new(self.0.iter().chain(other.0.iter()).copied())
    }

    pub fn intersection(&self, other: &Self) -> Self {
        if self == other {
            return self.clone();
        }
        Self::new(
            self.0
                .iter()
                .chain(other.0.iter())
                .copied()
                .filter(|field| self.contains(*field) && other.contains(*field)),
        )
    }

    /// A wildcard minus individual wheels cannot be represented by this finite positive set.
    /// Callers must keep that query unknown, or request individual wheel fields instead.
    pub fn difference(&self, other: &Self) -> Result<Self, IntentError> {
        require(
            !(self.contains(ProgrammingTraceField::ColorWheels)
                && !other.contains(ProgrammingTraceField::ColorWheels)
                && other
                    .fields()
                    .iter()
                    .any(|field| matches!(field, ProgrammingTraceField::ColorWheel(_)))),
            "wheel aggregate minus individual wheels is not an exact field scope",
        )?;
        Ok(Self::new(
            self.0
                .iter()
                .copied()
                .filter(|field| !other.contains(*field)),
        ))
    }

    pub fn validate(&self, owner: ProgrammingOwner) -> Result<(), IntentError> {
        require(
            self.0.iter().all(|field| field.owner() == owner),
            "trace field belongs to a different programming owner",
        )?;
        require(
            self.0.iter().all(|field| {
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
        Self::new(match component {
            P::Color(C::Red) => vec![F::ColorXyz, F::ColorRecipeRed],
            P::Color(C::Green) => vec![F::ColorXyz, F::ColorRecipeGreen],
            P::Color(C::Blue) => vec![F::ColorXyz, F::ColorRecipeBlue],
            P::Color(C::Amber) => vec![F::ColorXyz, F::ColorRecipeAmber],
            P::Color(C::Hue | C::Saturation) => vec![
                F::ColorXyz,
                F::ColorRecipeRed,
                F::ColorRecipeGreen,
                F::ColorRecipeBlue,
            ],
            P::Color(C::WhiteBlend) => vec![F::WhiteBlend],
            P::Color(C::Temperature) => vec![F::Temperature],
            P::Color(C::Duv) => vec![F::Duv],
            P::Color(C::Uv) => vec![F::Uv],
            P::Color(C::RelativeOutput) => vec![F::RelativeOutput],
            P::ColorWheel(wheel) => vec![F::ColorWheel(wheel)],
            P::NativeColor(binding) => vec![F::NativeColorChannel(binding.channel_id)],
            P::Pan => vec![F::Pan],
            P::Tilt => vec![F::Tilt],
            P::TargetReference => vec![F::TargetReference],
            P::TargetX => vec![F::TargetX],
            P::TargetY => vec![F::TargetY],
            P::TargetZ => vec![F::TargetZ],
            P::Focus => vec![F::Focus],
            P::Zoom => vec![F::Zoom],
        })
    }

    /// Materialize Whole against the actual endpoint representation. This is not a union of
    /// every possible representation of the owner (for example, Angle has no Target fields).
    pub fn for_value(owner: ProgrammingOwner, value: &AttributeValue) -> Result<Self, IntentError> {
        use ProgrammingTraceField as F;
        value.validate_programming_address(&owner.key())?;
        require(
            value.spread_control_points() == 0,
            "trace scope requires materialized programming values",
        )?;
        let fields = match (owner, value) {
            (ProgrammingOwner::Color, AttributeValue::ColorProgram(program)) => {
                match program.as_ref() {
                    ColorProgram::Semantic { .. } => vec![
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
                    ],
                    ColorProgram::Direct { recipe, .. } => {
                        let mut fields = vec![F::NativeColorIdentity, F::NativePrediction];
                        fields.extend(
                            recipe
                                .channels
                                .iter()
                                .map(|channel| F::NativeColorChannel(channel.channel_id)),
                        );
                        fields
                    }
                }
            }
            (ProgrammingOwner::Color, AttributeValue::ColorXyz(xyz)) => {
                require(
                    valid_xyz(*xyz),
                    "Color trace coordinates must be finite and nonnegative",
                )?;
                vec![F::ColorXyz]
            }
            (ProgrammingOwner::Position, AttributeValue::Position(position)) => {
                match position.as_ref() {
                    PositionIntent::Angles { .. } => vec![F::Pan, F::Tilt],
                    PositionIntent::Target { .. } => {
                        vec![F::TargetReference, F::TargetX, F::TargetY, F::TargetZ]
                    }
                }
            }
            (ProgrammingOwner::Focus, AttributeValue::Normalized(value)) => {
                require(
                    ScalarDomain::UNIT.contains(*value),
                    "Focus trace value is outside 0-1",
                )?;
                vec![F::Focus]
            }
            (ProgrammingOwner::Zoom, AttributeValue::Zoom(_)) => vec![F::Zoom, F::ZoomConvention],
            _ => {
                return Err(IntentError(
                    "value has no materialized trace fields for this owner".into(),
                ));
            }
        };
        Ok(Self::new(fields))
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
                        .iter()
                        .copied()
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
