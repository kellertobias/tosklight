use crate::{ProgrammerRegistry, ProgrammerSelection};
use light_core::SessionId;
use serde::{Deserialize, Serialize};

/// The desk-local default scope used when an operator starts a new command.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommandTarget {
    #[default]
    Fixture,
    Group,
}

impl CommandTarget {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fixture => "FIXTURE",
            Self::Group => "GROUP",
        }
    }
}

impl TryFrom<&str> for CommandTarget {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "FIXTURE" => Ok(Self::Fixture),
            "GROUP" => Ok(Self::Group),
            _ => Err(()),
        }
    }
}

/// One authoritative command-line snapshot for a desk interaction context.
///
/// `text` retains the legacy raw representation, where an empty string means the visible default
/// target. `visible_text` provides the normalized operator-facing value used by new adapters.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CommandLineState {
    pub text: String,
    pub target: CommandTarget,
    pub pristine: bool,
    pub revision: u64,
}

impl Default for CommandLineState {
    fn default() -> Self {
        Self {
            text: String::new(),
            target: CommandTarget::Fixture,
            pristine: true,
            revision: 0,
        }
    }
}

impl CommandLineState {
    pub fn visible_text(&self) -> &str {
        if self.text.trim().is_empty() {
            self.target.as_str()
        } else {
            &self.text
        }
    }

    /// Preserve the legacy `ProgrammerState.command_line` projection while the revisioned command
    /// state keeps pristine defaults canonical internally. Historically an untouched Fixture line
    /// was empty, while switching to the Group target stored `GROUP` in the legacy field.
    pub(crate) fn legacy_text(&self) -> &str {
        if self.text.trim().is_empty() && self.target == CommandTarget::Group {
            CommandTarget::Group.as_str()
        } else {
            &self.text
        }
    }
}

/// One coherent read of the desk-local command and ordered-selection context.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammerInteractionState {
    pub command_line: CommandLineState,
    pub selection: ProgrammerSelection,
}

/// Lightweight interaction metadata for change detection without cloning ordered selections.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgrammerInteractionVersion {
    pub command_line: CommandLineState,
    pub selection_revision: u64,
    pub capture_mode_active: bool,
}

pub(crate) fn canonical_command_text(text: String, pristine: bool) -> String {
    if pristine { String::new() } else { text }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandLineReplaceError {
    UnknownSession,
    RevisionConflict { expected: u64, actual: u64 },
}

impl ProgrammerRegistry {
    pub fn interaction_version(&self, session: SessionId) -> Option<ProgrammerInteractionVersion> {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        self.interaction_version_while_gated(session)
    }

    fn interaction_version_while_gated(
        &self,
        session: SessionId,
    ) -> Option<ProgrammerInteractionVersion> {
        if !self.sessions.read().contains_key(&session) {
            return None;
        }
        let context = self.command_context(session);
        let capture_mode_active = self
            .states
            .read()
            .get(&self.key(session))
            .is_some_and(|state| state.blind || state.preview);
        Some(ProgrammerInteractionVersion {
            command_line: self
                .command_states
                .read()
                .get(&context)
                .cloned()
                .unwrap_or_default(),
            selection_revision: self
                .selection_contexts
                .read()
                .get(&context)
                .map_or(0, |selection| selection.revision),
            capture_mode_active,
        })
    }

    pub fn interaction_state(&self, session: SessionId) -> Option<ProgrammerInteractionState> {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        self.interaction_state_while_gated(session)
    }

    fn interaction_state_while_gated(
        &self,
        session: SessionId,
    ) -> Option<ProgrammerInteractionState> {
        if !self.sessions.read().contains_key(&session) {
            return None;
        }
        let context = self.command_context(session);
        Some(ProgrammerInteractionState {
            command_line: self
                .command_states
                .read()
                .get(&context)
                .cloned()
                .unwrap_or_default(),
            selection: self.interaction_selection(context),
        })
    }

    fn interaction_selection(&self, context: SessionId) -> ProgrammerSelection {
        self.selection_contexts
            .read()
            .get(&context)
            .map(|selection| ProgrammerSelection {
                selected: selection.selected.clone(),
                expression: selection.expression.clone(),
                revision: selection.revision,
                gesture_open: selection.gesture_open,
            })
            .unwrap_or_default()
    }

    pub fn set_command_line(&self, session: SessionId, command_line: String) -> bool {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        self.update_command_line(session, |current| {
            let pristine = command_line.trim().is_empty()
                || command_line
                    .trim()
                    .eq_ignore_ascii_case(current.target.as_str());
            (command_line, current.target, pristine)
        })
        .is_some()
    }

    pub fn command_line_state(&self, session: SessionId) -> Option<CommandLineState> {
        if !self.sessions.read().contains_key(&session) {
            return None;
        }
        Some(
            self.command_states
                .read()
                .get(&self.command_context(session))
                .cloned()
                .unwrap_or_default(),
        )
    }

    pub fn replace_command_line(
        &self,
        session: SessionId,
        expected_revision: u64,
        text: String,
    ) -> Result<CommandLineState, CommandLineReplaceError> {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        if !self.sessions.read().contains_key(&session) {
            return Err(CommandLineReplaceError::UnknownSession);
        }
        let context = self.command_context(session);
        let mut commands = self.command_states.write();
        let current = commands.entry(context).or_default();
        if current.revision != expected_revision {
            return Err(CommandLineReplaceError::RevisionConflict {
                expected: expected_revision,
                actual: current.revision,
            });
        }
        let pristine =
            text.trim().is_empty() || text.trim().eq_ignore_ascii_case(current.target.as_str());
        let text = canonical_command_text(text, pristine);
        if current.text != text || current.pristine != pristine {
            current.text = text;
            current.pristine = pristine;
            current.revision += 1;
        }
        let result = current.clone();
        drop(commands);
        self.touch(session);
        Ok(result)
    }

    /// Atomically derive a new command-line state from the current desk-local snapshot.
    pub fn update_command_line<F>(&self, session: SessionId, update: F) -> Option<CommandLineState>
    where
        F: FnOnce(&CommandLineState) -> (String, CommandTarget, bool),
    {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        if !self.sessions.read().contains_key(&session) {
            return None;
        }
        let context = self.command_context(session);
        let mut commands = self.command_states.write();
        let current = commands.entry(context).or_default();
        let (text, target, pristine) = update(current);
        let text = canonical_command_text(text, pristine);
        if current.text != text || current.target != target || current.pristine != pristine {
            current.text = text;
            current.target = target;
            current.pristine = pristine;
            current.revision += 1;
        }
        let result = current.clone();
        drop(commands);
        self.touch(session);
        Some(result)
    }

    fn touch(&self, session: SessionId) {
        if let Some(state) = self.states.write().get_mut(&self.key(session)) {
            state.last_activity = self.clock.now();
        }
    }

    pub fn command_target(&self, session: SessionId) -> String {
        self.command_line_state(session)
            .map(|state| state.target.as_str().to_owned())
            .unwrap_or_else(|| CommandTarget::Fixture.as_str().to_owned())
    }
    pub fn set_command_target(&self, session: SessionId, target: String) -> bool {
        let mutation_gate = self.mutation_gate(session);
        let _mutation_guard = mutation_gate.lock();
        let Ok(target) = CommandTarget::try_from(target.as_str()) else {
            return false;
        };
        self.update_command_line(session, |current| {
            let pristine = current.text.trim().is_empty()
                || current.text.trim().eq_ignore_ascii_case(target.as_str());
            (current.text.clone(), target, pristine)
        })
        .is_some()
    }
}
