use super::{ActiveCueContext, UpdateMode, UpdatePreview, UpdateResult, UpdateTargetFilter};
use crate::{
    ActionContext, ActionError, ActiveShowObjectKind, ActiveShowPorts, ApplicationCommand,
    CommandFamily,
};
use light_core::{CueListId, Revision, ShowId};
use light_show::PortableShowRevision;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq)]
pub enum ProgrammingUpdateTargetRequest {
    Cue {
        cue_list_id: CueListId,
        playback_number: Option<u16>,
        cue_id: Option<Uuid>,
        cue_number: Option<f64>,
        validate_active_context: bool,
    },
    Preset {
        object_id: String,
    },
    Group {
        object_id: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingUpdatePreviewRequest {
    pub show_id: ShowId,
    pub target: ProgrammingUpdateTargetRequest,
    pub mode: UpdateMode,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingUpdateTargetsRequest {
    pub show_id: ShowId,
    pub filter: UpdateTargetFilter,
}

impl ApplicationCommand for ProgrammingUpdateTargetsRequest {
    type Value = ProgrammingUpdateTargetsResult;

    const FAMILY: CommandFamily = CommandFamily::Programmer;
}

impl ApplicationCommand for ProgrammingUpdatePreviewRequest {
    type Value = ProgrammingUpdatePreviewResult;

    const FAMILY: CommandFamily = CommandFamily::Programmer;
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingUpdateCommand {
    pub show_id: ShowId,
    pub target: ProgrammingUpdateTargetRequest,
    pub mode: UpdateMode,
    pub expected_object_revision: Option<Revision>,
    pub expected_programmer_revision: Option<String>,
    pub expected_show_revision: Option<PortableShowRevision>,
}

impl ApplicationCommand for ProgrammingUpdateCommand {
    type Value = ProgrammingUpdateResult;

    const FAMILY: CommandFamily = CommandFamily::Programmer;
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingUpdatePreviewResult {
    pub context: ActionContext,
    pub request_id: String,
    pub correlation_id: Uuid,
    pub show_revision: PortableShowRevision,
    pub object_revision: Revision,
    pub object: ProgrammingUpdateObjectReference,
    pub programmer_revision: String,
    pub preview: UpdatePreview,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingUpdateObjectReference {
    pub kind: ActiveShowObjectKind,
    pub object_id: String,
    pub object_revision: Revision,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingUpdateMenuEntry {
    pub target: ProgrammingUpdateTargetRequest,
    pub object_revision: Revision,
    pub object: ProgrammingUpdateObjectReference,
    pub programmer_revision: String,
    pub active_or_referenced: bool,
    pub existing_preview: UpdatePreview,
    pub add_new_preview: UpdatePreview,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingUpdateTargetsResult {
    pub context: ActionContext,
    pub request_id: String,
    pub correlation_id: Uuid,
    pub show_revision: PortableShowRevision,
    pub entries: Vec<ProgrammingUpdateMenuEntry>,
}

pub(crate) struct ProgrammingUpdateMenuInput {
    pub(crate) values: light_programmer::ProgrammerUpdateContent,
    pub(crate) selection: light_programmer::ProgrammerUpdateContent,
    pub(crate) values_fingerprint: String,
    pub(crate) selection_fingerprint: String,
    pub(crate) active_preset_id: Option<String>,
    pub(crate) referenced_group_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingUpdateProjection {
    pub show_id: ShowId,
    pub kind: ActiveShowObjectKind,
    pub object_id: String,
    pub object_revision: Revision,
    pub raw_body: Arc<serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingUpdateOutcome {
    pub projection: Arc<ProgrammingUpdateProjection>,
    pub show_revision: PortableShowRevision,
    pub event_sequence: u64,
    pub summary: UpdateResult,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingUpdateResult {
    pub context: ActionContext,
    pub request_id: String,
    pub correlation_id: Uuid,
    pub replayed: bool,
    pub outcome: ProgrammingUpdateOutcome,
}

pub trait ProgrammingUpdatePorts: ActiveShowPorts {
    fn authorize_programming_update(&self, context: &ActionContext) -> Result<(), ActionError>;

    fn active_update_cue_contexts(
        &self,
        context: &ActionContext,
    ) -> Result<Vec<ActiveCueContext>, ActionError>;

    fn reconcile_programming_update(&self, _projection: &ProgrammingUpdateProjection) {}
}
