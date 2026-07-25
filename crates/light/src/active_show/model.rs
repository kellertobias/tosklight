use crate::{ActionContext, ApplicationCommand, CommandFamily};
use light_core::{Revision, ShowId};
use light_output::OutputRoute;
use light_show::PortableShowRevision;
use serde_json::Value;

/// Portable show-object families whose runtime semantics are owned by the active-show boundary.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ActiveShowObjectKind {
    CueList,
    Group,
    PatchLayer,
    Playback,
    PlaybackPage,
    Preset,
    StageLayout,
    UserLayout,
}

impl ActiveShowObjectKind {
    pub fn from_storage_kind(kind: &str) -> Option<Self> {
        match kind {
            "cue_list" => Some(Self::CueList),
            "group" => Some(Self::Group),
            "patch_layer" => Some(Self::PatchLayer),
            "playback" => Some(Self::Playback),
            "playback_page" => Some(Self::PlaybackPage),
            "preset" => Some(Self::Preset),
            "stage_layout" => Some(Self::StageLayout),
            "user_layout" => Some(Self::UserLayout),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CueList => "cue_list",
            Self::Group => "group",
            Self::PatchLayer => "patch_layer",
            Self::Playback => "playback",
            Self::PlaybackPage => "playback_page",
            Self::Preset => "preset",
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
    Put { body: Value },
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
    pub body: Option<Value>,
    pub deleted: bool,
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
    Put { body: Value },
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
