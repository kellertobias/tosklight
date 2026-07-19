use uuid::Uuid;

/// A bounded application command family. Concrete commands implement [`ApplicationCommand`]
/// instead of joining one process-wide command enum.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CommandFamily {
    Programmer,
    Playback,
    Show,
    Desk,
    Output,
}

/// The surface that originated an application action.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ActionSource {
    UserInterface,
    Keyboard,
    Osc,
    Http,
    Midi,
    Matter,
    Cue,
    Timecode,
    Scheduler,
    Macro,
    System,
}

/// Identity and optimistic-concurrency information shared by every application mutation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionContext {
    pub desk_id: Uuid,
    pub user_id: Option<Uuid>,
    pub session_id: Option<Uuid>,
    pub source: ActionSource,
    pub correlation_id: Uuid,
    pub request_id: Option<String>,
    pub expected_revision: Option<u64>,
}

impl ActionContext {
    pub fn operator(desk_id: Uuid, user_id: Uuid, session_id: Uuid, source: ActionSource) -> Self {
        Self {
            desk_id,
            user_id: Some(user_id),
            session_id: Some(session_id),
            source,
            correlation_id: Uuid::new_v4(),
            request_id: None,
            expected_revision: None,
        }
    }

    pub fn system(desk_id: Uuid, source: ActionSource) -> Self {
        Self {
            desk_id,
            user_id: None,
            session_id: None,
            source,
            correlation_id: Uuid::new_v4(),
            request_id: None,
            expected_revision: None,
        }
    }

    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }

    pub const fn with_expected_revision(mut self, revision: u64) -> Self {
        self.expected_revision = Some(revision);
        self
    }
}

/// A concrete command declares its own typed result and bounded family.
pub trait ApplicationCommand: Send + 'static {
    type Value: Send + 'static;

    const FAMILY: CommandFamily;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionEnvelope<C> {
    pub context: ActionContext,
    pub command: C,
}

/// Successful application result with the authoritative post-mutation revision and event cursor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionOutcome<T> {
    pub value: T,
    pub revision: Option<u64>,
    pub event_sequence: Option<u64>,
}

impl<T> ActionOutcome<T> {
    pub const fn new(value: T) -> Self {
        Self {
            value,
            revision: None,
            event_sequence: None,
        }
    }

    pub const fn at_revision(mut self, revision: u64) -> Self {
        self.revision = Some(revision);
        self
    }

    pub const fn with_event_sequence(mut self, sequence: u64) -> Self {
        self.event_sequence = Some(sequence);
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ActionErrorKind {
    Invalid,
    Unauthorized,
    Forbidden,
    NotFound,
    Conflict,
    Busy,
    Unavailable,
    Internal,
}

/// Transport-independent failure. Adapters choose their own status code and serialized shape.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionError {
    pub kind: ActionErrorKind,
    pub message: String,
    pub current_revision: Option<u64>,
    /// Current revision of an auxiliary authority participating in the same atomic precondition.
    pub current_related_revision: Option<u64>,
    pub retryable: bool,
}

impl ActionError {
    pub fn new(kind: ActionErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            current_revision: None,
            current_related_revision: None,
            retryable: matches!(kind, ActionErrorKind::Busy | ActionErrorKind::Unavailable),
        }
    }

    pub const fn at_revision(mut self, revision: u64) -> Self {
        self.current_revision = Some(revision);
        self
    }

    pub const fn at_related_revision(mut self, revision: u64) -> Self {
        self.current_related_revision = Some(revision);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct PlaybackGo;

    impl ApplicationCommand for PlaybackGo {
        type Value = u64;

        const FAMILY: CommandFamily = CommandFamily::Playback;
    }

    #[test]
    fn bounded_commands_keep_context_and_typed_outcomes() {
        let context = ActionContext::operator(
            Uuid::nil(),
            Uuid::from_u128(1),
            Uuid::from_u128(2),
            ActionSource::Osc,
        )
        .with_request_id("osc-1")
        .with_expected_revision(7);
        let action = ActionEnvelope {
            context: context.clone(),
            command: PlaybackGo,
        };
        let outcome = ActionOutcome::new(3_u64)
            .at_revision(8)
            .with_event_sequence(11);

        assert_eq!(PlaybackGo::FAMILY, CommandFamily::Playback);
        assert_eq!(action.context, context);
        assert_eq!(outcome.revision, Some(8));
        assert_eq!(outcome.event_sequence, Some(11));
    }
}
