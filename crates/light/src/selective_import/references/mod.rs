mod descriptors;
mod fixtures;
mod locations;
mod rewrite;

use super::ImportObjectDescriptor;
use light_show::PortableShowObject;

pub(super) use fixtures::FixtureIdentityCatalog;
pub(super) use rewrite::{IdentityMap, ProfileMap, rewrite_body};

#[derive(Clone, Copy)]
enum RegisteredObjectKind {
    Fixture,
    PatchLayer,
    Group,
    Dynamic,
    Preset,
    CueList,
    Playback,
    PlaybackPage,
    Schedule,
    StageLayout,
    ControlMapping,
    Route,
    DependencyList,
    Effect,
    ManagedAsset,
}

impl RegisteredObjectKind {
    fn from_name(kind: &str) -> Option<Self> {
        match kind {
            "fixture" | "patched_fixture" => Some(Self::Fixture),
            "patch_layer" => Some(Self::PatchLayer),
            "group" => Some(Self::Group),
            "dynamic" => Some(Self::Dynamic),
            "preset" => Some(Self::Preset),
            "cue_list" => Some(Self::CueList),
            "playback" => Some(Self::Playback),
            "playback_page" => Some(Self::PlaybackPage),
            "schedule" => Some(Self::Schedule),
            "stage_layout" => Some(Self::StageLayout),
            "control_mapping" => Some(Self::ControlMapping),
            "route" => Some(Self::Route),
            // These exact shapes retain the early prototype fixtures while capability adapters can
            // supply descriptors for their finalized schemas.
            "macro" | "timecode" => Some(Self::DependencyList),
            "effect" => Some(Self::Effect),
            "managed_asset" => Some(Self::ManagedAsset),
            _ => None,
        }
    }
}

pub(super) fn is_registered_object_kind(kind: &str) -> bool {
    RegisteredObjectKind::from_name(kind).is_some()
}

pub(super) fn registered_descriptor(
    object: &PortableShowObject,
    source_fixtures: &FixtureIdentityCatalog,
    target_fixtures: &FixtureIdentityCatalog,
) -> Result<Option<ImportObjectDescriptor>, String> {
    let Some(kind) = RegisteredObjectKind::from_name(object.key().kind()) else {
        return Ok(None);
    };
    let descriptor = match kind {
        RegisteredObjectKind::Fixture => fixtures::fixture_descriptor(object)?,
        RegisteredObjectKind::PatchLayer => descriptors::patch_layer_descriptor(object)?,
        RegisteredObjectKind::Group => {
            descriptors::group_descriptor(object, source_fixtures, target_fixtures)?
        }
        RegisteredObjectKind::Dynamic => {
            descriptors::dynamic_descriptor(object, source_fixtures, target_fixtures)?
        }
        RegisteredObjectKind::Preset => {
            descriptors::preset_descriptor(object, source_fixtures, target_fixtures)?
        }
        RegisteredObjectKind::CueList => {
            descriptors::cue_list_descriptor(object, source_fixtures, target_fixtures)?
        }
        RegisteredObjectKind::Playback => {
            descriptors::playback_descriptor(object, source_fixtures, target_fixtures)?
        }
        RegisteredObjectKind::PlaybackPage => descriptors::playback_page_descriptor(object)?,
        RegisteredObjectKind::Schedule => descriptors::schedule_descriptor(object)?,
        RegisteredObjectKind::StageLayout => {
            descriptors::stage_layout_descriptor(object, source_fixtures, target_fixtures)?
        }
        RegisteredObjectKind::ControlMapping => descriptors::control_mapping_descriptor(object)?,
        RegisteredObjectKind::Route => locations::key_only_descriptor(object),
        RegisteredObjectKind::DependencyList => descriptors::dependency_list_descriptor(object)?,
        RegisteredObjectKind::Effect => descriptors::effect_descriptor(object)?,
        RegisteredObjectKind::ManagedAsset => locations::id_descriptor(object)?,
    };
    Ok(Some(descriptor))
}
