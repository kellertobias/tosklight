use super::{DynamicValue, DynamicValueAddress, address::ensure};
use light_core::{AttributeValue, programming::*};

/// Extract from an already materialized Preset/Current owner, retaining declared units
/// and exact native integers. None means incompatible representation/function; never
/// invent an Angle from a Target, a native layout, or a portable color conversion here.
/// Current callers must perform any explicit adoption in their coherent frame first.
pub fn extract_compatible_dynamic_value(
    family: &AttributeValue,
    address: &DynamicValueAddress,
    context: &FamilyEditContext<'_>,
) -> Result<Option<DynamicValue>, IntentError> {
    address.validate()?;
    family.validate_programming_address(address.owner().key_ref())?;
    ensure(
        family.spread_control_points() == 0 && !matches!(family, AttributeValue::GroupFamily(_)),
        "Dynamic extraction requires target materialization",
    )?;
    if !address.matches_representation(family) {
        return Ok(None);
    }
    let Some(component) = address.component else {
        return Ok(Some(DynamicValue::Family(family.clone())));
    };
    let scalar = |value: &ScalarIntent| -> Result<f32, IntentError> {
        match value {
            ScalarIntent::Value(value) => Ok(*value),
            _ => Err(IntentError(
                "Dynamic extraction requires a materialized scalar".into(),
            )),
        }
    };
    let value = match (component, family) {
        (
            ProgrammingComponent::Pan | ProgrammingComponent::Tilt,
            AttributeValue::Position(position),
        ) => {
            let PositionIntent::Angles {
                pan_degrees,
                tilt_degrees,
            } = position.as_ref()
            else {
                unreachable!("matched representation")
            };
            DynamicValue::Scalar(scalar(if component == ProgrammingComponent::Pan {
                pan_degrees
            } else {
                tilt_degrees
            })?)
        }
        (
            ProgrammingComponent::TargetX
            | ProgrammingComponent::TargetY
            | ProgrammingComponent::TargetZ,
            AttributeValue::Position(position),
        ) => {
            let PositionIntent::Target { offset_metres, .. } = position.as_ref() else {
                unreachable!("matched representation")
            };
            let index = match component {
                ProgrammingComponent::TargetX => 0,
                ProgrammingComponent::TargetY => 1,
                _ => 2,
            };
            DynamicValue::Scalar(scalar(&offset_metres[index])?)
        }
        (ProgrammingComponent::Color(component), AttributeValue::ColorProgram(color)) => {
            let ColorProgram::Semantic { intent } = color.as_ref() else {
                unreachable!("matched representation")
            };
            DynamicValue::Scalar(read_color_component(intent, component, context)?)
        }
        (ProgrammingComponent::NativeColor(binding), AttributeValue::ColorProgram(color)) => {
            let ColorProgram::Direct { recipe, .. } = color.as_ref() else {
                unreachable!("matched representation")
            };
            let Some(channel) = recipe.channels.iter().find(|channel| {
                channel.channel_id == binding.channel_id
                    && channel.function_id == binding.function_id
            }) else {
                return Ok(None);
            };
            DynamicValue::Native(channel.raw)
        }
        (ProgrammingComponent::Focus, AttributeValue::Normalized(value)) => {
            DynamicValue::Scalar(*value)
        }
        (ProgrammingComponent::Zoom, AttributeValue::Zoom(value)) => {
            DynamicValue::Scalar(scalar(&value.opening_degrees)?)
        }
        _ => {
            return Err(IntentError(
                "Dynamic extraction has incompatible component and owner".into(),
            ));
        }
    };
    address.validate_value_shape(&value)?;
    Ok(Some(value))
}
