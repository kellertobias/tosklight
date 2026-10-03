//! The one status a bound document shows, derived from the same facts every time.
//!
//! Precedence: Error, Conflict, Offline, Pending, Synced. Only Synced ever says "Saved to
//! Control"; every state can say "Saved on this computer", because the working file and the
//! journal are durable before the status changes.

use serde::Serialize;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncPhase {
    Synced,
    Pending,
    Offline,
    Conflict,
    Error,
}

/// Why the engine is not talking to the desk.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Connection {
    Connecting,
    Connected,
    Unreachable(String),
    /// The desk is reachable but has a different show open (or none).
    ShowNotActive(Option<Uuid>),
    /// The operator chose to work offline.
    Paused,
    /// The desk refused this binding, for example because a different desk answers at its
    /// address. Reconnecting will not help until something changes.
    Refused(String),
}

/// Everything the status is computed from.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatusFacts {
    pub connection: Connection,
    pub error: Option<String>,
    pub pending: usize,
    pub conflicts: usize,
    pub rejected: usize,
    /// A snapshot re-read is under way; the document is not yet known to match the desk.
    pub resynchronizing: bool,
}

impl Default for StatusFacts {
    fn default() -> Self {
        Self {
            connection: Connection::Connecting,
            error: None,
            pending: 0,
            conflicts: 0,
            rejected: 0,
            resynchronizing: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub state: SyncPhase,
    /// The chip's words.
    pub label: String,
    /// One sentence saying why, for the chip's tooltip and the status panel.
    pub detail: String,
    pub desk_name: String,
    pub pending: usize,
    pub conflicts: usize,
    /// `true` only when the desk has confirmed every edit.
    pub saved_to_control: bool,
    /// Always `true` for a bound document: edits are journaled before they are reported.
    pub saved_on_this_computer: bool,
}

impl StatusFacts {
    pub fn status(&self, desk_name: &str) -> SyncStatus {
        let refused = match &self.connection {
            Connection::Refused(reason) => Some(reason),
            _ => None,
        };
        let (state, label, detail) = if let Some(error) = self.error.as_ref().or(refused) {
            (SyncPhase::Error, "Sync error".to_owned(), error.clone())
        } else if self.rejected > 0 {
            (
                SyncPhase::Error,
                "Sync error".to_owned(),
                format!(
                    "{desk_name} refused {} change{}; it is kept on this computer.",
                    self.rejected,
                    plural(self.rejected)
                ),
            )
        } else if self.conflicts > 0 {
            (
                SyncPhase::Conflict,
                format!("{} conflict{}", self.conflicts, plural(self.conflicts)),
                "Someone on Control changed the same thing. Choose which version to keep."
                    .to_owned(),
            )
        } else if let Some(reason) = self.offline_reason(desk_name) {
            (SyncPhase::Offline, "Offline".to_owned(), reason)
        } else if self.pending > 0 || self.resynchronizing {
            (
                SyncPhase::Pending,
                if self.pending > 0 {
                    format!("{} pending", self.pending)
                } else {
                    "Syncing".to_owned()
                },
                format!("Saved on this computer; sending to {desk_name}."),
            )
        } else {
            (
                SyncPhase::Synced,
                "Synced".to_owned(),
                format!("Saved to Control ({desk_name})."),
            )
        };
        SyncStatus {
            saved_to_control: state == SyncPhase::Synced,
            saved_on_this_computer: true,
            state,
            label,
            detail,
            desk_name: desk_name.to_owned(),
            pending: self.pending,
            conflicts: self.conflicts,
        }
    }

    fn offline_reason(&self, desk_name: &str) -> Option<String> {
        let held = if self.pending > 0 {
            format!(
                " {} change{} saved on this computer.",
                self.pending,
                plural(self.pending)
            )
        } else {
            String::new()
        };
        match &self.connection {
            Connection::Connected | Connection::Refused(_) => None,
            Connection::Connecting => Some(format!("Connecting to {desk_name}.{held}")),
            Connection::Unreachable(_) => Some(format!("{desk_name} is unreachable.{held}")),
            Connection::ShowNotActive(_) => Some(format!("Show not active on Control.{held}")),
            Connection::Paused => Some(format!("Working offline.{held}")),
        }
    }
}

fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> StatusFacts {
        StatusFacts {
            connection: Connection::Connected,
            ..StatusFacts::default()
        }
    }

    #[test]
    fn only_a_confirmed_document_claims_to_be_saved_to_control() {
        assert_eq!(facts().status("FOH").state, SyncPhase::Synced);
        assert!(facts().status("FOH").saved_to_control);
        for facts in [
            StatusFacts {
                pending: 1,
                ..facts()
            },
            StatusFacts {
                connection: Connection::Unreachable("down".into()),
                ..facts()
            },
            StatusFacts {
                conflicts: 1,
                ..facts()
            },
            StatusFacts {
                error: Some("damaged".into()),
                ..facts()
            },
            StatusFacts {
                resynchronizing: true,
                ..facts()
            },
        ] {
            let status = facts.status("FOH");
            assert!(!status.saved_to_control, "{status:?}");
            assert!(!status.detail.contains("Saved to Control"), "{status:?}");
            assert!(status.saved_on_this_computer);
        }
    }

    #[test]
    fn precedence_is_error_conflict_offline_pending_synced() {
        let all = StatusFacts {
            connection: Connection::ShowNotActive(None),
            error: Some("broken".into()),
            pending: 2,
            conflicts: 1,
            rejected: 0,
            resynchronizing: false,
        };
        assert_eq!(all.status("FOH").state, SyncPhase::Error);
        let no_error = StatusFacts { error: None, ..all };
        assert_eq!(no_error.status("FOH").state, SyncPhase::Conflict);
        let no_conflict = StatusFacts {
            conflicts: 0,
            ..no_error
        };
        let offline = no_conflict.status("FOH");
        assert_eq!(offline.state, SyncPhase::Offline);
        assert_eq!(
            offline.detail,
            "Show not active on Control. 2 changes saved on this computer."
        );
        let online = StatusFacts {
            connection: Connection::Connected,
            ..no_conflict
        };
        assert_eq!(online.status("FOH").state, SyncPhase::Pending);
    }
}
