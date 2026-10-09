//! TL-638: Update on a universal Preset through the real Update writer and SQLite reopen.
//!
//! Recording one shared colour into a Color Preset makes it universal: the colour sits in
//! `universal_values` and no fixture is named. Recording an Aim target makes a universal Position
//! Preset the same way. Update must treat that universal value as the existing content of every
//! Programmer address that reads it on recall. These tests run the same real path as TL-627
//! (`preview_update` → `handle_update` → `prepare_show_candidate` → SQLite) and reopen the file
//! for every storage claim.
use super::*;

/// Builds a Color Preset the way Record does in a Color Intent show: one shared colour on every
/// recorded fixture, consolidated into a universal Preset.
fn recorded_universal_color(value: AttributeValue) -> Preset {
    let color_key = key(ProgrammingOwner::Color);
    let mut preset = Preset {
        fixture_replacement_projections: Default::default(),
        group_replacement_projections: Default::default(),
        name: "TL-638 shared colour".into(),
        family: PresetFamily::Color,
        number: 1,
        values: [fixture_a(), fixture_b()]
            .into_iter()
            .map(|id| (id, HashMap::from([(color_key.clone(), value.clone())])))
            .collect(),
        ..Preset::default()
    };
    preset.consolidate_universal_color();
    assert!(preset.is_universal(), "recording one colour is universal");
    assert!(preset.values.is_empty() && preset.group_values.is_empty());
    preset
}

fn universal(family: PresetFamily, owner: ProgrammingOwner, value: AttributeValue) -> Preset {
    Preset {
        fixture_replacement_projections: Default::default(),
        group_replacement_projections: Default::default(),
        name: format!("TL-638 universal {family:?}"),
        family,
        number: 1,
        universal_values: HashMap::from([(key(owner), value)]),
        ..Preset::default()
    }
}

fn fixtures(rows: &[(FixtureId, ProgrammingOwner, AttributeValue)]) -> Intent {
    Intent {
        fixtures: rows
            .iter()
            .map(|(fixture_id, owner, value)| (*fixture_id, key(*owner), value.clone()))
            .collect(),
        groups: Vec::new(),
    }
}

fn seeded_rig(family: PresetFamily, preset: &Preset, intent: &Intent) -> TestRig {
    let rig = TestRig::new();
    seed_group(&rig);
    rig.seed(
        "preset",
        preset_object_id(family),
        serde_json::to_value(preset).unwrap(),
    );
    program(&rig, intent);
    rig
}

fn reopened_preset(rig: &TestRig, family: PresetFamily) -> (Value, Preset) {
    let body = document(rig)
        .object("preset", preset_object_id(family))
        .unwrap()
        .body()
        .clone();
    let typed = serde_json::from_value(body.clone()).unwrap();
    (body, typed)
}

fn outcomes(preview: &ProgrammingUpdatePreviewResult) -> Vec<UpdateItemOutcome> {
    preview
        .preview
        .items
        .iter()
        .map(|item| item.outcome.clone())
        .collect()
}

/// Runs one confirmed Update through the real writer and returns its result.
fn update(
    rig: &TestRig,
    ports: &RetainingPorts<'_>,
    family: PresetFamily,
    mode: ExistingContentMode,
    label: &str,
) -> (Vec<UpdateItemOutcome>, ProgrammingUpdateResult) {
    let target = preset_target_of(family);
    let mode = UpdateMode::ExistingContent(mode);
    let preview = preview(
        rig,
        ports,
        target.clone(),
        mode,
        &format!("{label}-preview"),
    )
    .unwrap();
    let outcomes = outcomes(&preview);
    let command = command_from_preview(rig, target, preview);
    rig.clear_steps();
    let result = apply(rig, ports, command, label).unwrap();
    assert!(!result.replayed, "{label}");
    assert_one_commit_lifecycle(rig, label);
    (outcomes, result)
}

/// The same Programmer again after reopen is the documented Update no-op: Invalid, no commit,
/// no event, no install and byte-identical SQLite files.
fn assert_identical_update_is_no_op(
    rig: &TestRig,
    ports: &RetainingPorts<'_>,
    family: PresetFamily,
    mode: ExistingContentMode,
    label: &str,
) {
    let committed = observe(rig, ports);
    let preview = preview(
        rig,
        ports,
        preset_target_of(family),
        UpdateMode::ExistingContent(mode),
        &format!("{label}-identical-preview"),
    )
    .unwrap();
    assert!(
        outcomes(&preview)
            .iter()
            .all(|outcome| matches!(outcome, UpdateItemOutcome::Unchanged { .. })),
        "{label}: {:?}",
        preview.preview.items
    );
    let command = command_from_preview(rig, preset_target_of(family), preview);
    rig.clear_steps();
    let error = apply(rig, ports, command, &format!("{label}-identical")).unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Invalid, "{label}: {error:?}");
    assert!(
        error.message.contains("would not change"),
        "{label}: {error:?}"
    );
    assert_no_side_effects(rig, label);
    assert_eq!(observe(rig, ports), committed, "{label}: identical");
}

#[test]
fn update_on_a_recorded_universal_color_preset_stores_the_new_shared_intent_after_reopen() {
    for mode in [
        ExistingContentMode::UpdateExisting,
        ExistingContentMode::AddNew,
    ] {
        let label = format!("universal Color/{mode:?}");
        let family = PresetFamily::Color;
        let warm = color(warm_white_3200());
        let rig = seeded_rig(
            family,
            &recorded_universal_color(color(magenta())),
            &fixtures(&[
                (fixture_a(), ProgrammingOwner::Color, warm.clone()),
                (fixture_b(), ProgrammingOwner::Color, warm.clone()),
            ]),
        );
        let ports = RetainingPorts::new(&rig);

        let (outcomes, result) = update(&rig, &ports, family, mode, &label);
        assert_eq!(
            outcomes,
            vec![UpdateItemOutcome::UpdateExisting; 2],
            "{label}: the universal colour is existing content"
        );
        assert_eq!(result.outcome.summary.changed_count, 2, "{label}");
        assert_eq!(result.outcome.summary.added_count, 0, "{label}");
        assert_eq!(result.outcome.summary.ignored_count, 0, "{label}");

        let (body, typed) = reopened_preset(&rig, family);
        assert_eq!(
            body,
            *result.outcome.projection.raw_body.as_ref(),
            "{label}: committed projection is the reopened body"
        );
        assert_eq!(body["future"], json!({"keep": true}), "{label}");
        assert!(typed.is_universal(), "{label}: still universal");
        assert!(typed.values.is_empty(), "{label}: no fixture named");
        assert!(typed.group_values.is_empty(), "{label}");
        assert_eq!(
            typed.universal_values,
            HashMap::from([(key(ProgrammingOwner::Color), warm.clone())]),
            "{label}"
        );
        // Requested semantic intent, never a solved or achieved value.
        assert_exact_value(&body["universal_values"]["color"], &warm);
        assert_installed_is_reopened_compile(&rig, &ports);

        assert_identical_update_is_no_op(&rig, &ports, family, mode, &label);
    }
}

#[test]
fn universal_preset_update_with_disagreeing_values_stays_universal_and_stores_the_difference() {
    let family = PresetFamily::Color;
    let (magenta, warm) = (color(magenta()), color(warm_white_3200()));
    let rig = seeded_rig(
        family,
        &recorded_universal_color(magenta.clone()),
        &fixtures(&[
            (fixture_a(), ProgrammingOwner::Color, magenta.clone()),
            (fixture_b(), ProgrammingOwner::Color, warm.clone()),
        ]),
    );
    let ports = RetainingPorts::new(&rig);
    let label = "disagreeing";
    let (outcomes, result) = update(
        &rig,
        &ports,
        family,
        ExistingContentMode::UpdateExisting,
        label,
    );
    assert_eq!(
        outcomes,
        vec![
            UpdateItemOutcome::Unchanged { source: None },
            UpdateItemOutcome::UpdateExisting
        ]
    );
    assert_eq!(result.outcome.summary.changed_count, 1);

    let (body, typed) = reopened_preset(&rig, family);
    assert_eq!(
        typed.universal_values,
        HashMap::from([(key(ProgrammingOwner::Color), magenta)]),
        "the shared colour still reaches every other fixture"
    );
    assert_eq!(
        typed.values,
        HashMap::from([(
            fixture_b(),
            HashMap::from([(key(ProgrammingOwner::Color), warm.clone())])
        )]),
        "only the fixture with a different colour is named"
    );
    assert_exact_value(&body["values"][fixture_b().0.to_string()]["color"], &warm);
    assert_installed_is_reopened_compile(&rig, &ports);
    assert_identical_update_is_no_op(
        &rig,
        &ports,
        family,
        ExistingContentMode::UpdateExisting,
        label,
    );
}

/// The same blind spot covered every family: an Aim (universal Position Target) Preset and a
/// universal Zoom Preset are updated with new semantic intent and stay universal.
#[test]
fn universal_position_and_zoom_presets_take_the_new_shared_semantic_intent() {
    let cases = [
        (
            PresetFamily::Position,
            ProgrammingOwner::Position,
            point_target(POINT),
            unwrapped_angles(),
        ),
        (
            PresetFamily::Beam,
            ProgrammingOwner::Zoom,
            zoom(),
            narrow_zoom(),
        ),
    ];
    for (family, owner, stored, requested) in cases {
        for mode in [
            ExistingContentMode::UpdateExisting,
            ExistingContentMode::AddNew,
        ] {
            let label = format!("universal {family:?}/{mode:?}");
            let rig = seeded_rig(
                family,
                &universal(family, owner, stored.clone()),
                &fixtures(&[
                    (fixture_a(), owner, requested.clone()),
                    (fixture_b(), owner, requested.clone()),
                ]),
            );
            let ports = RetainingPorts::new(&rig);
            let (outcomes, result) = update(&rig, &ports, family, mode, &label);
            assert_eq!(
                outcomes,
                vec![UpdateItemOutcome::UpdateExisting; 2],
                "{label}"
            );
            assert_eq!(result.outcome.summary.changed_count, 2, "{label}");

            let (body, typed) = reopened_preset(&rig, family);
            assert!(typed.values.is_empty(), "{label}: no fixture named");
            assert!(typed.group_values.is_empty(), "{label}");
            assert_eq!(
                typed.universal_values,
                HashMap::from([(key(owner), requested.clone())]),
                "{label}"
            );
            assert_eq!(
                body["universal_values"][key(owner).0.as_ref()],
                serde_json::to_value(&requested).unwrap(),
                "{label}: exact requested intent"
            );
            assert_installed_is_reopened_compile(&rig, &ports);
            assert_identical_update_is_no_op(&rig, &ports, family, mode, &label);
        }
    }
}

/// Recall applies a universal value to a live Group, so the Group address reads it too. A plain
/// shared value stays universal; a Group family assignment with member exceptions can only live
/// on its Group owner and is stored there explicitly.
#[test]
fn live_group_update_on_a_universal_color_preset_keeps_group_assignments_on_their_owner() {
    let family = PresetFamily::Color;
    let (magenta, warm) = (color(magenta()), color(warm_white_3200()));
    let color_key = key(ProgrammingOwner::Color);

    let plain = Intent {
        fixtures: vec![(fixture_c(), color_key.clone(), warm.clone())],
        groups: vec![(GROUP.into(), color_key.clone(), warm.clone())],
    };
    let rig = seeded_rig(family, &recorded_universal_color(magenta.clone()), &plain);
    let ports = RetainingPorts::new(&rig);
    let (_, result) = update(
        &rig,
        &ports,
        family,
        ExistingContentMode::UpdateExisting,
        "plain Group",
    );
    assert_eq!(result.outcome.summary.changed_count, 2);
    let (_, typed) = reopened_preset(&rig, family);
    assert_eq!(
        typed.universal_values,
        HashMap::from([(color_key.clone(), warm.clone())])
    );
    assert!(typed.values.is_empty() && typed.group_values.is_empty());
    assert_installed_is_reopened_compile(&rig, &ports);

    let assignment = group_family(
        ProgrammingOwner::Color,
        warm.clone(),
        [(fixture_a().0, color(magenta_with_uv()))],
    );
    let family_assignment = Intent {
        fixtures: Vec::new(),
        groups: vec![(GROUP.into(), color_key.clone(), assignment.clone())],
    };
    let rig = seeded_rig(
        family,
        &recorded_universal_color(magenta.clone()),
        &family_assignment,
    );
    let ports = RetainingPorts::new(&rig);
    let (_, result) = update(
        &rig,
        &ports,
        family,
        ExistingContentMode::UpdateExisting,
        "Group family",
    );
    assert_eq!(result.outcome.summary.changed_count, 1);
    let (body, typed) = reopened_preset(&rig, family);
    assert_eq!(
        typed.universal_values,
        HashMap::from([(color_key.clone(), magenta)]),
        "a Group assignment never becomes universal"
    );
    assert_eq!(typed.group_values[GROUP][&color_key], assignment);
    assert_exact_value(&body["group_values"][GROUP]["color"], &assignment);
    assert_installed_is_reopened_compile(&rig, &ports);
}

/// A mixed Preset keeps source-aware Update: a named fixture updates its own stored value, and
/// an unnamed fixture updates the universal value it reads on recall.
#[test]
fn mixed_preset_update_changes_each_address_at_its_own_source() {
    let family = PresetFamily::Color;
    let color_key = key(ProgrammingOwner::Color);
    let (magenta, warm, black) = (
        color(magenta()),
        color(warm_white_3200()),
        color(uv_only_black()),
    );
    let mixed = Preset {
        fixture_replacement_projections: Default::default(),
        group_replacement_projections: Default::default(),
        name: "TL-638 mixed".into(),
        family,
        number: 1,
        values: HashMap::from([(
            fixture_a(),
            HashMap::from([(color_key.clone(), magenta.clone())]),
        )]),
        universal_values: HashMap::from([(color_key.clone(), black.clone())]),
        ..Preset::default()
    };
    let rig = seeded_rig(
        family,
        &mixed,
        &fixtures(&[
            (fixture_a(), ProgrammingOwner::Color, warm.clone()),
            (fixture_c(), ProgrammingOwner::Color, magenta.clone()),
        ]),
    );
    let ports = RetainingPorts::new(&rig);
    let (outcomes, result) = update(
        &rig,
        &ports,
        family,
        ExistingContentMode::UpdateExisting,
        "mixed",
    );
    assert_eq!(outcomes, vec![UpdateItemOutcome::UpdateExisting; 2]);
    assert_eq!(result.outcome.summary.changed_count, 2);
    let (_, typed) = reopened_preset(&rig, family);
    assert_eq!(
        typed.values,
        HashMap::from([(fixture_a(), HashMap::from([(color_key.clone(), warm)]))]),
        "the named fixture keeps its own explicit source"
    );
    assert_eq!(
        typed.universal_values,
        HashMap::from([(color_key, magenta)]),
        "the unnamed fixture updated the universal value it reads"
    );
    assert_installed_is_reopened_compile(&rig, &ports);
}
