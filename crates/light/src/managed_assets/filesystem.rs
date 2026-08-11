use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::*;

const INDEX_FILE: &str = "index.json";
const CHUNK_SIZE: usize = 64 * 1024;

/// Durable managed-asset storage rooted in one desk-owned directory.
///
/// Asset revision bytes are immutable. The small JSON index owns namespace membership and is
/// replaced atomically after every mutation, so a process crash cannot expose a partial index.
pub struct FilesystemManagedAssetStore {
    root: PathBuf,
    gate: Mutex<()>,
}

impl FilesystemManagedAssetStore {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, AssetError> {
        let root = root.into();
        fs::create_dir_all(root.join("assets")).map_err(io_error)?;
        let store = Self {
            root,
            gate: Mutex::new(()),
        };
        store.load_index()?;
        Ok(store)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn load_index(&self) -> Result<Index, AssetError> {
        let path = self.root.join(INDEX_FILE);
        match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| {
                AssetError::new(
                    AssetErrorKind::Invalid,
                    format!("managed asset index is invalid: {error}"),
                )
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Index::default()),
            Err(error) => Err(io_error(error)),
        }
    }

    fn save_index(&self, index: &Index) -> Result<(), AssetError> {
        let bytes = serde_json::to_vec_pretty(index).map_err(|error| {
            AssetError::new(
                AssetErrorKind::Invalid,
                format!("managed asset index cannot be encoded: {error}"),
            )
        })?;
        let temporary = self
            .root
            .join(format!(".{INDEX_FILE}.{}.tmp", Uuid::new_v4()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(io_error)?;
        file.write_all(&bytes).map_err(io_error)?;
        file.sync_all().map_err(io_error)?;
        fs::rename(&temporary, self.root.join(INDEX_FILE)).map_err(io_error)
    }

    fn asset_path(&self, asset: AssetReference) -> PathBuf {
        self.root
            .join("assets")
            .join(asset.id.0.to_string())
            .join(format!("{}.asset", asset.revision.0))
    }

    fn stored(&self, index: &Index, asset: AssetReference) -> Result<StoredDescriptor, AssetError> {
        index
            .assets
            .get(&StoredReference::from(asset))
            .cloned()
            .ok_or_else(|| {
                AssetError::new(
                    AssetErrorKind::NotFound,
                    "managed asset revision was not found",
                )
            })
    }
}

impl ManagedAssetStore for FilesystemManagedAssetStore {
    fn import(
        &self,
        request: ImportAssetRequest,
        source: &mut dyn AssetChunkSource,
    ) -> Result<AssetDescriptor, AssetError> {
        validate_request(&request)?;
        let _guard = self.gate.lock().expect("managed asset store lock poisoned");
        let mut index = self.load_index()?;
        let id = request.identity.unwrap_or(AssetId(Uuid::new_v4()));
        let revision = index
            .assets
            .keys()
            .filter(|asset| asset.id == id.0)
            .map(|asset| asset.revision)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(|| AssetError::new(AssetErrorKind::Conflict, "asset revision overflow"))?;
        let asset = AssetReference {
            id,
            revision: AssetRevision(revision),
        };
        let directory = self
            .asset_path(asset)
            .parent()
            .expect("asset path has a parent")
            .to_path_buf();
        fs::create_dir_all(&directory).map_err(io_error)?;
        let temporary = directory.join(format!(".{revision}.{}.tmp", Uuid::new_v4()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(io_error)?;
        let mut length = 0_u64;
        let mut digest = Sha256::new();
        loop {
            let Some(chunk) = source.read_chunk(CHUNK_SIZE)? else {
                break;
            };
            if chunk.is_empty() {
                continue;
            }
            file.write_all(&chunk).map_err(io_error)?;
            length = length.saturating_add(chunk.len() as u64);
            digest.update(&chunk);
        }
        file.sync_all().map_err(io_error)?;
        let actual_digest = hex_digest(digest.finalize().as_slice());
        if length != request.declared_length || actual_digest != request.declared_digest {
            let _ = fs::remove_file(&temporary);
            return Err(AssetError::new(
                AssetErrorKind::Invalid,
                format!(
                    "asset bytes do not match declaration (length {length}, digest {actual_digest})"
                ),
            ));
        }
        let final_path = self.asset_path(asset);
        fs::rename(&temporary, &final_path).map_err(io_error)?;
        let stored = StoredDescriptor {
            id: id.0,
            revision,
            name: request.name,
            media_type: request.media_type,
            length,
            digest: actual_digest,
        };
        index
            .assets
            .insert(StoredReference::from(asset), stored.clone());
        index
            .namespaces
            .entry(request.namespace.0)
            .or_default()
            .insert(StoredReference::from(asset));
        if let Err(error) = self.save_index(&index) {
            let _ = fs::remove_file(final_path);
            return Err(error);
        }
        Ok(stored.into())
    }

    fn validate(&self, asset: AssetReference) -> Result<AssetValidation, AssetError> {
        let _guard = self.gate.lock().expect("managed asset store lock poisoned");
        let index = self.load_index()?;
        let stored = self.stored(&index, asset)?;
        let descriptor = AssetDescriptor::from(stored.clone());
        let mut problems = Vec::new();
        match digest_file(&self.asset_path(asset)) {
            Ok((length, digest)) => {
                if stored.length != length {
                    problems.push("length mismatch".into());
                }
                if stored.digest != digest {
                    problems.push("digest mismatch".into());
                }
            }
            Err(error) if error.kind == AssetErrorKind::NotFound => {
                problems.push("asset bytes are missing".into());
            }
            Err(error) => return Err(error),
        }
        Ok(AssetValidation {
            descriptor,
            valid: problems.is_empty(),
            problems,
        })
    }

    fn stream(
        &self,
        asset: AssetReference,
        sink: &mut dyn AssetChunkSink,
    ) -> Result<AssetStreamReport, AssetError> {
        let _guard = self.gate.lock().expect("managed asset store lock poisoned");
        let index = self.load_index()?;
        self.stored(&index, asset)?;
        let bytes_written = stream_file(&self.asset_path(asset), sink)?;
        Ok(AssetStreamReport {
            asset,
            bytes_written,
        })
    }

    fn copy(&self, request: CopyAssetRequest) -> Result<AssetReference, AssetError> {
        if request.destination.0.trim().is_empty() {
            return Err(AssetError::new(
                AssetErrorKind::Invalid,
                "namespace is required",
            ));
        }
        let _guard = self.gate.lock().expect("managed asset store lock poisoned");
        let mut index = self.load_index()?;
        self.stored(&index, request.asset)?;
        index
            .namespaces
            .entry(request.destination.0)
            .or_default()
            .insert(StoredReference::from(request.asset));
        self.save_index(&index)?;
        Ok(request.asset)
    }

    fn export(
        &self,
        request: ExportAssetsRequest,
        sink: &mut dyn AssetExportSink,
    ) -> Result<AssetExportReport, AssetError> {
        let _guard = self.gate.lock().expect("managed asset store lock poisoned");
        let index = self.load_index()?;
        let mut manifest = AssetExportManifest { assets: Vec::new() };
        let mut bytes_written = 0_u64;
        for asset in request.assets {
            let descriptor = AssetDescriptor::from(self.stored(&index, asset)?);
            sink.begin_asset(&descriptor)?;
            let mut adapter = ExportSinkAdapter { sink };
            bytes_written =
                bytes_written.saturating_add(stream_file(&self.asset_path(asset), &mut adapter)?);
            sink.end_asset(asset)?;
            manifest.assets.push(descriptor);
        }
        Ok(AssetExportReport {
            assets_written: manifest.assets.len(),
            bytes_written,
            manifest,
        })
    }

    fn availability(&self, asset: AssetReference) -> Result<AssetAvailability, AssetError> {
        let _guard = self.gate.lock().expect("managed asset store lock poisoned");
        let index = self.load_index()?;
        let Some(stored) = index.assets.get(&StoredReference::from(asset)) else {
            return Ok(AssetAvailability::Missing(asset));
        };
        if self.asset_path(asset).is_file() {
            Ok(AssetAvailability::Available(stored.clone().into()))
        } else {
            Ok(AssetAvailability::Missing(asset))
        }
    }

    fn revisions(&self, id: AssetId) -> Result<Vec<AssetDescriptor>, AssetError> {
        let _guard = self.gate.lock().expect("managed asset store lock poisoned");
        let index = self.load_index()?;
        Ok(index
            .assets
            .values()
            .filter(|asset| asset.id == id.0)
            .cloned()
            .map(AssetDescriptor::from)
            .collect())
    }

    fn cleanup(&self, request: CleanupAssetsRequest) -> Result<AssetCleanupReport, AssetError> {
        let _guard = self.gate.lock().expect("managed asset store lock poisoned");
        let mut index = self.load_index()?;
        let namespace = request.namespace.0.clone();
        let members = index
            .namespaces
            .get(&namespace)
            .cloned()
            .unwrap_or_default();
        let retain = request
            .retain
            .iter()
            .copied()
            .map(StoredReference::from)
            .collect::<BTreeSet<_>>();
        let detached = members.difference(&retain).copied().collect::<Vec<_>>();
        let retained = members.intersection(&retain).copied().collect::<Vec<_>>();
        let mut removed = Vec::new();
        let mut bytes_reclaimed = 0_u64;
        for reference in &detached {
            let used_elsewhere = index
                .namespaces
                .iter()
                .any(|(name, assets)| name != &namespace && assets.contains(reference));
            if !used_elsewhere {
                removed.push(*reference);
                bytes_reclaimed = bytes_reclaimed.saturating_add(
                    index
                        .assets
                        .get(reference)
                        .map(|asset| asset.length)
                        .unwrap_or(0),
                );
            }
        }
        if !request.dry_run {
            index
                .namespaces
                .insert(namespace, retained.iter().copied().collect());
            for reference in &removed {
                index.assets.remove(reference);
            }
            self.save_index(&index)?;
            for reference in &removed {
                let _ = fs::remove_file(self.asset_path((*reference).into()));
            }
        }
        Ok(AssetCleanupReport {
            namespace: request.namespace,
            detached: detached.into_iter().map(AssetReference::from).collect(),
            removed: removed.into_iter().map(AssetReference::from).collect(),
            retained: retained.into_iter().map(AssetReference::from).collect(),
            bytes_reclaimed,
            dry_run: request.dry_run,
        })
    }
}

#[derive(Default, Serialize, Deserialize)]
struct Index {
    #[serde(default)]
    assets: BTreeMap<StoredReference, StoredDescriptor>,
    #[serde(default)]
    namespaces: BTreeMap<String, BTreeSet<StoredReference>>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
struct StoredReference {
    id: Uuid,
    revision: u64,
}

impl From<StoredReference> for String {
    fn from(value: StoredReference) -> Self {
        format!("{}/{}", value.id, value.revision)
    }
}

impl TryFrom<String> for StoredReference {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let (id, revision) = value
            .rsplit_once('/')
            .ok_or_else(|| "stored asset reference is malformed".to_owned())?;
        Ok(Self {
            id: Uuid::parse_str(id).map_err(|error| error.to_string())?,
            revision: revision.parse::<u64>().map_err(|error| error.to_string())?,
        })
    }
}

impl From<AssetReference> for StoredReference {
    fn from(value: AssetReference) -> Self {
        Self {
            id: value.id.0,
            revision: value.revision.0,
        }
    }
}

impl From<StoredReference> for AssetReference {
    fn from(value: StoredReference) -> Self {
        Self {
            id: AssetId(value.id),
            revision: AssetRevision(value.revision),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct StoredDescriptor {
    id: Uuid,
    revision: u64,
    name: String,
    media_type: String,
    length: u64,
    digest: String,
}

impl From<StoredDescriptor> for AssetDescriptor {
    fn from(value: StoredDescriptor) -> Self {
        Self {
            asset: AssetReference {
                id: AssetId(value.id),
                revision: AssetRevision(value.revision),
            },
            name: value.name,
            media_type: value.media_type,
            length: value.length,
            digest: value.digest,
        }
    }
}

struct ExportSinkAdapter<'a> {
    sink: &'a mut dyn AssetExportSink,
}

impl AssetChunkSink for ExportSinkAdapter<'_> {
    fn write_chunk(&mut self, bytes: &[u8]) -> Result<(), AssetError> {
        self.sink.write_asset_chunk(bytes)
    }
}

fn validate_request(request: &ImportAssetRequest) -> Result<(), AssetError> {
    if request.namespace.0.trim().is_empty()
        || request.name.trim().is_empty()
        || request.media_type.trim().is_empty()
        || request.declared_digest.len() != 64
    {
        return Err(AssetError::new(
            AssetErrorKind::Invalid,
            "namespace, name, media type, and SHA-256 digest are required",
        ));
    }
    Ok(())
}

fn stream_file(path: &Path, sink: &mut dyn AssetChunkSink) -> Result<u64, AssetError> {
    let mut file = File::open(path).map_err(path_io_error)?;
    let mut buffer = vec![0_u8; CHUNK_SIZE];
    let mut written = 0_u64;
    loop {
        let count = file.read(&mut buffer).map_err(io_error)?;
        if count == 0 {
            break;
        }
        sink.write_chunk(&buffer[..count])?;
        written = written.saturating_add(count as u64);
    }
    Ok(written)
}

fn digest_file(path: &Path) -> Result<(u64, String), AssetError> {
    let mut file = File::open(path).map_err(path_io_error)?;
    let mut buffer = vec![0_u8; CHUNK_SIZE];
    let mut digest = Sha256::new();
    let mut length = 0_u64;
    loop {
        let count = file.read(&mut buffer).map_err(io_error)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
        length = length.saturating_add(count as u64);
    }
    Ok((length, hex_digest(digest.finalize().as_slice())))
}

fn hex_digest(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn path_io_error(error: std::io::Error) -> AssetError {
    if error.kind() == std::io::ErrorKind::NotFound {
        AssetError::new(
            AssetErrorKind::NotFound,
            "managed asset bytes were not found",
        )
    } else {
        io_error(error)
    }
}

fn io_error(error: std::io::Error) -> AssetError {
    AssetError::new(
        AssetErrorKind::Io,
        format!("managed asset storage failed: {error}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Source(Vec<u8>);
    impl AssetChunkSource for Source {
        fn read_chunk(&mut self, maximum_bytes: usize) -> Result<Option<Vec<u8>>, AssetError> {
            if self.0.is_empty() {
                return Ok(None);
            }
            let remaining = self.0.split_off(self.0.len().min(maximum_bytes));
            Ok(Some(std::mem::replace(&mut self.0, remaining)))
        }
    }

    #[derive(Default)]
    struct Sink(Vec<u8>);
    impl AssetChunkSink for Sink {
        fn write_chunk(&mut self, bytes: &[u8]) -> Result<(), AssetError> {
            self.0.extend_from_slice(bytes);
            Ok(())
        }
    }

    fn test_root() -> PathBuf {
        std::env::temp_dir().join(format!("tosklight-managed-assets-{}", Uuid::new_v4()))
    }

    #[test]
    fn persists_immutable_revisions_and_namespace_ownership_across_reopen() {
        let root = test_root();
        let bytes = b"portable timecode audio";
        let digest = hex_digest(&Sha256::digest(bytes));
        let id = AssetId(Uuid::from_u128(42));
        let store = FilesystemManagedAssetStore::open(&root).unwrap();
        let first = store
            .import(
                ImportAssetRequest {
                    identity: Some(id),
                    namespace: AssetNamespace("show:main".into()),
                    name: "Intro.wav".into(),
                    media_type: "audio/wav".into(),
                    declared_length: bytes.len() as u64,
                    declared_digest: digest.clone(),
                },
                &mut Source(bytes.to_vec()),
            )
            .unwrap();
        let second = store
            .import(
                ImportAssetRequest {
                    identity: Some(id),
                    namespace: AssetNamespace("show:main".into()),
                    name: "Intro.wav".into(),
                    media_type: "audio/wav".into(),
                    declared_length: bytes.len() as u64,
                    declared_digest: digest,
                },
                &mut Source(bytes.to_vec()),
            )
            .unwrap();
        assert_eq!(first.asset.revision, AssetRevision(1));
        assert_eq!(second.asset.revision, AssetRevision(2));
        drop(store);

        let reopened = FilesystemManagedAssetStore::open(&root).unwrap();
        assert_eq!(reopened.revisions(id).unwrap().len(), 2);
        let mut sink = Sink::default();
        reopened.stream(first.asset, &mut sink).unwrap();
        assert_eq!(sink.0, bytes);
        assert!(reopened.validate(first.asset).unwrap().valid);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_declared_bytes_before_publishing_a_revision() {
        let root = test_root();
        let store = FilesystemManagedAssetStore::open(&root).unwrap();
        let error = store
            .import(
                ImportAssetRequest {
                    identity: None,
                    namespace: AssetNamespace("show:main".into()),
                    name: "Bad.wav".into(),
                    media_type: "audio/wav".into(),
                    declared_length: 3,
                    declared_digest: "0".repeat(64),
                },
                &mut Source(b"bad".to_vec()),
            )
            .unwrap_err();
        assert_eq!(error.kind, AssetErrorKind::Invalid);
        assert!(store.load_index().unwrap().assets.is_empty());
        fs::remove_dir_all(root).unwrap();
    }
}
