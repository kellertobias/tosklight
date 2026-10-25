use super::*;

#[test]
fn detailed_people_true_survives_text_and_json_while_old_inputs_default_off() {
    let settings = RendererSettings {
        detailed_people: true,
        ..RendererSettings::default()
    };
    assert!(settings.to_file().contains("detailed_people true\n"));
    assert_eq!(RendererSettings::from_file(&settings.to_file()), settings);
    let encoded = serde_json::to_value(&settings).unwrap();
    assert_eq!(encoded["detailedPeople"], true);
    assert_eq!(
        serde_json::from_value::<RendererSettings>(encoded.clone()).unwrap(),
        settings
    );
    let mut legacy = encoded;
    legacy.as_object_mut().unwrap().remove("detailedPeople");
    assert!(
        !serde_json::from_value::<RendererSettings>(legacy)
            .unwrap()
            .detailed_people
    );
    assert!(!RendererSettings::from_file("quality ultra\ncrowd_amount 0.5\n").detailed_people);
}

#[test]
fn detailed_people_text_is_strict_and_intents_apply_the_independent_local_bool() {
    let parsed = RendererSettings::from_file(
        "detailed_people true\ndetailed_people yes\ndetailed_people 1\n",
    );
    assert!(parsed.detailed_people);
    assert!(
        !RendererSettings::from_file("detailed_people false\ndetailed_people TRUE\n")
            .detailed_people
    );
    let intent: RendererSettingsIntent = serde_json::from_value(serde_json::json!({
        "requestId": "audience-detail", "source": "local-settings", "changes": [
            {"field": "detailedPeople", "value": true}
        ]
    }))
    .unwrap();
    let previous = RendererSettings::default();
    let mut current = previous.clone();
    current.apply(&intent.changes);
    current.validate().unwrap();
    assert!(current.detailed_people);
    assert_eq!(current.crowd_amount, previous.crowd_amount);
    assert_eq!(current.quality, previous.quality);
    assert_eq!(
        current.changes_from(&previous),
        vec![RendererSettingChange::DetailedPeople(true)]
    );
    assert_eq!(intent.changes[0].field_name(), "detailedPeople");
}
