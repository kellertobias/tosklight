//! Capture only existing edit-order identities under the shared active-show mutation gate.
use super::*;
use light_core::{AttributeKey, ReplacementProjectionMap, ReplacementRuntimeMigration};
use std::collections::HashMap;

#[derive(Clone, Default)]
pub(super) struct PreparedProgrammingReplacement {
    pub(super) plans: Vec<ReplacementRuntimeMigration>,
    pub(super) programmers: Vec<ProgrammerReplacement>,
}

#[derive(Clone)]
pub(super) struct ProgrammerReplacement {
    pub(super) before: light_programmer::ProgrammerState,
    pub(super) current: Vec<(u64, ReplacementProjectionMap)>,
    pub(super) undo: Vec<Vec<(u64, ReplacementProjectionMap)>>,
    pub(super) redo: Vec<Vec<(u64, ReplacementProjectionMap)>>,
}

pub(super) fn capture(
    state: &AppState,
    plans: &[ReplacementRuntimeMigration],
) -> Result<PreparedProgrammingReplacement, ActionError> {
    let snapshot = state.output.snapshot();
    let groups = snapshot
        .groups
        .iter()
        .map(|group| (group.id.clone(), group.clone()))
        .collect::<HashMap<_, _>>();
    let mut programmers = Vec::new();
    for programmer in state.programming.active() {
        let current = capture_lanes(
            [
                &*programmer.values,
                &programmer.preload_pending,
                &*programmer.preload_active,
            ],
            [
                &*programmer.group_values,
                &programmer.preload_group_pending,
                &*programmer.preload_group_active,
            ],
            &programmer.replacement_provenance,
            &groups,
            plans,
        )?;
        let history = |entries: &[std::sync::Arc<light_programmer::ProgrammerSnapshot>]| {
            entries
                .iter()
                .map(|entry| {
                    capture_lanes(
                        [
                            &*entry.values,
                            &entry.preload_pending,
                            &*entry.preload_active,
                        ],
                        [
                            &*entry.group_values,
                            &entry.preload_group_pending,
                            &*entry.preload_group_active,
                        ],
                        &entry.replacement_provenance,
                        &groups,
                        plans,
                    )
                })
                .collect::<Result<Vec<_>, ActionError>>()
        };
        let undo = history(&programmer.undo)?;
        let redo = history(&programmer.redo)?;
        if !current.is_empty() || undo.iter().chain(&redo).any(|entry| !entry.is_empty()) {
            state
                .programming
                .validate_replacement_migration_with_history(programmer.id, &current, &undo, &redo)
                .map_err(invalid)?;
            programmers.push(ProgrammerReplacement {
                before: programmer,
                current,
                undo,
                redo,
            });
        }
    }
    Ok(PreparedProgrammingReplacement {
        plans: plans.to_vec(),
        programmers,
    })
}

fn capture_lanes(
    values: [&[light_core::TimedValue]; 3],
    group_lanes: [&HashMap<String, HashMap<AttributeKey, light_programmer::GroupProgrammerValue>>;
        3],
    provenance: &HashMap<u64, ReplacementProjectionMap>,
    groups: &HashMap<String, light_programmer::GroupDefinition>,
    plans: &[ReplacementRuntimeMigration],
) -> Result<Vec<(u64, ReplacementProjectionMap)>, ActionError> {
    let mut origins = HashMap::new();
    for value in values.into_iter().flatten() {
        let mut map = provenance
            .get(&value.programmer_order)
            .cloned()
            .unwrap_or_default();
        migrate(&mut map, &value.attribute, &[value.fixture_id], plans)?;
        if !map.is_empty() && provenance.get(&value.programmer_order) != Some(&map) {
            origins.insert(value.programmer_order, map);
        }
    }
    for lane in group_lanes {
        for (group_id, attributes) in lane {
            // Undo can retain programming for a subsequently deleted Group. Absence is
            // dormant here, not an invented empty Group or an inferred membership list.
            // Already-proven envelopes still retarget; present invalid Groups remain errors.
            let members = if groups.contains_key(group_id) {
                light_programmer::resolve_group(group_id, groups).map_err(invalid)?
            } else {
                Vec::new()
            };
            for (attribute, value) in attributes {
                let mut map = provenance
                    .get(&value.programmer_order)
                    .cloned()
                    .unwrap_or_default();
                migrate(&mut map, attribute, &members, plans)?;
                if !map.is_empty() && provenance.get(&value.programmer_order) != Some(&map) {
                    origins.insert(value.programmer_order, map);
                }
            }
        }
    }
    Ok(origins.into_iter().collect())
}

fn migrate(
    map: &mut ReplacementProjectionMap,
    attribute: &AttributeKey,
    members: &[FixtureId],
    plans: &[ReplacementRuntimeMigration],
) -> Result<(), ActionError> {
    for plan in plans {
        if !members.contains(&plan.source_owner) && !map.contains_key(&plan.source_owner) {
            continue;
        }
        if let Some(projection) = plan
            .project_existing(attribute, plan.source_owner, map.get(&plan.source_owner))
            .map_err(invalid)?
        {
            map.insert(plan.source_owner, projection);
        }
    }
    Ok(())
}
fn invalid(error: impl ToString) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacement_history_absent_group_does_not_invent_members_or_block_capture() {
        let lane = HashMap::from([(
            "deleted".to_string(),
            HashMap::from([(
                AttributeKey::intensity(),
                light_programmer::GroupProgrammerValue {
                    value: light_core::AttributeValue::Normalized(0.4),
                    changed_at: chrono::Utc::now(),
                    programmer_order: 71,
                    fade: false,
                    fade_millis: None,
                    delay_millis: None,
                },
            )]),
        )]);
        let empty = HashMap::new();
        let result = capture_lanes(
            [&[], &[], &[]],
            [&lane, &empty, &empty],
            &HashMap::new(),
            &HashMap::new(),
            &[],
        );
        assert!(
            result.unwrap().is_empty(),
            "an absent historical Group has no inferred membership"
        );
        assert_eq!(
            lane["deleted"][&AttributeKey::intensity()].programmer_order,
            71
        );
    }
}
