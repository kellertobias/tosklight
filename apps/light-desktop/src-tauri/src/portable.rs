//! A portable installation keeps everything it writes beside itself.
//!
//! A `portable.txt` file next to the executable marks the folder as portable: the desk's data
//! directory, show database, extensions and log then live in `data/` inside that folder instead of
//! the per-user application data directory, so moving or deleting the folder takes them along.
//! The web view's browser profile and the Stage renderer's preferences and snapshots follow into
//! the same `data/` folder, so a portable desk leaves nothing behind in the user profile.

use std::path::{Path, PathBuf};

/// The marker an operator places beside the executable to make its folder portable.
pub(crate) const MARKER: &str = "portable.txt";

/// The renderer's preferences file override, owned by `viz-renderer`.
const RENDERER_PREFERENCES_ENV: &str = "TOSKLIGHT_VIZ_PREFERENCES";
/// The renderer's snapshot folder override, owned by `viz-snapshot`.
const RENDERER_SNAPSHOT_DIR_ENV: &str = "TOSKLIGHT_VIZ_SNAPSHOT_DIR";

/// The data directory of a portable installation, or `None` when the folder is not portable.
pub(crate) fn data_dir() -> Option<PathBuf> {
    let executable = std::env::current_exe().ok()?;
    data_dir_beside(executable.parent()?)
}

/// `data/` inside `directory` when `directory` carries the portable marker file.
fn data_dir_beside(directory: &Path) -> Option<PathBuf> {
    directory
        .join(MARKER)
        .is_file()
        .then(|| directory.join("data"))
}

/// The desk's data directory: the portable folder when there is one, the per-user application
/// data directory otherwise. An installation without the marker is unchanged.
pub(crate) fn resolve_data_dir<E>(
    portable: Option<PathBuf>,
    per_user: impl FnOnce() -> Result<PathBuf, E>,
) -> Result<PathBuf, E> {
    match portable {
        Some(portable) => Ok(portable),
        None => per_user(),
    }
}

/// The portable data directory of a release build. Development builds keep their repository
/// data locations whatever sits beside the executable.
fn release_data_dir() -> Option<PathBuf> {
    if cfg!(debug_assertions) {
        None
    } else {
        data_dir()
    }
}

/// Where a portable desk's web views keep their browser profile (WebView2 on Windows, WebKitGTK
/// on Linux). `None` keeps Tauri's per-user default.
pub(crate) fn webview_data_dir() -> Option<PathBuf> {
    release_data_dir().map(|data| webview_dir_in(&data))
}

/// Points a web view at the portable browser profile, when this desk is portable.
pub(crate) fn place_webview<R: tauri::Runtime>(
    builder: tauri::webview::WebviewBuilder<R>,
) -> tauri::webview::WebviewBuilder<R> {
    match webview_data_dir() {
        Some(directory) => builder.data_directory(directory),
        None => builder,
    }
}

fn webview_dir_in(data: &Path) -> PathBuf {
    data.join("webview")
}

/// A supervised Stage renderer that, for a portable desk, keeps its preferences and snapshots in
/// the portable folder too.
pub(crate) fn renderer(program: PathBuf, arguments: Vec<String>) -> viz_helper::SupervisedHelper {
    renderer_environment().into_iter().fold(
        viz_helper::SupervisedHelper::new(program, arguments),
        |helper, (key, value)| helper.with_environment(key, value),
    )
}

/// Environment for the Stage renderer of a portable desk. Empty for an ordinary installation.
fn renderer_environment() -> Vec<(String, String)> {
    release_data_dir()
        .map(|data| renderer_environment_in(&data))
        .unwrap_or_default()
}

fn renderer_environment_in(data: &Path) -> Vec<(String, String)> {
    let renderer = data.join("visualizer");
    [
        (RENDERER_PREFERENCES_ENV, renderer.join("preferences.conf")),
        (RENDERER_SNAPSHOT_DIR_ENV, renderer.join("snapshots")),
    ]
    .into_iter()
    // A path that is not valid Unicode cannot travel through the helper's string environment;
    // the renderer then falls back to its per-user default rather than a mangled path.
    .filter_map(|(key, path)| path.to_str().map(|path| (key.to_owned(), path.to_owned())))
    .collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "light-desktop-portable-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        directory
    }

    #[test]
    fn a_marker_beside_the_executable_keeps_data_in_the_folder() {
        let folder = scratch("marker");
        std::fs::write(folder.join(MARKER), "portable").unwrap();
        assert_eq!(data_dir_beside(&folder), Some(folder.join("data")));
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn a_folder_without_the_marker_is_not_portable() {
        let folder = scratch("plain");
        assert_eq!(data_dir_beside(&folder), None);
        // A directory named like the marker is not the marker.
        std::fs::create_dir(folder.join(MARKER)).unwrap();
        assert_eq!(data_dir_beside(&folder), None);
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn an_installation_without_the_marker_keeps_the_per_user_data_directory() {
        let per_user = PathBuf::from("per-user/de.tokenet.light");
        let resolved = resolve_data_dir::<()>(None, || Ok(per_user.clone()));
        assert_eq!(resolved, Ok(per_user));
    }

    #[test]
    fn a_portable_installation_never_asks_for_the_per_user_directory() {
        let resolved = resolve_data_dir::<()>(Some(PathBuf::from("stick/data")), || {
            panic!("the per-user directory must not be resolved or created")
        });
        assert_eq!(resolved, Ok(PathBuf::from("stick/data")));
    }

    #[test]
    fn a_portable_desk_keeps_browser_and_renderer_state_in_its_data_folder() {
        let data = PathBuf::from("stick").join("ToskLight").join("data");
        assert_eq!(webview_dir_in(&data), data.join("webview"));
        let environment = renderer_environment_in(&data);
        let visualizer = data.join("visualizer");
        assert_eq!(
            environment,
            vec![
                (
                    RENDERER_PREFERENCES_ENV.to_owned(),
                    visualizer
                        .join("preferences.conf")
                        .to_str()
                        .unwrap()
                        .to_owned()
                ),
                (
                    RENDERER_SNAPSHOT_DIR_ENV.to_owned(),
                    visualizer.join("snapshots").to_str().unwrap().to_owned()
                ),
            ]
        );
    }

    #[test]
    fn a_development_build_ignores_the_marker() {
        if cfg!(debug_assertions) {
            assert_eq!(webview_data_dir(), None);
            assert!(renderer_environment().is_empty());
        }
    }
}
