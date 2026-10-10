use super::*;

fn projection(source: FixtureId) -> light_core::ReplacementProgramProjection {
    let profile = light_core::ReplacementProfileContext {
        profile_id: FixtureId::new(),
        profile_revision: 1,
        mode_id: Uuid::new_v4(),
    };
    light_core::ReplacementProgramProjection {
        source_owner: source,
        source_profile: profile.clone(),
        source_head_id: Uuid::new_v4(),
        target_profile: profile,
        targets: vec![light_core::ReplacementHeadTarget {
            profile_head_id: Uuid::new_v4(),
            fixture_id: FixtureId::new(),
        }],
    }
}
#[test]
fn replacement_projection_equal_value_preset_update_detaches_only_explicit_address() {
    let first = fixture(1);
    let second = fixture(2);
    let attribute = attribute("intensity");
    let preset = Preset {
        instance_id: Some(Uuid::new_v4()),
        family: light_programmer::PresetFamily::Intensity,
        values: HashMap::from([
            (first, HashMap::from([(attribute.clone(), normalized(0.4))])),
            (
                second,
                HashMap::from([(attribute.clone(), normalized(0.4))]),
            ),
        ]),
        fixture_replacement_projections: HashMap::from([
            (
                first,
                HashMap::from([(attribute.clone(), projection(first))]),
            ),
            (
                second,
                HashMap::from([(attribute.clone(), projection(second))]),
            ),
        ]),
        ..Preset::default()
    };
    let incoming = content(vec![fixture_update(first, "intensity", 0.4, 1)]);
    let preview = preview_preset_update(
        "1.1",
        &preset,
        ExistingContentMode::UpdateExisting,
        &incoming,
    )
    .unwrap();
    assert!(preview.has_real_change());
    let updated = planned_preset(
        plan_preset_update(
            "1.1",
            &preset,
            1,
            1,
            ExistingContentMode::UpdateExisting,
            &incoming,
        )
        .unwrap(),
    );
    assert_eq!(updated.instance_id, preset.instance_id);
    assert_eq!(updated.values, preset.values);
    assert!(!updated.fixture_replacement_projections.contains_key(&first));
    assert_eq!(
        updated.fixture_replacement_projections[&second],
        preset.fixture_replacement_projections[&second]
    );
}
#[test]
fn replacement_projection_equal_value_cue_update_detaches_but_preserves_timing_and_other_address() {
    let first = fixture(1);
    let second = fixture(2);
    let mut first_change = CueChange::set(first, attribute("intensity"), normalized(0.4));
    first_change.replacement_projection = Some(projection(first));
    first_change.fade_millis = Some(500);
    let mut second_change = CueChange::set(second, attribute("intensity"), normalized(0.4));
    second_change.replacement_projection = Some(projection(second));
    let original = cue_list(vec![cue(1.0, vec![first_change, second_change])]);
    let mut changed = fixture_update(first, "intensity", 0.4, 1);
    changed.fade_millis = Some(500);
    let incoming = content(vec![changed]);
    let updated = planned_cue_list(
        plan_cue_update(
            &original,
            1,
            1,
            &target(&original, 0, None),
            CueUpdateMode::ExistingInCurrentCue,
            &incoming,
        )
        .unwrap(),
    );
    assert!(updated.cues[0].changes[0].replacement_projection.is_none());
    assert_eq!(updated.cues[0].changes[0].fade_millis, Some(500));
    assert_eq!(updated.cues[0].changes[1], original.cues[0].changes[1]);
}
