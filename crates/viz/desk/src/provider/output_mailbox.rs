//! Native output is latest-only. Configuration retains its ordered channel.

use super::{Message, desk_output::keeps_pending, output_demand::OutputRead};
use std::sync::{
    Arc, Mutex,
    mpsc::{self, Receiver, SendError, Sender},
};

#[derive(Default)]
pub(super) struct OutputMailbox(Mutex<PendingOutput>);

#[derive(Default)]
struct PendingOutput {
    marker: u64,
    read: Option<OutputRead>,
}

impl OutputMailbox {
    pub(super) fn take(&self, marker: u64) -> Option<OutputRead> {
        let mut pending = self.0.lock().unwrap();
        if pending.marker != marker {
            return None;
        }
        pending.read.take()
    }

    fn discard(&self) {
        let read = self.0.lock().unwrap().read.take();
        drop(read);
    }
}

/// The connection worker is the only producer. A marker orders a pending read alongside
/// configuration messages without putting the read's buffers in the unbounded channel.
pub(super) struct ProviderOutbox {
    messages: Sender<Message>,
    output: Arc<OutputMailbox>,
}

impl ProviderOutbox {
    pub(super) fn channel() -> (Self, Receiver<Message>, Arc<OutputMailbox>) {
        let (messages, receiver) = mpsc::channel();
        let output = Arc::new(OutputMailbox::default());
        (
            Self {
                messages,
                output: Arc::clone(&output),
            },
            receiver,
            output,
        )
    }

    pub(super) fn send(&self, message: Message) -> Result<(), SendError<Message>> {
        if matches!(
            &message,
            Message::Connection(_)
                | Message::Scene { .. }
                | Message::Delta { .. }
                | Message::Resync(_)
        ) {
            self.output.discard();
        }
        self.messages.send(message)
    }

    pub(super) fn send_output(&self, read: OutputRead) -> Result<(), SendError<Message>> {
        let mut pending = self.output.0.lock().unwrap();
        if let Some(previous) = &pending.read {
            if previous.request == read.request && keeps_pending(&previous.output, &read.output) {
                return Ok(());
            }
        } else {
            pending.marker = pending
                .marker
                .checked_add(1)
                .expect("output marker exhausted");
            self.messages.send(Message::OutputReady(pending.marker))?;
        }
        let previous = pending.read.replace(read);
        drop(pending);
        drop(previous);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
