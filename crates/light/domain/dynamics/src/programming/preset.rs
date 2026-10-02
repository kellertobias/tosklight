use super::{DynamicValueAddress, address::ensure};
use light_core::{AttributeValue, FixtureId, programming::*};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Semantic source retained independently of the live Preset. Keep spreads and live
/// Group member exceptions so future instance selections can be materialized correctly.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DynamicPresetTemplate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub universal: Option<AttributeValue>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<DynamicPresetGroupTemplate>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fixtures: Vec<DynamicPresetFixtureTemplate>,
    /// One bounded fallback for scopes still present in the latest authored source.
    /// Never a revision history: fallback templates cannot themselves contain a fallback.
    /// Values remain candidates until the cold compiler verifies their pinned source model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<Box<DynamicPresetTemplate>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DynamicPresetGroupTemplate {
    pub group_id: String,
    pub value: AttributeValue,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DynamicPresetFixtureTemplate {
    pub fixture_id: FixtureId,
    pub value: AttributeValue,
}

impl DynamicPresetTemplate {
    pub fn validate(&self, owner: ProgrammingOwner) -> Result<(), IntentError> {
        if let Some(fallback) = &self.fallback {
            ensure(
                fallback.fallback.is_none(),
                "retained Preset fallback cannot nest",
            )?;
            fallback.validate(owner)?;
        }
        let validate = |value: &AttributeValue, scope| {
            value.validate_programming_scope(scope)?;
            value.validate_programming_address(&owner.key())
        };
        if let Some(value) = &self.universal {
            validate(value, ProgrammingValueScope::Universal)?;
        }
        let mut groups = HashSet::new();
        for group in &self.groups {
            ensure(
                !group.group_id.trim().is_empty()
                    && !group.group_id.chars().any(char::is_control)
                    && groups.insert(&group.group_id),
                "retained Preset Group IDs must be valid and unique",
            )?;
            validate(&group.value, ProgrammingValueScope::LiveGroup)?;
        }
        let mut fixtures = HashSet::new();
        for fixture in &self.fixtures {
            ensure(
                !fixture.fixture_id.0.is_nil() && fixtures.insert(fixture.fixture_id),
                "retained Preset fixture IDs must be valid and unique",
            )?;
            validate(&fixture.value, ProgrammingValueScope::Fixture)?;
        }
        Ok(())
    }

    /// Reconcile authored scopes, not expanded fixture values. Removing a scope removes its
    /// future-member fallback; existing instances retain their own last valid resolved values.
    pub fn retain_fallback(&mut self, address: &DynamicValueAddress, previous: Option<&Self>) {
        self.retain_fallback_verified(address, previous, &|value| {
            // A shape-only caller cannot promote a native recipe to a verified fallback.
            !matches!(value, AttributeValue::ColorProgram(program) if matches!(program.as_ref(), light_core::programming::ColorProgram::Direct { .. }))
        });
    }

    /// The callback checks complete native ownership/function ranges and spread eligibility
    /// against the exact immutable source. Unknown/missing source data must return false.
    pub fn retain_fallback_verified(
        &mut self,
        address: &DynamicValueAddress,
        previous: Option<&Self>,
        verify: &dyn Fn(&AttributeValue) -> bool,
    ) {
        let previous = previous.map(|value| value.fallback.as_deref().unwrap_or(value));
        let prior_universal = previous.and_then(|value| value.universal.as_ref());
        let universal = self
            .universal
            .as_ref()
            .and_then(|value| compatible_value(value, prior_universal, address, verify));
        let groups = self
            .groups
            .iter()
            .filter_map(|group| {
                let prior = previous
                    .and_then(|value| {
                        value
                            .groups
                            .iter()
                            .find(|old| old.group_id == group.group_id)
                    })
                    .map(|group| &group.value);
                compatible_value(&group.value, prior, address, verify).map(|value| {
                    DynamicPresetGroupTemplate {
                        group_id: group.group_id.clone(),
                        value,
                    }
                })
            })
            .collect();
        let fixtures = self
            .fixtures
            .iter()
            .filter_map(|fixture| {
                let prior = previous
                    .and_then(|value| {
                        value
                            .fixtures
                            .iter()
                            .find(|old| old.fixture_id == fixture.fixture_id)
                    })
                    .map(|fixture| &fixture.value);
                compatible_value(&fixture.value, prior, address, verify).map(|value| {
                    DynamicPresetFixtureTemplate {
                        fixture_id: fixture.fixture_id,
                        value,
                    }
                })
            })
            .collect();
        self.fallback = Some(Box::new(Self {
            universal,
            groups,
            fixtures,
            fallback: None,
        }));
    }
}

fn compatible_value(
    value: &AttributeValue,
    previous: Option<&AttributeValue>,
    address: &DynamicValueAddress,
    verify: &dyn Fn(&AttributeValue) -> bool,
) -> Option<AttributeValue> {
    use light_core::programming::GroupFamilyAssignment;
    if let AttributeValue::GroupFamily(group) = value {
        let prior = |fixture| {
            previous.map(|value| match value {
                AttributeValue::GroupFamily(group) => group.for_member(fixture),
                value => value,
            })
        };
        let prior_template = previous.map(|value| match value {
            AttributeValue::GroupFamily(group) => &group.template,
            value => value,
        });
        let template = compatible_value(&group.template, prior_template, address, verify);
        let mut any_compatible = template.is_some();
        let members = group
            .members
            .iter()
            .map(|(id, value)| {
                let retained = compatible_value(value, prior(FixtureId(*id)), address, verify);
                any_compatible |= retained.is_some();
                (*id, retained.unwrap_or_else(|| value.clone()))
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        if !any_compatible {
            return None;
        }
        return Some(AttributeValue::GroupFamily(std::sync::Arc::new(
            GroupFamilyAssignment {
                owner: group.owner,
                template: template.unwrap_or_else(|| group.template.clone()),
                members,
            },
        )));
    }
    if address.matches_authored_source(value) && verify(value) {
        Some(value.clone())
    } else {
        previous
            .filter(|value| match value {
                AttributeValue::GroupFamily(group) => {
                    address.matches_authored_source(&group.template)
                        || group
                            .members
                            .values()
                            .any(|value| address.matches_authored_source(value))
                }
                value => address.matches_authored_source(value),
            })
            .cloned()
    }
}
