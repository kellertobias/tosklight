//! Choosing the folder that holds this server's configuration and media library.
//!
//! The API adapter never touches the filesystem, so browsing and changing the folder are process
//! capabilities handed in by the runtime. A test proves the route contract with a stand-in.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use media_application::MediaConfiguration;

/// One folder the operator can open or choose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderEntry {
    pub name: String,
    pub directory: PathBuf,
    pub has_configuration: bool,
}

/// The subfolders of one folder on the Media Server computer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderListing {
    pub directory: PathBuf,
    pub parent: Option<PathBuf>,
    pub has_configuration: bool,
    pub folders: Vec<FolderEntry>,
}

/// A folder that was accepted and will be used from the restart that follows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderChange {
    pub directory: PathBuf,
    /// The folder already held a usable configuration, which is what the server now loads.
    pub loaded_existing: bool,
}

/// Why a folder was not accepted. The active configuration is untouched in every case.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FolderRefusal {
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Unreadable(String),
    #[error("{0}")]
    Unwritable(String),
    #[error("{0}")]
    InvalidConfiguration(String),
}

impl FolderRefusal {
    /// A stable code a client can branch on.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Invalid(_) => "data-folder-invalid",
            Self::Unreadable(_) => "data-folder-unreadable",
            Self::Unwritable(_) => "data-folder-unwritable",
            Self::InvalidConfiguration(_) => "data-folder-configuration-invalid",
        }
    }
}

pub type BrowseFolders =
    Arc<dyn Fn(Option<&Path>) -> Result<FolderListing, FolderRefusal> + Send + Sync>;
pub type ChangeFolder =
    Arc<dyn Fn(&Path, &MediaConfiguration) -> Result<FolderChange, FolderRefusal> + Send + Sync>;

/// What the running process lets the API do with its data folder.
#[derive(Clone)]
pub struct DataFolders {
    pub browse: BrowseFolders,
    /// Validates and records the folder. Must leave everything untouched when it refuses.
    pub change: ChangeFolder,
    /// Stops this process and starts it again so the chosen folder is loaded.
    pub restart: Arc<dyn Fn() + Send + Sync>,
}

impl Default for DataFolders {
    fn default() -> Self {
        let unavailable =
            || FolderRefusal::Invalid("choosing a folder is unavailable in this process".into());
        Self {
            browse: Arc::new(move |_| Err(unavailable())),
            change: Arc::new(move |_, _| Err(unavailable())),
            restart: Arc::new(|| {}),
        }
    }
}

impl std::fmt::Debug for DataFolders {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DataFolders")
            .finish_non_exhaustive()
    }
}
