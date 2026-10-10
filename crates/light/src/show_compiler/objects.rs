use super::invalid_candidate;
use crate::ActionError;
use light_core::FixtureId;
use light_playback::{
    CueList, FlashReleaseMode, PlaybackButtonAction, PlaybackDefinition, PlaybackFaderMode,
    PlaybackPage, PlaybackTarget,
};
use light_programmer::{GroupDefinition, Preset, resolve_group};
use light_show::PortableShowCandidate;
use serde::de::DeserializeOwned;
use std::collections::HashMap;

pub(super) fn decode<T: DeserializeOwned>(
    candidate: PortableShowCandidate<'_>,
    kind: &str,
) -> Result<Vec<T>, ActionError> {
    candidate
        .objects_of_kind(kind)
        .map(|object| {
            serde_json::from_value(object.body().clone()).map_err(|error| {
                invalid_candidate(format!("invalid {kind} {}: {error}", object.key().id()))
            })
        })
        .collect()
}

pub(super) fn decode_cue_lists(
    candidate: PortableShowCandidate<'_>,
) -> Result<Vec<CueList>, ActionError> {
    let mut lists: Vec<CueList> = decode(candidate, "cue_list")?;
    if lists.iter().flat_map(|list| &list.cues).any(|cue| {
        cue.changes
            .iter()
            .any(|change| change.preset_reference.is_some())
            || cue
                .group_changes
                .iter()
                .any(|change| change.preset_reference.is_some())
    }) {
        let presets: Vec<Preset> = decode(candidate, "preset")?;
        let mut catalog = HashMap::new();
        for preset in presets {
            if let Some(id) = preset.instance_id
                && catalog.insert(id, preset).is_some()
            {
                return Err(invalid_candidate(format!(
                    "duplicate Preset instance identity {id}; recreate the copied Preset before linking Cues"
                )));
            }
        }
        // Derived Aim bodies intentionally have no literal Position map. Materialize only this
        // runtime projection; preserve the portable body and immutable live-reference identity.
        let requested = lists
            .iter()
            .flat_map(|list| &list.cues)
            .flat_map(|cue| {
                cue.changes
                    .iter()
                    .filter_map(|change| change.preset_reference.as_ref())
                    .chain(
                        cue.group_changes
                            .iter()
                            .filter_map(|change| change.preset_reference.as_ref()),
                    )
            })
            .filter(|reference| {
                matches!(
                    reference.source_owner,
                    light_core::PresetValueOwner::Universal
                ) && reference.source_attribute.0.as_ref() == "position"
            })
            .filter_map(|reference| {
                catalog
                    .get(&reference.preset_instance_id)?
                    .aim_at_fixture_number
            })
            .collect::<std::collections::HashSet<_>>();
        if !requested.is_empty() {
            let fixtures = super::patch::compile_patch(candidate)?;
            let targets = light_engine::saved_aim_targets(&fixtures, requested);
            for preset in catalog.values_mut() {
                if let Some(number) = preset.aim_at_fixture_number {
                    preset
                        .universal_values
                        .remove(&light_core::AttributeKey("position".into()));
                    if let Some(intent) = targets.get(&number) {
                        preset.universal_values.insert(
                            light_core::AttributeKey("position".into()),
                            light_core::AttributeValue::Position(std::sync::Arc::new(
                                intent.clone(),
                            )),
                        );
                    }
                }
            }
        }
        let presets = catalog;
        let native = super::native_sources::compile(candidate, None)?;
        for cue in lists.iter_mut().flat_map(|list| &mut list.cues) {
            for change in &mut cue.changes {
                if change.value.is_some()
                    && let Some(reference) = &change.preset_reference
                    && let Some(value) = super::cue_presets::resolve(reference, &presets, &native)
                {
                    change.replacement_projection = super::cue_presets::replacement_projection(
                        reference,
                        &presets,
                        change.replacement_projection.as_ref(),
                    );
                    change.value = Some(value);
                }
            }
            for change in &mut cue.group_changes {
                if change.value.is_some()
                    && let Some(reference) = &change.preset_reference
                    && let Some(value) = super::cue_presets::resolve(reference, &presets, &native)
                {
                    change.replacement_projections =
                        super::cue_presets::replacement_projections(reference, &presets);
                    change.value = Some(value);
                }
            }
        }
    }
    for list in &mut lists {
        if list.required_programming_contract()
            >= light_core::programming::LIVE_PRESET_REFERENCE_CONTRACT
        {
            // Cue-only baselines are derived from current live source values, never stale
            // materialized restoration rows persisted by an earlier source generation.
            light_playback::refresh_cue_only_restorations(list);
        }
        list.validate_programming().map_err(|error| {
            invalid_candidate(format!("invalid cue list {}: {error}", list.id.0))
        })?;
    }
    Ok(lists)
}

pub(super) fn decode_groups(
    candidate: PortableShowCandidate<'_>,
) -> Result<Vec<GroupDefinition>, ActionError> {
    candidate
        .objects_of_kind("group")
        .map(|object| {
            let mut group = serde_json::from_value::<GroupDefinition>(object.body().clone())
                .map_err(|error| {
                    invalid_candidate(format!("invalid group {}: {error}", object.key().id()))
                })?;
            light_core::programming::validate_programming_entries(
                light_core::programming::ProgrammingValueScope::LiveGroup,
                &group.programming,
            )
            .map_err(|error| {
                invalid_candidate(format!("invalid group {}: {error}", object.key().id()))
            })?;
            group.id = object.key().id().to_owned();
            Ok(group)
        })
        .collect()
}

pub(super) fn decode_dynamics(
    candidate: PortableShowCandidate<'_>,
    groups: &[GroupDefinition],
) -> Result<(Vec<light_dynamics::DynamicDefinition>, u16), ActionError> {
    // Dynamic pool objects are operator-repairable content. One malformed or semantically invalid
    // definition must remain visible through the object API without preventing the rest of the
    // active show from compiling. Runtime installation therefore receives only valid definitions.
    let mut dynamics = candidate
        .objects_of_kind("dynamic")
        .filter_map(|object| {
            let definition =
                serde_json::from_value::<light_dynamics::DynamicDefinition>(object.body().clone())
                    .ok()?;
            light_dynamics::validate_definition(&definition)
                .is_ok()
                .then_some(definition)
        })
        .collect::<Vec<_>>();
    let presets = candidate
        .objects_of_kind("preset")
        .map(|object| {
            serde_json::from_value::<Preset>(object.body().clone())
                .map_err(|error| error.to_string())
                .and_then(|preset| {
                    preset
                        .validate_programming()
                        .map_err(|error| error.to_string())?;
                    Ok((object.key().id().to_owned(), preset))
                })
                .map_err(|error| {
                    invalid_candidate(format!("invalid preset {}: {error}", object.key().id()))
                })
        })
        .collect::<Result<HashMap<_, _>, _>>()?;
    let groups = groups
        .iter()
        .cloned()
        .map(|group| (group.id.clone(), group))
        .collect::<HashMap<_, _>>();
    let native = super::dynamic_presets::NativeTemplateValidator::new(candidate);
    for dynamic in &mut dynamics {
        hydrate_dynamic_preset_fallbacks(dynamic, &presets, &groups, &|value| native.allows(value));
    }
    let required = presets
        .values()
        .map(Preset::required_programming_contract)
        .chain(
            dynamics
                .iter()
                .map(light_dynamics::DynamicDefinition::required_programming_contract),
        )
        .chain(
            candidate
                .objects_of_kind("cue_list")
                .chain(candidate.objects_of_kind("group"))
                .map(|object| light_show::required_object_programming_contract(object.body()))
                // Portable literal writers advertise1 even with no semantic values. Here only
                // additive addressing/reference features augment the decoded value requirement.
                .filter(|required| *required > 1),
        )
        .max()
        .unwrap_or(0);
    Ok((dynamics, required))
}

pub(super) fn hydrate_dynamic_preset_fallbacks(
    dynamic: &mut light_dynamics::DynamicDefinition,
    presets: &HashMap<String, Preset>,
    groups: &HashMap<String, GroupDefinition>,
    verify_native: &dyn Fn(&light_core::AttributeValue) -> bool,
) {
    super::dynamic_presets::retain_templates(dynamic, presets, verify_native);
    for common in &mut dynamic.lanes {
        let Some(lane) = common.legacy_mut() else {
            continue;
        };
        for source in lane
            .keyframes
            .points
            .iter_mut()
            .map(|point| &mut point.source)
            .chain([&mut lane.max_min.minimum, &mut lane.max_min.maximum])
            .chain([&mut lane.middle_amplitude.middle])
        {
            hydrate_preset_source(source, presets, groups);
        }
    }
    for group in &mut dynamic.random_groups {
        if let light_dynamics::DynamicRandomRange::LegacyScalar { low, high } = &mut group.range {
            hydrate_preset_source(low, presets, groups);
            hydrate_preset_source(high, presets, groups);
        }
    }
}

fn hydrate_preset_source(
    source: &mut light_dynamics::ScalarSource,
    presets: &HashMap<String, Preset>,
    groups: &HashMap<String, GroupDefinition>,
) {
    let light_dynamics::ScalarSource::Preset {
        preset_id,
        attribute,
        last_valid_by_target,
    } = source
    else {
        return;
    };
    let Some(preset) = presets.get(preset_id) else {
        return;
    };
    let mut values = last_valid_by_target
        .iter()
        .map(|fallback| (fallback.target, fallback.value))
        .collect::<HashMap<_, _>>();
    for (group_id, attributes) in &preset.group_values {
        let Some(value) = attributes
            .get(attribute)
            .and_then(light_core::AttributeValue::normalized)
        else {
            continue;
        };
        if let Ok(targets) = resolve_group(group_id, groups) {
            for target in targets {
                values.insert(target, value);
            }
        }
    }
    for (target, attributes) in &preset.values {
        if let Some(value) = attributes
            .get(attribute)
            .and_then(light_core::AttributeValue::normalized)
        {
            values.insert(*target, value);
        }
    }
    let mut hydrated = values
        .into_iter()
        .map(|(target, value)| light_dynamics::TargetScalarFallback { target, value })
        .collect::<Vec<_>>();
    hydrated.sort_by_key(|fallback| fallback.target.0);
    *last_valid_by_target = hydrated;
}

pub(super) fn decode_dynamic_stage_positions(
    candidate: PortableShowCandidate<'_>,
) -> Result<HashMap<FixtureId, light_dynamics::SpatialPosition>, ActionError> {
    let layouts = decode::<crate::StageLayout>(candidate, "stage_layout")?;
    let mut positions = HashMap::new();
    for layout in layouts {
        positions.extend(layout.fixture_spatial_positions());
    }
    Ok(positions)
}

pub(super) fn supply_playback_defaults(
    cue_lists: &[CueList],
    playbacks: &mut Vec<PlaybackDefinition>,
    pages: &mut Vec<PlaybackPage>,
) {
    // Only a genuinely legacy document with no Playback topology receives seeded controls.
    // A persisted page with no definitions is an intentional empty topology (for example after
    // clearing the final assignment) and must not recreate the detached target.
    if playbacks.is_empty()
        && pages.is_empty()
        && !cue_lists
            .iter()
            .any(|list| list.pool_number.is_some() || !list.legacy_pool_aliases.is_empty())
    {
        playbacks.extend(
            cue_lists
                .iter()
                .take(1_000)
                .enumerate()
                .map(default_playback),
        );
    }
    if pages.is_empty() {
        pages.push(PlaybackPage {
            number: 1,
            name: "Main".into(),
            slots: HashMap::new(),
            virtual_playbacks: HashMap::new(),
        });
    }
}

fn default_playback((index, cue_list): (usize, &CueList)) -> PlaybackDefinition {
    PlaybackDefinition {
        number: index as u16 + 1,
        name: cue_list.name.clone(),
        target: PlaybackTarget::CueList {
            cue_list_id: cue_list.id,
        },
        buttons: [
            PlaybackButtonAction::GoMinus,
            PlaybackButtonAction::Go,
            PlaybackButtonAction::Flash,
        ],
        button_count: 3,
        fader: PlaybackFaderMode::Master,
        has_fader: true,
        footprint: light_playback::PlaybackFootprint::Normal,
        go_activates: true,
        auto_off: true,
        xfade_millis: 0,
        color: "#20c997".into(),
        flash_release: FlashReleaseMode::default(),
        protect_from_swap: false,
        presentation_icon: None,
        presentation_image: None,
    }
}
