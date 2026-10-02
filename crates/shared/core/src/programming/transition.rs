//! Shared interpolation of materialized owner values. Live geometry/optical conversions are
//! explicit requirements: a caller must retain both endpoints and resolve them in its frame,
//! never turn an unresolved transition into a stored midpoint or a made-up zero family.
use super::*;
use crate::{AttributeValue, NativeColorBinding, Xyz};
use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransitionRequirement {
    /// Resolve ordered Group membership and component spreads before compiling this transition.
    MaterializedEndpoints,
    /// Evaluate both aim expressions in the same live world frame, then interpolate and solve once.
    LiveTargetPoints,
    /// Angle/Target crossing needs live, reachable, unwrapped joints and lane-local continuity.
    LiveJointAngles,
    /// Same-source Direct interpolation needs its immutable, verified native model.
    NativeColorModel,
    /// Cross-representation Color requires modeled visible appearance and independent UV knowledge.
    ColorAppearance,
    /// Beam and Field angles are different measurements; conversion needs the fixture's optics.
    ZoomConvention,
    /// A complete family cannot be mixed with a legacy component or an unrelated family.
    CompatibleOwners,
}

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum TransitionError {
    #[error("transition requires {0:?}")]
    Requires(TransitionRequirement),
    #[error(transparent)]
    Invalid(#[from] IntentError),
}

/// Cheap evaluator for compatible complete values. Inputs have already passed the storage or
/// mutation boundary; expensive profile lookup, validation and channel correspondence do not
/// happen here. Exact endpoints keep their original Arc and authored recipe.
pub fn interpolate_programming_value(
    from: &AttributeValue,
    to: &AttributeValue,
    progress: f32,
) -> Result<AttributeValue, TransitionError> {
    require(progress.is_finite(), "transition progress must be finite")?;
    require_materialized(from, to)?;
    if progress <= 0.0 {
        return Ok(from.clone());
    }
    if progress >= 1.0 {
        return Ok(to.clone());
    }
    // Equality is particularly useful for retained semantic recipes and stationary live Targets.
    if from == to {
        return Ok(from.clone());
    }
    Ok(match (from, to) {
        (AttributeValue::Normalized(a), AttributeValue::Normalized(b)) => {
            AttributeValue::Normalized(linear(*a, *b, progress))
        }
        (AttributeValue::ColorXyz(a), AttributeValue::ColorXyz(b)) => {
            AttributeValue::ColorXyz(interpolate_xyz(*a, *b, progress))
        }
        (AttributeValue::Position(a), AttributeValue::Position(b)) => {
            AttributeValue::Position(Arc::new(match (a.as_ref(), b.as_ref()) {
                (
                    PositionIntent::Angles {
                        pan_degrees: ap,
                        tilt_degrees: at,
                    },
                    PositionIntent::Angles {
                        pan_degrees: bp,
                        tilt_degrees: bt,
                    },
                ) => PositionIntent::angles(
                    linear(scalar(ap), scalar(bp), progress),
                    linear(scalar(at), scalar(bt), progress),
                ),
                (
                    PositionIntent::Target {
                        reference: ar,
                        offset_metres: a,
                    },
                    PositionIntent::Target {
                        reference: br,
                        offset_metres: b,
                    },
                ) if ar == br => PositionIntent::target(
                    *ar,
                    std::array::from_fn(|i| linear(scalar(&a[i]), scalar(&b[i]), progress)),
                ),
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
            }))
        }
        (AttributeValue::Zoom(a), AttributeValue::Zoom(b)) => {
            if a.convention != b.convention {
                return Err(TransitionError::Requires(
                    TransitionRequirement::ZoomConvention,
                ));
            }
            AttributeValue::Zoom(Arc::new(ZoomIntent {
                opening_degrees: ScalarIntent::Value(linear(
                    scalar(&a.opening_degrees),
                    scalar(&b.opening_degrees),
                    progress,
                )),
                convention: a.convention,
            }))
        }
        (AttributeValue::ColorProgram(a), AttributeValue::ColorProgram(b)) => {
            match (a.as_ref(), b.as_ref()) {
                (ColorProgram::Semantic { intent: a }, ColorProgram::Semantic { intent: b }) => {
                    let mut intent = a.clone();
                    if a.base_xyz != b.base_xyz || a.recipe != b.recipe {
                        VirtualColorAuthoringV1.set_coordinates(
                            &mut intent,
                            interpolate_xyz(a.base_xyz, b.base_xyz, progress),
                        )?;
                    }
                    intent.white_blend = linear(a.white_blend, b.white_blend, progress);
                    intent.white_target = WhiteTarget {
                        kelvin: interpolate_scalar(
                            a.white_target.kelvin,
                            b.white_target.kelvin,
                            progress,
                            ScalarInterpolation::Reciprocal,
                        ),
                        duv: linear(a.white_target.duv, b.white_target.duv, progress),
                    };
                    intent.uv.amount = linear(a.uv.amount, b.uv.amount, progress);
                    intent.relative_output = linear(a.relative_output, b.relative_output, progress);
                    // Allocation and steady wheel constraints hold until the exact endpoint.
                    AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic { intent }))
                }
                (
                    ColorProgram::Direct { recipe: a, .. },
                    ColorProgram::Direct { recipe: b, .. },
                ) if a.source == b.source => {
                    return Err(TransitionError::Requires(
                        TransitionRequirement::NativeColorModel,
                    ));
                }
                _ => {
                    return Err(TransitionError::Requires(
                        TransitionRequirement::ColorAppearance,
                    ));
                }
            }
        }
        _ if from.programming_owner().is_some() || to.programming_owner().is_some() => {
            return Err(TransitionError::Requires(
                TransitionRequirement::CompatibleOwners,
            ));
        }
        // Existing discrete/control/raw domains retain their steady selection until completion.
        _ => from.clone(),
    })
}

fn require_materialized(from: &AttributeValue, to: &AttributeValue) -> Result<(), TransitionError> {
    if [from, to].iter().any(|value| {
        value.spread_control_points() != 0 || matches!(value, AttributeValue::GroupFamily(_))
    }) {
        return Err(TransitionError::Requires(
            TransitionRequirement::MaterializedEndpoints,
        ));
    }
    Ok(())
}
fn scalar(value: &ScalarIntent) -> f32 {
    let ScalarIntent::Value(value) = value else {
        unreachable!("materialized endpoint")
    };
    *value
}
fn linear(from: f32, to: f32, progress: f32) -> f32 {
    interpolate_scalar(from, to, progress, ScalarInterpolation::Linear)
}
fn interpolate_xyz(from: Xyz, to: Xyz, progress: f32) -> Xyz {
    Xyz {
        x: linear(from.x, to.x, progress),
        y: linear(from.y, to.y, progress),
        z: linear(from.z, to.z, progress),
    }
}

/// Runtime-only compilation. Pinning verifies the complete source identity, not merely a matching
/// layout signature. Native channel correspondence is prepared once; sampling predicts the varied
/// recipe exactly once. Persist only the authored endpoint values, never this compiled object.
#[derive(Clone)]
pub struct CompiledProgrammingTransition {
    from: AttributeValue,
    to: AttributeValue,
    native: Option<Arc<CompiledNativeTransition>>,
    trace: Arc<OnceLock<CompiledTracePlans>>,
}
struct CompiledTracePlans {
    owner: ProgrammingOwner,
    from: Result<ProgrammingTransitionTrace, TransitionError>,
    to: Result<ProgrammingTransitionTrace, TransitionError>,
    interior: Result<ProgrammingTransitionTrace, TransitionError>,
    extrapolated: Result<ProgrammingTransitionTrace, TransitionError>,
}
struct CompiledNativeTransition {
    model: Arc<dyn NativeColorEditModel + Send + Sync>,
    channels: Vec<NativeTransitionChannel>,
    trace_channels: Vec<(uuid::Uuid, bool)>,
}
struct NativeTransitionChannel {
    destination_index: usize,
    continuous: bool,
    minimum: u32,
    maximum: u32,
}
impl CompiledProgrammingTransition {
    pub fn new(
        from: AttributeValue,
        to: AttributeValue,
        native_model: Option<Arc<dyn NativeColorEditModel + Send + Sync>>,
    ) -> Result<Self, TransitionError> {
        require_materialized(&from, &to)?;
        for endpoint in [&from, &to] {
            if let Some(owner) = endpoint.programming_owner() {
                endpoint.validate_programming_address(&owner.key())?;
            }
        }
        let native = match (&from, &to, native_model) {
            (AttributeValue::ColorProgram(a), AttributeValue::ColorProgram(b), Some(model)) => {
                match (a.as_ref(), b.as_ref()) {
                    (
                        ColorProgram::Direct { recipe: a, .. },
                        ColorProgram::Direct { recipe: b, .. },
                    ) if a.source == b.source => {
                        require(
                            model.source() == &a.source,
                            "native transition model differs from pinned source identity",
                        )?;
                        Some(Arc::new(compile_native(a, b, model)?))
                    }
                    _ => None,
                }
            }
            _ => None,
        };
        Ok(Self {
            from,
            to,
            native,
            trace: Arc::default(),
        })
    }
    /// A live frame resolver uses the retained expressions on every tick, including while time
    /// is paused. Interruption must create a new source from the current solved physical pose.
    pub fn endpoints(&self) -> (&AttributeValue, &AttributeValue) {
        (&self.from, &self.to)
    }

    /// Successful values remain available when exact field lineage needs a live resolver.
    /// Sampling and native prediction run once; structural transfer plans are compiled lazily.
    pub fn sample_with_trace(
        &self,
        owner: ProgrammingOwner,
        progress: f32,
    ) -> Result<(AttributeValue, Option<ProgrammingTransitionTrace>), TransitionError> {
        let value = self.sample(progress)?;
        let trace = self.trace_after_sample(owner, progress, false).ok();
        Ok((value, trace))
    }

    pub fn scale_with_trace(
        &self,
        owner: ProgrammingOwner,
        factor: f32,
    ) -> Result<(AttributeValue, Option<ProgrammingTransitionTrace>), TransitionError> {
        let value = self.scale(factor)?;
        let trace = self.trace_after_sample(owner, factor, true).ok();
        Ok((value, trace))
    }

    pub fn sample_trace(
        &self,
        owner: ProgrammingOwner,
        progress: f32,
    ) -> Result<ProgrammingTransitionTrace, TransitionError> {
        self.sample(progress)?;
        self.trace_after_sample(owner, progress, false)
    }

    pub fn scale_trace(
        &self,
        owner: ProgrammingOwner,
        factor: f32,
    ) -> Result<ProgrammingTransitionTrace, TransitionError> {
        self.scale(factor)?;
        self.trace_after_sample(owner, factor, true)
    }

    fn trace_after_sample(
        &self,
        owner: ProgrammingOwner,
        amount: f32,
        scale: bool,
    ) -> Result<ProgrammingTransitionTrace, TransitionError> {
        let build = |amount, scale| {
            field_trace::successful_transition_trace(
                owner,
                &self.from,
                &self.to,
                amount,
                scale,
                self.native
                    .as_ref()
                    .map(|native| native.trace_channels.as_slice()),
            )
        };
        let plans = self.trace.get_or_init(|| CompiledTracePlans {
            owner,
            from: build(0.0, false),
            to: build(1.0, false),
            interior: build(0.5, false),
            extrapolated: build(2.0, true),
        });
        if plans.owner != owner {
            return build(amount, scale);
        }
        if amount <= 0.0 {
            plans.from.clone()
        } else if amount == 1.0 || (!scale && amount >= 1.0) {
            plans.to.clone()
        } else if scale && amount > 1.0 {
            plans.extrapolated.clone()
        } else if scale && matches!(self.from, AttributeValue::ColorXyz(_)) {
            // Legacy scale holds ColorXyz even for an interior factor.
            plans.extrapolated.clone()
        } else {
            plans.interior.clone()
        }
    }

    pub fn sample(&self, progress: f32) -> Result<AttributeValue, TransitionError> {
        require(progress.is_finite(), "transition progress must be finite")?;
        if !(0.0 < progress && progress < 1.0) || self.from == self.to || self.native.is_none() {
            return interpolate_programming_value(&self.from, &self.to, progress);
        }
        let native = self.native.as_ref().expect("compiled native transition");
        let (AttributeValue::ColorProgram(a), AttributeValue::ColorProgram(b)) =
            (&self.from, &self.to)
        else {
            unreachable!()
        };
        let (ColorProgram::Direct { recipe: a, .. }, ColorProgram::Direct { recipe: b, .. }) =
            (a.as_ref(), b.as_ref())
        else {
            unreachable!()
        };
        let mut recipe = a.clone();
        for (index, compiled) in native.channels.iter().enumerate() {
            if compiled.continuous {
                recipe.channels[index].raw = scale_native_delta(
                    a.channels[index].raw,
                    i64::from(b.channels[compiled.destination_index].raw)
                        - i64::from(a.channels[index].raw),
                    progress,
                    0,
                    u32::MAX,
                )?;
            }
        }
        let portable = native.model.predict(&recipe)?;
        require(
            portable.model_revision == recipe.source.model_revision,
            "native transition prediction differs from pinned source model revision",
        )?;
        portable.validate()?;
        Ok(AttributeValue::ColorProgram(Arc::new(
            ColorProgram::Direct { recipe, portable },
        )))
    }

    /// Scale the complete destination difference from the original source. A factor above one
    /// extrapolates within each declared component domain; unresolved representation crossings
    /// retain the same explicit requirements as interpolation. Exact endpoints keep their
    /// authored values, recipes and shared storage identity.
    pub fn scale(&self, factor: f32) -> Result<AttributeValue, TransitionError> {
        require(
            factor.is_finite() && factor >= 0.0,
            "transition size must be finite and nonnegative",
        )?;
        if factor == 0.0 {
            return Ok(self.from.clone());
        }
        if factor == 1.0 {
            return Ok(self.to.clone());
        }
        if self.from == self.to {
            return Ok(self.from.clone());
        }
        Ok(match (&self.from, &self.to) {
            (AttributeValue::Normalized(a), AttributeValue::Normalized(b)) => {
                AttributeValue::Normalized(scale_bounded(*a, *b, factor, 0.0, 1.0))
            }
            (AttributeValue::Position(a), AttributeValue::Position(b)) => {
                AttributeValue::Position(Arc::new(match (a.as_ref(), b.as_ref()) {
                    (
                        PositionIntent::Angles {
                            pan_degrees: ap,
                            tilt_degrees: at,
                        },
                        PositionIntent::Angles {
                            pan_degrees: bp,
                            tilt_degrees: bt,
                        },
                    ) => PositionIntent::angles(
                        scale_finite(scalar(ap), scalar(bp), factor)?,
                        scale_finite(scalar(at), scalar(bt), factor)?,
                    ),
                    (
                        PositionIntent::Target {
                            reference: ar,
                            offset_metres: a,
                        },
                        PositionIntent::Target {
                            reference: br,
                            offset_metres: b,
                        },
                    ) if ar == br => PositionIntent::target(
                        *ar,
                        [
                            scale_finite(scalar(&a[0]), scalar(&b[0]), factor)?,
                            scale_finite(scalar(&a[1]), scalar(&b[1]), factor)?,
                            scale_finite(scalar(&a[2]), scalar(&b[2]), factor)?,
                        ],
                    ),
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
                }))
            }
            (AttributeValue::Zoom(a), AttributeValue::Zoom(b)) => {
                if a.convention != b.convention {
                    return Err(TransitionError::Requires(
                        TransitionRequirement::ZoomConvention,
                    ));
                }
                AttributeValue::Zoom(Arc::new(ZoomIntent {
                    opening_degrees: ScalarIntent::Value(scale_bounded(
                        scalar(&a.opening_degrees),
                        scalar(&b.opening_degrees),
                        factor,
                        0.0,
                        180.0,
                    )),
                    convention: a.convention,
                }))
            }
            (AttributeValue::ColorProgram(a), AttributeValue::ColorProgram(b)) => {
                match (a.as_ref(), b.as_ref()) {
                    (
                        ColorProgram::Semantic { intent: a },
                        ColorProgram::Semantic { intent: b },
                    ) => AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
                        intent: scale_semantic(a, b, factor)?,
                    })),
                    (
                        ColorProgram::Direct { recipe: a, .. },
                        ColorProgram::Direct { recipe: b, .. },
                    ) if a.source == b.source => {
                        let native = self.native.as_ref().ok_or(TransitionError::Requires(
                            TransitionRequirement::NativeColorModel,
                        ))?;
                        let mut recipe = a.clone();
                        for (index, compiled) in native.channels.iter().enumerate() {
                            let destination = &b.channels[compiled.destination_index];
                            if compiled.continuous {
                                recipe.channels[index].raw = scale_native_delta(
                                    a.channels[index].raw,
                                    i64::from(destination.raw) - i64::from(a.channels[index].raw),
                                    factor,
                                    compiled.minimum,
                                    compiled.maximum,
                                )?;
                            } else if factor >= 1.0 {
                                recipe.channels[index] = destination.clone();
                            }
                        }
                        let portable = native.model.predict(&recipe)?;
                        require(
                            portable.model_revision == recipe.source.model_revision,
                            "native size prediction differs from pinned source model revision",
                        )?;
                        portable.validate()?;
                        AttributeValue::ColorProgram(Arc::new(ColorProgram::Direct {
                            recipe,
                            portable,
                        }))
                    }
                    _ => {
                        return Err(TransitionError::Requires(
                            TransitionRequirement::ColorAppearance,
                        ));
                    }
                }
            }
            _ if self.from.programming_owner().is_some()
                || self.to.programming_owner().is_some() =>
            {
                return Err(TransitionError::Requires(
                    TransitionRequirement::CompatibleOwners,
                ));
            }
            _ => self.from.clone(),
        })
    }
}

fn scale_finite(from: f32, to: f32, factor: f32) -> Result<f32, IntentError> {
    let value = f64::from(from) + (f64::from(to) - f64::from(from)) * f64::from(factor);
    require(
        value.is_finite() && value >= f64::from(f32::MIN) && value <= f64::from(f32::MAX),
        "scaled component exceeds the finite domain",
    )?;
    Ok(value as f32)
}

fn scale_bounded(from: f32, to: f32, factor: f32, minimum: f32, maximum: f32) -> f32 {
    (f64::from(from) + (f64::from(to) - f64::from(from)) * f64::from(factor))
        .clamp(f64::from(minimum), f64::from(maximum)) as f32
}

fn scale_reciprocal_kelvin(from: f32, to: f32, factor: f32) -> f32 {
    let reciprocal =
        1.0 / f64::from(from) + (1.0 / f64::from(to) - 1.0 / f64::from(from)) * f64::from(factor);
    (1.0 / reciprocal.clamp(1.0 / 20_000.0, 1.0 / 1_000.0)) as f32
}

fn scale_semantic(
    a: &ColorIntent,
    b: &ColorIntent,
    factor: f32,
) -> Result<ColorIntent, IntentError> {
    let mut intent = a.clone();
    if a.base_xyz != b.base_xyz || a.recipe != b.recipe {
        VirtualColorAuthoringV1.set_coordinates(
            &mut intent,
            Xyz {
                x: scale_bounded(a.base_xyz.x, b.base_xyz.x, factor, 0.0, f32::MAX),
                y: scale_bounded(a.base_xyz.y, b.base_xyz.y, factor, 0.0, f32::MAX),
                z: scale_bounded(a.base_xyz.z, b.base_xyz.z, factor, 0.0, f32::MAX),
            },
        )?;
    }
    intent.white_blend = scale_bounded(a.white_blend, b.white_blend, factor, 0.0, 1.0);
    intent.white_target = WhiteTarget {
        kelvin: scale_reciprocal_kelvin(a.white_target.kelvin, b.white_target.kelvin, factor),
        duv: scale_bounded(a.white_target.duv, b.white_target.duv, factor, -0.03, 0.03),
    };
    intent.uv.amount = scale_bounded(a.uv.amount, b.uv.amount, factor, 0.0, 1.0);
    intent.relative_output =
        scale_bounded(a.relative_output, b.relative_output, factor, 0.0, f32::MAX);
    if factor >= 1.0 {
        intent.allocation = b.allocation;
        intent.wheel_constraints.clone_from(&b.wheel_constraints);
    }
    intent.validate()?;
    Ok(intent)
}

fn compile_native(
    from: &NativeColorRecipe,
    to: &NativeColorRecipe,
    model: Arc<dyn NativeColorEditModel + Send + Sync>,
) -> Result<CompiledNativeTransition, IntentError> {
    let destination: HashMap<_, _> = to
        .channels
        .iter()
        .enumerate()
        .map(|(i, value)| (value.channel_id, (i, value)))
        .collect();
    require(
        from.channels.len() == to.channels.len(),
        "native transition recipes have different channel ownership",
    )?;
    let mut channels = Vec::with_capacity(from.channels.len());
    for value in &from.channels {
        let Some((destination_index, target)) = destination.get(&value.channel_id) else {
            return Err(IntentError(
                "native transition recipe is missing a channel".into(),
            ));
        };
        let describe = |value: &crate::NativeColorValue| -> Result<NativeColorComponentDescriptor, IntentError> {
            let binding = NativeColorBinding { channel_id: value.channel_id, function_id: value.function_id };
            let descriptor = model.descriptor(binding).ok_or_else(|| IntentError("native transition function is absent from pinned source".into()))?;
            require(descriptor.binding == binding &&
                (descriptor.raw_from.min(descriptor.raw_to)..=descriptor.raw_from.max(descriptor.raw_to)).contains(&value.raw),
                "native transition value is outside its pinned function")?;
            Ok(descriptor)
        };
        let descriptor = describe(value)?;
        describe(target)?;
        channels.push(NativeTransitionChannel {
            destination_index: *destination_index,
            continuous: descriptor.continuous && value.function_id == target.function_id,
            minimum: descriptor.raw_from.min(descriptor.raw_to),
            maximum: descriptor.raw_from.max(descriptor.raw_to),
        });
    }
    let trace_channels = from
        .channels
        .iter()
        .zip(&channels)
        .map(|(value, compiled)| (value.channel_id, compiled.continuous))
        .collect();
    Ok(CompiledNativeTransition {
        model,
        channels,
        trace_channels,
    })
}
