//! A portable installation keeps everything it writes beside itself.
//!
//! A `portable.txt` file next to the executable marks the folder as portable: the desk's data
//! directory, show database, extensions and log then live in `data/` inside that folder instead of
//! the per-user application data directory, so moving or deleting the folder takes them along.

use std::path::PathBuf;

/// The marker an operator places beside the executable to make its folder portable.
pub(crate) const MARKER: &str = "portable.txt";

/// The data directory of a portable installation, or `None` when the folder is not portable.
pub(crate) fn data_dir() -> Option<PathBuf> {
    let executable = std::env::current_exe().ok()?;
    let directory = executable.parent()?;
    directory
        .join(MARKER)
        .is_file()
        .then(|| directory.join("data"))
}

/// On Windows, stops with an actionable message when the WebView2 runtime the window needs is
/// missing, instead of failing to open a window at all.
pub(crate) fn prepare(product: &str) {
    #[cfg(windows)]
    if tauri::webview_version().is_err() {
        rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Error)
            .set_title(product)
            .set_description(format!(
                "{product} needs the Microsoft Edge WebView2 Runtime, which is not installed on \
                 this computer.\n\nInstall the Evergreen WebView2 Runtime from \
                 https://developer.microsoft.com/microsoft-edge/webview2/ and start {product} again."
            ))
            .show();
        std::process::exit(1);
    }
    #[cfg(not(windows))]
    let _ = product;
}
