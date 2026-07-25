use super::support::{fixture, no_groups, selection};
use crate::SelectionExpression;
use crate::highlight::{HighlightAction, HighlightMode, HighlightRegistry};
use light_core::UserId;
use uuid::Uuid;

#[test]
fn high_is_independent_accepts_empty_selection_and_follows_later_external_selection() {
    let registry = HighlightRegistry::default();
    let desk = Uuid::new_v4();
    let user = UserId::new();
    let fixtures = vec![fixture(1), fixture(2)];
    let groups = no_groups();
    let empty = selection(Vec::new(), Some(SelectionExpression::Static), 1);
    let on = registry
        .action(
            desk,
            user,
            None,
            HighlightAction::On,
            &empty,
            &fixtures,
            &groups,
            false,
        )
        .unwrap();
    assert!(on.state.active);
    assert!(on.state.output_enabled);
    assert!(on.output_fixtures.is_empty());

    let selected = selection(
        vec![fixtures[1].fixture_id],
        Some(SelectionExpression::Static),
        2,
    );
    let followed = registry.status(desk, user, None, &selected, &fixtures, &groups, false);
    assert!(followed.state.active);
    assert_eq!(followed.state.mode, HighlightMode::Selection);
    assert_eq!(followed.output_fixtures, vec![fixtures[1].fixture_id]);
}
