use super::{ExecutionPolicy, PendingCommandChoice, ProgrammingShowUndoTarget};
use crate::{ActionContext, ActionError};
use light_core::{AttributeKey, FixtureId};
use light_programmer::GroupDefinition;
use light_programmer::ProgrammerRegistry;
use std::collections::HashMap;
use std::collections::HashSet;

#[derive(Clone, Debug, Default)]
pub struct ProgrammingSelectionEnvironment {
    pub show_revision: u64,
    pub selectable_fixtures: HashMap<FixtureId, Vec<FixtureId>>,
    pub groups: HashMap<String, GroupDefinition>,
}

#[derive(Clone, Debug, Default)]
pub struct ProgrammingValuesEnvironment {
    pub fixture_ids: HashSet<FixtureId>,
    /// Group id → resolved ordered-membership size, so value validation can reject
    /// multi-point spreads with more control points than the Group has members.
    pub group_memberships: HashMap<String, usize>,
    /// Group id → evaluated spatial-rank count. Equal spatial keys share one rank and therefore
    /// one spread value. Missing entries retain legacy membership-count validation.
    pub group_rank_counts: HashMap<String, usize>,
    /// Group id → resolved ordered membership. Relative Group intents use this frozen membership
    /// and the same current-value view as fixture intents.
    pub group_members: HashMap<String, Vec<FixtureId>>,
    /// One frozen view of values explicitly resolved by the engine. Linked captures must only use
    /// this view so an unowned profile default does not silently become Programmer ownership.
    pub current_values: light_engine::ResolvedValues,
    /// Profile defaults for addresses absent from `current_values`. Relative intents may use this
    /// fallback as their starting value without materializing unrelated activation links.
    pub default_values: light_engine::ResolvedValues,
    /// Attributes supported by each fixture or logical-head identity.
    pub supported_attributes: HashMap<FixtureId, HashSet<AttributeKey>>,
    /// Application policy input. Empty in current production configuration; tests and the future
    /// attribute registry can inject ordered linked attributes without changing the transport.
    pub activation_links: HashMap<AttributeKey, Vec<AttributeKey>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProgrammingSelectionQuery {
    Fixtures(Vec<FixtureId>),
    Groups(Vec<String>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProgrammingExecution {
    Accepted {
        applied: usize,
        warning: Option<String>,
        /// The owning application action was replayed and must not repeat interaction cleanup.
        replayed: bool,
    },
    ChoiceRequired {
        pending_choice: PendingCommandChoice,
    },
    Rejected {
        error: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgrammingReconciliation {
    SelectionChanged,
    CaptureModeChanged,
}

/// Server-owned capabilities needed while the legacy parser, persistence, and Preload output
/// transaction are moved behind application boundaries. Transport adapters implement this port;
/// the service remains the sole owner of ordering, replay, and Programmer mutations.
pub trait ProgrammingPorts: Send + Sync {
    fn authorize(&self, _context: &ActionContext) -> Result<(), ActionError> {
        Ok(())
    }

    fn execute(
        &self,
        programmers: &ProgrammerRegistry,
        context: &ActionContext,
        command: &str,
        policy: ExecutionPolicy,
    ) -> ProgrammingExecution;

    fn selection_environment(
        &self,
        _context: &ActionContext,
        _query: &ProgrammingSelectionQuery,
    ) -> Result<ProgrammingSelectionEnvironment, ActionError> {
        Err(ActionError::new(
            crate::ActionErrorKind::Unavailable,
            "selection environment is unavailable",
        ))
    }

    fn values_environment(
        &self,
        _context: &ActionContext,
    ) -> Result<ProgrammingValuesEnvironment, ActionError> {
        Err(ActionError::new(
            crate::ActionErrorKind::Unavailable,
            "Programmer values environment is unavailable",
        ))
    }

    fn persist(&self, context: &ActionContext, operation: &'static str) -> Option<String>;

    /// Whether relative movement for one normalized fixture attribute wraps at its endpoints.
    /// The default keeps transports without fixture-profile metadata on ordinary clamp behavior.
    fn programmer_attribute_wraps(
        &self,
        _context: &ActionContext,
        _fixture_id: FixtureId,
        _attribute: &AttributeKey,
    ) -> bool {
        false
    }

    /// Notifies transient Highlight state that exact fixture attributes were explicitly authored.
    /// This is intentionally infallible and idempotent: the Programmer mutation is authoritative,
    /// while adapters use the callback only to remove matching temporary look attributes.
    fn mark_highlight_explicit_fixture_attributes(
        &self,
        _context: &ActionContext,
        _touched: &[(FixtureId, AttributeKey)],
    ) {
    }

    fn undo_show_recording(
        &self,
        _context: &ActionContext,
        _target: &ProgrammingShowUndoTarget,
    ) -> Result<light_core::Revision, ActionError> {
        Err(ActionError::new(
            crate::ActionErrorKind::Unavailable,
            "show recording undo is unavailable",
        ))
    }

    fn capture_programmer_on_preload(&self, _context: &ActionContext) -> bool {
        true
    }

    /// Reconciles selection-derived state before the authoritative projection is captured and
    /// published. Implementations must not re-enter the Programming desk gate.
    fn reconcile(&self, context: &ActionContext, reason: ProgrammingReconciliation);

    fn commit_preload(&self, context: &ActionContext) -> Result<Option<String>, String>;
}
