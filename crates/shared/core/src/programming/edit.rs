//! Pure, atomic edits of a complete owner. The application supplies the once-per-gesture base;
//! profile/virtual-model services supply conversions. No edit reads a previously fitted channel.
use super::*;
use crate::{AttributeValue, NativeColorBinding, NativeColorIdentity, Xyz};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, sync::Arc};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ScalarEdit {
    Set(ScalarIntent),
    Relative(f32),
}
impl ScalarEdit {
    pub fn apply(
        &self,
        base: &ScalarIntent,
        domain: ScalarDomain,
    ) -> Result<ScalarIntent, IntentError> {
        base.validate(domain)?;
        match self {
            Self::Set(value) => {
                value.validate(domain)?;
                Ok(value.clone())
            }
            Self::Relative(delta) => {
                require(delta.is_finite(), "relative component step must be finite")?;
                let shift = |value: f32| {
                    let shifted = f64::from(value) + f64::from(*delta);
                    // Bounded domains can clamp before conversion; unbounded physical values
                    // must report overflow, never silently become a percentage or infinity.
                    let shifted = match domain {
                        ScalarDomain::Bounded { bounds } => {
                            shifted.clamp(f64::from(bounds.min), f64::from(bounds.max))
                        }
                        ScalarDomain::Cyclic { bounds } => {
                            f64::from(bounds.min)
                                + (shifted - f64::from(bounds.min))
                                    .rem_euclid(f64::from(bounds.max) - f64::from(bounds.min))
                        }
                        ScalarDomain::Finite => shifted,
                    };
                    domain.constrain(shifted as f32)
                };
                match base {
                    ScalarIntent::Value(value) => Ok(ScalarIntent::Value(shift(*value)?)),
                    ScalarIntent::Spread(points) => Ok(ScalarIntent::Spread(
                        points
                            .iter()
                            .copied()
                            .map(shift)
                            .collect::<Result<_, _>>()?,
                    )),
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum NativeColorEdit {
    Set(u32),
    Spread(Vec<u32>),
    Relative(i64),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ComponentEdit {
    /// Explicit mode activation can take over even when the current solved pose is unchanged.
    ActivateAngles,
    Scalar {
        component: ProgrammingComponent,
        operation: ScalarEdit,
    },
    Target {
        reference: TargetReference,
    },
    Coordinates {
        xyz: Xyz,
    },
    Native {
        binding: NativeColorBinding,
        operation: NativeColorEdit,
    },
}
impl ComponentEdit {
    pub fn owner(&self) -> ProgrammingOwner {
        match self {
            Self::Scalar { component, .. } => component.owner(),
            Self::ActivateAngles | Self::Target { .. } => ProgrammingOwner::Position,
            Self::Coordinates { .. } | Self::Native { .. } => ProgrammingOwner::Color,
        }
    }
}

/// Pinned virtual authoring model. Implementations update XYZ and the displayed recipe together;
/// this interface deliberately has no fixture/destination argument.
pub trait ColorAuthoringModel {
    fn read_base_component(
        &self,
        intent: &ColorIntent,
        component: ColorComponent,
    ) -> Result<f32, IntentError>;
    fn set_base_component(
        &self,
        intent: &mut ColorIntent,
        component: ColorComponent,
        value: f32,
    ) -> Result<(), IntentError>;
    fn set_coordinates(&self, intent: &mut ColorIntent, xyz: Xyz) -> Result<(), IntentError>;
}
/// A verified pinned native source, not the destination selected for best-effort replay.
pub trait NativeColorEditModel {
    fn source(&self) -> &NativeColorIdentity;
    fn descriptor(&self, binding: NativeColorBinding) -> Option<NativeColorComponentDescriptor>;
    /// Predict one materialized recipe with no unresolved spreads. Editing predicts its
    /// reference channels; selection sampling predicts each member's resolved recipe.
    fn predict(&self, recipe: &NativeColorRecipe) -> Result<PortableColorEstimate, IntentError>;
    /// The same single prediction plus the typed source drive diagnostic used by Direct
    /// capture. Models without drive evidence report `Unknown`, never `Within`.
    fn predict_with_status(
        &self,
        recipe: &NativeColorRecipe,
    ) -> Result<NativeColorPrediction, IntentError> {
        Ok(NativeColorPrediction {
            portable: self.predict(recipe)?,
            drive_limit: NativeDriveLimit::Unknown,
        })
    }
}
#[derive(Default)]
pub struct FamilyEditContext<'a> {
    /// Current solved *unwrapped* joints when a gesture takes over from Target.
    pub solved_angles: Option<JointAngles>,
    /// Explicit modeled adoption (or explicitly chosen starting color) when leaving Direct.
    pub semantic_color_adoption: Option<&'a ColorIntent>,
    pub color_model: Option<&'a dyn ColorAuthoringModel>,
    pub native_model: Option<&'a dyn NativeColorEditModel>,
}

/// One gesture sample yields one complete value or no value at all. Navigation is not an edit.
pub fn edit_family(
    base: &AttributeValue,
    edits: &[ComponentEdit],
    context: &FamilyEditContext<'_>,
) -> Result<AttributeValue, IntentError> {
    if edits.is_empty() {
        return Ok(base.clone());
    }
    let owner = edits[0].owner();
    require(
        edits.iter().all(|edit| edit.owner() == owner),
        "one edit transaction must address one complete family",
    )?;
    validate_component_edits(edits)?;
    base.validate_programming_address(&owner.key())?;
    let result = match (owner, base) {
        (ProgrammingOwner::Position, AttributeValue::Position(base)) => {
            AttributeValue::Position(Arc::new(edit_position(base, edits, context)?))
        }
        (ProgrammingOwner::Color, AttributeValue::ColorProgram(base)) => {
            AttributeValue::ColorProgram(Arc::new(edit_color(base, edits, context)?))
        }
        (ProgrammingOwner::Zoom, AttributeValue::Zoom(base)) => {
            let mut value = base.as_ref().clone();
            for edit in edits {
                let ComponentEdit::Scalar {
                    component: ProgrammingComponent::Zoom,
                    operation,
                } = edit
                else {
                    unreachable!()
                };
                value.opening_degrees = operation.apply(
                    &value.opening_degrees,
                    ProgrammingComponent::Zoom.descriptor().domain.unwrap(),
                )?;
            }
            AttributeValue::Zoom(Arc::new(value))
        }
        (ProgrammingOwner::Focus, AttributeValue::Normalized(base)) => {
            let mut value = ScalarIntent::Value(*base);
            for edit in edits {
                let ComponentEdit::Scalar {
                    component: ProgrammingComponent::Focus,
                    operation,
                } = edit
                else {
                    unreachable!()
                };
                value = operation.apply(&value, ScalarDomain::UNIT)?;
            }
            match value {
                ScalarIntent::Value(value) => AttributeValue::Normalized(value),
                ScalarIntent::Spread(points) => AttributeValue::Spread(points),
            }
        }
        (ProgrammingOwner::Focus, AttributeValue::Spread(base)) => {
            let mut value = ScalarIntent::Spread(base.clone());
            for edit in edits {
                let ComponentEdit::Scalar { operation, .. } = edit else {
                    unreachable!()
                };
                value = operation.apply(&value, ScalarDomain::UNIT)?;
            }
            match value {
                ScalarIntent::Value(value) => AttributeValue::Normalized(value),
                ScalarIntent::Spread(points) => AttributeValue::Spread(points),
            }
        }
        _ => {
            return Err(IntentError(
                "edit requires a complete seed for its owner".into(),
            ));
        }
    };
    result.validate_programming_address(&owner.key())?;
    Ok(result)
}
/// Validate the request without needing a selected fixture or an adoption seed. Empty-target
/// actions stay quiet, but malformed syntax and contradictory representations remain errors.
pub fn validate_component_edits(edits: &[ComponentEdit]) -> Result<(), IntentError> {
    if let Some(first) = edits.first() {
        require(
            edits.iter().all(|edit| edit.owner() == first.owner()),
            "one edit transaction must address one complete family",
        )?;
    }
    require(edits.len() <= 512, "too many component edits")?;
    let mut components = HashSet::new();
    let mut angle = false;
    let mut activates_angles = false;
    let mut target = false;
    let mut native = false;
    let mut semantic = false;
    let mut recipe = false;
    let mut coordinates = false;
    for edit in edits {
        match edit {
            ComponentEdit::ActivateAngles => {
                require(!activates_angles, "duplicate Angle activation")?;
                activates_angles = true;
                angle = true;
            }
            ComponentEdit::Scalar {
                component,
                operation,
            } => {
                let descriptor = component.descriptor();
                require(
                    descriptor.domain.is_some(),
                    "discrete/native components require their typed edit",
                )?;
                match operation {
                    ScalarEdit::Set(value) => value.validate(descriptor.domain.unwrap())?,
                    ScalarEdit::Relative(delta) => {
                        require(delta.is_finite(), "relative component step must be finite")?
                    }
                }
                require(components.insert(*component), "duplicate component edit")?;
                angle |= descriptor.role == ComponentRole::Angle;
                target |= descriptor.role == ComponentRole::Target;
                semantic |= descriptor.owner == ProgrammingOwner::Color;
                recipe |= descriptor.role == ComponentRole::ColorRecipe;
                coordinates |= descriptor.role == ComponentRole::ColorCoordinate;
            }
            ComponentEdit::Target { reference } => {
                if let TargetReference::Point { point_id } = reference {
                    require(!point_id.is_nil(), "target point must have a stable UUID")?;
                }
                target = true;
                require(
                    components.insert(ProgrammingComponent::TargetReference),
                    "duplicate target edit",
                )?;
            }
            ComponentEdit::Coordinates { xyz } => {
                require(
                    [xyz.x, xyz.y, xyz.z]
                        .iter()
                        .all(|value| value.is_finite() && *value >= 0.0),
                    "Color coordinates must be finite and non-negative",
                )?;
                semantic = true;
                coordinates = true;
            }
            ComponentEdit::Native { binding, operation } => {
                if let NativeColorEdit::Relative(delta) = operation {
                    require(
                        delta.unsigned_abs() <= u64::from(u32::MAX),
                        "native relative step exceeds the supported integer domain",
                    )?;
                }
                if let NativeColorEdit::Spread(points) = operation {
                    require(
                        (2..=4096).contains(&points.len()),
                        "native spread requires 2-4096 control points",
                    )?;
                }
                native = true;
                require(
                    components.insert(ProgrammingComponent::NativeColor(*binding)),
                    "duplicate native edit",
                )?;
            }
        }
    }
    require(!(angle && target), "Angle and Target edits are exclusive")?;
    require(
        !(native && semantic),
        "Semantic and Direct edits are exclusive",
    )?;
    require(
        !(recipe && coordinates),
        "recipe and coordinate edits cannot write the same Color base",
    )?;
    let coordinate_edits = edits
        .iter()
        .filter(|edit| matches!(edit, ComponentEdit::Coordinates { .. }))
        .count();
    require(
        coordinate_edits <= 1
            && !(coordinate_edits == 1
                && edits.iter().any(|edit| {
                    matches!(
                        edit,
                        ComponentEdit::Scalar {
                            component: ProgrammingComponent::Color(
                                ColorComponent::Hue | ColorComponent::Saturation
                            ),
                            ..
                        }
                    )
                })),
        "coordinate replacement and component edits are exclusive",
    )
}
fn edit_position(
    base: &PositionIntent,
    edits: &[ComponentEdit],
    context: &FamilyEditContext<'_>,
) -> Result<PositionIntent, IntentError> {
    let mut value = base.clone();
    if edits.iter().any(|edit| {
        matches!(
            edit,
            ComponentEdit::ActivateAngles
                | ComponentEdit::Scalar {
                    component: ProgrammingComponent::Pan | ProgrammingComponent::Tilt,
                    ..
                }
        )
    }) && matches!(value, PositionIntent::Target { .. })
    {
        let pose = context.solved_angles.ok_or_else(|| {
            IntentError("Angle takeover requires the current solved unwrapped pose".into())
        })?;
        value = PositionIntent::angles(pose.pan_degrees, pose.tilt_degrees);
    }
    // An explicit reference activation seeds Target once; scalar offsets never invent a target.
    if let Some(reference) = edits.iter().find_map(|edit| match edit {
        ComponentEdit::Target { reference } => Some(*reference),
        _ => None,
    }) {
        match &mut value {
            PositionIntent::Target {
                reference: existing,
                ..
            } => *existing = reference,
            PositionIntent::Angles { .. } => value = PositionIntent::target(reference, [0.0; 3]),
        }
    }
    for edit in edits {
        let ComponentEdit::Scalar {
            component,
            operation,
        } = edit
        else {
            continue;
        };
        let current = match (&mut value, component) {
            (PositionIntent::Angles { pan_degrees, .. }, ProgrammingComponent::Pan) => pan_degrees,
            (PositionIntent::Angles { tilt_degrees, .. }, ProgrammingComponent::Tilt) => {
                tilt_degrees
            }
            (PositionIntent::Target { offset_metres, .. }, ProgrammingComponent::TargetX) => {
                &mut offset_metres[0]
            }
            (PositionIntent::Target { offset_metres, .. }, ProgrammingComponent::TargetY) => {
                &mut offset_metres[1]
            }
            (PositionIntent::Target { offset_metres, .. }, ProgrammingComponent::TargetZ) => {
                &mut offset_metres[2]
            }
            _ => {
                return Err(IntentError(
                    "target offset edit requires an explicit target reference".into(),
                ));
            }
        };
        *current = operation.apply(current, ScalarDomain::Finite)?;
    }
    value.validate()?;
    if matches!(base, PositionIntent::Target { .. })
        && !edits
            .iter()
            .any(|edit| matches!(edit, ComponentEdit::ActivateAngles))
        && let Some(pose) = context.solved_angles
        && value == PositionIntent::angles(pose.pan_degrees, pose.tilt_degrees)
    {
        return Ok(base.clone());
    }
    Ok(value)
}
fn edit_color(
    base: &ColorProgram,
    edits: &[ComponentEdit],
    context: &FamilyEditContext<'_>,
) -> Result<ColorProgram, IntentError> {
    if edits
        .iter()
        .any(|edit| matches!(edit, ComponentEdit::Native { .. }))
    {
        return edit_native(base, edits, context);
    }
    let mut intent = match base {
        ColorProgram::Semantic { intent } => intent.clone(),
        ColorProgram::Direct { .. } => {
            context.semantic_color_adoption.cloned().ok_or_else(|| {
                IntentError("Semantic takeover requires an explicit complete Color adoption".into())
            })?
        }
    };
    // Hue and saturation form one coordinate edit. Applying saturation first preserves the
    // requested hue when the seed is achromatic, independently of request field order.
    let is_hue = |edit: &&ComponentEdit| {
        matches!(
            edit,
            ComponentEdit::Scalar {
                component: ProgrammingComponent::Color(ColorComponent::Hue),
                ..
            }
        )
    };
    for edit in edits
        .iter()
        .filter(|edit| !is_hue(edit))
        .chain(edits.iter().filter(is_hue))
    {
        match edit {
            ComponentEdit::Coordinates { xyz } => {
                color_model(context)?.set_coordinates(&mut intent, *xyz)?;
                intent.spreads.retain(|spread| {
                    !matches!(
                        ProgrammingComponent::Color(spread.component)
                            .descriptor()
                            .role,
                        ComponentRole::ColorRecipe | ComponentRole::ColorCoordinate
                    )
                });
            }
            ComponentEdit::Scalar {
                component: ProgrammingComponent::Color(component),
                operation,
            } => {
                let descriptor = ProgrammingComponent::Color(*component).descriptor();
                let scalar = intent
                    .spreads
                    .iter()
                    .find(|spread| spread.component == *component)
                    .map(|spread| ScalarIntent::Spread(spread.points.clone()))
                    .unwrap_or(ScalarIntent::Value(read_color_component(
                        &intent, *component, context,
                    )?));
                let changed = operation.apply(&scalar, descriptor.domain.unwrap())?;
                let first = match &changed {
                    ScalarIntent::Value(value) => *value,
                    ScalarIntent::Spread(points) => points[0],
                };
                write_color_component(&mut intent, *component, first, context)?;
                intent.spreads.retain(|spread| {
                    let role = ProgrammingComponent::Color(spread.component)
                        .descriptor()
                        .role;
                    spread.component != *component
                        && !matches!(
                            (descriptor.role, role),
                            (ComponentRole::ColorRecipe, ComponentRole::ColorCoordinate)
                                | (ComponentRole::ColorCoordinate, ComponentRole::ColorRecipe)
                        )
                });
                if let ScalarIntent::Spread(points) = changed {
                    intent.spreads.push(ColorComponentSpread {
                        component: *component,
                        points,
                    });
                }
            }
            _ => unreachable!(),
        }
    }
    intent.validate()?;
    Ok(ColorProgram::Semantic { intent })
}
fn color_model<'a>(
    context: &'a FamilyEditContext<'_>,
) -> Result<&'a dyn ColorAuthoringModel, IntentError> {
    context.color_model.ok_or_else(|| {
        IntentError("base Color edits require the pinned virtual authoring model".into())
    })
}
pub fn read_color_component(
    intent: &ColorIntent,
    component: ColorComponent,
    context: &FamilyEditContext<'_>,
) -> Result<f32, IntentError> {
    Ok(match component {
        ColorComponent::Red => intent.recipe.rgb[0],
        ColorComponent::Green => intent.recipe.rgb[1],
        ColorComponent::Blue => intent.recipe.rgb[2],
        ColorComponent::Amber => intent.recipe.amber,
        ColorComponent::WhiteBlend => intent.white_blend,
        ColorComponent::Temperature => intent.white_target.kelvin,
        ColorComponent::Duv => intent.white_target.duv,
        ColorComponent::Uv => intent.uv.amount,
        ColorComponent::RelativeOutput => intent.relative_output,
        _ => return color_model(context)?.read_base_component(intent, component),
    })
}
fn write_color_component(
    intent: &mut ColorIntent,
    component: ColorComponent,
    value: f32,
    context: &FamilyEditContext<'_>,
) -> Result<(), IntentError> {
    match component {
        ColorComponent::WhiteBlend => intent.white_blend = value,
        ColorComponent::Temperature => intent.white_target.kelvin = value,
        ColorComponent::Duv => intent.white_target.duv = value,
        ColorComponent::Uv => intent.uv.amount = value,
        ColorComponent::RelativeOutput => intent.relative_output = value,
        _ => color_model(context)?.set_base_component(intent, component, value)?,
    }
    Ok(())
}
fn edit_native(
    base: &ColorProgram,
    edits: &[ComponentEdit],
    context: &FamilyEditContext<'_>,
) -> Result<ColorProgram, IntentError> {
    let ColorProgram::Direct { recipe, .. } = base else {
        return Err(IntentError(
            "native edit requires a complete Direct adoption".into(),
        ));
    };
    let model = context
        .native_model
        .ok_or_else(|| IntentError("native edit requires its verified source model".into()))?;
    require(
        model.source() == &recipe.source,
        "native edit source identity changed",
    )?;
    let mut recipe = recipe.clone();
    for edit in edits {
        let ComponentEdit::Native { binding, operation } = edit else {
            unreachable!()
        };
        let descriptor = model.descriptor(*binding).ok_or_else(|| {
            IntentError("native function is not owned by the verified source".into())
        })?;
        require(
            descriptor.binding == *binding,
            "native descriptor must match the requested binding",
        )?;
        let channel = recipe
            .channels
            .iter_mut()
            .find(|value| value.channel_id == binding.channel_id)
            .ok_or_else(|| {
                IntentError("native channel is absent from the complete recipe".into())
            })?;
        require(
            channel.function_id == binding.function_id,
            "native function changes require a complete recipe adoption",
        )?;
        let min = descriptor.raw_from.min(descriptor.raw_to);
        let max = descriptor.raw_from.max(descriptor.raw_to);
        let existing_spread = recipe
            .spreads
            .iter()
            .find(|spread| spread.binding == *binding);
        let points = match operation {
            NativeColorEdit::Set(value) => {
                require(
                    (min..=max).contains(value),
                    "native value is outside its function",
                )?;
                vec![*value]
            }
            NativeColorEdit::Spread(points) => points.clone(),
            NativeColorEdit::Relative(delta) => {
                require(
                    descriptor.continuous,
                    "native relative edit requires a continuous function",
                )?;
                let shift = |value: u32| {
                    (i128::from(value) + i128::from(*delta)).clamp(i128::from(min), i128::from(max))
                        as u32
                };
                existing_spread.map_or_else(
                    || vec![shift(channel.raw)],
                    |spread| spread.points.iter().copied().map(shift).collect(),
                )
            }
        };
        let spreading = matches!(operation, NativeColorEdit::Spread(_)) || points.len() > 1;
        if spreading {
            NativeColorSpread {
                binding: *binding,
                points: points.clone(),
            }
            .validate(descriptor)?;
        }
        channel.raw = points[0];
        recipe
            .spreads
            .retain(|spread| spread.binding.channel_id != binding.channel_id);
        if spreading {
            recipe.spreads.push(NativeColorSpread {
                binding: *binding,
                points,
            });
        }
    }
    recipe.validate()?;
    for spread in &recipe.spreads {
        let descriptor = model.descriptor(spread.binding).ok_or_else(|| {
            IntentError("native spread function is not owned by the verified source".into())
        })?;
        spread.validate(descriptor)?;
    }
    // Authoring keeps all curves, but its portable reference predicts only the reference
    // channels. The per-member sampler later resolves and predicts each curve independently.
    let spreads = std::mem::take(&mut recipe.spreads);
    let portable = model.predict(&recipe)?;
    recipe.spreads = spreads;
    let result = ColorProgram::Direct { recipe, portable };
    result.validate()?;
    Ok(result)
}
