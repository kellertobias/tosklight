//! Read-only startup feedback survives a frontend that mounts after recent-show refusal.
use super::{Answer, DocumentSummary, Session};
use serde::Serialize;
use std::path::Path;

/// A startup refusal retained until the interface can read it, including late sibling windows.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupRecovery {
    pub path: String,
    pub reason: String,
    pub fallback: Option<StartupFallback>,
    pub fallback_error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupFallback {
    pub name: String,
    pub path: String,
}

impl Session {
    pub(super) fn record_startup_refusal(&self, path: &Path, reason: String) {
        *self.startup_recovery.lock() = Some(StartupRecovery {
            path: path.to_string_lossy().into_owned(),
            reason,
            fallback: None,
            fallback_error: None,
        });
    }
    pub fn startup_recovery(&self) -> Option<StartupRecovery> {
        self.startup_recovery.lock().clone()
    }

    pub(crate) fn complete_startup_fallback(&self, outcome: &Answer<DocumentSummary>) {
        if let Some(recovery) = self.startup_recovery.lock().as_mut() {
            match outcome {
                Ok(summary) => {
                    recovery.fallback = Some(StartupFallback {
                        name: summary.name.clone(),
                        path: summary.path.clone(),
                    })
                }
                Err(reason) => recovery.fallback_error = Some(reason.clone()),
            }
        }
    }
}

#[tauri::command]
pub fn startup_recovery(session: tauri::State<'_, Session>) -> Option<StartupRecovery> {
    session.startup_recovery()
}
