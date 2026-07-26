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
    /// Every currently selectable fixture or logical-head identity in deterministic desk order.
    /// Unpatched fixtures remain in this catalog; deleted identities do not.
    pub selectable_targets: Arc<Vec<light_core::FixtureId>>,
    /// Stored whole-fixture owners expand through the same logical-head contract as an ordinary
    /// fixture selection. Logical-head owners map to themselves.
    pub target_expansions: Arc<HashMap<light_core::FixtureId, Vec<light_core::FixtureId>>>,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgrammingPresetRecallDisposition {
    Recalled,
    TargetsSelected,
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
    pub disposition: ProgrammingPresetRecallDisposition,
    pub applied_fixtures: usize,
    pub selected_targets: usize,
    pub selection_revision: u64,
    pub interaction_event_sequence: Option<u64>,
    pub capture_mode_revision: u64,
    pub active_context: Option<String>,
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
