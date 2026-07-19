use crate::EventDraft;

/// A named adapter-owned Playback unit of work serialized with ordinary Playback commands.
///
/// The operation receives no service internals or domain lock. Its implementation can only use
/// the typed ports it already owns and returns event drafts for ordered publication.
pub trait PlaybackUnitOfWork {
    type Output;

    fn execute(self) -> PlaybackOperation<Self::Output>;
}

pub struct PlaybackOperation<T> {
    pub output: T,
    pub events: Vec<EventDraft>,
}

impl<T> PlaybackOperation<T> {
    pub fn new(output: T) -> Self {
        Self {
            output,
            events: Vec::new(),
        }
    }

    pub fn with_events(output: T, events: Vec<EventDraft>) -> Self {
        Self { output, events }
    }
}

pub struct PlaybackOperationResult<T> {
    pub output: T,
    pub event_sequences: Vec<u64>,
}
