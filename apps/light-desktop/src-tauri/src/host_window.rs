//! The desk's window, built so a renderer can draw inside it.
//!
//! It used to come from the Tauri configuration as a webview window: one native view, filled by
//! the interface, with nowhere for anything else to go. The Stage view was therefore always going
//! to be drawn by the web renderer, because there was no surface underneath it to draw on.
//!
//! So the window is built here instead, the other way round — a native window with the interface
//! added on top as a transparent child. Everything the desk draws is still the same web interface
//! in the same place; what changes is that there is now a native surface beneath it for the
//! visualizer to draw the Stage into, with the sheet, the menus and the dialogs above it rather
//! than covered by it.
//!
//! The arrangement is the one `experiments/embedded-renderer-pane` established and an operator
//! confirmed: a transparent child webview over a native surface keeps its z-order, so web chrome
//! draws above the rendered image rather than the other way round.
//!
//! The failure this can produce is silent — everything runs, the server comes ready, and the
//! window is black because the webview never rendered. So it is checked rather than assumed:
//! launching with `LIGHT_DESKTOP_TEST_READY_FILE` set writes the marker only once the frontend has
//! mounted and called `frontend_ready`, which it cannot do from a window that did not draw.

use tauri::{LogicalPosition, LogicalSize, WebviewUrl};

/// What the window is called, matching what the configuration used to say.
const TITLE: &str = "ToskLight";
const LABEL: &str = "main";

/// Build the desk window and put the interface in it.
///
/// The webview keeps the label the rest of the application already uses, so every existing
/// `get_webview_window("main")` continues to find it.
pub(crate) fn install(app: &mut tauri::App) -> tauri::Result<()> {
    let window = tauri::window::WindowBuilder::new(app, LABEL)
        .title(TITLE)
        .inner_size(1440.0, 900.0)
        .min_inner_size(1024.0, 640.0)
        .decorations(false)
        .resizable(true)
        .build()?;

    let size = window.inner_size()?;
    let scale = window.scale_factor()?;
    let logical = size.to_logical::<f64>(scale);

    // Transparent so the surface beneath shows through wherever the interface paints nothing,
    // which is what makes a Stage pane possible at all. `auto_resize` keeps it filling the window
    // without the desk having to follow every resize itself.
    let webview = window.add_child(
        tauri::webview::WebviewBuilder::new(LABEL, WebviewUrl::default())
            .transparent(true)
            .auto_resize(),
        LogicalPosition::new(0.0, 0.0),
        LogicalSize::new(logical.width, logical.height),
    )?;

    // "On top" has to be true of the layers, not merely of the order they were added.
    //
    // The Stage is drawn into this window by another process, and everything the interface puts
    // over it — a menu across the pane, a dialog, the pane's own settings — is drawn by this
    // webview. Without raising it, all of that lands behind the picture inside the pane rectangle
    // while looking correct everywhere else, which reads as those controls doing nothing at all.
    //
    // The interface is the half to raise. Sinking the picture instead puts it behind the window's
    // own backing, and then there is no picture.
    #[cfg(target_os = "macos")]
    {
        let _ = webview.with_webview(|webview| {
            viz_surface::raise_view_above_siblings(webview.inner());
        });
    }
    #[cfg(not(target_os = "macos"))]
    let _ = webview;
    Ok(())
}
