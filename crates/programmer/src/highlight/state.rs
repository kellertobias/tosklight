use crate::SelectionExpression;
use light_core::{FixtureId, UserId};
use std::collections::HashMap;
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
    /// Revision of the actual programmer selection last observed or explicitly acknowledged as
    /// our own PREV/NEXT/ALL write.
    pub(super) observed_selection_revision: Option<u64>,
    pub(super) message: Option<String>,
}

#[derive(Clone, Default)]
pub(super) struct HighlightRuntime {
    pub(super) operators: HashMap<OperatorKey, OperatorState>,
    pub(super) output_owners: HashMap<Uuid, UserId>,
}

pub(super) type OperatorKey = (Uuid, UserId);
pub(super) type RecentHighlightActions = HashMap<OperatorKey, (&'static str, Instant)>;
