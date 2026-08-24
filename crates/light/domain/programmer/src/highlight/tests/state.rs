use super::support::{fixture, no_groups, selection};
use crate::SelectionExpression;
use crate::highlight::{HighlightAction, HighlightMode, HighlightRegistry};
use uuid::Uuid;

#[test]
fn high_freezes_the_non_empty_selection_captured_on_activation() {
    let registry = HighlightRegistry::default();
    let fixtures = vec![fixture(1), fixture(2)];
    let groups = no_groups();
    let original = selection(
        vec![fixtures[0].fixture_id],
        Some(SelectionExpression::Static),
        1,
    );
    let on = registry.action(HighlightAction::On, &original, &fixtures, &groups, false);
    assert!(on.state.active);
    assert!(on.state.output_enabled);
    assert_eq!(on.output_fixtures, vec![fixtures[0].fixture_id]);

    let selected = selection(
        vec![fixtures[1].fixture_id],
        Some(SelectionExpression::Static),
        2,
    );
    let followed = registry.status(&selected, &fixtures, &groups, false);
    assert!(followed.state.active);
    assert_eq!(followed.state.mode, HighlightMode::Selection);
    assert_eq!(followed.output_fixtures, vec![fixtures[0].fixture_id]);
    assert_eq!(followed.output_layers[0].fixture_id, fixtures[0].fixture_id);
}

#[test]
fn next_projects_high_and_low_layers_and_explicit_attributes_survive_navigation() {
    use crate::highlight::HighlightOutputRole;
    use light_core::AttributeKey;

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
    let next = registry.action(HighlightAction::Next, &complete, &fixtures, &groups, false);
    let write = next.working_selection.as_ref().unwrap();
    let stepped = selection(write.selected.clone(), write.expression.clone(), 2);
    registry.acknowledge_internal_selection(desk, &stepped);

    let layers = registry.output_layers();
    assert_eq!(
        layers
            .iter()
            .filter(|layer| layer.role == HighlightOutputRole::Highlight)
            .count(),
        1
    );
    assert_eq!(
        layers
            .iter()
            .filter(|layer| layer.role == HighlightOutputRole::LowLight)
            .count(),
        2
    );
    assert!(registry.mark_explicit_fixture_attributes(
        desk,
        [
            (ids[0], AttributeKey::intensity()),
            (ids[1], AttributeKey("color".into()))
        ],
    ));
    let layers = registry.output_layers();
    assert!(
        layers
            .iter()
            .find(|layer| layer.fixture_id == ids[0])
            .unwrap()
            .suppressed_attributes
            .contains(&AttributeKey::intensity())
    );
    assert!(
        layers
            .iter()
            .find(|layer| layer.fixture_id == ids[1])
            .unwrap()
            .suppressed_attributes
            .is_empty()
    );

    let next_again = registry.action(HighlightAction::Next, &stepped, &fixtures, &groups, false);
    let write = next_again.working_selection.as_ref().unwrap();
    let stepped_again = selection(write.selected.clone(), write.expression.clone(), 3);
    registry.acknowledge_internal_selection(desk, &stepped_again);
    let all = registry.action(
        HighlightAction::All,
        &stepped_again,
        &fixtures,
        &groups,
        false,
    );
    assert!(
        all.output_layers
            .iter()
            .all(|layer| layer.role == HighlightOutputRole::Highlight)
    );
    assert!(
        all.output_layers
            .iter()
            .find(|layer| layer.fixture_id == ids[0])
            .unwrap()
            .suppressed_attributes
            .contains(&AttributeKey::intensity())
    );
    let restored = selection(
        all.working_selection.as_ref().unwrap().selected.clone(),
        all.working_selection.as_ref().unwrap().expression.clone(),
        4,
    );
    registry.acknowledge_internal_selection(desk, &restored);
    let off = registry.action(HighlightAction::Off, &restored, &fixtures, &groups, false);
    assert!(off.output_layers.is_empty());
    let reactivated = registry.action(HighlightAction::On, &restored, &fixtures, &groups, false);
    assert!(
        reactivated
            .output_layers
            .iter()
            .all(|layer| layer.suppressed_attributes.is_empty())
    );
}

#[test]
fn an_explicitly_programmed_attribute_is_suppressed_on_the_desks_highlight() {
    use crate::highlight::HighlightOutputRole;
    use light_core::AttributeKey;

    let registry = HighlightRegistry::default();
    let desk = Uuid::new_v4();
    let fixtures = vec![fixture(1), fixture(2)];
    let ids = fixtures
        .iter()
        .map(|fixture| fixture.fixture_id)
        .collect::<Vec<_>>();
    let groups = no_groups();
    let complete = selection(ids.clone(), Some(SelectionExpression::Static), 1);
    registry.action(HighlightAction::On, &complete, &fixtures, &groups, false);

    // Authoring the attribute in the normal Programmer suppresses the temporary look for it.
    // There is one Highlight, so one surface saying so is the desk saying so.
    registry.mark_explicit_fixture_attributes(desk, [(ids[0], AttributeKey::intensity())]);
    assert!(
        registry
            .output_layers()
            .iter()
            .find(|layer| layer.fixture_id == ids[0])
            .unwrap()
            .suppressed_attributes
            .contains(&AttributeKey::intensity())
    );

    // Stepping focuses one fixture: it stays Highlight and the rest fall to Low Light. With one
    // Highlight there is no second operator's un-stepped layer covering them.
    let next = registry.action(HighlightAction::Next, &complete, &fixtures, &groups, false);
    let stepped = selection(
        next.working_selection.as_ref().unwrap().selected.clone(),
        next.working_selection.as_ref().unwrap().expression.clone(),
        2,
    );
    registry.acknowledge_internal_selection(desk, &stepped);
    let focused = next.working_selection.as_ref().unwrap().selected.clone();
    for layer in registry.output_layers() {
        let expected = if focused.contains(&layer.fixture_id) {
            HighlightOutputRole::Highlight
        } else {
            HighlightOutputRole::LowLight
        };
        assert_eq!(layer.role, expected, "fixture {:?}", layer.fixture_id);
    }
}
