//! Portable media identities are anchored beside the clip before a catalog publishes them.

use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

use media_domain::AssetId;
use serde_json::{Map, Value};

use crate::{naming, storage};

const ID: &str = "assetId";

fn metadata_path(clip: &Path, file: u8) -> io::Result<PathBuf> {
    let directory = clip
        .parent()
        .ok_or_else(|| io::Error::new(ErrorKind::InvalidInput, "clip has no parent folder"))?;
    Ok(directory
        .join(naming::METADATA_DIRECTORY)
        .join(naming::metadata_filename(file)))
}

fn read(path: &Path) -> io::Result<Map<String, Value>> {
    match std::fs::read(path) {
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(Map::new()),
        Err(error) => Err(error),
        Ok(bytes) => serde_json::from_slice::<Value>(&bytes)
            .map_err(|error| io::Error::new(ErrorKind::InvalidData, format!("metadata is not readable JSON: {error}; preserve the original and repair it")))?
            .as_object().cloned().ok_or_else(|| io::Error::new(ErrorKind::InvalidData, "metadata must be a JSON object; preserve the original and repair it")),
    }
}

fn stored_id(metadata: &Map<String, Value>) -> io::Result<Option<AssetId>> {
    metadata
        .get(ID)
        .map(|value| {
            let id = value.as_str().and_then(|value| uuid::Uuid::parse_str(value).ok())
            .filter(|id| !id.is_nil()).ok_or_else(|| io::Error::new(ErrorKind::InvalidData,
                "assetId must be a nonempty UUID; preserve the original metadata and repair it"))?;
            Ok::<_, io::Error>(AssetId::from_uuid(id))
        })
        .transpose()
}

/// Returns an existing anchored ID without rewriting metadata, including on read-only media.
/// Metadata travels with a file through address changes and copies to other computers.
pub(crate) fn ensure(clip: &Path, file: u8) -> io::Result<AssetId> {
    let path = metadata_path(clip, file)?;
    let mut metadata = read(&path)?;
    let present = stored_id(&metadata)?;
    if let Some(id) = present {
        return Ok(id);
    }
    let _lock = IdentityLock::acquire(&path)?;
    // Another process may have anchored the same clip while this process waited for the lock.
    metadata = read(&path)?;
    if let Some(id) = stored_id(&metadata)? {
        return Ok(id);
    }
    anchor(&path, &mut metadata)
}

/// Commit a converted staging clip only after its replacement identity can be stored. An ordinary
/// filesystem refusal restores the old metadata; callers discard staging on failure or cancellation.
/// Clip and metadata publication are separate renames, retaining the importer's existing crash window.
pub(crate) fn publish(staged: &Path, destination: &Path, file: u8) -> io::Result<AssetId> {
    let path = metadata_path(destination, file)?;
    let _lock = IdentityLock::acquire(&path)?;
    let original = match std::fs::read(&path) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    let mut metadata = read(&path)?;
    let result = anchor(&path, &mut metadata).and_then(|id| {
        std::fs::rename(staged, destination)?;
        Ok(id)
    });
    if let Err(error) = result {
        let restored = match original {
            Some(bytes) => atomic_metadata(&path, &bytes),
            None => match std::fs::remove_file(&path) {
                Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
                result => result,
            },
        };
        return Err(match restored {
            Ok(()) => error,
            Err(restore) => io::Error::other(format!(
                "{error}; previous metadata also needs recovery: {restore}"
            )),
        });
    }
    result
}

fn anchor(path: &Path, metadata: &mut Map<String, Value>) -> io::Result<AssetId> {
    let id = AssetId::new();
    metadata.insert(ID.into(), Value::String(id.to_string()));
    storage::write_item_metadata(path, metadata)
        .map_err(|error| io::Error::other(error.to_string()))?;
    Ok(id)
}

/// A short, cross-process lease only for assigning or rotating an identity. Existing anchored
/// libraries do not acquire it and remain usable on read-only storage.
struct IdentityLock {
    file: Option<std::fs::File>,
    path: PathBuf,
}

impl IdentityLock {
    fn acquire(metadata: &Path) -> io::Result<Self> {
        let parent = metadata
            .parent()
            .ok_or_else(|| io::Error::other("metadata has no parent"))?;
        std::fs::create_dir_all(parent)?;
        let path = metadata.with_extension("identity-lock");
        for _ in 0..50 {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(file) => {
                    return Ok(Self {
                        file: Some(file),
                        path,
                    });
                }
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                    std::thread::sleep(std::time::Duration::from_millis(10))
                }
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            ErrorKind::WouldBlock,
            format!(
                "identity initialization is busy; after stopping other Pixel processes, remove a stale lock only if it remains: {}",
                path.display()
            ),
        ))
    }
}

impl Drop for IdentityLock {
    fn drop(&mut self) {
        drop(self.file.take());
        let _ = std::fs::remove_file(&self.path);
    }
}

pub(crate) fn atomic_metadata(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let temporary = path.with_extension(format!("json.writing-{}", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temporary, path)?;
        #[cfg(unix)]
        if let Some(parent) = path.parent() {
            std::fs::File::open(parent)?.sync_all()?;
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
#[path = "identity_tests.rs"]
mod tests;
