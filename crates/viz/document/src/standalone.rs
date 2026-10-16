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

use std::path::{Path, PathBuf};

/// The Viz application's bundle identifier, as `apps/viz-editor/src-tauri/tauri.conf.json` states.
pub const VIZ_IDENTIFIER: &str = "de.tokenet.tosklight.visualizer";

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
    recent_show_at(&config_dir()?.join(RECENT_SHOW_FILE))
}

fn recent_show_at(record: &Path) -> Option<PathBuf> {
    let recorded = std::fs::read_to_string(record).ok()?;
    // Architect stores newest-first JSON paths; older installations stored one
    // plain path. Reading either representation must never rewrite the record.
    let paths: Vec<PathBuf> =
        serde_json::from_str(&recorded).unwrap_or_else(|_| vec![PathBuf::from(recorded.trim())]);
    paths.into_iter().find(|path| path.is_file())
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

    struct RecentFixture(PathBuf);

    impl RecentFixture {
        fn new() -> Self {
            let base = std::env::var_os("LIGHT_TMP_DIR")
                .expect("tests use the repository's canonical LIGHT_TMP_DIR");
            let root =
                PathBuf::from(base).join(format!("viz-standalone-recent-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&root).expect("isolated recent fixture");
            Self(root)
        }

        fn record(&self) -> PathBuf {
            self.0.join(RECENT_SHOW_FILE)
        }

        fn show(&self, name: &str) -> PathBuf {
            let path = self.0.join(name);
            std::fs::write(&path, b"original portable show bytes").expect("show fixture");
            path
        }

        fn assert_read(&self, bytes: &[u8], expected: Option<PathBuf>) {
            std::fs::write(self.record(), bytes).expect("recent fixture");
            assert_eq!(recent_show_at(&self.record()), expected);
            assert_eq!(
                std::fs::read(self.record()).expect("record unchanged"),
                bytes
            );
        }
    }

    impl Drop for RecentFixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn architect_json_list_reopens_the_first_existing_file_in_recorded_order() {
        let fixture = RecentFixture::new();
        let first = fixture.show("first existing with spaces.show");
        let second = fixture.show("second.show");
        let missing = fixture.0.join("missing.show");
        let directory = fixture.0.join("directory.show");
        std::fs::create_dir(&directory).expect("a directory is not a show file");
        let bytes = serde_json::to_vec(&vec![missing, directory, first.clone(), second.clone()])
            .expect("Architect's ordered JSON path list");
        fixture.assert_read(&bytes, Some(first.clone()));
        let reversed = serde_json::to_vec(&vec![second.clone(), first.clone()]).unwrap();
        fixture.assert_read(&reversed, Some(second.clone()));
        for show in [first, second] {
            assert_eq!(
                std::fs::read(show).unwrap(),
                b"original portable show bytes"
            );
        }
    }

    #[test]
    fn legacy_plain_path_and_current_single_entry_list_are_both_read_only() {
        let fixture = RecentFixture::new();
        let show = fixture.show("legacy show.show");
        fixture.assert_read(
            format!("  {}\n", show.display()).as_bytes(),
            Some(show.clone()),
        );
        fixture.assert_read(
            &serde_json::to_vec(&vec![show.clone()]).unwrap(),
            Some(show.clone()),
        );
        assert_eq!(
            std::fs::read(show).unwrap(),
            b"original portable show bytes"
        );
    }

    #[test]
    fn empty_malformed_missing_and_unreadable_records_offer_no_recent_show() {
        let fixture = RecentFixture::new();
        assert_eq!(recent_show_at(&fixture.record()), None, "missing record");
        for bytes in [
            b"".as_slice(),
            b"  \n",
            b"[]",
            b"[broken",
            b"{}",
            b"null",
            b"[null]",
            b"\xff",
        ] {
            fixture.assert_read(bytes, None);
        }
        let missing = fixture.0.join("gone.show");
        fixture.assert_read(missing.to_string_lossy().as_bytes(), None);
        fixture.assert_read(&serde_json::to_vec(&vec![missing]).unwrap(), None);
        std::fs::remove_file(fixture.record()).unwrap();
        std::fs::create_dir(fixture.record()).unwrap();
        assert_eq!(
            recent_show_at(&fixture.record()),
            None,
            "record cannot be read as a file"
        );
        assert!(fixture.record().is_dir(), "unreadable input remains intact");
    }

    #[test]
    fn the_configuration_directory_is_named_after_the_product() {
        let directory = config_dir().expect("a configuration directory on this platform");
        assert!(directory.ends_with(VIZ_IDENTIFIER));
    }
}
