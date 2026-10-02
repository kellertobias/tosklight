//! TL-552 follow-up: Dynamic-programmer writes (FixAT, Release) never store a legacy family
//! address at programming contract ≥ 1, so a stored Programmer or a recorded Cue can never make
//! the next open fail the load-time validator.
use super::*;

/// The semantic owner whose Release means the same as releasing this native family channel:
/// `pan`/`tilt` → `position`, a Color component → `color`. Other addresses are unchanged.
fn semantic_release_attribute(attribute: &AttributeKey) -> AttributeKey {
    use light_core::programming::ProgrammingOwner;
    match light_show::legacy_programming_address(&attribute.0, Some("release")) {
        Some(light_show::LegacyProgrammingFamily::Position) => ProgrammingOwner::Position.key(),
        Some(light_show::LegacyProgrammingFamily::Color) => ProgrammingOwner::Color.key(),
        _ => attribute.clone(),
    }
}

/// Releases at contract ≥ 1 address the semantic owner instead of its native channels, keeping
/// the first occurrence of each target address in order.
pub(super) fn semantic_release_command(
    command: &DynamicReleaseCommand,
    supported: u16,
) -> DynamicReleaseCommand {
    if supported == 0 {
        return command.clone();
    }
    let mut fixtures = std::collections::HashSet::new();
    let mut groups = std::collections::HashSet::new();
    DynamicReleaseCommand {
        fixture_values: command
            .fixture_values
            .iter()
            .map(|value| ReleaseProgrammerFixtureValue {
                fixture_id: value.fixture_id,
                attribute: semantic_release_attribute(&value.attribute),
            })
            .filter(|value| fixtures.insert((value.fixture_id, value.attribute.clone())))
            .collect(),
        group_values: command
            .group_values
            .iter()
            .map(|value| ReleaseProgrammerGroupValue {
                group_id: value.group_id.clone(),
                attribute: semantic_release_attribute(&value.attribute),
            })
            .filter(|value| groups.insert((value.group_id.clone(), value.attribute.clone())))
            .collect(),
    }
}

/// Refuses a Dynamic-programmer value the load-time validator would reject (for example a FixAT
/// percentage at `pan` or `zoom`), before anything changes.
pub(super) fn refuse_legacy_dynamic_values(
    mutations: &[DynamicProgrammerValueMutation],
    supported: u16,
) -> Result<(), ActionError> {
    if supported == 0 {
        return Ok(());
    }
    for mutation in mutations {
        let DynamicProgrammerValueMutation::Set {
            attribute, value, ..
        } = mutation
        else {
            continue;
        };
        if light_show::legacy_programming_family(&attribute.0).is_none() {
            continue;
        }
        let row = serde_json::json!({"attribute": attribute.0, "value": value});
        if let Some(found) = light_show::legacy_programming_attributes(&row)
            .into_iter()
            .next()
        {
            let family = light_show::legacy_programming_family(&found)
                .expect("the validator only reports legacy family addresses");
            return Err(ActionError::new(
                ActionErrorKind::Invalid,
                light_show::legacy_live_write_message(&found, family, supported),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn releases_address_semantic_owners_and_percentage_fix_at_is_refused_at_contract_one() {
        let fixture = FixtureId::new();
        let release = |attribute: &str| ReleaseProgrammerFixtureValue {
            fixture_id: fixture,
            attribute: AttributeKey(attribute.into()),
        };
        let command = DynamicReleaseCommand {
            fixture_values: [
                "pan",
                "tilt",
                "color.red",
                "color.wheel.1",
                "zoom",
                "intensity",
            ]
            .map(release)
            .to_vec(),
            group_values: vec![ReleaseProgrammerGroupValue {
                group_id: "1".into(),
                attribute: AttributeKey("color.blue".into()),
            }],
        };
        let semantic = semantic_release_command(&command, 1);
        assert_eq!(
            semantic
                .fixture_values
                .iter()
                .map(|value| &*value.attribute.0)
                .collect::<Vec<_>>(),
            ["position", "color", "color.wheel.1", "zoom", "intensity"]
        );
        assert_eq!(&*semantic.group_values[0].attribute.0, "color");
        assert_eq!(semantic_release_command(&command, 0), command);

        let fix_at = |attribute: &str| DynamicProgrammerValueMutation::Set {
            fixture_id: fixture,
            attribute: AttributeKey(attribute.into()),
            value: DynamicSemanticValue::FixAt {
                value: 0.5,
                timing: Default::default(),
            },
        };
        for attribute in ["pan", "color.red", "zoom"] {
            let error = refuse_legacy_dynamic_values(&[fix_at(attribute)], 1).unwrap_err();
            assert_eq!(error.kind, ActionErrorKind::Invalid);
            assert!(error.message.contains(attribute), "{}", error.message);
        }
        assert!(refuse_legacy_dynamic_values(&[fix_at("intensity"), fix_at("focus")], 1).is_ok());
        assert!(refuse_legacy_dynamic_values(&[fix_at("pan")], 0).is_ok());
        let release = DynamicProgrammerValueMutation::Set {
            fixture_id: fixture,
            attribute: AttributeKey("zoom".into()),
            value: DynamicSemanticValue::Release,
        };
        assert!(refuse_legacy_dynamic_values(&[release], 1).is_ok());
    }
}
