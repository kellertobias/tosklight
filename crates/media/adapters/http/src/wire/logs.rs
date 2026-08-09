//! The log, as the API reports it.
//!
//! An operator diagnosing a machine at a venue has no terminal and no log file they can reach.
//! What they have is a browser, so the process keeps a bounded window of recent records and this
//! projects it — including how many records the window has had to discard, because a log viewer
//! that quietly loses records is worse than one that admits it.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::diagnostics::{LogEntry, LogPage};

/// The tracing filter currently installed for this process.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct ServerLogLevelView {
    pub level: String,
    /// This is always true: the maintainer chose parity with the reference's runtime-only control.
    pub resets_on_restart: bool,
}

impl ServerLogLevelView {
    pub fn of(level: String) -> Self {
        Self {
            level,
            resets_on_restart: true,
        }
    }
}

/// Changes the process tracing filter without writing configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateServerLogLevel {
    pub request_id: String,
    pub level: String,
}

/// One emitted record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct LogRecordView {
    /// Monotonically increasing, so a viewer asks for everything after what it already holds.
    #[ts(type = "number")]
    pub sequence: u64,
    #[ts(type = "number")]
    pub millis_since_start: u64,
    /// `error`, `warn`, `info`, `debug`, or `trace`.
    pub level: String,
    pub target: String,
    pub message: String,
}

impl LogRecordView {
    pub fn of(entry: &LogEntry) -> Self {
        Self {
            sequence: entry.sequence,
            millis_since_start: entry.millis_since_start,
            level: entry.level.clone(),
            target: entry.target.clone(),
            message: entry.message.clone(),
        }
    }
}

/// A window of the log.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct LogsView {
    pub records: Vec<LogRecordView>,
    /// The newest sequence the process holds, whether or not this window reached it.
    #[ts(type = "number")]
    pub newest: u64,
    /// How many records have been discarded since the process started.
    #[ts(type = "number")]
    pub dropped: u64,
    pub capacity: usize,
}

impl LogsView {
    pub fn of(page: &LogPage) -> Self {
        Self {
            records: page.entries.iter().map(LogRecordView::of).collect(),
            newest: page.newest,
            dropped: page.dropped,
            capacity: page.capacity,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_window_reports_what_it_holds_and_what_was_lost() {
        let page = LogPage {
            entries: vec![LogEntry {
                sequence: 7,
                millis_since_start: 1_200,
                level: "warn".to_owned(),
                target: "media_runtime".to_owned(),
                message: "no audio input".to_owned(),
            }],
            newest: 9,
            dropped: 4,
            capacity: 2_000,
        };
        let view = LogsView::of(&page);

        assert_eq!(view.records.len(), 1);
        assert_eq!(view.records[0].sequence, 7);
        assert_eq!(view.records[0].level, "warn");
        assert_eq!(
            view.newest, 9,
            "a viewer can tell it is behind rather than up to date"
        );
        assert_eq!(view.dropped, 4);
    }
}
