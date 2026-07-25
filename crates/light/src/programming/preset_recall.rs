use super::ProgrammingValuesProjection;
use crate::{ActionContext, ActionError, ApplicationCommand, CommandFamily};
use light_core::{Revision, ShowId};
use light_programmer::{GroupDefinition, Preset, PresetAddress};
use light_show::PortableShowRevision;
use std::{collections::HashMap, sync::Arc};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgrammingPresetRecallRevisionExpectation {
    Exact(u64),
    Current,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingPresetRecallRequest {
    pub show_id: ShowId,
    pub address: PresetAddress,
    pub expected_preset_revision: ProgrammingPresetRecallRevisionExpectation,
    pub expected_show_revision: ProgrammingPresetRecallRevisionExpectation,
    pub expected_values_revision: ProgrammingPresetRecallRevisionExpectation,
    pub expected_capture_mode_revision: ProgrammingPresetRecallRevisionExpectation,
    pub expected_selection_revision: ProgrammingPresetRecallRevisionExpectation,
}

impl ApplicationCommand for ProgrammingPresetRecallRequest {
    type Value = ProgrammingPresetRecallResult;

    const FAMILY: CommandFamily = CommandFamily::Programmer;
}

#[derive(Clone, Debug)]
pub struct ProgrammingPresetRecallEnvironment {
    pub show_id: ShowId,
    pub show_revision: PortableShowRevision,
    pub object_id: String,
    pub object_revision: Revision,
    pub address: PresetAddress,
    pub raw_body: Arc<serde_json::Value>,
    pub preset: Arc<Preset>,
    pub groups: Arc<HashMap<String, GroupDefinition>>,
    pub programmer_fade_millis: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingRecalledPresetProjection {
    pub show_id: ShowId,
    pub show_revision: PortableShowRevision,
    pub object_id: String,
    pub object_revision: Revision,
    pub address: PresetAddress,
    /// Exact persisted body, including fields unknown to this build.
    pub raw_body: Arc<serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ProgrammingPresetRecallOutcome {
    Changed {
        values_revision: u64,
        projection: Option<Arc<ProgrammingValuesProjection>>,
        values_event_sequence: Option<u64>,
    },
    NoChange {
        values_revision: u64,
    },
}

impl ProgrammingPresetRecallOutcome {
    pub const fn values_revision(&self) -> u64 {
        match self {
            Self::Changed {
                values_revision, ..
            }
            | Self::NoChange { values_revision } => *values_revision,
        }
    }

    pub const fn values_event_sequence(&self) -> Option<u64> {
        match self {
            Self::Changed {
                values_event_sequence,
                ..
            } => *values_event_sequence,
            Self::NoChange { .. } => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingPresetRecallResult {
    pub context: ActionContext,
    pub request_id: String,
    pub replayed: bool,
    pub applied_fixtures: usize,
    pub selection_revision: u64,
    pub interaction_event_sequence: Option<u64>,
    pub capture_mode_revision: u64,
    pub active_context: String,
    pub preset: ProgrammingRecalledPresetProjection,
    pub outcome: ProgrammingPresetRecallOutcome,
    pub warning: Option<String>,
}

pub trait ProgrammingPresetRecallPorts: Send + Sync {
    fn authorize_preset_recall(&self, context: &ActionContext) -> Result<(), ActionError>;

    /// Resolves one exact Preset and the compiled Group graph from one coherent active Show.
    fn preset_recall_environment(
        &self,
        context: &ActionContext,
        request: &ProgrammingPresetRecallRequest,
    ) -> Result<ProgrammingPresetRecallEnvironment, ActionError>;

    fn persist_preset_recall(
        &self,
        context: &ActionContext,
        operation: &'static str,
    ) -> Option<String>;
}
