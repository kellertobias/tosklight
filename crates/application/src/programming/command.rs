use crate::{ActionContext, ApplicationCommand, CommandFamily};
use light_programmer::CommandLineState;
use light_programmer::command_line::{CommandKey, CommandKeyPhase};

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
    pub replayed: bool,
}
