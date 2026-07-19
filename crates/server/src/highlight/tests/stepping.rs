use super::support::{apply_write, fixture, no_groups, selection};
use crate::highlight::{HighlightAction, HighlightMode, HighlightRegistry};
use light_core::UserId;
use light_programmer::{GroupDefinition, SelectionExpression};
use std::collections::HashMap;
use uuid::Uuid;

#[test]
fn prev_next_all_write_real_selection_wrap_and_never_activate_high() {
    let registry = HighlightRegistry::default();
    let desk = Uuid::new_v4();
    let user = UserId::new();
    let fixtures = vec![fixture(1), fixture(2), fixture(3), fixture(4)];
    let ids = fixtures
        .iter()
        .map(|fixture| fixture.fixture_id)
        .collect::<Vec<_>>();
    let groups = no_groups();
    let complete = selection(ids.clone(), Some(SelectionExpression::Static), 1);

    let next = registry
        .action(
            desk,
            user,
            None,
            HighlightAction::Next,
            &complete,
            &fixtures,
            &groups,
            false,
        )
        .unwrap();
    assert!(!next.state.active);
    assert_eq!(
        next.working_selection.as_ref().unwrap().selected,
        vec![ids[0]]
    );
    let mut actual = apply_write(&registry, desk, user, &next, 2).unwrap();
    for expected in [ids[1], ids[2], ids[3], ids[0]] {
        let next = registry
            .action(
                desk,
                user,
                None,
                HighlightAction::Next,
                &actual,
                &fixtures,
                &groups,
                false,
            )
            .unwrap();
        assert_eq!(
            next.working_selection.as_ref().unwrap().selected,
            vec![expected]
        );
        actual = apply_write(&registry, desk, user, &next, actual.revision + 1).unwrap();
    }

    let all = registry
        .action(
            desk,
            user,
            None,
            HighlightAction::All,
            &actual,
            &fixtures,
            &groups,
            false,
        )
        .unwrap();
    assert_eq!(all.state.mode, HighlightMode::Selection);
    assert_eq!(all.working_selection.as_ref().unwrap().selected, ids);
    actual = apply_write(&registry, desk, user, &all, actual.revision + 1).unwrap();
    let previous = registry
        .action(
            desk,
            user,
            None,
            HighlightAction::Previous,
            &actual,
            &fixtures,
            &groups,
            false,
        )
        .unwrap();
    assert_eq!(
        previous.working_selection.as_ref().unwrap().selected,
        vec![ids[3]]
    );
    assert!(previous.state.can_next && previous.state.can_previous);
}

#[test]
fn external_same_membership_revision_resets_step_but_value_changes_do_not() {
    let registry = HighlightRegistry::default();
    let desk = Uuid::new_v4();
    let user = UserId::new();
    let fixtures = vec![fixture(1), fixture(2), fixture(3)];
    let ids = fixtures
        .iter()
        .map(|fixture| fixture.fixture_id)
        .collect::<Vec<_>>();
    let groups = no_groups();
    let complete = selection(ids.clone(), Some(SelectionExpression::Static), 1);
    let first = registry
        .action(
            desk,
            user,
            None,
            HighlightAction::Next,
            &complete,
            &fixtures,
            &groups,
            false,
        )
        .unwrap();
    let stepped = apply_write(&registry, desk, user, &first, 2).unwrap();

    // Programmer values may change repeatedly while the selection revision is unchanged.
    let unchanged = registry.status(desk, user, None, &stepped, &fixtures, &groups, false);
    assert_eq!(unchanged.state.mode, HighlightMode::Step);

    // A deliberate external selection operation has a new revision even if it resolves to the
    // same singleton ID, so it becomes a new complete basis.
    let external_same = selection(stepped.selected.clone(), stepped.expression.clone(), 3);
    let reset = registry.status(desk, user, None, &external_same, &fixtures, &groups, false);
    assert_eq!(reset.state.mode, HighlightMode::Selection);
    let next = registry
        .action(
            desk,
            user,
            None,
            HighlightAction::Next,
            &external_same,
            &fixtures,
            &groups,
            false,
        )
        .unwrap();
    assert_eq!(next.working_selection.unwrap().selected, vec![ids[0]]);
}

#[test]
fn all_reresolves_the_live_group_source_after_membership_changes() {
    let registry = HighlightRegistry::default();
    let desk = Uuid::new_v4();
    let user = UserId::new();
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
            rule: light_programmer::SelectionRule::All,
        }),
        1,
    );
    let first = registry
        .action(
            desk,
            user,
            None,
            HighlightAction::Next,
            &complete,
            &fixtures,
            &groups,
            false,
        )
        .unwrap();
    let stepped = apply_write(&registry, desk, user, &first, 2).unwrap();
    groups.get_mut("1").unwrap().fixtures = vec![ids[3], ids[1]];
    let all = registry
        .action(
            desk,
            user,
            None,
            HighlightAction::All,
            &stepped,
            &fixtures,
            &groups,
            false,
        )
        .unwrap();
    assert_eq!(
        all.working_selection.as_ref().unwrap().selected,
        vec![ids[3], ids[1]]
    );
    assert!(matches!(
        all.working_selection.unwrap().expression,
        Some(SelectionExpression::LiveGroup { ref group_id, .. }) if group_id == "1"
    ));
}

#[test]
fn removed_items_keep_live_sequence_deterministic_and_high_active_when_empty() {
    let registry = HighlightRegistry::default();
    let desk = Uuid::new_v4();
    let user = UserId::new();
    let fixtures = vec![fixture(1), fixture(2), fixture(3)];
    let ids = fixtures
        .iter()
        .map(|fixture| fixture.fixture_id)
        .collect::<Vec<_>>();
    let groups = no_groups();
    let complete = selection(ids.clone(), Some(SelectionExpression::Static), 1);
    registry
        .action(
            desk,
            user,
            None,
            HighlightAction::On,
            &complete,
            &fixtures,
            &groups,
            false,
        )
        .unwrap();
    let first = registry
        .action(
            desk,
            user,
            None,
            HighlightAction::Next,
            &complete,
            &fixtures,
            &groups,
            false,
        )
        .unwrap();
    let stepped = apply_write(&registry, desk, user, &first, 2).unwrap();
    let remaining = vec![fixtures[1].clone(), fixtures[2].clone()];
    let reconciled = registry.status(desk, user, None, &stepped, &remaining, &groups, false);
    assert_eq!(
        reconciled.working_selection.as_ref().unwrap().selected,
        vec![ids[1]]
    );
    assert_eq!(reconciled.output_fixtures, vec![ids[1]]);

    let corrected = apply_write(&registry, desk, user, &reconciled, 3).unwrap();
    let only_active = vec![fixtures[1].clone()];
    let inactive_removed =
        registry.status(desk, user, None, &corrected, &only_active, &groups, false);
    assert_eq!(inactive_removed.state.remembered.len(), 1);
    assert!(inactive_removed.working_selection.is_none());
    let wrapped = registry
        .action(
            desk,
            user,
            None,
            HighlightAction::Next,
            &corrected,
            &only_active,
            &groups,
            false,
        )
        .unwrap();
    assert_eq!(wrapped.working_selection.unwrap().selected, vec![ids[1]]);

    let none = registry.status(desk, user, None, &corrected, &[], &groups, false);
    assert!(none.state.active);
    assert!(none.state.output_enabled);
    assert!(none.output_fixtures.is_empty());
}
