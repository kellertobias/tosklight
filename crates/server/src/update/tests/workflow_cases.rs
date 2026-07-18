use super::*;

#[test]
fn pool_cuelist_requires_one_concrete_active_playback_context() {
    let cue_list_id = CueListId(Uuid::from_u128(10));
    let contexts = vec![
        ActiveCueContext {
            playback_number: 1,
            cue_list_id,
            cue_id: Uuid::from_u128(11),
            cue_number: 1.0,
        },
        ActiveCueContext {
            playback_number: 2,
            cue_list_id,
            cue_id: Uuid::from_u128(12),
            cue_number: 2.0,
        },
    ];
    assert_eq!(
        resolve_cue_target(&CueTargetRequest::PoolCueList { cue_list_id }, &contexts),
        Err(UpdateError::AmbiguousPlaybackContext {
            target: format!("Cuelist {}", cue_list_id.0),
            contexts: 2
        })
    );
    let concrete = resolve_cue_target(
        &CueTargetRequest::ActivePlayback { playback_number: 2 },
        &contexts,
    )
    .unwrap();
    assert_eq!(concrete.playback_number, Some(2));
    assert_eq!(concrete.cue_number, 2.0);
}

#[test]
fn old_settings_receive_documented_defaults_and_confirmation_paths_are_distinct() {
    let settings: UpdateSettings = serde_json::from_str("{}").unwrap();
    assert_eq!(settings.cue_mode, CueUpdateMode::AddToCurrentCue);
    assert_eq!(settings.preset_mode, ExistingContentMode::UpdateExisting);
    assert!(settings.show_update_modal_on_touch);
    assert_eq!(
        settings.confirmation_behavior(&UpdateTargetFamily::Cue, UpdateConfirmationPath::Touch),
        UpdateConfirmationBehavior::OpenModal
    );
    assert_eq!(
        settings.confirmation_behavior(&UpdateTargetFamily::Cue, UpdateConfirmationPath::Enter),
        UpdateConfirmationBehavior::ApplyDefault(UpdateMode::Cue(CueUpdateMode::AddToCurrentCue))
    );

    let settings = UpdateSettings {
        show_update_modal_on_touch: false,
        ..settings
    };
    assert_eq!(
        settings.confirmation_behavior(&UpdateTargetFamily::Preset, UpdateConfirmationPath::Touch),
        UpdateConfirmationBehavior::ApplyDefault(UpdateMode::ExistingContent(
            ExistingContentMode::UpdateExisting
        ))
    );
}

#[test]
fn one_atomic_cuelist_plan_reports_every_changed_source_and_retains_programmer_values() {
    let first = fixture(1);
    let second = fixture(2);
    let list = cue_list(vec![
        cue(1.0, vec![change(first, "intensity", 0.2)]),
        cue(2.0, vec![change(second, "pan", 0.3)]),
        cue(3.0, vec![]),
    ]);
    let target = target(&list, 2, Some(1));
    let programmer = content(vec![
        fixture_update(first, "intensity", 0.8, 1),
        fixture_update(second, "pan", 0.9, 2),
    ]);
    let plan = plan_cue_update(
        &list,
        11,
        11,
        &target,
        CueUpdateMode::ExistingOnly,
        &programmer,
    )
    .unwrap();
    assert_eq!(plan.object_kind(), "cue_list");
    assert!(plan.body().is_ok());
    let result = plan.complete(12);
    assert_eq!(result.changed_count, 2);
    assert_eq!(result.changed_cues.len(), 2);
    assert_eq!(result.revision_before, 11);
    assert_eq!(result.revision_after, 12);
    assert!(result.programmer_values_retained);
    assert_eq!(programmer.fixture_values.len(), 2);
}

#[test]
fn eligible_menu_filter_excludes_no_ops_but_show_all_keeps_them_distinguishable() {
    let fixture = fixture(1);
    let preset = Preset {
        name: "Intensity".into(),
        family: light_programmer::PresetFamily::Intensity,
        number: 1,
        values: HashMap::from([(
            fixture,
            HashMap::from([(attribute("intensity"), normalized(0.5))]),
        )]),
        group_values: HashMap::new(),
    };
    let changed = content(vec![fixture_update(fixture, "intensity", 0.8, 1)]);
    let unchanged = content(vec![fixture_update(fixture, "intensity", 0.5, 1)]);
    let changed_preview =
        preview_preset_update("1", &preset, ExistingContentMode::UpdateExisting, &changed).unwrap();
    let no_op_preview = preview_preset_update(
        "2",
        &preset,
        ExistingContentMode::UpdateExisting,
        &unchanged,
    )
    .unwrap();
    let entries = vec![
        UpdateMenuEntry {
            target: changed_preview.target.clone(),
            active_or_referenced: true,
            existing_preview: changed_preview,
            add_new_preview: None,
        },
        UpdateMenuEntry {
            target: no_op_preview.target.clone(),
            active_or_referenced: true,
            existing_preview: no_op_preview,
            add_new_preview: None,
        },
    ];
    assert_eq!(
        filter_update_menu(&entries, UpdateTargetFilter::EligibleForUpdateExisting).len(),
        1
    );
    assert_eq!(
        filter_update_menu(&entries, UpdateTargetFilter::ShowAllActive).len(),
        2
    );
    assert!(entries[1].is_no_op(UpdateMode::ExistingContent(
        ExistingContentMode::UpdateExisting
    )));
}
