use super::support::{fixture, no_groups, selection};
use crate::highlight::{HighlightAction, HighlightRegistry, is_duplicate_osc_action};
use light_core::UserId;
use light_programmer::SelectionExpression;
use std::time::{Duration, Instant};
use uuid::Uuid;

#[test]
fn authoritative_repeat_guard_prevents_cross_surface_double_steps() {
    let registry = HighlightRegistry::default();
    let desk = Uuid::new_v4();
    let user = UserId::new();
    let fixtures = vec![fixture(1), fixture(2), fixture(3)];
    let ids = fixtures
        .iter()
        .map(|fixture| fixture.fixture_id)
        .collect::<Vec<_>>();
    let groups = no_groups();
    let complete = selection(ids, Some(SelectionExpression::Static), 1);
    let software = registry
        .action_guarded(
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
    let simultaneous_hardware = registry
        .action_guarded(
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
    assert_eq!(software.state.active_index, Some(0));
    assert_eq!(simultaneous_hardware.state.active_index, Some(0));
}

#[test]
fn osc_repeat_guard_normalizes_aliases_and_has_an_exact_boundary() {
    let received_at = Instant::now();
    let previous_at = received_at - Duration::from_millis(149);
    assert!(is_duplicate_osc_action(
        Some(("previous", previous_at)),
        HighlightAction::Previous,
        received_at,
    ));
    assert!(!is_duplicate_osc_action(
        Some(("next", previous_at)),
        HighlightAction::Previous,
        received_at,
    ));
    assert!(!is_duplicate_osc_action(
        Some(("previous", received_at - Duration::from_millis(150))),
        HighlightAction::Previous,
        received_at,
    ));
}
