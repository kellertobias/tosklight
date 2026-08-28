use super::super::config::{PsnBinding, PsnConfiguration};
use super::*;

fn configuration(binding: PsnBinding) -> PsnConfiguration {
    PsnConfiguration {
        enabled: true,
        bindings: vec![binding],
        ..PsnConfiguration::default()
    }
}

fn binding(point: Uuid) -> PsnBinding {
    PsnBinding {
        id: Uuid::from_u128(1),
        tracker_id: 3,
        point_fixture_id: point,
        enabled: true,
    }
}

fn axis(overrides: &[TrackedOverride], attribute: &str) -> f32 {
    overrides
        .iter()
        .find(|held| &*held.attribute.0 == attribute)
        .and_then(|held| held.value.normalized())
        .unwrap()
}

#[test]
fn a_point_patched_at_the_origin_holds_the_marker_where_it_is() {
    let point = Uuid::from_u128(9);
    let held = HashMap::from([(Uuid::from_u128(1), [2.0, 1.5, -3.0])]);
    let patched = HashMap::from([(point, [0.0, 0.0, 0.0])]);

    let (overrides, placements) = placements(&configuration(binding(point)), &held, &patched);

    assert_eq!(overrides.len(), 3);
    assert_eq!(axis(&overrides, "point.position.x"), normalized_axis(2.0));
    assert_eq!(axis(&overrides, "point.position.y"), normalized_axis(1.5));
    assert_eq!(axis(&overrides, "point.position.z"), normalized_axis(-3.0));
    assert_eq!(placements[0].position_metres, [2.0, 1.5, -3.0]);
    assert!(!placements[0].out_of_reach);
}

#[test]
fn a_point_patched_away_from_the_origin_is_offset_from_where_it_was_patched() {
    // The stored value is an offset, so a point hung above the stage has to be told to move down
    // to reach a marker standing on it.
    let point = Uuid::from_u128(9);
    let held = HashMap::from([(Uuid::from_u128(1), [1.0, 0.0, 0.0])]);
    let patched = HashMap::from([(point, [1.0, 6.0, 0.0])]);

    let (overrides, placements) = placements(&configuration(binding(point)), &held, &patched);

    assert_eq!(axis(&overrides, "point.position.x"), normalized_axis(0.0));
    assert_eq!(axis(&overrides, "point.position.y"), normalized_axis(-6.0));
    assert_eq!(placements[0].position_metres, [1.0, 0.0, 0.0]);
}

#[test]
fn a_marker_further_away_than_a_point_can_reach_stops_and_says_so() {
    let point = Uuid::from_u128(9);
    let held = HashMap::from([(Uuid::from_u128(1), [500.0, 0.0, 0.0])]);
    let patched = HashMap::from([(point, [0.0, 0.0, 0.0])]);

    let (overrides, placements) = placements(&configuration(binding(point)), &held, &patched);

    assert_eq!(axis(&overrides, "point.position.x"), 1.0);
    assert!(placements[0].out_of_reach);
    // It stops at the end of its travel rather than wrapping to somewhere else entirely.
    assert_eq!(placements[0].position_metres[0], POINT_AXIS_METRES);
}

#[test]
fn a_binding_whose_point_was_deleted_is_skipped_rather_than_breaking_the_rest() {
    let deleted = Uuid::from_u128(9);
    let live = Uuid::from_u128(10);
    let mut configuration = configuration(binding(deleted));
    configuration.bindings.push(PsnBinding {
        id: Uuid::from_u128(2),
        tracker_id: 4,
        point_fixture_id: live,
        enabled: true,
    });
    let held = HashMap::from([
        (Uuid::from_u128(1), [1.0, 0.0, 0.0]),
        (Uuid::from_u128(2), [2.0, 0.0, 0.0]),
    ]);
    let patched = HashMap::from([(live, [0.0, 0.0, 0.0])]);

    let (overrides, placements) = placements(&configuration, &held, &patched);

    assert_eq!(placements.len(), 1);
    assert_eq!(placements[0].point_fixture_id, live);
    assert_eq!(overrides.len(), 3);
}

#[test]
fn a_tracker_that_has_never_reported_holds_nothing() {
    // Not the same as holding zero: a point bound to a tracker nobody has heard from stays
    // wherever the show put it, and the operator can still move it by hand.
    let point = Uuid::from_u128(9);
    let patched = HashMap::from([(point, [0.0, 0.0, 0.0])]);

    let (overrides, placements) =
        placements(&configuration(binding(point)), &HashMap::new(), &patched);

    assert!(overrides.is_empty());
    assert!(placements.is_empty());
}

#[test]
fn a_disabled_binding_holds_nothing() {
    let point = Uuid::from_u128(9);
    let mut configuration = configuration(binding(point));
    configuration.bindings[0].enabled = false;
    let held = HashMap::from([(Uuid::from_u128(1), [2.0, 0.0, 0.0])]);
    let patched = HashMap::from([(point, [0.0, 0.0, 0.0])]);

    let (overrides, _) = placements(&configuration, &held, &patched);

    assert!(overrides.is_empty());
}
