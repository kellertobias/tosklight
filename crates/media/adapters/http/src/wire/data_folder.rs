//! Browsing for, and switching to, another configuration and media folder.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::data_folder::{FolderChange, FolderEntry, FolderListing};

/// One folder on the Media Server computer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct DataFolderEntryView {
    pub name: String,
    pub directory: String,
    pub has_configuration: bool,
}

/// The subfolders of one folder, for the folder picker.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct DataFolderListingView {
    pub directory: String,
    pub parent: Option<String>,
    /// The folder already holds a Media Server configuration, which choosing it loads.
    pub has_configuration: bool,
    pub folders: Vec<DataFolderEntryView>,
}

impl DataFolderListingView {
    pub fn of(listing: &FolderListing) -> Self {
        Self {
            directory: listing.directory.display().to_string(),
            parent: listing
                .parent
                .as_ref()
                .map(|parent| parent.display().to_string()),
            has_configuration: listing.has_configuration,
            folders: listing
                .folders
                .iter()
                .map(DataFolderEntryView::of)
                .collect(),
        }
    }
}

impl DataFolderEntryView {
    fn of(entry: &FolderEntry) -> Self {
        Self {
            name: entry.name.clone(),
            directory: entry.directory.display().to_string(),
            has_configuration: entry.has_configuration,
        }
    }
}

/// Makes a folder the configuration and media-library root.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDataFolder {
    pub request_id: String,
    pub directory: String,
}

/// The accepted folder. The server restarts to load it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
pub struct DataFolderChangeView {
    pub directory: String,
    pub loaded_existing: bool,
    pub restarting: bool,
}

impl DataFolderChangeView {
    pub fn of(change: &FolderChange) -> Self {
        Self {
            directory: change.directory.display().to_string(),
            loaded_existing: change.loaded_existing,
            restarting: true,
        }
    }
}

/// Which folder the picker should list; absent means the current data folder.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct DataFolderQuery {
    pub directory: Option<String>,
}
