use super::support::{fixture, no_groups, selection};
use crate::SelectionExpression;
use crate::highlight::{HighlightAction, HighlightRegistry, is_duplicate_osc_action};
use std::time::{Duration, Instant};
use uuid::Uuid;

#[test]
fn authoritative_repeat_guard_prevents_cross_surface_double_steps() {
    let now = Instant::now();
    let registry = HighlightRegistry::with_action_clock(std::sync::Arc::new(move || now));
    let desk = Uuid::new_v4();
    let fixtures = vec![fixture(1), fixture(2), fixture(3)];
    let ids = fixtures
        .iter()
        .map(|fixture| fixture.fixture_id)
        .collect::<Vec<_>>();
    let groups = no_groups();
    let complete = selection(ids, Some(SelectionExpression::Static), 1);
    registry.action(HighlightAction::On, &complete, &fixtures, &groups, false);
    let software = registry.action_guarded(
        desk,
        HighlightAction::Next,
        &complete,
        &fixtures,
        &groups,
        false,
    );
    let simultaneous_hardware = registry.action_guarded(
        desk,
        HighlightAction::Next,
        &complete,
        &fixtures,
        &groups,
        false,
    );
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

#[test]
fn owned_highlight_clocks_are_isolated_and_expire_at_exact_boundary() {
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    let base = Instant::now();
    let elapsed_a = Arc::new(AtomicU64::new(0));
    let elapsed_b = Arc::new(AtomicU64::new(0));
    let make = |elapsed: Arc<AtomicU64>| {
        HighlightRegistry::with_action_clock(Arc::new(move || {
            base + Duration::from_millis(elapsed.load(Ordering::SeqCst))
        }))
    };
    let a = make(Arc::clone(&elapsed_a));
    let b = make(elapsed_b);
    let desk = Uuid::new_v4();
    let fixtures = vec![fixture(1), fixture(2), fixture(3)];
    let complete = selection(
        fixtures.iter().map(|f| f.fixture_id).collect(),
        Some(SelectionExpression::Static),
        1,
    );
    let groups = no_groups();
    for registry in [&a, &b] {
        registry.action(HighlightAction::On, &complete, &fixtures, &groups, false);
        assert_eq!(
            registry
                .action_guarded(
                    desk,
                    HighlightAction::Next,
                    &complete,
                    &fixtures,
                    &groups,
                    false
                )
                .state
                .active_index,
            Some(0)
        );
    }
    elapsed_a.store(149, Ordering::SeqCst);
    assert_eq!(
        a.action_guarded(
            desk,
            HighlightAction::Next,
            &complete,
            &fixtures,
            &groups,
            false
        )
        .state
        .active_index,
        Some(0)
    );
    elapsed_a.store(150, Ordering::SeqCst);
    assert_eq!(
        a.action_guarded(
            desk,
            HighlightAction::Next,
            &complete,
            &fixtures,
            &groups,
            false
        )
        .state
        .active_index,
        Some(1)
    );
    assert_eq!(
        b.action_guarded(
            desk,
            HighlightAction::Next,
            &complete,
            &fixtures,
            &groups,
            false
        )
        .state
        .active_index,
        Some(0)
    );
}
