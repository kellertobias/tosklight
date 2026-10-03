//! The binding store: `<app data>/show-sync/index.json` maps a working document to its
//! association, and `<association>/binding.json` holds the binding itself.
//!
//! It replaces the `*.show.desk-source.json` sidecar that used to sit beside the document. Those
//! sidecars are no longer read: the feature had not shipped, so there are no bindings to carry
//! over, and a stray sidecar is simply ignored.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use uuid::Uuid;

type Answer<T> = Result<T, String>;

/// One Architect document bound to one show on one desk installation.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct SyncBinding {
    /// Scopes every sync request identity this document sends.
    pub association_id: Uuid,
    /// The desk installation, as its readiness reports it. Absent for a desk that predates
    /// identities; adopted on the first connection that confirms the desk holds the show.
    pub desk_identity: Option<Uuid>,
    pub show_id: Uuid,
    /// Where the desk was reached, in the order worth trying.
    pub base_urls: Vec<String>,
    pub desk_name: String,
    /// The desk's show revision this document last confirmed.
    pub acknowledged_show_revision: u64,
    /// When the document was bound, in milliseconds since the Unix epoch.
    pub created_at_millis: u64,
}

impl SyncBinding {
    pub(crate) fn new(
        desk_identity: Option<Uuid>,
        show_id: Uuid,
        base_url: String,
        desk_name: String,
        acknowledged_show_revision: u64,
    ) -> Self {
        Self {
            association_id: Uuid::new_v4(),
            desk_identity,
            show_id,
            base_urls: vec![base_url],
            desk_name,
            acknowledged_show_revision,
            created_at_millis: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |elapsed| {
                    u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
                }),
        }
    }

    /// The address to try first.
    pub(crate) fn base_url(&self) -> Option<&str> {
        self.base_urls.first().map(String::as_str)
    }
}

#[derive(Default, Deserialize, Serialize)]
struct BindingIndex {
    #[serde(default)]
    documents: BTreeMap<String, Uuid>,
}

/// Bindings for every document this installation has opened from a desk.
#[derive(Clone, Debug)]
pub(crate) struct SyncBindingStore {
    root: PathBuf,
}

impl SyncBindingStore {
    pub(crate) fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The binding of the document at `working_path`, if it has one.
    ///
    /// A damaged index or binding is an error rather than "unbound", so the caller can say so
    /// instead of quietly turning a bound document into a standalone one.
    pub(crate) fn for_document(&self, working_path: &Path) -> Answer<Option<SyncBinding>> {
        let index = self.read_index()?;
        let Some(association) = index.documents.get(&document_key(working_path)) else {
            return Ok(None);
        };
        let path = self.binding_path(*association);
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("reading {}: {error}", path.display())),
        };
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|error| format!("the sync binding {} is damaged: {error}", path.display()))
    }

    /// Binds `working_path` to `binding`, replacing any binding it had.
    pub(crate) fn bind(&self, working_path: &Path, binding: &SyncBinding) -> Answer<()> {
        self.write_binding(binding)?;
        let mut index = self.read_index().unwrap_or_default();
        index
            .documents
            .insert(document_key(working_path), binding.association_id);
        self.write_index(&index)
    }

    /// Records a changed binding, such as a newly confirmed revision.
    pub(crate) fn update(&self, binding: &SyncBinding) -> Answer<()> {
        self.write_binding(binding)
    }

    /// Removes the document's binding; the association directory stays for its journal.
    pub(crate) fn unbind(&self, working_path: &Path) -> Answer<()> {
        let mut index = self.read_index()?;
        if index
            .documents
            .remove(&document_key(working_path))
            .is_some()
        {
            self.write_index(&index)?;
        }
        Ok(())
    }

    fn index_path(&self) -> PathBuf {
        self.root.join("index.json")
    }

    fn binding_path(&self, association: Uuid) -> PathBuf {
        self.root.join(association.to_string()).join("binding.json")
    }

    fn read_index(&self) -> Answer<BindingIndex> {
        let path = self.index_path();
        match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| {
                format!(
                    "the sync binding index {} is damaged: {error}",
                    path.display()
                )
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(BindingIndex::default())
            }
            Err(error) => Err(format!("reading {}: {error}", path.display())),
        }
    }

    fn write_index(&self, index: &BindingIndex) -> Answer<()> {
        write_atomically(&self.index_path(), index)
    }

    fn write_binding(&self, binding: &SyncBinding) -> Answer<()> {
        write_atomically(&self.binding_path(binding.association_id), binding)
    }
}

/// The key a document is indexed under: its canonical path when it exists, so the same file
/// reached through a different spelling is the same document.
fn document_key(path: &Path) -> String {
    std::fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .into_owned()
}

/// Writes beside the destination and renames over it, so a crash leaves the old or the new file
/// and never half of one.
fn write_atomically<T: Serialize>(path: &Path, value: &T) -> Answer<()> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", path.display()))?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let staged = parent.join(format!(".{}.tmp", Uuid::new_v4()));
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    std::fs::write(&staged, bytes).map_err(|error| error.to_string())?;
    std::fs::rename(&staged, path).map_err(|error| {
        let _ = std::fs::remove_file(&staged);
        error.to_string()
    })
}

#[cfg(test)]
#[path = "binding_tests.rs"]
mod tests;
