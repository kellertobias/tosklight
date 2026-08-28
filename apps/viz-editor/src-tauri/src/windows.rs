//! Editor windows, and what one window has to tell the others.
//!
//! The Architect is one application an operator may want on two screens at once: the rig in CAD
//! here, the patch sheet there. Both are the same window kind over the same open document, so a
//! second window is another copy of the same surface rather than a separate CAD application.
//! Because the document lives in the Rust session, the only thing the windows owe each other is
//! word that it changed — every window then reads the authority again rather than guessing.

use serde::Serialize;
use tauri::{Emitter, EventTarget, Manager};

/// The open document was replaced or renamed. Listening windows reload their summary and rig.
pub const DOCUMENT_CHANGED_EVENT: &str = "document-changed";
/// One patch mutation, as the sheet's own transport delivers it.
pub const PATCH_CHANGE_EVENT: &str = "patch-change";

/// Tell every window except the one that caused the change.
///
/// The window that made the edit already has the outcome the command returned. Sending it the
/// same change again would either be a no-op it has to detect or a second application of an edit
/// it has already made, so it is left out of the broadcast instead.
pub fn broadcast<P: Serialize + Clone>(
    app: &tauri::AppHandle,
    origin: &str,
    event: &str,
    payload: P,
) -> Result<(), String> {
    let origin = origin.to_owned();
    app.emit_filter(event, payload, move |target| match target {
        EventTarget::WebviewWindow { label } => label != &origin,
        _ => false,
    })
    .map_err(|error| error.to_string())
}

/// Open another window of the editor on the same document.
///
/// Every window is the same surface, so this is deliberately not a second kind of window with its
/// own state: it loads the same entry point and reads the same session. The label is the first
/// free one so an operator may open as many as the work needs.
#[tauri::command]
pub fn open_editor_window(app: tauri::AppHandle) -> Result<String, String> {
    let label = next_label(&app);
    // Offset from the window that asked, so the new one is visibly a second window rather than
    // an exact overlay that looks like nothing happened.
    let offset = 32.0 * (label_index(&label) as f64);
    tauri::WebviewWindowBuilder::new(&app, &label, tauri::WebviewUrl::App("index.html".into()))
        .title("ToskLight Architect")
        .decorations(false)
        .inner_size(1440.0, 900.0)
        .min_inner_size(1024.0, 640.0)
        .position(80.0 + offset, 80.0 + offset)
        .build()
        .map_err(|error| error.to_string())?;
    Ok(label)
}

fn next_label(app: &tauri::AppHandle) -> String {
    for index in 2..1000 {
        let label = format!("architect-{index}");
        if app.get_webview_window(&label).is_none() {
            return label;
        }
    }
    "architect-overflow".to_owned()
}

fn label_index(label: &str) -> u32 {
    label
        .rsplit('-')
        .next()
        .and_then(|tail| tail.parse::<u32>().ok())
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::label_index;

    #[test]
    fn the_offset_grows_with_the_window_number() {
        assert_eq!(label_index("architect-2"), 2);
        assert_eq!(label_index("architect-7"), 7);
        assert_eq!(label_index("architect-overflow"), 1);
    }
}
