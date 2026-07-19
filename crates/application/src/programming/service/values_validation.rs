use super::super::{ProgrammingValueMutation, ProgrammingValuesEnvironment};
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
) -> Result<(), ActionError> {
    if mutations.len() > MUTATION_LIMIT {
        return Err(invalid(
            "a Programmer values batch must not exceed 10000 mutations",
        ));
    }
    let mut addresses = HashSet::with_capacity(mutations.len());
    for mutation in mutations {
        validate_mutation(mutation, environment)?;
        if !addresses.insert(address(mutation)) {
            return Err(invalid(
                "a Programmer values batch must address each fixture or Group attribute once",
            ));
        }
    }
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
            validate_group_value(value)
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
        .group_ids
        .contains(group_id)
        .then_some(())
        .ok_or_else(|| invalid("Group does not exist"))
}

fn validate_identifier(value: &str, field: &str) -> Result<(), ActionError> {
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

fn validate_timing(timing: super::super::ProgrammingValueTiming) -> Result<(), ActionError> {
    for duration in [timing.fade_millis, timing.delay_millis]
        .into_iter()
        .flatten()
    {
        if duration > JAVASCRIPT_MAX_SAFE_INTEGER {
            return Err(invalid(
                "Programmer value timing exceeds the safe integer limit",
            ));
        }
    }
    Ok(())
}

fn validate_fixture_value(value: &AttributeValue) -> Result<(), ActionError> {
    if matches!(value, AttributeValue::Spread(_)) {
        return Err(invalid("spread values require a Group Programmer address"));
    }
    validate_value(value)
}

fn validate_group_value(value: &AttributeValue) -> Result<(), ActionError> {
    if let AttributeValue::Spread(values) = value
        && (values.len() < 2 || values.iter().any(|value| !unit_value(*value)))
    {
        return Err(invalid("spread requires at least two values within 0-1"));
    }
    validate_value(value)
}

fn validate_value(value: &AttributeValue) -> Result<(), ActionError> {
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

fn unit_value(value: f32) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

fn invalid(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message)
}
