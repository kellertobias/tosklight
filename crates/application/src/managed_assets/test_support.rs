use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Mutex,
};

use super::*;

#[derive(Clone)]
struct StoredAsset {
    descriptor: AssetDescriptor,
    bytes: Vec<u8>,
}

#[derive(Default)]
struct FakeState {
    assets: BTreeMap<AssetReference, StoredAsset>,
    namespaces: BTreeMap<AssetNamespace, BTreeSet<AssetReference>>,
    copies: Vec<(AssetReference, AssetNamespace)>,
}

#[derive(Default)]
pub(crate) struct FakeAssetStore {
    state: Mutex<FakeState>,
}

impl FakeAssetStore {
    pub(crate) fn copies(&self) -> Vec<(AssetReference, AssetNamespace)> {
        self.state.lock().unwrap().copies.clone()
    }
}

impl ManagedAssetStore for FakeAssetStore {
    fn import(
        &self,
        request: ImportAssetRequest,
        source: &mut dyn AssetChunkSource,
    ) -> Result<AssetDescriptor, AssetError> {
        let bytes = read_all(source)?;
        validate_import(&request, &bytes)?;
        let mut state = self.state.lock().unwrap();
        let id = request.identity.unwrap_or(AssetId(Uuid::new_v4()));
        let revision = state
            .assets
            .keys()
            .filter(|asset| asset.id == id)
            .map(|asset| asset.revision.0)
            .max()
            .unwrap_or(0)
            + 1;
        let descriptor = AssetDescriptor {
            asset: AssetReference {
                id,
                revision: AssetRevision(revision),
            },
            name: request.name,
            media_type: request.media_type,
            length: bytes.len() as u64,
            digest: fake_digest(&bytes),
        };
        state.assets.insert(
            descriptor.asset,
            StoredAsset {
                descriptor: descriptor.clone(),
                bytes,
            },
        );
        state
            .namespaces
            .entry(request.namespace.clone())
            .or_default()
            .insert(descriptor.asset);
        state.copies.push((descriptor.asset, request.namespace));
        Ok(descriptor)
    }

    fn validate(&self, asset: AssetReference) -> Result<AssetValidation, AssetError> {
        let state = self.state.lock().unwrap();
        let stored = state.assets.get(&asset).ok_or_else(not_found)?;
        let mut problems = Vec::new();
        if stored.descriptor.length != stored.bytes.len() as u64 {
            problems.push("length mismatch".into());
        }
        if stored.descriptor.digest != fake_digest(&stored.bytes) {
            problems.push("digest mismatch".into());
        }
        Ok(AssetValidation {
            descriptor: stored.descriptor.clone(),
            valid: problems.is_empty(),
            problems,
        })
    }

    fn stream(
        &self,
        asset: AssetReference,
        sink: &mut dyn AssetChunkSink,
    ) -> Result<AssetStreamReport, AssetError> {
        let state = self.state.lock().unwrap();
        let stored = state.assets.get(&asset).ok_or_else(not_found)?;
        for chunk in stored.bytes.chunks(3) {
            sink.write_chunk(chunk)?;
        }
        Ok(AssetStreamReport {
            asset,
            bytes_written: stored.bytes.len() as u64,
        })
    }

    fn copy(&self, request: CopyAssetRequest) -> Result<AssetReference, AssetError> {
        let mut state = self.state.lock().unwrap();
        if !state.assets.contains_key(&request.asset) {
            return Err(not_found());
        }
        state
            .namespaces
            .entry(request.destination.clone())
            .or_default()
            .insert(request.asset);
        state.copies.push((request.asset, request.destination));
        Ok(request.asset)
    }

    fn export(
        &self,
        request: ExportAssetsRequest,
        sink: &mut dyn AssetExportSink,
    ) -> Result<AssetExportReport, AssetError> {
        let state = self.state.lock().unwrap();
        let mut bytes_written = 0;
        let mut manifest = AssetExportManifest { assets: Vec::new() };
        for asset in &request.assets {
            let stored = state.assets.get(asset).ok_or_else(not_found)?;
            sink.begin_asset(&stored.descriptor)?;
            for chunk in stored.bytes.chunks(3) {
                sink.write_asset_chunk(chunk)?;
            }
            sink.end_asset(*asset)?;
            manifest.assets.push(stored.descriptor.clone());
            bytes_written += stored.bytes.len() as u64;
        }
        Ok(AssetExportReport {
            manifest,
            assets_written: request.assets.len(),
            bytes_written,
        })
    }

    fn availability(&self, asset: AssetReference) -> Result<AssetAvailability, AssetError> {
        Ok(self
            .state
            .lock()
            .unwrap()
            .assets
            .get(&asset)
            .map(|stored| AssetAvailability::Available(stored.descriptor.clone()))
            .unwrap_or(AssetAvailability::Missing(asset)))
    }

    fn revisions(&self, id: AssetId) -> Result<Vec<AssetDescriptor>, AssetError> {
        Ok(self
            .state
            .lock()
            .unwrap()
            .assets
            .values()
            .filter(|asset| asset.descriptor.asset.id == id)
            .map(|asset| asset.descriptor.clone())
            .collect())
    }

    fn cleanup(&self, request: CleanupAssetsRequest) -> Result<AssetCleanupReport, AssetError> {
        let mut state = self.state.lock().unwrap();
        let members = state
            .namespaces
            .get(&request.namespace)
            .cloned()
            .unwrap_or_default();
        let detached = members
            .iter()
            .filter(|asset| !request.retain.contains(asset))
            .copied()
            .collect::<Vec<_>>();
        let removed = detached
            .iter()
            .filter(|asset| {
                !state.namespaces.iter().any(|(namespace, references)| {
                    namespace != &request.namespace && references.contains(asset)
                })
            })
            .copied()
            .collect::<Vec<_>>();
        let bytes_reclaimed = removed
            .iter()
            .filter_map(|asset| state.assets.get(asset))
            .map(|asset| asset.bytes.len() as u64)
            .sum();
        if !request.dry_run {
            if let Some(references) = state.namespaces.get_mut(&request.namespace) {
                for asset in &detached {
                    references.remove(asset);
                }
            }
            for asset in &removed {
                state.assets.remove(asset);
            }
        }
        let retained = members
            .iter()
            .filter(|asset| request.retain.contains(asset))
            .copied()
            .collect::<Vec<_>>();
        Ok(AssetCleanupReport {
            namespace: request.namespace,
            detached,
            removed,
            retained,
            bytes_reclaimed,
            dry_run: request.dry_run,
        })
    }
}

fn read_all(source: &mut dyn AssetChunkSource) -> Result<Vec<u8>, AssetError> {
    let mut bytes = Vec::new();
    while let Some(chunk) = source.read_chunk(4)? {
        bytes.extend(chunk);
    }
    Ok(bytes)
}

fn validate_import(request: &ImportAssetRequest, bytes: &[u8]) -> Result<(), AssetError> {
    if request.declared_length != bytes.len() as u64
        || request.declared_digest != fake_digest(bytes)
    {
        return Err(AssetError::new(
            AssetErrorKind::Invalid,
            "declared asset metadata does not match the stream",
        ));
    }
    Ok(())
}

fn not_found() -> AssetError {
    AssetError::new(AssetErrorKind::NotFound, "asset revision")
}

pub(crate) fn fake_digest(bytes: &[u8]) -> String {
    format!(
        "fake:{:x}:{}",
        bytes.iter().map(|byte| u64::from(*byte)).sum::<u64>(),
        bytes.len()
    )
}

pub(crate) struct VecSource {
    bytes: Vec<u8>,
    cursor: usize,
}

impl VecSource {
    pub(crate) fn new(bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            bytes: bytes.into(),
            cursor: 0,
        }
    }
}

impl AssetChunkSource for VecSource {
    fn read_chunk(&mut self, maximum_bytes: usize) -> Result<Option<Vec<u8>>, AssetError> {
        if self.cursor == self.bytes.len() {
            return Ok(None);
        }
        let end = (self.cursor + maximum_bytes).min(self.bytes.len());
        let chunk = self.bytes[self.cursor..end].to_vec();
        self.cursor = end;
        Ok(Some(chunk))
    }
}

#[derive(Default)]
pub(crate) struct VecSink(pub(crate) Vec<u8>);

impl AssetChunkSink for VecSink {
    fn write_chunk(&mut self, bytes: &[u8]) -> Result<(), AssetError> {
        self.0.extend_from_slice(bytes);
        Ok(())
    }
}

#[derive(Default)]
pub(crate) struct FramedVecSink {
    pub(crate) assets: Vec<(AssetDescriptor, Vec<u8>)>,
    current: Option<(AssetDescriptor, Vec<u8>)>,
}

impl AssetExportSink for FramedVecSink {
    fn begin_asset(&mut self, descriptor: &AssetDescriptor) -> Result<(), AssetError> {
        if self.current.is_some() {
            return Err(AssetError::new(
                AssetErrorKind::Invalid,
                "nested asset frame",
            ));
        }
        self.current = Some((descriptor.clone(), Vec::new()));
        Ok(())
    }

    fn write_asset_chunk(&mut self, bytes: &[u8]) -> Result<(), AssetError> {
        let (_, body) = self
            .current
            .as_mut()
            .ok_or_else(|| AssetError::new(AssetErrorKind::Invalid, "missing asset frame"))?;
        body.extend_from_slice(bytes);
        Ok(())
    }

    fn end_asset(&mut self, asset: AssetReference) -> Result<(), AssetError> {
        let frame = self
            .current
            .take()
            .ok_or_else(|| AssetError::new(AssetErrorKind::Invalid, "missing asset frame"))?;
        if frame.0.asset != asset {
            return Err(AssetError::new(
                AssetErrorKind::Invalid,
                "asset frame identity mismatch",
            ));
        }
        self.assets.push(frame);
        Ok(())
    }
}
