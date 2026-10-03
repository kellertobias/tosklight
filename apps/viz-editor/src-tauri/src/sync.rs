//! Keeping an Architect document and the Control show it came from in step.
//!
//! A document opened from a desk is *bound* to that desk's show: the binding names the desk
//! installation, the show UUID and an association identity that scopes every sync request. The
//! binding is installation state on this computer — never part of the portable show — so a copy
//! of the file carries no binding and can never write into the desk by accident.
//!
//! The synchronization itself is `viz_sync`, the same engine the end-to-end harness runs. This
//! module connects it to the editor: the session's gesture lock, the windows that must hear about
//! a change Control made, and the commands the status chip and the conflict panel use.

use crate::session::Session;
use std::sync::Arc;
use tauri::Manager;
use viz_sync::{ConflictView, DocumentHost, Resolution, SyncStatus};
pub(crate) use viz_sync::{SyncBinding, SyncBindingStore};

/// Windows hear this when the bound document's sync status changes. Carries the status.
pub const SYNC_STATUS_EVENT: &str = "sync-status-changed";

/// The editor as the sync engine's host.
pub(crate) struct EditorHost {
    pub app: tauri::AppHandle,
}

impl DocumentHost for EditorHost {
    fn apply_remote(
        &self,
        edit: &mut dyn FnMut(&viz_document::PlanningDocument) -> Result<bool, String>,
    ) -> Result<(), String> {
        let session = self.app.state::<Session>();
        if !session.remote_edit(edit)? {
            return Ok(());
        }
        // Control changed the show: every window reads the document again, and the CAD views
        // redraw the rig, exactly as for an edit made in another window.
        crate::windows::broadcast_all(&self.app, crate::windows::DOCUMENT_CHANGED_EVENT, ())?;
        crate::windows::broadcast_all(&self.app, crate::windows::MEDIA_LAYOUT_CHANGED_EVENT, ())?;
        let cad = self.app.state::<crate::cad::CadState>();
        let revision = session
            .with(|document| document.patch_revision().map_err(|error| error.to_string()))?;
        crate::cad::emit_scene_state_delta(&self.app, &session, &cad, revision)
    }

    fn status_changed(&self, status: &SyncStatus) {
        let _ = crate::windows::broadcast_all(&self.app, SYNC_STATUS_EVENT, status.clone());
    }
}

/// Prepares the session to synchronize bound documents. Call before any document opens.
pub(crate) fn install(app: &tauri::AppHandle, session: &Session) {
    let tauri::async_runtime::RuntimeHandle::Tokio(handle) = tauri::async_runtime::handle();
    session.set_sync_host(Arc::new(EditorHost { app: app.clone() }), handle);
}

/// The open document's sync status, or `None` for a document bound to no desk.
#[tauri::command]
pub fn sync_status(session: tauri::State<'_, Session>) -> Option<SyncStatus> {
    session.sync_engine().map(|engine| engine.status())
}

/// Everything awaiting the operator's decision.
#[tauri::command]
pub fn sync_conflicts(session: tauri::State<'_, Session>) -> Result<Vec<ConflictView>, String> {
    match session.sync_engine() {
        Some(engine) => engine.conflicts(),
        None => Ok(Vec::new()),
    }
}

/// Keeps Control's version, or sends the Architect's version again over it.
#[tauri::command]
pub fn resolve_sync_conflict(
    session: tauri::State<'_, Session>,
    entry: i64,
    resolution: Resolution,
) -> Result<(), String> {
    session
        .sync_engine()
        .ok_or("This document is not synchronized with a desk")?
        .resolve(entry, resolution)
}

#[tauri::command]
pub fn dismiss_sync_error(session: tauri::State<'_, Session>) {
    if let Some(engine) = session.sync_engine() {
        engine.dismiss_error();
    }
}

/// Works offline, or reconnects to the desk.
#[tauri::command]
pub fn set_sync_online(session: tauri::State<'_, Session>, online: bool) {
    if let Some(engine) = session.sync_engine() {
        engine.set_online(online);
    }
}
