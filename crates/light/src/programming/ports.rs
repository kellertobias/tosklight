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

#[derive(Clone, Debug)]
pub struct ProgrammingValuesEnvironment {
    /// Production advertises only complete runtime contracts. Domain fixtures default to the
    /// current contract so foundation behavior remains independently testable.
    pub supported_programming_contract: u16,
    pub fixture_ids: HashSet<FixtureId>,
    /// Group id → resolved ordered-membership size, so value validation can reject
    /// multi-point spreads with more control points than the Group has members.
    pub group_memberships: HashMap<String, usize>,
    /// Group id → evaluated spatial-rank count. Equal spatial keys share one rank and therefore
    /// one spread value. Missing entries retain legacy membership-count validation.
    pub group_rank_counts: HashMap<String, usize>,
    /// Authoritative evaluated rank per current member, including equal spatial ranks.
    pub group_ranks: HashMap<String, HashMap<FixtureId, usize>>,
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
    /// Frozen semantic adoption data. Concrete feature adapters populate these from one frame.
    pub family_contexts: HashMap<FixtureId, ProgrammingFamilyContext>,
    /// Explicit shared adoption context for a live Group owner, never inferred from its first lamp.
    pub group_family_contexts: HashMap<String, ProgrammingFamilyContext>,
    /// Declarative complete templates for future members. Adapters supply physical Zoom and
    /// pinned Direct defaults here; these are never inferred from the first member.
    pub group_family_templates: HashMap<(String, AttributeKey), light_core::AttributeValue>,
    /// TL-594: set by `prepare_family_edit_context` when the edit named a displayed source
    /// that cannot be resolved. The service then holds the whole action quietly.
    pub displayed_source_hold: Option<super::ProgrammingValuesHold>,
    /// TL-554: set during capture when the first semantic edit adopted Direct values.
    pub color_adoption: Option<super::ProgrammingColorAdoption>,
}

impl Default for ProgrammingValuesEnvironment {
    fn default() -> Self {
        Self {
            supported_programming_contract: light_core::programming::PROGRAMMING_CONTRACT_VERSION,
            fixture_ids: Default::default(),
            group_memberships: Default::default(),
            group_rank_counts: Default::default(),
            group_ranks: Default::default(),
            group_members: Default::default(),
            current_values: Default::default(),
            default_values: Default::default(),
            supported_attributes: Default::default(),
            activation_links: Default::default(),
            family_contexts: Default::default(),
            group_family_contexts: Default::default(),
            group_family_templates: Default::default(),
            displayed_source_hold: None,
            color_adoption: None,
        }
    }
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

    /// Whether this surface may change what is programmed on the desk.
    ///
    /// Separate from `authorize` because presenting the desk and changing it are different
    /// permissions: a Not Editable screen reads the Programmer, the fixture sheet and the Stage
    /// exactly as any other screen does, and is refused only when it tries to alter them.
    fn authorize_programming_change(&self, context: &ActionContext) -> Result<(), ActionError> {
        self.authorize(context)
    }

    /// Whether this surface may run one command line.
    ///
    /// A guest surface may still run the commands that only *operate* the desk — going to a Cue,
    /// selecting a playback, setting a speed-group speed — because none of those touch the
    /// Programmer. The default treats every command as a programming change, so a transport that
    /// does not classify commands stays closed rather than open.
    fn authorize_command(
        &self,
        context: &ActionContext,
        _command: Option<&str>,
    ) -> Result<(), ActionError> {
        self.authorize_programming_change(context)
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

    /// Capture immutable family adoption inputs for the first real component edit of a
    /// gesture, or once for a one-shot edit. The application has validated the intent and
    /// capture lane under its Programmer/desk boundary. A retained gesture, including one
    /// whose first capture was absent, does not call this port again.
    fn prepare_family_edit_context(
        &self,
        _context: &ActionContext,
        _preload: bool,
        _intent: &super::ProgrammingValueIntent,
        _environment: &mut ProgrammingValuesEnvironment,
    ) -> Result<(), ActionError> {
        Ok(())
    }

    /// TL-554: the verified original native Color model of `source` from the active runtime
    /// generation, used to forward-evaluate a Direct value's estimate for semantic adoption.
    /// `None` keeps the recorded estimate (transports without a model catalogue).
    fn native_color_model(
        &self,
        _context: &ActionContext,
        _source: &light_core::NativeColorIdentity,
    ) -> Option<std::sync::Arc<dyn light_core::programming::NativeColorEditModel + Send + Sync>>
    {
        None
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

    /// Undoes the desk's most recent Fixture Freeze or Unfreeze when it is the latest Programmer
    /// Undo step. `Ok(None)` means no Freeze step applies, so the ordinary Programmer Undo
    /// continues; `Ok(Some(changed))` means the Freeze step was consumed. This is the same
    /// Freeze-aware step the HTTP and WebSocket Programmer Undo actions run first, so `[UND]`
    /// behaves identically from every attached surface.
    fn undo_fixture_freeze(&self, _context: &ActionContext) -> Result<Option<bool>, ActionError> {
        Ok(None)
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

pub use light_core::programming::OwnedFamilyEditContext as ProgrammingFamilyContext;
