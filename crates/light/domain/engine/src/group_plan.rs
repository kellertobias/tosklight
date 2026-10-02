//! Group programming, reduced to slots once per generation.
//!
//! A Group fans each programmed attribute out to every member fixture on every tick. Membership
//! and programming only change with the show, so the fan-out is worked out when the generation
//! is built: which slot each member's attribute lives in, and how the value merges. The tick
//! then offers numbers.

use light_core::{AttributeValue, MergeMode};
use light_programmer::GroupDefinition;
use std::collections::HashMap;

/// One Group member's programmed attribute, ready to offer.
#[derive(Clone, Debug)]
pub(crate) struct GroupContributionEntry {
    pub(crate) slot: crate::Slot,
    pub(crate) value: AttributeValue,
    pub(crate) merge_mode: MergeMode,
}

/// Every Group's programming for one generation, as slots.
#[derive(Debug, Default)]
pub(crate) struct GroupContributionPlan {
    entries: Vec<GroupContributionEntry>,
}

impl GroupContributionPlan {
    pub(crate) fn compile(
        groups: &[GroupDefinition],
        rankings: &HashMap<String, light_dynamics::RankedSelection>,
        slots: &crate::SlotTable,
    ) -> Self {
        let mut entries = Vec::new();
        for group in groups {
            if group.programming.is_empty() {
                continue;
            }
            let Some(ranking) = rankings.get(&group.id) else {
                continue;
            };
            let programming = group
                .programming
                .iter()
                .filter_map(|(attribute, value)| {
                    Some((
                        slots.attribute_id(attribute)?,
                        value,
                        if attribute.is_intensity() {
                            MergeMode::Htp
                        } else {
                            MergeMode::Ltp
                        },
                    ))
                })
                .collect::<Vec<_>>();
            for (attribute, value, merge_mode) in programming {
                let Ok(values) = crate::group_programming::compile_group_values(value, ranking)
                else {
                    // Unresolved source models must never emit an unsampled native recipe.
                    continue;
                };
                for (fixture_id, value) in values {
                    let Some(slot) = slots.slot_of(fixture_id, attribute) else {
                        continue;
                    };
                    entries.push(GroupContributionEntry {
                        slot,
                        value,
                        merge_mode,
                    });
                }
            }
        }
        Self { entries }
    }

    pub(crate) fn entries(&self) -> &[GroupContributionEntry] {
        &self.entries
    }
}
