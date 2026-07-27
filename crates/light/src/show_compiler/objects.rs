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
            group.id = object.key().id().to_owned();
            Ok(group)
        })
        .collect()
}

pub(super) fn decode_dynamics(
    candidate: PortableShowCandidate<'_>,
    groups: &[GroupDefinition],
) -> Result<Vec<light_dynamics::DynamicDefinition>, ActionError> {
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
                .map(|preset| (object.key().id().to_owned(), preset))
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
    for dynamic in &mut dynamics {
        hydrate_dynamic_preset_fallbacks(dynamic, &presets, &groups);
    }
    Ok(dynamics)
}

fn hydrate_dynamic_preset_fallbacks(
    dynamic: &mut light_dynamics::DynamicDefinition,
    presets: &HashMap<String, Preset>,
    groups: &HashMap<String, GroupDefinition>,
) {
    for lane in &mut dynamic.lanes {
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
        hydrate_preset_source(&mut group.low, presets, groups);
        hydrate_preset_source(&mut group.high, presets, groups);
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
        for (id, position) in layout.positions {
            let Ok(id) = uuid::Uuid::parse_str(&id) else {
                continue;
            };
            positions.insert(
                FixtureId(id),
                light_dynamics::SpatialPosition {
                    x: position.x as f32,
                    z: position.y as f32,
                },
            );
        }
        for (id, position) in layout.positions_3d {
            let Ok(id) = uuid::Uuid::parse_str(&id) else {
                continue;
            };
            positions.insert(
                FixtureId(id),
                light_dynamics::SpatialPosition {
                    x: position.x as f32,
                    z: position.z as f32,
                },
            );
        }
    }
    Ok(positions)
}

pub(super) fn supply_playback_defaults(
    cue_lists: &[CueList],
    playbacks: &mut Vec<PlaybackDefinition>,
    pages: &mut Vec<PlaybackPage>,
) {
    if playbacks.is_empty() {
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
