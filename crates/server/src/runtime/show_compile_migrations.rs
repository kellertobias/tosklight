use super::*;

pub(super) fn reconcile_show_logical_heads(entry: &ShowEntry) -> Result<(), String> {
    let store = ShowStore::open(&entry.path).map_err(|error| error.to_string())?;
    for object in store
        .objects("patched_fixture")
        .map_err(|error| error.to_string())?
    {
        let mut fixture = serde_json::from_value::<light_fixture::PatchedFixture>(object.body)
            .map_err(|error| error.to_string())?;
        if light_fixture::reconcile_logical_heads(&mut fixture) {
            store
                .put_object(
                    "patched_fixture",
                    &object.id,
                    &serde_json::to_value(fixture).map_err(|error| error.to_string())?,
                    object.revision,
                )
                .map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}
pub(super) fn reconcile_show_cue_identities(entry: &ShowEntry) -> Result<(), String> {
    let store = ShowStore::open(&entry.path).map_err(|error| error.to_string())?;
    for object in store
        .objects("cue_list")
        .map_err(|error| error.to_string())?
    {
        let missing_identity = object
            .body
            .get("cues")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|cues| {
                cues.iter()
                    .any(|cue| cue.get("id").and_then(serde_json::Value::as_str).is_none())
            });
        if !missing_identity {
            continue;
        }
        let cue_list = serde_json::from_value::<light_playback::CueList>(object.body)
            .map_err(|error| error.to_string())?;
        store
            .put_object(
                "cue_list",
                &object.id,
                &serde_json::to_value(cue_list).map_err(|error| error.to_string())?,
                object.revision,
            )
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
pub(super) fn compile_active_show_for_startup(
    engine: &Engine,
    entry: &ShowEntry,
) -> Option<String> {
    load_engine_snapshot(entry)
        .and_then(|snapshot| engine.replace_snapshot(snapshot).map_err(|error| error.to_string()))
        .err()
        .map(|error| format!("The active show '{}' could not be loaded and might be corrupted or incompatible: {error}", entry.name))
}
pub(super) fn load_engine_snapshot_with_override(
    entry: &ShowEntry,
    override_value: Option<(&str, &str, &serde_json::Value)>,
) -> Result<EngineSnapshot, String> {
    let store = ShowStore::open(&entry.path).map_err(|error| error.to_string())?;
    let mut revision = entry.revision;
    let mut read_kind = |kind: &str| -> Result<Vec<light_show::VersionedObject>, String> {
        let mut objects = store.objects(kind).map_err(|error| error.to_string())?;
        if let Some((override_kind, override_id, body)) = override_value
            && override_kind == kind
        {
            if let Some(object) = objects.iter_mut().find(|object| object.id == override_id) {
                object.body = body.clone();
            } else {
                objects.push(light_show::VersionedObject {
                    kind: kind.into(),
                    id: override_id.into(),
                    body: body.clone(),
                    revision: 0,
                    updated_at: String::new(),
                });
            }
        }
        revision = revision.max(
            objects
                .iter()
                .map(|object| object.revision)
                .max()
                .unwrap_or(0),
        );
        Ok(objects)
    };
    let fixtures = read_kind("patched_fixture")?
        .into_iter()
        .map(|object| serde_json::from_value(object.body))
        .collect::<Result<Vec<light_fixture::PatchedFixture>, _>>()
        .map_err(|error| format!("invalid patched fixture: {error}"))?;
    let cue_lists = read_kind("cue_list")?
        .into_iter()
        .map(|object| serde_json::from_value(object.body))
        .collect::<Result<Vec<light_playback::CueList>, _>>()
        .map_err(|error| format!("invalid cue list: {error}"))?;
    let mut playbacks = read_kind("playback")?
        .into_iter()
        .map(|object| serde_json::from_value(object.body))
        .collect::<Result<Vec<light_playback::PlaybackDefinition>, _>>()
        .map_err(|error| format!("invalid playback: {error}"))?;
    let mut playback_pages = read_kind("playback_page")?
        .into_iter()
        .map(|object| serde_json::from_value(object.body))
        .collect::<Result<Vec<light_playback::PlaybackPage>, _>>()
        .map_err(|error| format!("invalid playback page: {error}"))?;
    let routes = read_kind("route")?
        .into_iter()
        .map(|object| serde_json::from_value(object.body))
        .collect::<Result<Vec<light_output::OutputRoute>, _>>()
        .map_err(|error| format!("invalid output route: {error}"))?;
    let control_mappings = read_kind("control_mapping")?
        .into_iter()
        .map(|object| serde_json::from_value(object.body))
        .collect::<Result<Vec<light_control::ControlMapping>, _>>()
        .map_err(|error| format!("invalid control mapping: {error}"))?;
    let groups = read_kind("group")?
        .into_iter()
        .map(|object| {
            serde_json::from_value::<light_programmer::GroupDefinition>(object.body).map(
                |mut group| {
                    group.id = object.id;
                    group
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("invalid group: {error}"))?;
    if playbacks.is_empty() {
        playbacks = cue_lists
            .iter()
            .take(1_000)
            .enumerate()
            .map(|(index, cue)| light_playback::PlaybackDefinition {
                number: index as u16 + 1,
                name: cue.name.clone(),
                target: light_playback::PlaybackTarget::CueList {
                    cue_list_id: cue.id,
                },
                buttons: [
                    light_playback::PlaybackButtonAction::GoMinus,
                    light_playback::PlaybackButtonAction::Go,
                    light_playback::PlaybackButtonAction::Flash,
                ],
                button_count: 3,
                fader: light_playback::PlaybackFaderMode::Master,
                has_fader: true,
                go_activates: true,
                auto_off: true,
                xfade_millis: 0,
                color: "#20c997".into(),
                flash_release: light_playback::FlashReleaseMode::default(),
                protect_from_swap: false,
                presentation_icon: None,
                presentation_image: None,
            })
            .collect();
    }
    if playback_pages.is_empty() {
        playback_pages.push(light_playback::PlaybackPage {
            number: 1,
            name: "Main".into(),
            slots: HashMap::new(),
        });
    }
    Ok(EngineSnapshot {
        fixtures,
        cue_lists,
        playbacks,
        playback_pages,
        routes,
        control_mappings,
        groups,
        revision,
    })
}
