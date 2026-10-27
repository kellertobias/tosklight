use super::*;

#[test]
fn detailed_people_preferences_round_trip_explicit_true_without_population_or_quality_changes() {
    let options = Options::default();
    let mut preferences = Preferences::from_options(&options);
    preferences.detailed_people = true;
    preferences.crowd_amount = 0.4;
    let file = preferences.to_file();
    assert!(file.contains("detailed_people true\n"));
    let mut restored = Preferences::from_options(&options);
    restored.adopt_file(&file, &options);
    assert!(restored.detailed_people);
    assert_eq!(restored.crowd_amount, 0.4);
    assert_eq!(restored.quality_override, preferences.quality_override);
    assert!(restored.renderer_settings().detailed_people);
}

#[test]
fn detailed_people_legacy_text_defaults_off_and_only_literal_bool_changes_it() {
    let options = Options::default();
    let mut preferences = Preferences::from_options(&options);
    preferences.adopt_file("crowd_amount 0.5\nquality draft\n", &options);
    assert!(!preferences.detailed_people);
    preferences.adopt_file(
        "detailed_people true\ndetailed_people 1\ndetailed_people yes\n",
        &options,
    );
    assert!(preferences.detailed_people);
    preferences.adopt_file("detailed_people false\ndetailed_people TRUE\n", &options);
    assert!(!preferences.detailed_people);
}
