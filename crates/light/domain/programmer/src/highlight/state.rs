use crate::SelectionExpression;
use light_core::{AttributeKey, FixtureId, UserId};
use std::collections::{HashMap, HashSet};
use std::time::Instant;
use uuid::Uuid;

#[derive(Clone, Debug, Default)]
pub(super) struct OperatorState {
    pub(super) active: bool,
    pub(super) output_enabled: bool,
    pub(super) remembered: Vec<FixtureId>,
    pub(super) remembered_expression: Option<SelectionExpression>,
    pub(super) stepping: bool,
    pub(super) active_fixture: Option<FixtureId>,
    pub(super) explicit_attributes: HashMap<FixtureId, HashSet<AttributeKey>>,
    /// Revision of the actual programmer selection last observed or explicitly acknowledged as
    /// our own PREV/NEXT/ALL write.
    pub(super) observed_selection_revision: Option<u64>,
    pub(super) message: Option<String>,
}

/// The desk's Highlight.
///
/// One desk, one Highlight. It used to be keyed by desk and user, from when a desk could hold
/// several of each: two operators could then own Highlight output separately, and one would be
/// refused while the other held it. There is nobody to be refused any more — every surface is
/// looking at the same lamp — so the state is a single value and the owner is a name to show,
/// not a claim to enforce.
#[derive(Clone, Default)]
pub(super) struct HighlightRuntime {
    pub(super) operator: OperatorState,
    /// The identity Highlight output is currently attributed to, for the operator-facing label.
    pub(super) output_owner: Option<UserId>,
}

pub(super) type RecentHighlightActions = HashMap<OperatorKey, (&'static str, Instant)>;
/// Repeat guarding stays per physical surface: two adapters must not advance the selection twice
/// for one press, and that is a property of the press, not of the desk.
pub(super) type OperatorKey = (Uuid, UserId);
