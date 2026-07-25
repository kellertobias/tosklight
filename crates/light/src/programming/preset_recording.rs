use crate::{
    ActionContext, ActionError, ActionErrorKind, ActiveShowPorts, ApplicationCommand, CommandFamily,
};
use light_core::{Revision, ShowId};
use light_programmer::{Preset, PresetAddress, PresetStoreMode};
use light_show::PortableShowRevision;
use std::sync::Arc;

/// Records the normal Programmer into one Preset at action time.
///
/// Values are intentionally absent: the Programming service captures them under the user's
/// serialization gate so a client cannot forge another Programmer or race a later edit.
#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingPresetRecordRequest {
    pub show_id: ShowId,
    pub address: PresetAddress,
    pub name: String,
    pub mode: PresetStoreMode,
    pub expected_object_revision: ProgrammingPresetRevisionExpectation,
    pub expected_show_revision: Option<PortableShowRevision>,
}

/// Object concurrency policy for surfaces that either hold a version token or resolve the target
/// at action time (command line, keyboard, and OSC).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgrammingPresetRevisionExpectation {
    Exact(Revision),
    Current,
}

impl ApplicationCommand for ProgrammingPresetRecordRequest {
    type Value = ProgrammingPresetRecordResult;

    const FAMILY: CommandFamily = CommandFamily::Programmer;
}

/// Application-owned semantic input to the one adapter-owned active-show transaction.
///
/// The port must call [`Self::merged_with`] against the typed object decoded while its active-show
/// transaction is open, then losslessly merge the returned known fields into the original raw
/// body. That keeps merge behavior in the domain while preserving unknown persisted fields.
#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingPresetCommit {
    pub show_id: ShowId,
    pub address: PresetAddress,
    pub mode: PresetStoreMode,
    pub expected_object_revision: ProgrammingPresetRevisionExpectation,
    pub expected_show_revision: Option<PortableShowRevision>,
    captured: Preset,
}

impl ProgrammingPresetCommit {
    pub(crate) fn new(request: &ProgrammingPresetRecordRequest, captured: Preset) -> Self {
        Self {
            show_id: request.show_id,
            address: request.address,
            mode: request.mode,
            expected_object_revision: request.expected_object_revision,
            expected_show_revision: request.expected_show_revision,
            captured,
        }
    }

    pub fn merged_with(&self, existing: Option<&Preset>) -> Result<Preset, ActionError> {
        let mut merged = existing.cloned().unwrap_or_else(|| self.empty_target());
        validate_existing_address(&merged, self.address)?;
        merged.number = self.address.number;
        merged.store(self.captured.clone(), self.mode);
        Ok(merged)
    }

    fn empty_target(&self) -> Preset {
        Preset {
            family: self.address.family,
            number: self.address.number,
            ..Preset::default()
        }
    }
}

/// Complete typed Preset returned by the atomic port transaction.
#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingPresetProjection {
    pub show_id: ShowId,
    pub object_id: String,
    pub address: PresetAddress,
    pub object_revision: Revision,
    /// Exact losslessly merged persisted body, including fields unknown to this build.
    pub raw_body: Arc<serde_json::Value>,
}

/// Adapter completion after one atomic active-show transaction or a verified no-change.
#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingPresetCommitResult {
    pub changed: bool,
    pub projection: ProgrammingPresetProjection,
    pub show_revision: PortableShowRevision,
    pub event_sequence: Option<u64>,
}

pub trait ProgrammingPresetRecordingPorts: Send + Sync {
    fn authorize_preset_recording(&self, context: &ActionContext) -> Result<(), ActionError>;

    /// Atomically validates show/object revisions, applies the core-owned merge, compiles and
    /// installs the candidate, and publishes exactly one retained raw show-object event when the
    /// object changes. Implementations must not re-enter a Programming user or desk gate.
    fn commit_preset(
        &self,
        context: &ActionContext,
        commit: &ProgrammingPresetCommit,
    ) -> Result<ProgrammingPresetCommitResult, ActionError>;
}

/// Active-show adapter hooks used by the narrow application-owned Preset transaction.
pub trait ProgrammingPresetActiveShowPorts: ActiveShowPorts {
    fn reconcile_programming_preset(&self, _projection: &ProgrammingPresetProjection) {}
}

#[derive(Clone, Debug, PartialEq)]
pub enum ProgrammingPresetRecordOutcome {
    Changed {
        projection: Arc<ProgrammingPresetProjection>,
        show_revision: PortableShowRevision,
        event_sequence: u64,
    },
    NoChange {
        projection: Arc<ProgrammingPresetProjection>,
        show_revision: PortableShowRevision,
    },
}

impl ProgrammingPresetRecordOutcome {
    pub fn projection(&self) -> &ProgrammingPresetProjection {
        match self {
            Self::Changed { projection, .. } | Self::NoChange { projection, .. } => projection,
        }
    }

    pub const fn show_revision(&self) -> PortableShowRevision {
        match self {
            Self::Changed { show_revision, .. } | Self::NoChange { show_revision, .. } => {
                *show_revision
            }
        }
    }

    pub const fn event_sequence(&self) -> Option<u64> {
        match self {
            Self::Changed { event_sequence, .. } => Some(*event_sequence),
            Self::NoChange { .. } => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgrammingPresetRecordResult {
    pub context: ActionContext,
    pub request_id: String,
    pub replayed: bool,
    pub outcome: ProgrammingPresetRecordOutcome,
}

fn validate_existing_address(preset: &Preset, address: PresetAddress) -> Result<(), ActionError> {
    if preset.family == address.family && (preset.number == 0 || preset.number == address.number) {
        Ok(())
    } else {
        Err(ActionError::new(
            ActionErrorKind::Invalid,
            "stored Preset identity does not match the requested pool address",
        ))
    }
}
