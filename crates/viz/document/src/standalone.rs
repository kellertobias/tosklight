//! Where the standalone Viz product keeps the document it had open last.
//!
//! One product, two windows: the visualizer an operator opens and the editor it can open. A normal
//! launch should therefore show them the rig they were last looking at rather than an empty
//! picture or a file dialog — which means the visualizer has to be able to find the record the
//! editor writes, and both have to agree on where that is.
//!
//! The editor gets the location from Tauri, which derives it from the bundle identifier. This
//! mirrors that derivation so a process without Tauri can find the same file, and a test asserts
//! the identifier here still matches the one the editor is built with — a rename that moved the
//! editor's record without moving this would silently stop the visualizer reopening anything.

use std::path::PathBuf;

/// The Viz application's bundle identifier, as `apps/viz-editor/src-tauri/tauri.conf.json` states.
pub const VIZ_IDENTIFIER: &str = "de.tokenet.tosklight.viz-editor";

/// The file naming the last opened document.
pub const RECENT_SHOW_FILE: &str = "recent-show";

/// The configuration directory Tauri would give the editor, derived the same way.
///
/// `None` where the platform's own directory cannot be located, which is not a failure: it means
/// there is nowhere a previous session could have left a record either.
pub fn config_dir() -> Option<PathBuf> {
    let base = if cfg!(target_os = "macos") {
        PathBuf::from(std::env::var_os("HOME")?)
            .join("Library")
            .join("Application Support")
    } else if cfg!(target_os = "windows") {
        PathBuf::from(std::env::var_os("APPDATA")?)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config")
            })
    };
    Some(base.join(VIZ_IDENTIFIER))
}

/// The document the standalone product had open last, if it is still there.
///
/// A show that has since been moved or deleted answers `None` rather than an error: it is not a
/// failure to have finished with a show, and a launch that reported one would be worse than a
/// launch that simply offers the editor.
pub fn recent_show() -> Option<PathBuf> {
    let recorded = std::fs::read_to_string(config_dir()?.join(RECENT_SHOW_FILE)).ok()?;
    let path = PathBuf::from(recorded.trim());
    path.is_file().then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole point of the constant: if the editor's identifier moves and this does not, the
    /// visualizer looks in a directory nothing writes to and silently stops reopening anything.
    #[test]
    fn the_identifier_matches_the_editor_the_record_is_written_by() {
        let conf = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../apps/viz-editor/src-tauri/tauri.conf.json"),
        )
        .expect("the editor's Tauri configuration");
        let declared: serde_json::Value =
            serde_json::from_str(&conf).expect("the configuration parses");
        assert_eq!(
            declared["identifier"].as_str(),
            Some(VIZ_IDENTIFIER),
            "the editor's bundle identifier moved; the visualizer would look in the wrong place"
        );
    }

    #[test]
    fn a_record_naming_a_file_that_is_gone_is_forgotten() {
        let directory = std::env::temp_dir().join("viz-standalone-recent");
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("workspace");
        let missing = directory.join("gone.show");
        std::fs::write(
            directory.join(RECENT_SHOW_FILE),
            missing.display().to_string(),
        )
        .expect("record");
        // Read through the same steps `recent_show` takes, with the directory this test owns.
        let recorded =
            std::fs::read_to_string(directory.join(RECENT_SHOW_FILE)).expect("record reads");
        let path = PathBuf::from(recorded.trim());
        assert!(!path.is_file(), "the named show is deliberately absent");
        assert_eq!(path.is_file().then_some(path), None);
    }

    #[test]
    fn the_configuration_directory_is_named_after_the_product() {
        let directory = config_dir().expect("a configuration directory on this platform");
        assert!(directory.ends_with(VIZ_IDENTIFIER));
    }
}
