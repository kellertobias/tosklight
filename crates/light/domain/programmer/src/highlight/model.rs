use crate::SelectionExpression;
use light_core::{FixtureId, UserId};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// OSC buttons commonly repeat a press while a physical contact settles. Treat aliases for the
/// same authoritative action as one press inside this window, while allowing another action or a
/// deliberate later press through immediately.
pub const OSC_REPEAT_GUARD: Duration = Duration::from_millis(150);

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HighlightFixture {
    pub fixture_id: FixtureId,
    pub name: Option<String>,
    pub number: Option<u32>,
}

/// Selection stepping is independent of whether HIGH is active. `Selection` means the complete
/// remembered source is the actual programmer selection; `Step` means one item is selected.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HighlightMode {
    Selection,
    Step,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HighlightAction {
    On,
    Off,
    Toggle,
    Next,
    Previous,
    All,
}

impl HighlightAction {
    pub const fn osc_dedupe_key(self) -> &'static str {
        match self {
            Self::On => "on",
            Self::Off => "off",
            Self::Toggle => "toggle",
            Self::Next => "next",
            Self::Previous => "previous",
            Self::All => "all",
        }
    }
}

pub fn is_duplicate_osc_action(
    previous: Option<(&str, Instant)>,
    action: HighlightAction,
    received_at: Instant,
) -> bool {
    previous.is_some_and(|(previous, previous_at)| {
        previous == action.osc_dedupe_key()
            && received_at.saturating_duration_since(previous_at) < OSC_REPEAT_GUARD
    })
}

#[derive(Clone, Debug, Serialize)]
pub struct HighlightState {
    /// HIGH state only. This is deliberately independent of `mode` and selection emptiness.
    pub active: bool,
    pub mode: HighlightMode,
    pub output_enabled: bool,
    /// Compatibility name for the Blind/Preview output-suppression state.
    pub capture_only: bool,
    /// Current valid resolution of the remembered live selection source.
    pub remembered: Vec<HighlightFixture>,
    pub active_index: Option<usize>,
    pub active_fixture: Option<HighlightFixture>,
    pub can_previous: bool,
    pub can_next: bool,
    pub owner_user_id: Option<UserId>,
    pub owner_user_name: Option<String>,
    pub message: Option<String>,
}

#[derive(Clone, Debug)]
pub struct HighlightSelectionWrite {
    pub selected: Vec<FixtureId>,
    pub expression: Option<SelectionExpression>,
}

#[derive(Clone, Debug)]
pub struct HighlightTransition {
    pub state: HighlightState,
    /// Fixture identities whose raw Highlight Look should be overlaid by the engine.
    pub output_fixtures: Vec<FixtureId>,
    /// Authoritative actual programmer selection requested by PREV, NEXT, ALL, or reconciliation
    /// after an item disappeared. Attribute values are never touched by this write.
    pub working_selection: Option<HighlightSelectionWrite>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HighlightError {
    OwnedByAnotherUser(UserId),
}

impl std::fmt::Display for HighlightError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OwnedByAnotherUser(_) => {
                formatter.write_str("Highlight output is active for another user on this desk")
            }
        }
    }
}
