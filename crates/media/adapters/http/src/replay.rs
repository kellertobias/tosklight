//! Request identity for edits.
//!
//! An edit must never be accidentally redone. A dropped response, a proxy timeout, or a client
//! retry all look the same from here, so an edit carries a client-generated request id and this
//! keeps a window of what each one produced: a resend returns the stored outcome instead of
//! executing again.
//!
//! Live control has no such machinery on purpose. A caller that sends GO twice meant GO twice; it
//! is *edits* that must be idempotent.

use std::collections::VecDeque;
use std::sync::Mutex;

/// How many recent edits are remembered.
///
/// A window rather than a permanent record: retries arrive within seconds, and an unbounded map
/// keyed by client-supplied strings is something a peer could grow without limit.
pub const WINDOW: usize = 256;

/// What one edit produced, kept so a resend can be answered with it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Executed {
    request_id: String,
    body: String,
}

/// The remembered outcomes of recent edits.
#[derive(Debug, Default)]
pub struct Replays {
    executed: Mutex<VecDeque<Executed>>,
}

impl Replays {
    pub fn new() -> Self {
        Self::default()
    }

    /// What this request id produced before, if it is still in the window.
    pub fn stored(&self, request_id: &str) -> Option<String> {
        let executed = self.executed.lock().unwrap_or_else(|poisoned| {
            // A panic while holding this lock must not take the API down with it: the worst case
            // is that one edit is executed twice, which is the situation without any window.
            poisoned.into_inner()
        });
        executed
            .iter()
            .find(|entry| entry.request_id == request_id)
            .map(|entry| entry.body.clone())
    }

    /// Remembers what an edit produced.
    pub fn remember(&self, request_id: &str, body: String) {
        if request_id.is_empty() {
            return;
        }
        let mut executed = self
            .executed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        executed.retain(|entry| entry.request_id != request_id);
        executed.push_back(Executed {
            request_id: request_id.to_owned(),
            body,
        });
        while executed.len() > WINDOW {
            executed.pop_front();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_resend_gets_the_first_answer_rather_than_a_second_execution() {
        let replays = Replays::new();
        assert_eq!(replays.stored("abc"), None);

        replays.remember("abc", "{\"done\":true}".into());
        assert_eq!(replays.stored("abc"), Some("{\"done\":true}".into()));
    }

    #[test]
    fn two_different_edits_do_not_answer_for_each_other() {
        let replays = Replays::new();
        replays.remember("first", "one".into());
        replays.remember("second", "two".into());

        assert_eq!(replays.stored("first"), Some("one".into()));
        assert_eq!(replays.stored("second"), Some("two".into()));
    }

    #[test]
    fn the_window_is_bounded_so_a_peer_cannot_grow_it_without_limit() {
        let replays = Replays::new();
        for index in 0..WINDOW + 10 {
            replays.remember(&format!("edit-{index}"), index.to_string());
        }

        assert_eq!(replays.stored("edit-0"), None, "the oldest fell out");
        assert_eq!(
            replays.stored(&format!("edit-{}", WINDOW + 9)),
            Some((WINDOW + 9).to_string()),
            "and the newest is still there"
        );
    }

    #[test]
    fn an_edit_with_no_request_id_is_not_remembered_at_all() {
        // Remembering the empty string would make every unidentified edit a replay of the last.
        let replays = Replays::new();
        replays.remember("", "one".into());
        assert_eq!(replays.stored(""), None);
    }
}
