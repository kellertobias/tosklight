//! Coherent adoption of an underlay into a sample's authored representation.
use super::*;

pub(super) fn requirement(address: &DynamicValueAddress) -> TransitionRequirement {
    match address.representation {
        DynamicFamilyRepresentation::Angles => TransitionRequirement::LiveJointAngles,
        DynamicFamilyRepresentation::Target { .. } => TransitionRequirement::LiveTargetPoints,
        DynamicFamilyRepresentation::SemanticColor { .. }
        | DynamicFamilyRepresentation::DirectColor { .. } => TransitionRequirement::ColorAppearance,
        DynamicFamilyRepresentation::Zoom { .. } => TransitionRequirement::ZoomConvention,
        DynamicFamilyRepresentation::Focus => TransitionRequirement::CompatibleOwners,
    }
}

pub(super) fn adopt(
    value: AttributeValue,
    address: &DynamicValueAddress,
    context: &FamilyCompositionContext<'_>,
    original_base: &AttributeValue,
) -> Result<AttributeValue, TransitionError> {
    if address.matches_authored_source(&value) {
        return Ok(value);
    }
    if let Some(resolve) = context.resolve_adoption {
        let adopted = resolve(&value, address)?;
        adopted.validate_programming_address(&address.owner().key())?;
        ensure(
            adopted.spread_control_points() == 0 && address.matches_authored_source(&adopted),
            "frame resolver returned an incompatible or unmaterialized family",
        )?;
        validate_native_function_adoption(&value, &adopted, address)?;
        return Ok(adopted);
    }
    if &value != original_base {
        return Err(TransitionError::Requires(requirement(address)));
    }
    if let Some(adopted) = context.adopted_base {
        adopted.validate_programming_address(&address.owner().key())?;
        ensure(
            adopted.spread_control_points() == 0 && address.matches_authored_source(adopted),
            "coherent Dynamic adoption has a different representation or remains unmaterialized",
        )?;
        validate_native_function_adoption(&value, adopted, address)?;
        return Ok(adopted.clone());
    }
    match &address.representation {
        DynamicFamilyRepresentation::Angles if context.edit.solved_angles.is_some() => {
            let pose = context.edit.solved_angles.expect("checked pose");
            let value = AttributeValue::Position(Arc::new(PositionIntent::angles(
                pose.pan_degrees,
                pose.tilt_degrees,
            )));
            value.validate_programming_address(&address.owner().key())?;
            Ok(value)
        }
        DynamicFamilyRepresentation::SemanticColor { .. }
            if context.edit.semantic_color_adoption.is_some() =>
        {
            let intent = context
                .edit
                .semantic_color_adoption
                .expect("checked Color")
                .clone();
            intent.validate()?;
            ensure(
                intent.spreads.is_empty(),
                "Dynamic Color adoption must be materialized",
            )?;
            Ok(AttributeValue::ColorProgram(Arc::new(
                ColorProgram::Semantic { intent },
            )))
        }
        _ => Err(TransitionError::Requires(requirement(address))),
    }
}

fn validate_native_function_adoption(
    previous: &AttributeValue,
    adopted: &AttributeValue,
    address: &DynamicValueAddress,
) -> Result<(), IntentError> {
    let Some(ProgrammingComponent::NativeColor(binding)) = address.component else {
        return Ok(());
    };
    let (AttributeValue::ColorProgram(previous), AttributeValue::ColorProgram(adopted)) =
        (previous, adopted)
    else {
        return Ok(());
    };
    let (
        ColorProgram::Direct {
            recipe: previous, ..
        },
        ColorProgram::Direct {
            recipe: adopted, ..
        },
    ) = (previous.as_ref(), adopted.as_ref())
    else {
        return Ok(());
    };
    if previous.source == adopted.source {
        ensure(
            previous.channels.len() == adopted.channels.len()
                && previous
                    .channels
                    .iter()
                    .filter(|channel| channel.channel_id != binding.channel_id)
                    .all(|channel| adopted.channels.contains(channel)),
            "native function adoption must preserve the other source channels",
        )?;
    }
    Ok(())
}
