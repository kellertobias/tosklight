use crate::{ActionContext, ApplicationCommand, CommandFamily};
use light_core::FixtureId;
use light_programmer::command_line::{CommandKey, CommandKeyPhase};
use light_programmer::{CommandLineState, ProgrammerSelection, SelectionRule};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ExecutionPolicy {
    AtomicProgrammer,
    Compatibility,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum ProgrammingCommand {
    ApplyKey {
        key: CommandKey,
        phase: CommandKeyPhase,
        execute_policy: ExecutionPolicy,
    },
    ReplaceCommandLine {
        text: String,
        expected_revision: u64,
    },
    Execute {
        command: Option<String>,
        policy: ExecutionPolicy,
    },
    ClearStep,
    Undo,
    Preload {
        capture_programmer: bool,
    },
    ReplaceSelection {
        fixtures: Vec<FixtureId>,
        expected_revision: u64,
    },
    ApplySelectionGesture {
        source: SelectionGestureSource,
        remove: bool,
    },
    SelectGroup {
        group_id: String,
        frozen: bool,
        rule: SelectionRule,
        expected_revision: u64,
    },
    ApplySelectionRule {
        rule: SelectionRule,
    },
}

impl ProgrammingCommand {
    pub const fn returns_selection(&self) -> bool {
        matches!(
            self,
            Self::ReplaceSelection { .. }
                | Self::ApplySelectionGesture { .. }
                | Self::SelectGroup { .. }
                | Self::ApplySelectionRule { .. }
        )
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum SelectionGestureSource {
    Fixture { fixture_id: FixtureId },
    LiveGroup { group_id: String },
    DereferencedGroup { group_id: String },
}

impl ApplicationCommand for ProgrammingCommand {
    type Value = ProgrammingResult;

    const FAMILY: CommandFamily = CommandFamily::Programmer;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgrammingAction {
    Edited,
    Executed,
    ClearedCommandLine,
    ClearedPreload,
    ClearedSelection,
    ClearedValues,
    Undone,
    NoChange,
    PreloadEntered,
    PreloadCommitted,
    ShiftPressed,
    ShiftReleased,
    IgnoredRelease,
    SelectionReplaced,
    SelectionGestureApplied,
    GroupSelected,
    SelectionRuleApplied,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CueTransferOperation {
    Copy,
    Move,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgrammingChoiceOptionId {
    Plain,
    Status,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammingChoiceOption {
    pub id: ProgrammingChoiceOptionId,
    pub label: String,
    pub command: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CueMoveCopyChoice {
    pub operation: CueTransferOperation,
    pub command: String,
    pub options: Vec<ProgrammingChoiceOption>,
    pub cancel_label: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProgrammingOutcome {
    Accepted {
        action: ProgrammingAction,
        applied: Option<usize>,
        warning: Option<String>,
    },
    ChoiceRequired {
        pending_choice: CueMoveCopyChoice,
    },
    Rejected {
        error: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammingResult {
    pub context: ActionContext,
    pub outcome: ProgrammingOutcome,
    pub command_line_before: CommandLineState,
    pub command_line: CommandLineState,
    pub selection_revision_before: u64,
    pub selection_revision: u64,
    pub selection: Option<ProgrammerSelection>,
    pub interaction_event_sequence: Option<u64>,
    pub capture_mode_event_sequence: Option<u64>,
    pub values_event_sequence: Option<u64>,
    pub preload_values_event_sequence: Option<u64>,
    pub replayed: bool,
}
