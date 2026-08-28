use super::super::config::{PsnConfiguration, PsnZone};
use super::*;

const ZONE: Uuid = Uuid::from_u128(1);

fn downstage(dwell_millis: u64) -> PsnConfiguration {
    PsnConfiguration {
        enabled: true,
        zones: vec![PsnZone {
            id: ZONE,
            name: "Downstage".into(),
            min_metres: [-2.0, 0.0, -2.0],
            max_metres: [2.0, 3.0, 2.0],
            tracker_ids: Vec::new(),
            enter_macro_id: Some(Uuid::from_u128(50)),
            leave_macro_id: Some(Uuid::from_u128(51)),
            dwell_millis,
        }],
        ..PsnConfiguration::default()
    }
}

fn at(x: f32) -> HashMap<u16, [f32; 3]> {
    HashMap::from([(3, [x, 1.0, 0.0])])
}

#[test]
fn walking_in_fires_the_enter_macro_once_the_dwell_has_passed() {
    let configuration = downstage(250);
    let mut states = HashMap::new();

    assert!(advance(&configuration, &at(0.0), &mut states, 1_000).is_empty());
    assert!(advance(&configuration, &at(0.0), &mut states, 1_100).is_empty());
    assert_eq!(
        advance(&configuration, &at(0.0), &mut states, 1_250),
        vec![(ZONE, ZoneTransition::Entered)]
    );
    // Standing there is not a reason to run it again.
    assert!(advance(&configuration, &at(0.0), &mut states, 2_000).is_empty());
}

#[test]
fn walking_out_fires_the_leave_macro() {
    let configuration = downstage(250);
    let mut states = HashMap::new();
    advance(&configuration, &at(0.0), &mut states, 0);
    advance(&configuration, &at(0.0), &mut states, 500);

    assert!(advance(&configuration, &at(9.0), &mut states, 1_000).is_empty());
    assert_eq!(
        advance(&configuration, &at(9.0), &mut states, 1_260),
        vec![(ZONE, ZoneTransition::Left)]
    );
}

#[test]
fn a_marker_flickering_on_the_edge_never_fires_anything() {
    // The failure this dwell exists for: a performer standing on the boundary, whose position
    // crosses it every frame. Nothing should run.
    let configuration = downstage(250);
    let mut states = HashMap::new();
    let mut now = 0;
    for step in 0..40 {
        let position = if step % 2 == 0 { 1.99 } else { 2.01 };
        assert!(
            advance(&configuration, &at(position), &mut states, now).is_empty(),
            "fired at step {step}"
        );
        now += 40;
    }
}

#[test]
fn a_zone_with_no_dwell_fires_on_the_first_look() {
    let configuration = downstage(0);
    let mut states = HashMap::new();
    assert_eq!(
        advance(&configuration, &at(0.0), &mut states, 0),
        vec![(ZONE, ZoneTransition::Entered)]
    );
}

#[test]
fn a_zone_only_watches_the_trackers_it_was_given() {
    let mut configuration = downstage(0);
    configuration.zones[0].tracker_ids = vec![7];
    let mut states = HashMap::new();

    assert!(advance(&configuration, &at(0.0), &mut states, 0).is_empty());

    let watched = HashMap::from([(7, [0.0, 1.0, 0.0])]);
    assert_eq!(
        advance(&configuration, &watched, &mut states, 0),
        vec![(ZONE, ZoneTransition::Entered)]
    );
}

#[test]
fn switching_psn_off_does_not_fire_every_leave_macro() {
    // Turning the source off is not the show being told that everybody left the stage.
    let mut configuration = downstage(0);
    let mut states = HashMap::new();
    advance(&configuration, &at(0.0), &mut states, 0);
    configuration.enabled = false;

    assert!(advance(&configuration, &HashMap::new(), &mut states, 1_000).is_empty());
    assert!(states[&ZONE].occupied);
}

#[test]
fn a_deleted_zone_is_forgotten_rather_than_remembered_as_occupied() {
    let configuration = downstage(0);
    let mut states = HashMap::new();
    advance(&configuration, &at(0.0), &mut states, 0);
    assert!(states[&ZONE].occupied);

    let emptied = PsnConfiguration {
        enabled: true,
        ..PsnConfiguration::default()
    };
    advance(&emptied, &at(0.0), &mut states, 100);
    assert!(states.is_empty());
}

#[test]
fn a_marker_that_leaves_and_comes_back_within_the_dwell_never_left() {
    let configuration = downstage(250);
    let mut states = HashMap::new();
    advance(&configuration, &at(0.0), &mut states, 0);
    advance(&configuration, &at(0.0), &mut states, 300);
    assert!(states[&ZONE].occupied);

    advance(&configuration, &at(9.0), &mut states, 400);
    assert!(advance(&configuration, &at(0.0), &mut states, 500).is_empty());
    assert!(advance(&configuration, &at(0.0), &mut states, 900).is_empty());
    assert!(states[&ZONE].occupied);
}
