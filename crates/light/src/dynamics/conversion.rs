use crate::{ActionError, ActionErrorKind};
use light_dynamics::DynamicRuntimeError;

pub(super) fn factor_rational(value: f32) -> Result<light_dynamics::Rational, ActionError> {
    if !value.is_finite() || value <= 0.0 {
        return Err(ActionError::new(
            ActionErrorKind::Invalid,
            "Dynamic speed multiplier must be positive",
        ));
    }
    let denominator = 1_000_u32;
    let numerator = (value * denominator as f32)
        .round()
        .clamp(1.0, u32::MAX as f32) as u32;
    let divisor = greatest_common_divisor(numerator, denominator);
    Ok(light_dynamics::Rational {
        numerator: numerator / divisor,
        denominator: denominator / divisor,
    })
}

const fn greatest_common_divisor(mut left: u32, mut right: u32) -> u32 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    if left == 0 { 1 } else { left }
}

pub(super) fn runtime_error(error: DynamicRuntimeError) -> ActionError {
    let kind = match error {
        DynamicRuntimeError::MissingDefinition
        | DynamicRuntimeError::MissingInstance
        | DynamicRuntimeError::MissingController => ActionErrorKind::NotFound,
        DynamicRuntimeError::EmptyTargets
        | DynamicRuntimeError::InvalidController
        | DynamicRuntimeError::InvalidSpatialMapping(_)
        | DynamicRuntimeError::InvalidDefinition(_)
        | DynamicRuntimeError::InvalidSnapshot(_) => ActionErrorKind::Invalid,
    };
    ActionError::new(kind, error.to_string())
}
