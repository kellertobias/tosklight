use crate::{ActionContext, ApplicationCommand, CommandFamily};
use light_core::{Revision, ShowId};
use light_dynamics::DynamicDefinition;
use light_output::OutputRoute;
use light_playback::{CueList, PlaybackDefinition, PlaybackPage};
use light_programmer::{GroupDefinition, Preset};
use light_show::{LosslessBody, PortableJson, PortableShowRevision};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Portable show-object families whose runtime semantics are owned by the active-show boundary.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ActiveShowObjectKind {
    CueList,
    Dynamic,
    Group,
    PatchLayer,
    Playback,
    PlaybackPage,
    Preset,
    Schedule,
    StageLayout,
    UserLayout,
}

/// One decoded active-show body. The enum is the application contract: a mutation cannot pair a
/// known storage kind with another family's body. Exact extensible JSON remains encapsulated by
/// `light-show::LosslessBody`.
#[derive(Clone, Debug, PartialEq)]
pub enum ActiveShowObjectBody {
    CueList(LosslessBody<CueList>),
    Dynamic(LosslessBody<DynamicDefinition>),
    Group(LosslessBody<GroupDefinition>),
    PatchLayer(LosslessBody<PatchLayer>),
    Playback(LosslessBody<PlaybackDefinition>),
    PlaybackPage(LosslessBody<PlaybackPage>),
    Preset(LosslessBody<Preset>),
    Schedule(LosslessBody<crate::ScheduleDefinition>),
    StageLayout(LosslessBody<StageLayout>),
    UserLayout(LosslessBody<UserLayout>),
}

impl ActiveShowObjectBody {
    pub fn decode(kind: ActiveShowObjectKind, raw: serde_json::Value) -> serde_json::Result<Self> {
        validate_family_shape(kind, &raw)?;
        Ok(match kind {
            ActiveShowObjectKind::CueList => Self::CueList(LosslessBody::decode(raw)?),
            ActiveShowObjectKind::Dynamic => Self::Dynamic(LosslessBody::decode(raw)?),
            ActiveShowObjectKind::Group => Self::Group(LosslessBody::decode(raw)?),
            ActiveShowObjectKind::PatchLayer => Self::PatchLayer(LosslessBody::decode(raw)?),
            ActiveShowObjectKind::Playback => Self::Playback(LosslessBody::decode(raw)?),
            ActiveShowObjectKind::PlaybackPage => Self::PlaybackPage(LosslessBody::decode(raw)?),
            ActiveShowObjectKind::Preset => Self::Preset(LosslessBody::decode(raw)?),
            ActiveShowObjectKind::Schedule => Self::Schedule(LosslessBody::decode(raw)?),
            ActiveShowObjectKind::StageLayout => Self::StageLayout(LosslessBody::decode(raw)?),
            ActiveShowObjectKind::UserLayout => Self::UserLayout(LosslessBody::decode(raw)?),
        })
    }

    pub const fn kind(&self) -> ActiveShowObjectKind {
        match self {
            Self::CueList(_) => ActiveShowObjectKind::CueList,
            Self::Dynamic(_) => ActiveShowObjectKind::Dynamic,
            Self::Group(_) => ActiveShowObjectKind::Group,
            Self::PatchLayer(_) => ActiveShowObjectKind::PatchLayer,
            Self::Playback(_) => ActiveShowObjectKind::Playback,
            Self::PlaybackPage(_) => ActiveShowObjectKind::PlaybackPage,
            Self::Preset(_) => ActiveShowObjectKind::Preset,
            Self::Schedule(_) => ActiveShowObjectKind::Schedule,
            Self::StageLayout(_) => ActiveShowObjectKind::StageLayout,
            Self::UserLayout(_) => ActiveShowObjectKind::UserLayout,
        }
    }

    pub fn encode(&self) -> serde_json::Value {
        match self {
            Self::CueList(body) => body.encode(),
            Self::Dynamic(body) => body.encode(),
            Self::Group(body) => body.encode(),
            Self::PatchLayer(body) => body.encode(),
            Self::Playback(body) => body.encode(),
            Self::PlaybackPage(body) => body.encode(),
            Self::Preset(body) => body.encode(),
            Self::Schedule(body) => body.encode(),
            Self::StageLayout(body) => body.encode(),
            Self::UserLayout(body) => body.encode(),
        }
    }

    pub(crate) fn cue_list(&self) -> Option<&LosslessBody<CueList>> {
        match self {
            Self::CueList(body) => Some(body),
            _ => None,
        }
    }

    pub(crate) fn dynamic(&self) -> Option<&LosslessBody<DynamicDefinition>> {
        match self {
            Self::Dynamic(body) => Some(body),
            _ => None,
        }
    }

    pub(crate) fn group(&self) -> Option<&LosslessBody<GroupDefinition>> {
        match self {
            Self::Group(body) => Some(body),
            _ => None,
        }
    }

    pub(crate) fn patch_layer(&self) -> Option<&LosslessBody<PatchLayer>> {
        match self {
            Self::PatchLayer(body) => Some(body),
            _ => None,
        }
    }

    pub(crate) fn playback(&self) -> Option<&LosslessBody<PlaybackDefinition>> {
        match self {
            Self::Playback(body) => Some(body),
            _ => None,
        }
    }

    pub(crate) fn playback_page(&self) -> Option<&LosslessBody<PlaybackPage>> {
        match self {
            Self::PlaybackPage(body) => Some(body),
            _ => None,
        }
    }

    pub(crate) fn preset(&self) -> Option<&LosslessBody<Preset>> {
        match self {
            Self::Preset(body) => Some(body),
            _ => None,
        }
    }

    pub(crate) fn schedule(&self) -> Option<&LosslessBody<crate::ScheduleDefinition>> {
        match self {
            Self::Schedule(body) => Some(body),
            _ => None,
        }
    }

    pub(crate) fn stage_layout(&self) -> Option<&LosslessBody<StageLayout>> {
        match self {
            Self::StageLayout(body) => Some(body),
            _ => None,
        }
    }

    pub(crate) fn user_layout(&self) -> Option<&LosslessBody<UserLayout>> {
        match self {
            Self::UserLayout(body) => Some(body),
            _ => None,
        }
    }
}

fn validate_family_shape(
    kind: ActiveShowObjectKind,
    raw: &serde_json::Value,
) -> serde_json::Result<()> {
    let object = raw.as_object().ok_or_else(|| {
        <serde_json::Error as serde::de::Error>::custom(format!(
            "{} body must be an object",
            kind.as_str()
        ))
    })?;
    if kind == ActiveShowObjectKind::UserLayout {
        let has_desks = object.contains_key("desks");
        let has_active_desk = object.contains_key("activeDeskId");
        if has_desks && has_active_desk {
            return Ok(());
        }
        if !has_desks && !has_active_desk && !looks_like_other_family(object) {
            // Early show files permitted opaque per-user layout payloads. They remain readable and
            // lossless, while bodies recognizable as another supported family are still rejected.
            return Ok(());
        }
    }
    let required: &[&str] = match kind {
        ActiveShowObjectKind::CueList => &["id", "name", "cues"],
        ActiveShowObjectKind::Dynamic => &[
            "id",
            "pool_number",
            "revision",
            "name",
            "target_binding",
            "lanes",
            "phase",
            "speed",
            "default_activation",
        ],
        // Legacy/group recording clients omit `id`; normalization supplies the storage identity.
        ActiveShowObjectKind::Group => &["name", "fixtures"],
        ActiveShowObjectKind::PatchLayer => &["id", "name", "order"],
        ActiveShowObjectKind::Playback => &["number", "name", "target"],
        ActiveShowObjectKind::PlaybackPage => &["number", "name", "slots", "virtual_playbacks"],
        // `values` was absent from early empty Presets and is defaulted by the typed model.
        ActiveShowObjectKind::Preset => &["name", "family", "number"],
        ActiveShowObjectKind::Schedule => &["id", "name", "enabled", "trigger", "target"],
        ActiveShowObjectKind::StageLayout => &["positions"],
        ActiveShowObjectKind::UserLayout => &["desks", "activeDeskId"],
    };
    if let Some(field) = required.iter().find(|field| !object.contains_key(**field)) {
        return Err(<serde_json::Error as serde::de::Error>::custom(format!(
            "{} body is missing required field {field}",
            kind.as_str()
        )));
    }
    Ok(())
}

fn looks_like_other_family(object: &serde_json::Map<String, serde_json::Value>) -> bool {
    [
        &["id", "name", "cues"][..],
        &["id", "pool_number", "revision", "target_binding", "lanes"],
        &["name", "fixtures"],
        &["name", "order"],
        &["number", "name", "target"],
        &["number", "name", "slots", "virtual_playbacks"],
        &["name", "family", "number"],
        &["id", "name", "enabled", "trigger", "target"],
        &["positions"],
    ]
    .into_iter()
    .any(|required| required.iter().all(|field| object.contains_key(*field)))
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PatchLayer {
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub order: i32,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct StageLayout {
    #[serde(default)]
    pub version: u64,
    #[serde(default)]
    pub positions: HashMap<String, StagePosition2d>,
    #[serde(default, rename = "positions3d")]
    pub positions_3d: HashMap<String, StagePosition3d>,
    #[serde(default, rename = "camera3d", skip_serializing_if = "Option::is_none")]
    pub camera_3d: Option<StageCamera3d>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct StagePosition2d {
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub rotation: f64,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StagePosition3d {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    #[serde(default)]
    pub rotation_x: f64,
    #[serde(default)]
    pub rotation_y: f64,
    #[serde(default)]
    pub rotation_z: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct StageCamera3d {
    pub position: [f64; 3],
    pub target: [f64; 3],
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserLayout {
    #[serde(default)]
    pub desks: Vec<PortableJson>,
    #[serde(default)]
    pub active_desk_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_settings: Option<PortableJson>,
}

impl ActiveShowObjectKind {
    pub fn from_storage_kind(kind: &str) -> Option<Self> {
        match kind {
            "cue_list" => Some(Self::CueList),
            "dynamic" => Some(Self::Dynamic),
            "group" => Some(Self::Group),
            "patch_layer" => Some(Self::PatchLayer),
            "playback" => Some(Self::Playback),
            "playback_page" => Some(Self::PlaybackPage),
            "preset" => Some(Self::Preset),
            "schedule" => Some(Self::Schedule),
            "stage_layout" => Some(Self::StageLayout),
            "user_layout" => Some(Self::UserLayout),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CueList => "cue_list",
            Self::Dynamic => "dynamic",
            Self::Group => "group",
            Self::PatchLayer => "patch_layer",
            Self::Playback => "playback",
            Self::PlaybackPage => "playback_page",
            Self::Preset => "preset",
            Self::Schedule => "schedule",
            Self::StageLayout => "stage_layout",
            Self::UserLayout => "user_layout",
        }
    }
}

/// One optimistic show-object edit within a whole-show transaction.
#[derive(Clone, Debug, PartialEq)]
pub struct ActiveShowObjectMutation {
    pub kind: ActiveShowObjectKind,
    pub object_id: String,
    pub expected_object_revision: Revision,
    pub mutation: ActiveShowObjectMutationKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ActiveShowObjectMutationKind {
    Put { body: ActiveShowObjectBody },
    Delete,
}

/// One atomic batch of active-show object edits.
#[derive(Clone, Debug, PartialEq)]
pub struct MutateActiveShowObjectsCommand {
    pub show_id: ShowId,
    pub mutations: Vec<ActiveShowObjectMutation>,
}

impl ApplicationCommand for MutateActiveShowObjectsCommand {
    type Value = MutateActiveShowObjectsResult;

    const FAMILY: CommandFamily = CommandFamily::Show;
}

#[derive(Clone, Debug, PartialEq)]
pub struct ActiveShowObjectChange {
    pub kind: ActiveShowObjectKind,
    pub object_id: String,
    pub object_revision: Revision,
    pub body: Option<ActiveShowObjectBody>,
    pub deleted: bool,
}

impl ActiveShowObjectChange {
    pub fn present(
        kind: ActiveShowObjectKind,
        object_id: String,
        object_revision: Revision,
        raw: serde_json::Value,
    ) -> serde_json::Result<Self> {
        Ok(Self {
            kind,
            object_id,
            object_revision,
            body: Some(ActiveShowObjectBody::decode(kind, raw)?),
            deleted: false,
        })
    }

    pub fn deleted(
        kind: ActiveShowObjectKind,
        object_id: String,
        object_revision: Revision,
    ) -> Self {
        Self {
            kind,
            object_id,
            object_revision,
            body: None,
            deleted: true,
        }
    }
}

/// One committed semantic batch of active-show object changes.
#[derive(Clone, Debug, PartialEq)]
pub struct ActiveShowObjectsChange {
    pub show_id: ShowId,
    pub show_revision: PortableShowRevision,
    pub changes: Vec<ActiveShowObjectChange>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MutateActiveShowObjectsResult {
    pub context: ActionContext,
    pub show_revision: PortableShowRevision,
    pub changes: Vec<ActiveShowObjectChange>,
    /// Compatibility-migration write-backs committed alongside the requested changes. Reported so
    /// adapters can publish their revision bumps instead of leaving them silent.
    pub migration_changes: Vec<ActiveShowObjectChange>,
    /// Route-kind compatibility-migration write-backs committed alongside the request.
    pub migrated_routes: Vec<OutputRouteChange>,
    pub event_sequence: u64,
}

/// Restores the latest retained version of one object in the active portable show.
#[derive(Clone, Debug, PartialEq)]
pub struct UndoActiveShowObjectCommand {
    pub show_id: ShowId,
    pub kind: ActiveShowObjectKind,
    pub object_id: String,
    pub expected_object_revision: Revision,
}

impl ApplicationCommand for UndoActiveShowObjectCommand {
    type Value = UndoActiveShowObjectResult;

    const FAMILY: CommandFamily = CommandFamily::Show;
}

#[derive(Clone, Debug, PartialEq)]
pub struct UndoActiveShowObjectResult {
    pub context: ActionContext,
    pub show_revision: PortableShowRevision,
    pub change: ActiveShowObjectChange,
    /// Compatibility-migration write-backs committed alongside the undone object. Reported so
    /// adapters can publish their revision bumps instead of leaving them silent.
    pub migration_changes: Vec<ActiveShowObjectChange>,
    /// Route-kind compatibility-migration write-backs committed alongside the undo.
    pub migrated_routes: Vec<OutputRouteChange>,
    pub event_sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UndoActiveShowRecordingOperation {
    RestorePrevious,
    DeleteCreated,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UndoActiveShowRecordingObject {
    pub kind: ActiveShowObjectKind,
    pub object_id: String,
    pub expected_object_revision: Revision,
    pub operation: UndoActiveShowRecordingOperation,
}

/// Reverses every portable object changed by one programmer recording in one transaction.
#[derive(Clone, Debug, PartialEq)]
pub struct UndoActiveShowRecordingCommand {
    pub show_id: ShowId,
    pub objects: Vec<UndoActiveShowRecordingObject>,
}

impl ApplicationCommand for UndoActiveShowRecordingCommand {
    type Value = MutateActiveShowObjectsResult;

    const FAMILY: CommandFamily = CommandFamily::Show;
}

/// One typed output-route edit performed against the active portable show.
#[derive(Clone, Debug, PartialEq)]
pub struct MutateOutputRouteCommand {
    pub show_id: ShowId,
    pub route_id: String,
    /// Compatibility revision from the v1 object endpoint. The application service still commits
    /// the complete candidate against the document's whole-show revision.
    pub expected_object_revision: Revision,
    pub mutation: OutputRouteMutation,
}

impl ApplicationCommand for MutateOutputRouteCommand {
    type Value = MutateOutputRouteResult;

    const FAMILY: CommandFamily = CommandFamily::Output;
}

#[derive(Clone, Debug, PartialEq)]
pub enum OutputRouteMutation {
    Put { body: LosslessBody<OutputRoute> },
    Delete,
}

/// Targeted active-show projection published after one committed route edit.
#[derive(Clone, Debug, PartialEq)]
pub struct OutputRouteChange {
    pub show_id: ShowId,
    pub show_revision: PortableShowRevision,
    pub route_id: String,
    pub object_revision: Revision,
    pub route: Option<OutputRoute>,
    pub deleted: bool,
}

/// Mutation result plus the one old network route requiring targeted termination, if any.
#[derive(Clone, Debug, PartialEq)]
pub struct MutateOutputRouteResult {
    pub context: ActionContext,
    pub change: OutputRouteChange,
    /// Compatibility-migration write-backs of typed show objects committed alongside the route.
    pub migration_changes: Vec<ActiveShowObjectChange>,
    /// Compatibility-migration write-backs of other routes committed alongside the request.
    pub migrated_routes: Vec<OutputRouteChange>,
    pub route_to_terminate: Option<OutputRoute>,
    pub event_sequence: u64,
}
