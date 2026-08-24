use super::support::{apply_write, fixture, no_groups, selection};
use crate::highlight::{HighlightAction, HighlightMode, HighlightRegistry};
use crate::{GroupDefinition, SelectionExpression};
use std::collections::HashMap;
use uuid::Uuid;

#[test]
fn active_prev_next_all_write_real_selection_and_wrap() {
    let registry = HighlightRegistry::default();
    let desk = Uuid::new_v4();
    let fixtures = vec![fixture(1), fixture(2), fixture(3), fixture(4)];
    let ids = fixtures
        .iter()
        .map(|fixture| fixture.fixture_id)
        .collect::<Vec<_>>();
    let groups = no_groups();
    let complete = selection(ids.clone(), Some(SelectionExpression::Static), 1);

    let inactive_next =
        registry.action(HighlightAction::Next, &complete, &fixtures, &groups, false);
    assert!(inactive_next.working_selection.is_none());
    assert!(!inactive_next.state.can_next && !inactive_next.state.can_previous);
    registry.action(HighlightAction::On, &complete, &fixtures, &groups, false);

    let next = registry.action(HighlightAction::Next, &complete, &fixtures, &groups, false);
    assert!(next.state.active);
    assert_eq!(
        next.working_selection.as_ref().unwrap().selected,
        vec![ids[0]]
    );
    let mut actual = apply_write(&registry, desk, &next, 2).unwrap();
    for expected in [ids[1], ids[2], ids[3], ids[0]] {
        let next = registry.action(HighlightAction::Next, &actual, &fixtures, &groups, false);
        assert_eq!(
            next.working_selection.as_ref().unwrap().selected,
            vec![expected]
        );
        actual = apply_write(&registry, desk, &next, actual.revision + 1).unwrap();
    }

    let all = registry.action(HighlightAction::All, &actual, &fixtures, &groups, false);
    assert_eq!(all.state.mode, HighlightMode::Selection);
    assert_eq!(all.working_selection.as_ref().unwrap().selected, ids);
    actual = apply_write(&registry, desk, &all, actual.revision + 1).unwrap();
    let previous = registry.action(
        HighlightAction::Previous,
        &actual,
        &fixtures,
        &groups,
        false,
    );
    assert_eq!(
        previous.working_selection.as_ref().unwrap().selected,
        vec![ids[3]]
    );
    assert!(previous.state.can_next && previous.state.can_previous);
}

#[test]
fn active_step_keeps_its_frozen_basis_across_external_selection_revisions() {
    let registry = HighlightRegistry::default();
    let desk = Uuid::new_v4();
    let fixtures = vec![fixture(1), fixture(2), fixture(3)];
    let ids = fixtures
        .iter()
        .map(|fixture| fixture.fixture_id)
        .collect::<Vec<_>>();
    let groups = no_groups();
    let complete = selection(ids.clone(), Some(SelectionExpression::Static), 1);
    registry.action(HighlightAction::On, &complete, &fixtures, &groups, false);
    let first = registry.action(HighlightAction::Next, &complete, &fixtures, &groups, false);
    let stepped = apply_write(&registry, desk, &first, 2).unwrap();

    // Programmer values may change repeatedly while the selection revision is unchanged.
    let unchanged = registry.status(&stepped, &fixtures, &groups, false);
    assert_eq!(unchanged.state.mode, HighlightMode::Step);

    // A deliberate external selection revision cannot replace the original activation set.
    let external_same = selection(stepped.selected.clone(), stepped.expression.clone(), 3);
    let frozen = registry.status(&external_same, &fixtures, &groups, false);
    assert_eq!(frozen.state.mode, HighlightMode::Step);
    let next = registry.action(
        HighlightAction::Next,
        &external_same,
        &fixtures,
        &groups,
        false,
    );
    assert_eq!(next.working_selection.unwrap().selected, vec![ids[1]]);
}

#[test]
fn active_all_restores_the_frozen_group_snapshot_after_membership_changes() {
    let registry = HighlightRegistry::default();
    let desk = Uuid::new_v4();
    let fixtures = vec![fixture(1), fixture(2), fixture(3), fixture(4)];
    let ids = fixtures
        .iter()
        .map(|fixture| fixture.fixture_id)
        .collect::<Vec<_>>();
    let mut groups = HashMap::from([(
        "1".into(),
        GroupDefinition {
            id: "1".into(),
            fixtures: ids[..3].to_vec(),
            ..Default::default()
        },
    )]);
    let complete = selection(
        ids[..3].to_vec(),
        Some(SelectionExpression::LiveGroup {
            group_id: "1".into(),
            rule: crate::SelectionRule::All,
        }),
        1,
    );
    registry.action(HighlightAction::On, &complete, &fixtures, &groups, false);
    let first = registry.action(HighlightAction::Next, &complete, &fixtures, &groups, false);
    let stepped = apply_write(&registry, desk, &first, 2).unwrap();
    groups.get_mut("1").unwrap().fixtures = vec![ids[3], ids[1]];
    let all = registry.action(HighlightAction::All, &stepped, &fixtures, &groups, false);
    assert_eq!(
        all.working_selection.as_ref().unwrap().selected,
        ids[..3].to_vec()
    );
    assert!(matches!(
        all.working_selection.unwrap().expression,
        Some(SelectionExpression::Static)
    ));
}

#[test]
fn removed_items_keep_live_sequence_deterministic_and_high_active_when_empty() {
    let registry = HighlightRegistry::default();
    let desk = Uuid::new_v4();
    let fixtures = vec![fixture(1), fixture(2), fixture(3)];
    let ids = fixtures
        .iter()
        .map(|fixture| fixture.fixture_id)
        .collect::<Vec<_>>();
    let groups = no_groups();
    let complete = selection(ids.clone(), Some(SelectionExpression::Static), 1);
    registry.action(HighlightAction::On, &complete, &fixtures, &groups, false);
    let first = registry.action(HighlightAction::Next, &complete, &fixtures, &groups, false);
    let stepped = apply_write(&registry, desk, &first, 2).unwrap();
    let remaining = vec![fixtures[1].clone(), fixtures[2].clone()];
    let reconciled = registry.status(&stepped, &remaining, &groups, false);
    assert_eq!(
        reconciled.working_selection.as_ref().unwrap().selected,
        vec![ids[1]]
    );
    assert_eq!(reconciled.output_fixtures, vec![ids[1]]);

    let corrected = apply_write(&registry, desk, &reconciled, 3).unwrap();
    let only_active = vec![fixtures[1].clone()];
    let inactive_removed = registry.status(&corrected, &only_active, &groups, false);
    assert_eq!(inactive_removed.state.remembered.len(), 1);
    assert!(inactive_removed.working_selection.is_none());
    let wrapped = registry.action(
        HighlightAction::Next,
        &corrected,
        &only_active,
        &groups,
        false,
    );
    assert_eq!(wrapped.working_selection.unwrap().selected, vec![ids[1]]);

    let none = registry.status(&corrected, &[], &groups, false);
    assert!(none.state.active);
    assert!(none.state.output_enabled);
    assert!(none.output_fixtures.is_empty());
}
