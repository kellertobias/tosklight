use super::*;

#[test]
fn detailed_people_toggle_is_local_and_operable_while_draft_remains_draft() {
    let mut preferences = Preferences::from_options(&crate::settings::Options::default());
    preferences.quality_override = Some(viz_scene::RenderQuality::Draft);
    let original_population = preferences.crowd_amount;
    let mut settings = QuickSettings::new(&preferences, false);
    settings.tab = QuickSettingsTab::Features;
    settings.selected = settings
        .rows()
        .iter()
        .position(|row| *row == Row::DetailedPeople)
        .unwrap();
    assert_eq!(settings.value(Row::DetailedPeople, &preferences), "Off");
    assert_eq!(
        settings.activate(&mut preferences),
        QuickSettingsOutcome::AppliedLocally
    );
    assert!(preferences.detailed_people);
    assert_eq!(settings.value(Row::DetailedPeople, &preferences), "On");
    assert!(settings.message.contains("Ultra and 4K only"));
    assert_eq!(
        preferences.quality_override,
        Some(viz_scene::RenderQuality::Draft)
    );
    assert_eq!(preferences.crowd_amount, original_population);
    assert_eq!(
        settings.adjust(-1, &mut preferences),
        QuickSettingsOutcome::AppliedLocally
    );
    assert!(!preferences.detailed_people);
}
