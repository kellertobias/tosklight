//! Cold membership/rank expansion shared by static Group and Programmer contribution plans.
use light_core::{AttributeValue, FixtureId, programming::*};
use light_dynamics::RankedSelection;

pub(crate) fn compile_group_values(
    value: &AttributeValue,
    ranking: &RankedSelection,
) -> Result<Vec<(FixtureId, AttributeValue)>, IntentError> {
    let members = ranking
        .ordered_fixture_ids
        .iter()
        .map(|id| (*id, ranking.rank_by_fixture[id]))
        .collect::<Vec<_>>();
    compile_group_member_values(value, &members, ranking.rank_count, |_| {
        Ok(FamilyEditContext {
            color_model: Some(&VirtualColorAuthoringV1),
            ..Default::default()
        })
    })
}
