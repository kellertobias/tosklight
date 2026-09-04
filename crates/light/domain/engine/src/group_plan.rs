//! Group programming, reduced to slots once per generation.
//!
//! A Group fans each programmed attribute out to every member fixture on every tick. Membership
//! and programming only change with the show, so the fan-out is worked out when the generation
//! is built: which slot each member's attribute lives in, and how the value merges. The tick
//! then offers numbers.

use light_core::{AttributeValue, MergeMode};
use light_programmer::{GroupDefinition, resolve_group};
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
        definitions: &HashMap<String, GroupDefinition>,
        slots: &crate::SlotTable,
    ) -> Self {
        let mut entries = Vec::new();
        for group in groups {
            if group.programming.is_empty() {
                continue;
            }
            let fixtures = resolve_group(&group.id, definitions).unwrap_or_default();
            if fixtures.is_empty() {
                continue;
            }
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
            for fixture_id in fixtures {
                for (attribute, value, merge_mode) in &programming {
                    // A member that does not have this attribute is left alone rather than given
                    // a value nothing would ever project.
                    let Some(slot) = slots.slot_of(fixture_id, *attribute) else {
                        continue;
                    };
                    entries.push(GroupContributionEntry {
                        slot,
                        value: (*value).clone(),
                        merge_mode: *merge_mode,
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
