use super::super::{
    ProgrammingPreloadValueMutation, ProgrammingValueMutation, ProgrammingValuesEnvironment,
};
use crate::{ActionError, ActionErrorKind};
use light_core::{AttributeKey, AttributeValue, FixtureId};
use std::collections::HashSet;

const MUTATION_LIMIT: usize = 10_000;
const IDENTIFIER_LIMIT: usize = 256;
const JAVASCRIPT_MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Eq, Hash, PartialEq)]
enum ValueAddress {
    Fixture(FixtureId, AttributeKey),
    Group(String, AttributeKey),
}

pub(super) fn validate_value_mutations(
    mutations: &[ProgrammingValueMutation],
    environment: &ProgrammingValuesEnvironment,
    check_new_curve_capacity: bool,
) -> Result<(), ActionError> {
    if mutations.len() > MUTATION_LIMIT {
        return Err(invalid(
            "a Programmer values batch must not exceed 10000 mutations",
        ));
    }
    let mut addresses = HashSet::with_capacity(mutations.len());
    for mutation in mutations {
        validate_mutation(mutation, environment, check_new_curve_capacity)?;
        if !addresses.insert(address(mutation)) {
            return Err(invalid(
                "a Programmer values batch must address each fixture or Group attribute once",
            ));
        }
    }
    light_core::programming::validate_targeted_programming_entries(
        light_core::programming::ProgrammingValueScope::Fixture,
        mutations.iter().filter_map(|mutation| match mutation {
            ProgrammingValueMutation::SetFixture {
                fixture_id,
                attribute,
                value,
                ..
            } => Some((*fixture_id, attribute, value)),
            _ => None,
        }),
    )
    .map_err(|error| invalid(error.to_string()))?;
    light_core::programming::validate_targeted_programming_entries(
        light_core::programming::ProgrammingValueScope::LiveGroup,
        mutations.iter().filter_map(|mutation| match mutation {
            ProgrammingValueMutation::SetGroup {
                group_id,
                attribute,
                value,
                ..
            } => Some((group_id, attribute, value)),
            _ => None,
        }),
    )
    .map_err(|error| invalid(error.to_string()))?;
    Ok(())
}

pub(super) fn validate_preload_value_mutations(
    mutations: &[ProgrammingPreloadValueMutation],
    environment: &ProgrammingValuesEnvironment,
    check_new_curve_capacity: bool,
) -> Result<(), ActionError> {
    if mutations.len() > MUTATION_LIMIT {
        return Err(invalid(
            "a Preload values batch must not exceed 10000 mutations",
        ));
    }
    let mut addresses = HashSet::with_capacity(mutations.len());
    for mutation in mutations {
        validate_preload_mutation(mutation, environment, check_new_curve_capacity)?;
        if !addresses.insert(preload_address(mutation)) {
            return Err(invalid(
                "a Preload values batch must address each fixture or Group attribute once",
            ));
        }
    }
    light_core::programming::validate_targeted_programming_entries(
        light_core::programming::ProgrammingValueScope::Fixture,
        mutations.iter().filter_map(|mutation| match mutation {
            ProgrammingPreloadValueMutation::SetFixture {
                fixture_id,
                attribute,
                value,
                ..
            } => Some((*fixture_id, attribute, value)),
            _ => None,
        }),
    )
    .map_err(|error| invalid(error.to_string()))?;
    light_core::programming::validate_targeted_programming_entries(
        light_core::programming::ProgrammingValueScope::LiveGroup,
        mutations.iter().filter_map(|mutation| match mutation {
            ProgrammingPreloadValueMutation::SetGroup {
                group_id,
                attribute,
                value,
                ..
            } => Some((group_id, attribute, value)),
            _ => None,
        }),
    )
    .map_err(|error| invalid(error.to_string()))?;
    Ok(())
}

pub(super) fn validate_request_id(request_id: &str) -> Result<(), ActionError> {
    if request_id.trim().is_empty()
        || request_id.len() > 128
        || request_id.chars().any(char::is_control)
    {
        return Err(invalid("request_id must contain 1-128 printable bytes"));
    }
    Ok(())
}

fn validate_mutation(
    mutation: &ProgrammingValueMutation,
    environment: &ProgrammingValuesEnvironment,
    check_new_curve_capacity: bool,
) -> Result<(), ActionError> {
    match mutation {
        ProgrammingValueMutation::SetFixture {
            fixture_id,
            attribute,
            value,
            timing,
        } => {
            validate_fixture(*fixture_id, environment)?;
            validate_identifier(&attribute.0, "attribute")?;
            validate_timing(*timing)?;
            validate_runtime_contract(value, environment)?;
            super::values_legacy::refuse_legacy_value(
                attribute,
                value,
                environment.supported_programming_contract,
            )?;
            validate_owner(attribute, value)?;
            validate_fixture_value(value)
        }
        ProgrammingValueMutation::ReleaseFixture {
            fixture_id,
            attribute,
        } => {
            validate_fixture(*fixture_id, environment)?;
            validate_identifier(&attribute.0, "attribute")
        }
        ProgrammingValueMutation::SetGroup {
            group_id,
            attribute,
            value,
            timing,
        } => {
            validate_group(group_id, environment)?;
            validate_identifier(&attribute.0, "attribute")?;
            validate_timing(*timing)?;
            validate_runtime_contract(value, environment)?;
            super::values_legacy::refuse_legacy_value(
                attribute,
                value,
                environment.supported_programming_contract,
            )?;
            validate_owner(attribute, value)?;
            validate_group_value(group_id, value, environment, check_new_curve_capacity)
        }
        ProgrammingValueMutation::ReleaseGroup {
            group_id,
            attribute,
        } => {
            validate_group(group_id, environment)?;
            validate_identifier(&attribute.0, "attribute")
        }
    }
}

fn validate_preload_mutation(
    mutation: &ProgrammingPreloadValueMutation,
    environment: &ProgrammingValuesEnvironment,
    check_new_curve_capacity: bool,
) -> Result<(), ActionError> {
    match mutation {
        ProgrammingPreloadValueMutation::SetFixture {
            fixture_id,
            attribute,
            value,
            timing,
        } => {
            validate_fixture(*fixture_id, environment)?;
            validate_identifier(&attribute.0, "attribute")?;
            validate_preload_timing(*timing)?;
            validate_runtime_contract(value, environment)?;
            super::values_legacy::refuse_legacy_value(
                attribute,
                value,
                environment.supported_programming_contract,
            )?;
            validate_owner(attribute, value)?;
            validate_fixture_value(value)
        }
        ProgrammingPreloadValueMutation::ReleaseFixture {
            fixture_id,
            attribute,
        } => {
            validate_fixture(*fixture_id, environment)?;
            validate_identifier(&attribute.0, "attribute")
        }
        ProgrammingPreloadValueMutation::SetGroup {
            group_id,
            attribute,
            value,
            timing,
        } => {
            validate_group(group_id, environment)?;
            validate_identifier(&attribute.0, "attribute")?;
            validate_preload_timing(*timing)?;
            validate_runtime_contract(value, environment)?;
            super::values_legacy::refuse_legacy_value(
                attribute,
                value,
                environment.supported_programming_contract,
            )?;
            validate_owner(attribute, value)?;
            validate_group_value(group_id, value, environment, check_new_curve_capacity)
        }
        ProgrammingPreloadValueMutation::ReleaseGroup {
            group_id,
            attribute,
        } => {
            validate_group(group_id, environment)?;
            validate_identifier(&attribute.0, "attribute")
        }
    }
}

fn validate_runtime_contract(
    value: &AttributeValue,
    environment: &ProgrammingValuesEnvironment,
) -> Result<(), ActionError> {
    let required = value.required_programming_contract();
    if required > environment.supported_programming_contract {
        return Err(invalid(format!(
            "value requires programming contract {required}; this runtime supports {}",
            environment.supported_programming_contract
        )));
    }
    Ok(())
}

fn validate_fixture(
    fixture_id: FixtureId,
    environment: &ProgrammingValuesEnvironment,
) -> Result<(), ActionError> {
    environment
        .fixture_ids
        .contains(&fixture_id)
        .then_some(())
        .ok_or_else(|| invalid("fixture does not exist"))
}

fn validate_group(
    group_id: &str,
    environment: &ProgrammingValuesEnvironment,
) -> Result<(), ActionError> {
    validate_identifier(group_id, "group_id")?;
    environment
        .group_memberships
        .contains_key(group_id)
        .then_some(())
        .ok_or_else(|| invalid("Group does not exist"))
}

pub(super) fn validate_identifier(value: &str, field: &str) -> Result<(), ActionError> {
    if value.trim().is_empty()
        || value.len() > IDENTIFIER_LIMIT
        || value.chars().any(char::is_control)
    {
        Err(invalid(format!(
            "{field} must contain 1-256 printable bytes"
        )))
    } else {
        Ok(())
    }
}

pub(super) fn validate_timing(
    timing: super::super::ProgrammingValueTiming,
) -> Result<(), ActionError> {
    validate_durations(timing.fade_millis, timing.delay_millis)
}

fn validate_preload_timing(
    timing: super::super::ProgrammingPreloadValueTiming,
) -> Result<(), ActionError> {
    validate_durations(timing.fade_millis, timing.delay_millis)
}

fn validate_durations(
    fade_millis: Option<u64>,
    delay_millis: Option<u64>,
) -> Result<(), ActionError> {
    for duration in [fade_millis, delay_millis].into_iter().flatten() {
        if duration > JAVASCRIPT_MAX_SAFE_INTEGER {
            return Err(invalid(
                "Programmer value timing exceeds the safe integer limit",
            ));
        }
    }
    Ok(())
}

fn validate_fixture_value(value: &AttributeValue) -> Result<(), ActionError> {
    if matches!(
        value,
        AttributeValue::Spread(_) | AttributeValue::GroupFamily(_)
    ) || value.spread_control_points() > 0
    {
        return Err(invalid("spread values require a Group Programmer address"));
    }
    validate_value(value)
}

// @tour value-spreading:20 Reject invalid curves before mutation
// Group spreads require normalized control points and reject multi-point curves that cannot fit
// the resolved membership, leaving the action atomic.
fn validate_group_value(
    group_id: &str,
    value: &AttributeValue,
    environment: &ProgrammingValuesEnvironment,
    check_new_curve_capacity: bool,
) -> Result<(), ActionError> {
    if let AttributeValue::Spread(values) = value {
        if values.len() < 2 || values.iter().any(|value| !unit_value(*value)) {
            return Err(invalid("spread requires at least two values within 0-1"));
        }
    }
    let control_points = value.spread_control_points();
    if check_new_curve_capacity && control_points > 2 {
        let ranks = environment
            .group_rank_counts
            .get(group_id)
            .or_else(|| environment.group_memberships.get(group_id))
            .copied()
            .unwrap_or(0);
        if control_points > ranks {
            return Err(invalid(format!(
                "spread has {} control points but the Group has only {ranks} ranks",
                control_points
            )));
        }
    }
    validate_value(value)
}

fn validate_owner(attribute: &AttributeKey, value: &AttributeValue) -> Result<(), ActionError> {
    value
        .validate_programming_address(attribute)
        .map_err(|error| invalid(error.to_string()))
}

pub(super) fn validate_value(value: &AttributeValue) -> Result<(), ActionError> {
    match value {
        AttributeValue::Normalized(value) if !unit_value(*value) => {
            Err(invalid("normalized value must be within 0-1"))
        }
        AttributeValue::Spread(_) | AttributeValue::Normalized(_) => Ok(()),
        AttributeValue::Discrete(value) => validate_identifier(value, "discrete value"),
        AttributeValue::ColorXyz(value)
            if !value.x.is_finite()
                || !value.y.is_finite()
                || !value.z.is_finite()
                || value.x < 0.0
                || value.y < 0.0
                || value.z < 0.0 =>
        {
            Err(invalid(
                "XYZ color components must be finite and non-negative",
            ))
        }
        AttributeValue::ColorProgram(value) => {
            value.validate().map_err(|error| invalid(error.to_string()))
        }
        AttributeValue::Position(value) => {
            value.validate().map_err(|error| invalid(error.to_string()))
        }
        AttributeValue::Zoom(value) => value.validate().map_err(|error| invalid(error.to_string())),
        AttributeValue::GroupFamily(value) => {
            value.validate().map_err(|error| invalid(error.to_string()))
        }
        AttributeValue::ColorXyz(_)
        | AttributeValue::RawDmx(_)
        | AttributeValue::RawDmxExact(_) => Ok(()),
    }
}

fn address(mutation: &ProgrammingValueMutation) -> ValueAddress {
    match mutation {
        ProgrammingValueMutation::SetFixture {
            fixture_id,
            attribute,
            ..
        }
        | ProgrammingValueMutation::ReleaseFixture {
            fixture_id,
            attribute,
        } => ValueAddress::Fixture(*fixture_id, attribute.clone()),
        ProgrammingValueMutation::SetGroup {
            group_id,
            attribute,
            ..
        }
        | ProgrammingValueMutation::ReleaseGroup {
            group_id,
            attribute,
        } => ValueAddress::Group(group_id.clone(), attribute.clone()),
    }
}

fn preload_address(mutation: &ProgrammingPreloadValueMutation) -> ValueAddress {
    match mutation {
        ProgrammingPreloadValueMutation::SetFixture {
            fixture_id,
            attribute,
            ..
        }
        | ProgrammingPreloadValueMutation::ReleaseFixture {
            fixture_id,
            attribute,
        } => ValueAddress::Fixture(*fixture_id, attribute.clone()),
        ProgrammingPreloadValueMutation::SetGroup {
            group_id,
            attribute,
            ..
        }
        | ProgrammingPreloadValueMutation::ReleaseGroup {
            group_id,
            attribute,
        } => ValueAddress::Group(group_id.clone(), attribute.clone()),
    }
}

fn unit_value(value: f32) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

fn invalid(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use light_core::programming::{PositionIntent, ProgrammingOwner, ScalarIntent};
    use std::sync::Arc;

    #[test]
    fn production_contract_gates_actual_normal_and_preload_writes_but_allows_empty_actions() {
        let fixture_id = FixtureId::new();
        let mut environment = ProgrammingValuesEnvironment {
            supported_programming_contract: 0,
            ..Default::default()
        };
        environment.fixture_ids.insert(fixture_id);
        environment.group_memberships.insert("empty".into(), 0);
        let value = AttributeValue::Position(Arc::new(PositionIntent::angles(720.0, 0.0)));
        for grouped in [false, true] {
            let normal = if grouped {
                ProgrammingValueMutation::SetGroup {
                    group_id: "empty".into(),
                    attribute: ProgrammingOwner::Position.key(),
                    value: value.clone(),
                    timing: Default::default(),
                }
            } else {
                ProgrammingValueMutation::SetFixture {
                    fixture_id,
                    attribute: ProgrammingOwner::Position.key(),
                    value: value.clone(),
                    timing: Default::default(),
                }
            };
            let preload = if grouped {
                ProgrammingPreloadValueMutation::SetGroup {
                    group_id: "empty".into(),
                    attribute: ProgrammingOwner::Position.key(),
                    value: value.clone(),
                    timing: Default::default(),
                }
            } else {
                ProgrammingPreloadValueMutation::SetFixture {
                    fixture_id,
                    attribute: ProgrammingOwner::Position.key(),
                    value: value.clone(),
                    timing: Default::default(),
                }
            };
            assert!(validate_value_mutations(&[normal], &environment, true).is_err());
            assert!(validate_preload_value_mutations(&[preload], &environment, true).is_err());
        }
        assert!(validate_value_mutations(&[], &environment, true).is_ok());
        assert!(validate_preload_value_mutations(&[], &environment, true).is_ok());
        assert!(
            validate_runtime_contract(
                &AttributeValue::ColorXyz(light_core::color_intent::D65_WHITE),
                &environment
            )
            .is_ok()
        );
        assert!(validate_runtime_contract(&AttributeValue::Normalized(0.5), &environment).is_ok());
    }
    #[test]
    fn nested_spreads_obey_rank_capacity_and_complete_owner_addresses() {
        let value = AttributeValue::Position(Arc::new(PositionIntent::Angles {
            pan_degrees: ScalarIntent::Spread(vec![-720.0, 0.0, 720.0]),
            tilt_degrees: ScalarIntent::Value(90.0),
        }));
        let mut environment = ProgrammingValuesEnvironment::default();
        environment.group_memberships.insert("1".into(), 2);
        assert!(validate_fixture_value(&value).is_err());
        assert!(validate_group_value("1", &value, &environment, true).is_err());
        environment.group_memberships.insert("1".into(), 3);
        assert!(validate_group_value("1", &value, &environment, true).is_ok());
        assert!(validate_owner(&ProgrammingOwner::Position.key(), &value).is_ok());
        assert!(validate_owner(AttributeKey::color_ref(), &value).is_err());
        let normal = ProgrammingValueMutation::SetGroup {
            group_id: "1".into(),
            attribute: AttributeKey::color(),
            value: value.clone(),
            timing: Default::default(),
        };
        assert!(validate_value_mutations(&[normal], &environment, true).is_err());
        let preload = ProgrammingPreloadValueMutation::SetGroup {
            group_id: "1".into(),
            attribute: AttributeKey::color(),
            value,
            timing: Default::default(),
        };
        assert!(validate_preload_value_mutations(&[preload], &environment, true).is_err());
    }
}
