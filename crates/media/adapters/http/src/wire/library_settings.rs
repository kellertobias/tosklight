//! Media-library storage settings.

use std::path::PathBuf;

use media_application::configuration::LibraryConfiguration;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The saved library directory and the directory this process is currently using.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySettingsView {
    pub stored_directory: String,
    pub active_directory: String,
    pub takes_effect_on_restart: bool,
    pub pending_restart: bool,
}

impl LibrarySettingsView {
    pub fn of(library: &LibraryConfiguration, active: &LibraryConfiguration) -> Self {
        Self {
            stored_directory: library.root.to_string_lossy().into_owned(),
            active_directory: active.root.to_string_lossy().into_owned(),
            takes_effect_on_restart: true,
            pending_restart: library.root != active.root,
        }
    }
}

/// An intent-shaped edit of the directory containing addressed media and its metadata.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLibrarySettings {
    pub request_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub directory: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LibrarySettingsEditError {
    #[error("directory must name a media-library folder")]
    EmptyDirectory,
}

impl UpdateLibrarySettings {
    pub fn applied(
        &self,
        current: &LibraryConfiguration,
    ) -> Result<LibraryConfiguration, LibrarySettingsEditError> {
        let mut next = current.clone();
        if let Some(directory) = &self.directory {
            let directory = directory.trim();
            if directory.is_empty() {
                return Err(LibrarySettingsEditError::EmptyDirectory);
            }
            next.root = PathBuf::from(directory);
        }
        Ok(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_edit_changes_only_the_library_directory_and_rejects_an_empty_path() {
        let current = LibraryConfiguration::default();
        let next = UpdateLibrarySettings {
            request_id: "move".into(),
            directory: Some("  D:\\Show Media  ".into()),
        }
        .applied(&current)
        .unwrap();
        assert_eq!(next.root, PathBuf::from("D:\\Show Media"));
        assert_eq!(next.target_codec, current.target_codec);

        let empty = UpdateLibrarySettings {
            request_id: "empty".into(),
            directory: Some("  ".into()),
        };
        assert_eq!(
            empty.applied(&current),
            Err(LibrarySettingsEditError::EmptyDirectory)
        );
    }

    #[test]
    fn the_view_distinguishes_saved_and_current_directories() {
        let active = LibraryConfiguration::default();
        let mut stored = active.clone();
        stored.root = PathBuf::from("D:\\Show Media");
        let view = LibrarySettingsView::of(&stored, &active);
        assert!(view.pending_restart);
        assert!(view.takes_effect_on_restart);
        assert_eq!(view.stored_directory, "D:\\Show Media");
        assert_eq!(view.active_directory, "media");
    }
}
