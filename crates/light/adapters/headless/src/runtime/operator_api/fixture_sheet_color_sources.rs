//! Preset names describe proven authorship, never equality with a stored color value.
use super::*;
use light_engine::{
    ContributionFamilyRole, ContributionProgrammerLane, ContributionSourceDescriptor,
};
type Groups = HashMap<String, light_programmer::GroupDefinition>;

pub(super) fn color_preset_names(
    state: &AppState,
    session: &Session,
    preload: bool,
    sources: &light_engine::FrameValues,
    fixture_ids: Option<&HashSet<Uuid>>,
) -> Vec<light_wire::v2::visualization::FixtureSheetColorPresetName> {
    let snapshot = state.output.snapshot();
    let programmers = state
        .sessions
        .sessions()
        .into_iter()
        .filter_map(|session| state.programming.get(session.id))
        .map(|programmer| (programmer.id, programmer))
        .collect::<HashMap<_, _>>();
    let groups = snapshot
        .groups
        .iter()
        .map(|group| (group.id.clone(), group.clone()))
        .collect::<Groups>();
    let mut overrides = if preload {
        state
            .programming
            .get(session.id)
            .map(|programmer| preload_references(&programmer, &groups))
            .unwrap_or_default()
    } else {
        HashMap::new()
    };
    let color = light_core::AttributeKey::color();
    let mut identities = HashMap::new();
    for (fixture, attribute) in sources.values().keys() {
        if attribute != &color || fixture_ids.is_some_and(|ids| !ids.contains(&fixture.0)) {
            continue;
        }
        let identity = overrides.remove(fixture).unwrap_or_else(|| {
            authored_preset(sources, *fixture, &color, &programmers, &snapshot, &groups)
        });
        if let Some(identity) = identity {
            identities.insert(*fixture, identity);
        }
    }
    // A pending-only Color has no Normal entry to visit above.
    for (fixture, identity) in overrides {
        if let Some(identity) = identity
            && fixture_ids.is_none_or(|ids| ids.contains(&fixture.0))
        {
            identities.insert(fixture, identity);
        }
    }
    if identities.is_empty() {
        return Vec::new();
    }
    let names = active_show_store(state)
        .ok()
        .and_then(|(_, store)| store.objects("preset").ok())
        .unwrap_or_default()
        .iter()
        .filter_map(|object| decode_preset_object(object).ok())
        .filter_map(|(_, preset)| preset.instance_id.map(|id| (id, preset.name)))
        .collect::<HashMap<_, _>>();
    identities
        .into_iter()
        .filter_map(|(fixture, identity)| {
            Some(light_wire::v2::visualization::FixtureSheetColorPresetName {
                fixture_id: fixture.0,
                preset_instance_id: identity,
                name: names.get(&identity)?.clone(),
            })
        })
        .collect()
}

fn preload_references(
    programmer: &light_programmer::ProgrammerState,
    groups: &Groups,
) -> HashMap<light_core::FixtureId, Option<Uuid>> {
    let reference = |order| {
        programmer
            .preset_provenance
            .get(&order)
            .map(|reference| reference.preset_instance_id)
    };
    let mut overrides = HashMap::new();
    for value in programmer
        .preload_active
        .iter()
        .chain(&programmer.preload_pending)
    {
        if value.attribute.0.as_ref() == "color" {
            overrides.insert(value.fixture_id, reference(value.programmer_order));
        }
    }
    for (group, attributes) in programmer
        .preload_group_active
        .iter()
        .chain(&programmer.preload_group_pending)
    {
        for (attribute, value) in attributes {
            if attribute.0.as_ref() != "color" {
                continue;
            }
            if let Ok(fixtures) = light_programmer::resolve_group(group, groups) {
                for fixture in fixtures {
                    overrides.insert(fixture, reference(value.programmer_order));
                }
            }
        }
    }
    overrides
}

fn authored_preset(
    sources: &light_engine::FrameValues,
    fixture: light_core::FixtureId,
    color: &light_core::AttributeKey,
    programmers: &HashMap<light_core::ProgrammerId, light_programmer::ProgrammerState>,
    snapshot: &EngineSnapshot,
    groups: &Groups,
) -> Option<Uuid> {
    let origin = sources.contribution_origin(fixture, color)?;
    let evidence = sources.contribution_family_evidence(fixture, color)?;
    one_preset(
        evidence
            .entries()
            .iter()
            .filter(|entry| entry.role() == ContributionFamilyRole::Authored)
            .map(|entry| match entry.source().descriptor() {
                ContributionSourceDescriptor::Programmer {
                    programmer_id,
                    lane,
                } => {
                    if matches!(lane, ContributionProgrammerLane::Transient(_)) {
                        return None;
                    }
                    programmers
                        .get(&programmer_id)?
                        .preset_provenance
                        .get(&entry.stamp().programmer_order)
                        .map(|reference| reference.preset_instance_id)
                }
                ContributionSourceDescriptor::Playback(source) => {
                    let cue = snapshot
                        .cue_lists
                        .iter()
                        .find(|list| list.id == source.cue_list_id)?
                        .cues
                        .iter()
                        .find(|cue| Some(cue.id) == entry.authored_cue_id())?;
                    cue_preset(cue, origin.authored_fixture_id(), color, groups)
                }
            }),
    )
}

fn cue_preset(
    cue: &light_playback::Cue,
    authored: light_core::FixtureId,
    color: &light_core::AttributeKey,
    groups: &Groups,
) -> Option<Uuid> {
    // A retained trace identifies the Cue and authored fixture, but not the original Group.
    // When that Cue carries conflicting Direct/Group origins, keep the color estimate rather
    // than inventing which Preset won from equal numeric values.
    let direct = cue
        .changes
        .iter()
        .filter(|change| change.fixture_id == authored && &change.attribute == color)
        .map(|change| {
            change
                .preset_reference
                .as_ref()
                .map(|reference| reference.preset_instance_id)
        });
    let group = cue
        .group_changes
        .iter()
        .filter(|change| {
            &change.attribute == color
                && light_programmer::resolve_group(&change.group_id, groups)
                    .is_ok_and(|members| members.contains(&authored))
        })
        .map(|change| {
            change
                .preset_reference
                .as_ref()
                .map(|reference| reference.preset_instance_id)
        });
    one_preset(direct.chain(group))
}

/// All authored fields must retain the same explicit reference. One literal edit clears the name.
fn one_preset(references: impl IntoIterator<Item = Option<Uuid>>) -> Option<Uuid> {
    let mut references = references.into_iter();
    let first = references.next()??;
    references
        .all(|reference| reference == Some(first))
        .then_some(first)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authored_literal_or_mixed_presets_never_receive_a_preset_name() {
        let a = Uuid::from_u128(1);
        let b = Uuid::from_u128(2);
        assert_eq!(one_preset([]), None);
        assert_eq!(one_preset([Some(a), None]), None);
        assert_eq!(one_preset([None, Some(a)]), None);
        assert_eq!(one_preset([Some(a), Some(b)]), None);
        assert_eq!(one_preset([Some(a), Some(a)]), Some(a));
    }
}
