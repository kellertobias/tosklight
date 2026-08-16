use std::collections::HashSet;

use light_playback::CueNumber;

fn cue(value: &str) -> CueNumber {
    value.parse().expect("valid Cue path")
}

#[test]
fn preserves_distinct_paths_and_orders_numeric_components() {
    let mut paths = ["3", "2.1.1", "2.0", "2.2", "2", "2.1.0", "2.1"]
        .map(cue)
        .to_vec();
    paths.sort();
    assert_eq!(
        paths.iter().map(ToString::to_string).collect::<Vec<_>>(),
        ["2", "2.0", "2.1", "2.1.0", "2.1.1", "2.2", "3"]
    );
    assert!(cue("2.2") < cue("2.10"));
    assert_eq!(paths.iter().cloned().collect::<HashSet<_>>().len(), 7);
}

#[test]
fn rejects_noncanonical_components_and_round_trips_as_json_strings() {
    for invalid in ["", ".2", "2.", "2..1", "02", "2.01", "2.a"] {
        assert!(invalid.parse::<CueNumber>().is_err(), "accepted {invalid}");
    }
    let path = cue("2.1.0");
    let json = serde_json::to_string(&path).unwrap();
    assert_eq!(json, r#""2.1.0""#);
    assert_eq!(serde_json::from_str::<CueNumber>(&json).unwrap(), path);
}

#[test]
fn reads_legacy_json_numbers_with_canonical_integer_components() {
    assert_eq!(serde_json::from_str::<CueNumber>("2.0").unwrap(), cue("2"));
    assert_eq!(
        serde_json::from_str::<CueNumber>("2.05").unwrap(),
        cue("2.5")
    );
}
